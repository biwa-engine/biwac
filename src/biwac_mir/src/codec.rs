//! `.biwamir` — MIR のディスク形式。
//!
//! # 何を書いているのか
//!
//! `.biwameta` が HIR (シグニチャ) レベルのディスク形式であるのに対し、
//! `.biwamir` はその上に載る「各シンボルの本体はこうなっている」という表である。
//! **新しい id は振らない。** `.biwameta` が確立した DefId に対して MIR を並べるだけである。
//!
//! `.biwameta` はシンボルを並べ直して 0 から採番し、
//! 消費側はその索引をそのまま [`biwac_span::PackageLocalDefId`] として使う。
//! つまり std のあるシンボルは
//!
//! - std 自身のビルド中: `(SELF, def_collector の連番)`
//! - test1 から見たとき: `(std の PackageId, .biwameta のシンボル索引)`
//!
//! と別物になる。メモリ上の MIR は HIR と同じ前者の空間のままにして、
//! [`encode`] のときだけ後者に読み替える。[`decode`] は後者を返すので、
//! 下流から見た依存シンボルの DefId は HIR でも MIR でも同一になる。
//!
//! # 形式
//!
//! テキスト。**1 行 1 要素、先頭トークンがタグ**。
//! 入れ子は表の索引で表すので、行を空白で分割して先頭を見るだけで読める。
//!
//! - 先頭の空白は無視する (インデントは人が読むためだけのもの)。
//! - トークンの先頭にある `#` から行末はコメント。
//!   `p0:19#0` のようにトークンの内側に現れる `#` はコメントではない。
//! - 空行は無視する。
//! - 前方参照はしない。`ty` / `ga` / `str` は使う前に定義されている。
//!
//! ```text
//! biwamir 1
//! pkg 0 std 2734891234        # 0 番は必ず自パッケージ
//! pkg 1 color 9182736455      # 参照する他パッケージ。PackageId 昇順
//! meta-svh a3f19c0e7b2d4851   # 自パッケージの .biwameta の SVH
//!
//! str 0 "Hello\n"
//!
//! ty 0 int
//! ty 1 def p0:5
//! ty 2 def p0:7 1 0           # Pair[<ty1>, <ty0>]
//! ty 3 gen p0:7#0             # 型定義のジェネリック引数
//! ty 4 locgen p0:11#1         # 関数のローカルジェネリック引数
//! ty 5 fn 0 0 -> 1
//!
//! ga 0                        # 空。呼び出しの大半はこれ
//! ga 1 p0:11#0 1 p0:11#1 0    # (ジェネリック引数, 型索引) の並び
//!
//! native p0:19
//!   genargs p0:19#0
//!   self 1
//!   args 0 0
//!   rty 0
//!   body "return self + other;"
//!
//! fn p0:11
//!   genargs p0:11#0
//!   argc 2
//!   local 0 1
//!   local 1 0
//!   local 2 0
//!   bb 0
//!     _2 = add _1 _1
//!     call _0 = d p0:19 ga1 _1 _2 -> 1
//!   bb 1
//!     switch _2 0:2 else:3
//!   bb 2
//!     ret
//!   bb 3
//!     unreachable
//! ```
//!
//! span は載せていない。依存パッケージのソースは消費側に無く、
//! `.biwameta` 由来の外部シンボルも既に span がダミーなので、今は使い道がないため。
//! デバッグ情報を入れる段で版数を上げて足す。

mod decode;
mod encode;

use std::fmt;

use biwac_base::{IdentInterner, PackageId};
use biwac_dependency_metadata::SymbolIndexMap;
use biwac_hash::Hash64;

use crate::Mir;

pub const BIWAC_MIR_FORMAT_VERSION: u32 = 1;

pub const MIR_FILE_EXTENSION: &str = "biwamir";

/// [`encode`] に要る、MIR の外にある情報。
pub struct EncodeCtx<'a> {
    /// このパッケージの [`PackageId`]。`SELF` のシンボルはこれに読み替えられる。
    pub pkg_id: PackageId,

    /// 同じビルドで書いた `.biwameta` の SVH。
    /// 読み込み側がメタデータとの食い違いを検出するのに使う。
    pub meta_svh: Hash64,

    /// 自パッケージのローカル id を `.biwameta` のシンボル索引に読み替える表。
    ///
    /// [`decode`] した MIR にはもう `SELF` が無いので、
    /// それを書き直すだけなら `None` でよい。
    pub symbols: Option<&'a SymbolIndexMap>,

    /// 構造体のメンバ名を文字列に戻すのに使う。
    pub interner: &'a IdentInterner,
}

/// [`decode`] の結果。
pub struct DecodedMir {
    pub mir: Mir,

    /// 書いた時点でのそのパッケージの `.biwameta` の SVH。
    /// 呼び出し側が、いま手元にある `.biwameta` の SVH と照合する。
    pub meta_svh: Hash64,
}

/// `.biwamir` を読めなかった。
///
/// 起きるのはコンパイラのバグか、キャッシュが壊れているときだけなので、
/// 種別を細かく分けず、どの行で何が起きたかだけを持つ。
#[derive(Debug, Clone)]
pub struct MirDecodeError {
    /// 1 始まりの行番号。ファイル全体に関わるものは 0。
    pub line: usize,
    pub message: String,
}

impl fmt::Display for MirDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

pub fn encode(mir: &Mir, ctx: &EncodeCtx) -> String {
    encode::encode(mir, ctx)
}

pub fn decode(text: &str, interner: &mut IdentInterner) -> Result<DecodedMir, MirDecodeError> {
    decode::decode(text, interner)
}
