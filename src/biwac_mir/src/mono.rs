//! 単相化された MIR。
//!
//! [`Mir`](crate::Mir) が「パッケージ 1 つ分のジェネリックな MIR」であるのに対し、
//! これは **リンク後のプログラム全体** である。
//! `Pair::new[Int, Int]` のような上流のジェネリックシンボルの実体も、
//! 実際に依存している側 (playable パッケージ) のここに入る。
//!
//! 本体の表現は [`MirItem`] のまま変えない。
//! 単相化後の本体は「型がすべて具体になった [`Body`](crate::Body)」であって、
//! 形が変わるわけではないからである。
//!
//! シンボルの id は [`crate::Mir`] と同じ流儀で、
//! 自パッケージのものは `SELF` のまま、依存のものは
//! `(本当の PackageId, .biwameta のシンボル索引)` である。

use biwac_base::InternedIdent;
use biwac_hir::Ty;
use biwac_span::{TyDefId, ValDefId};

use crate::{GenArgs, MirItem, StringPool};

/// 単相化された 1 実体の同一性。
///
/// 同じ関数でもジェネリック引数が違えば別の実体になる。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceKey {
    pub def_id: ValDefId,

    /// すべて具体。多相でなければ空。[`biwac_span::LocalGenDefId`] 順。
    pub args: GenArgs,
}

impl InstanceKey {
    pub fn new(def_id: ValDefId, mut args: GenArgs) -> Self {
        args.sort_by_key(|(g, _)| g.value());
        Self { def_id, args }
    }
}

/// 具体化された型の同一性。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TyInstanceKey {
    pub def_id: TyDefId,
    /// すべて具体。宣言順。
    pub args: Vec<Ty>,
}

#[derive(Debug, Clone)]
pub struct MonoMir {
    /// 実体の一覧。根から辿った発見順に並ぶ。
    /// 索引がそのまま実体の番号になる。
    pub instances: Vec<MonoInstance>,

    /// 参照された具体型の定義。発見順。
    pub types: Vec<MonoTyDef>,

    /// 全パッケージ分をまとめた文字列。
    ///
    /// 元の [`crate::Mir`] の文字列プールはパッケージ相対なので、
    /// 単相化のときにここへ移し替えて `Const::Str` を付け替えてある。
    pub strings: StringPool,

    /// エントリポイント (`scene main`) の実体索引。
    pub entry: Option<usize>,

    /// モジュール全体に前置されるネイティブコード。
    ///
    /// 実体を提供したパッケージのものだけを、
    /// [`biwac_base::PackageId`] 昇順 (自パッケージが最後) に並べる。
    /// 使われないパッケージの import まで並べると、
    /// ホストが用意していない関数を要求してインスタンス化に失敗する。
    pub module_natives: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MonoInstance {
    pub key: InstanceKey,

    /// 型がすべて具体になった本体。
    /// [`crate::Callee::Direct`] の `genargs` も具体なので、
    /// そのまま呼び先の [`InstanceKey`] になる。
    pub item: MirItem,
}

#[derive(Debug, Clone)]
pub struct MonoTyDef {
    pub key: TyInstanceKey,
    pub kind: MonoTyDefKind,
}

#[derive(Debug, Clone)]
pub enum MonoTyDefKind {
    /// メンバは名前順に並ぶ。
    /// HIR 側の表が `HashMap` なので、ここで順序を正準化している。
    Struct { members: Vec<(InternedIdent, Ty)> },

    /// `[[native(arch = "...")]] type Vec[T] = {{ ... }}`
    ///
    /// 中身は不透明な文字列のまま。どの arch 向けかは HIR がまだ持っていない。
    Native { code: String },
}

impl MonoMir {
    /// エントリポイントの実体。
    pub fn entry_instance(&self) -> Option<&MonoInstance> {
        self.entry.map(|i| &self.instances[i])
    }

    /// この実体の索引。
    pub fn index_of(&self, key: &InstanceKey) -> Option<usize> {
        self.instances.iter().position(|i| &i.key == key)
    }
}
