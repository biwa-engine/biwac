use biwac_hir::Ty;
use biwac_span::{LocalGenDefId, Span, ValDefId};

use crate::{Operand, Place, Rvalue};

/// 局所変数の番号。
///
/// `_0` は戻り値スロット、`_1 ..= arg_count` が引数である (rustc と同じ規約)。
/// メソッドなら `_1` が self にあたる。
///
/// HIR の [`biwac_span::VarId`] とは対応しない。
/// 引数でも宣言された変数でも、式の途中結果でも、区別なく local になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Local(u32);

impl Local {
    /// 戻り値スロット。
    pub const RETURN: Self = Self(0);

    #[inline]
    pub fn new(idx: u32) -> Self {
        Self(idx)
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// 基本ブロックの番号。
///
/// `bb0` が関数の入口である。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasicBlock(u32);

impl BasicBlock {
    /// 入口ブロック。
    pub const START: Self = Self(0);

    #[inline]
    pub fn new(idx: u32) -> Self {
        Self(idx)
    }

    #[inline]
    pub fn index(&self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub fn value(&self) -> u32 {
        self.0
    }
}

/// 関数 1 つ分の本体。
///
/// scene も普通の関数と同じくこれになる。
/// scene がエントリポイントであるかどうかは
/// [`biwac_scene::WellKnownScenes`] が既に持っているので、ここには重複させない。
#[derive(Debug, Clone)]
pub struct Body {
    pub def_id: ValDefId,

    /// 引数の個数。メソッドなら self を含む。
    /// `_1 ..= arg_count` が引数の local である。
    pub arg_count: usize,

    /// `Local` で添字を引く。`locals[0]` は戻り値スロット。
    pub locals: Vec<LocalDecl>,

    /// `BasicBlock` で添字を引く。`blocks[0]` が入口。
    pub blocks: Vec<BasicBlockData>,

    /// 多相なら空でない。
    ///
    /// 単相化はバックエンドではなく MIR→MIR のパスで行う。
    /// パッケージをまたぐ実体化には依存パッケージのジェネリックな本体が要るので、
    /// lowering の時点で単相化することはできない。
    pub genargs: Vec<LocalGenDefId>,

    pub span: Span,
}

impl Body {
    #[inline]
    pub fn local_decl(&self, local: Local) -> &LocalDecl {
        &self.locals[local.index()]
    }

    #[inline]
    pub fn block(&self, bb: BasicBlock) -> &BasicBlockData {
        &self.blocks[bb.index()]
    }

    /// 戻り値の型。
    #[inline]
    pub fn return_ty(&self) -> &Ty {
        &self.locals[Local::RETURN.index()].ty
    }

    /// 引数の local を宣言順に返す。
    pub fn arg_locals(&self) -> impl Iterator<Item = Local> + use<> {
        (1..=self.arg_count as u32).map(Local::new)
    }
}

#[derive(Debug, Clone)]
pub struct LocalDecl {
    pub ty: Ty,

    /// この local が生まれた位置。
    /// 引数・宣言された変数ならその宣言、式の途中結果ならその式を指す。
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BasicBlockData {
    pub stmts: Vec<Statement>,
    pub term: Terminator,
}

#[derive(Debug, Clone)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

/// 制御フローを動かさない操作。
///
/// 今のところ代入しか無い。
/// 破棄も記憶域の生死も biwa には無いので、rustc のように種類が増えない。
#[derive(Debug, Clone)]
pub enum StatementKind {
    Assign(Place, Rvalue),
}

#[derive(Debug, Clone)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}

/// 基本ブロックの末尾。ここでのみ制御が移る。
#[derive(Debug, Clone)]
pub enum TerminatorKind {
    Goto {
        target: BasicBlock,
    },

    /// 整数値による分岐。
    ///
    /// `if` も `while` の条件も、将来の `match` もこれになる。
    /// 2 分岐専用の形を別に作る理由が無い。
    SwitchInt {
        discr: Operand,
        targets: SwitchTargets,
    },

    /// 関数呼び出し。
    ///
    /// 呼び出しは制御フローを移すので文ではなく終端子である。
    /// 戻り値は `dest` に書かれ、制御は `target` に移る。
    Call {
        callee: Callee,
        args: Vec<Operand>,
        dest: Place,
        target: BasicBlock,
    },

    /// `_0` を返して関数を抜ける。
    Return,

    /// ここには到達しない。
    ///
    /// 値を返す関数の末尾など、lowering の都合で行き先が無いブロックに置く。
    Unreachable,
}

/// [`TerminatorKind::SwitchInt`] の行き先表。
///
/// `values[i]` に一致したとき `targets[i]` へ、
/// どれにも一致しなければ `otherwise` へ移る。
#[derive(Debug, Clone)]
pub struct SwitchTargets {
    values: Vec<u128>,
    targets: Vec<BasicBlock>,
    otherwise: BasicBlock,
}

impl SwitchTargets {
    pub fn new(values: Vec<u128>, targets: Vec<BasicBlock>, otherwise: BasicBlock) -> Self {
        assert_eq!(
            values.len(),
            targets.len(),
            "compiler bug: switch value and target count mismatched"
        );
        Self {
            values,
            targets,
            otherwise,
        }
    }

    /// `Bool` の分岐。偽なら `false_bb`、真なら `true_bb`。
    pub fn if_bool(true_bb: BasicBlock, false_bb: BasicBlock) -> Self {
        Self::new(vec![0], vec![false_bb], true_bb)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u128, BasicBlock)> + use<'_> {
        self.values
            .iter()
            .copied()
            .zip(self.targets.iter().copied())
    }

    pub fn otherwise(&self) -> BasicBlock {
        self.otherwise
    }

    /// 行き先すべて。
    pub fn all_targets(&self) -> impl Iterator<Item = BasicBlock> + use<'_> {
        self.targets.iter().copied().chain([self.otherwise])
    }
}

/// ジェネリック型への割り当て。
///
/// 位置ではなく [`LocalGenDefId`] との組で持つ。
/// 具体化は [`biwac_hir::Ty::embody_by_loc_gen_ty_id`] が
/// `LocalGenDefId` をキーに行うためで、宣言順を別に持ち回らずに済む。
/// [`LocalGenDefId`] 順に並ぶ (ビルドを決定論的にするため)。
pub type GenArgs = Vec<(LocalGenDefId, Ty)>;

/// 呼び出し先。
#[derive(Debug, Clone)]
pub enum Callee {
    /// 呼び先が静的に決まっている呼び出し。
    ///
    /// `genargs` はこの呼び出し位置でのジェネリック型への割り当てで、
    /// 単相化パスの入力になる。
    Direct { def_id: ValDefId, genargs: GenArgs },

    /// 関数を保持する値を通した呼び出し。
    Indirect(Operand),
}

impl Terminator {
    /// このブロックから移りうる行き先。
    pub fn successors(&self) -> Vec<BasicBlock> {
        match &self.kind {
            TerminatorKind::Goto { target } => vec![*target],
            TerminatorKind::SwitchInt { targets, .. } => targets.all_targets().collect(),
            TerminatorKind::Call { target, .. } => vec![*target],
            TerminatorKind::Return | TerminatorKind::Unreachable => vec![],
        }
    }
}

impl BasicBlockData {
    pub fn new(term: Terminator) -> Self {
        Self {
            stmts: Vec::new(),
            term,
        }
    }
}

impl StatementKind {
    pub fn with_span(self, span: Span) -> Statement {
        Statement { kind: self, span }
    }
}

impl TerminatorKind {
    pub fn with_span(self, span: Span) -> Terminator {
        Terminator { kind: self, span }
    }
}
