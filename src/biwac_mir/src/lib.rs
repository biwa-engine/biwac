//! MIR は
//! Mid-level Intermediate Representation (中レベル中間表現) である。
//!
//! HIR (式と文の木) と、アセンブリレベルのターゲット (WASM など) の間に置く。
//! 制御フローグラフ (CFG) と、局所変数 (local) への代入の列で関数を表す。
//!
//! ## 何のためにあるか
//!
//! 1. HIR から直接アセンブリレベルの表現に落とすと、
//!    ターゲットが増えるたびに「式の木を崩して制御フローにする」処理を書き直すことになる。
//! 2. 最適化パスを掛ける場所が要る。木のままでは掛けにくい。
//!
//! ## MIR が持たないもの
//!
//! rustc の MIR にあって、ここに無いものは意図的に落としている。
//!
//! - **借用検査・drop・巻き戻しのための構成要素**
//!   (`Drop`, unwind edge, `Assert`, `FakeRead`, `Retag`,
//!    `Operand` の `Copy`/`Move` の区別, `StorageLive`/`StorageDead`)。
//!   biwa には所有権もデストラクタも panic も無い。
//! - **中断 (`Yield`) とコルーチン**。
//!   エンジンの機能の呼び出しは同期的なホスト関数の呼び出し (syscall) であり、
//!   ブロックするかすぐ戻すかはホスト側の都合である。
//!   ゲスト側のコードには中断が現れないので、MIR にも現れない。
//!   したがって scene も MIR 上はただの関数であり、`Body` に種別の区別を持たない。
//! - **可読な出力のための情報** (変数名、スコープ)。
//!   MIR は TypeScript を吐くためには使わない。
//!   TypeScript は HIR から直接生成しており、そちらは MIR を通らない。
//!
//! 逆に [`Span`] は持つ。MIR レベルでの診断と、
//! 将来のデバッグ情報のために必要で、かつ後から復元できない唯一の情報だからである。

mod body;
mod codec;
mod mono;
mod place;
mod rvalue;
mod validate;

use std::collections::BTreeMap;

use biwac_base::{PackageId, PackageName};
use biwac_hir::Ty;
use biwac_span::{LocalGenDefId, Span, ValDefId};

pub use body::{
    BasicBlock, BasicBlockData, Body, Callee, GenArgs, Local, LocalDecl, Statement, StatementKind,
    SwitchTargets, Terminator, TerminatorKind,
};
pub use codec::{
    BIWAC_MIR_FORMAT_VERSION, DecodedMir, EncodeCtx, MIR_FILE_EXTENSION, MirDecodeError, decode,
    encode,
};
pub use mono::{
    InstanceKey, MonoInstance, MonoMir, MonoTyDef, MonoTyDefKind, MonoVariant, TyInstanceKey,
};
pub use place::{Place, PlaceElem};
pub use rvalue::{AggregateKind, BinOp, Const, Operand, Rvalue, StrId, StringPool, UnOp};
pub use validate::{ValidationError, ValidationErrorKind, validate, validate_body};

/// 1 パッケージ分の MIR。
#[derive(Debug, Clone)]
pub struct Mir {
    pub pkg_name: PackageName,

    /// このパッケージの [`PackageId`]。`(name, version)` のハッシュ由来の一意な id。
    ///
    /// メモリ上では自パッケージのシンボルの `def_id.pkg()` は
    /// [`PackageId::SELF_PACKAGE`] のままだが (HIR と同じ流儀)、
    /// この `Mir` がどのパッケージのものかはここで分かる。
    ///
    /// [`Const::Str`] が指す文字列プールはパッケージ相対なので、
    /// 依存の MIR を読み込んだ後は `(pkg_id, StrId)` で初めて一意になる。
    pub pkg_id: PackageId,

    /// 値名前空間のシンボルごとの本体。
    ///
    /// [`BTreeMap`] なのは走査順を固定するため。
    /// ビルドが決定論的であることは差分ビルドの前提なので、
    /// 新しく増える出力も最初から順序を固定しておく。
    pub items: BTreeMap<ValDefId, MirItem>,

    /// 文字列リテラルのプール。
    ///
    /// 文字列の表現の仕方 (TypeScript のリテラル / WASM のデータセグメント / GC array) は
    /// ターゲットごとに違うので、MIR は中身の文字列を持つだけにする。
    pub strings: StringPool,

    /// モジュール全体に前置されるネイティブコード。
    ///
    /// どのシンボルからも参照されないが、生成物の先頭に置かれるものである。
    /// wasm ではホスト関数の import 宣言がここに入る。
    ///
    /// 単相化するターゲットでは **依存パッケージのものも要る**。
    /// std が宣言した import を実際に使うのは、std の関数を取り込んだ
    /// 下流の生成物だからである。したがって `.biwamir` に載せて運ぶ。
    ///
    /// 宣言順に並ぶ。
    pub module_natives: Vec<String>,
}

impl Mir {
    pub fn new(pkg_name: PackageName, pkg_id: PackageId) -> Self {
        Self {
            pkg_name,
            pkg_id,
            items: BTreeMap::new(),
            strings: StringPool::default(),
            module_natives: Vec::new(),
        }
    }
}

/// 値名前空間のシンボル 1 つ分。
#[derive(Debug, Clone)]
pub enum MirItem {
    /// biwa で書かれた関数・関連関数・メソッド・scene。
    Body(Body),

    /// `[[native(arch = "...")]]` で書かれたもの。
    ///
    /// 本体は不透明な文字列のままで、展開はバックエンドの仕事である。
    Native(NativeItem),
}

/// ネイティブ実装されたシンボル。
///
/// [`Body`] と違って local を持たないので、
/// バックエンドが呼び出し側の型を知れるようにシグニチャをここに持つ。
#[derive(Debug, Clone)]
pub struct NativeItem {
    pub def_id: ValDefId,

    /// メソッドなら self の型。
    pub self_ty: Option<Ty>,

    pub args: Vec<Ty>,

    /// 値を返さないなら [`biwac_hir::TyKind::Void`]。
    pub rty: Ty,

    /// 多相なら空でない。
    pub genargs: Vec<LocalGenDefId>,

    /// ネイティブコードの中身。
    ///
    /// NOTE: どの arch 向けのコードかは HIR がまだ持っていない
    /// (`[[native(arch = "...")]]` の arch は属性検査で妥当性を見るだけで、
    ///  HIR には落ちていない)。ターゲットが 2 つになった時点で必要になる。
    pub native_body: String,

    pub native_span: Span,
    pub span: Span,
}
