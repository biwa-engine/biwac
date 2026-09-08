use std::cell::OnceCell;

use biwac_ast::{
    BinOperator, BoolLiteral, FloatLiteral, IntegerLiteral, StringLiteral, UnOperator, VariantShape,
};
use biwac_base::InternedIdent;
use biwac_span::{Span, TyDefId, ValDefId, VarId, VariantDefId};

use crate::{Ident, Stmt, Ty};

// ExprId
// function local expression id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExprId(usize);

#[derive(Debug, Clone)]
pub struct Expr {
    pub expr: ExprVal,
    pub id: ExprId,
}

impl ExprId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone)]
pub enum ExprVal {
    Primary(Primary),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
}

impl Expr {
    pub fn span(&self) -> Span {
        match &self.expr {
            ExprVal::Primary(p) => p.span(),
            ExprVal::Unary(u) => u.span.clone(),
            ExprVal::Binary(b) => b.span(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Primary {
    Literal(Literal),
    Variable(Variable), // TODO: support using external module variables
    FnCall(FnCall),
    MemberAccess(MemberAccess),
    IfExpr(IfExpr),
    Match(MatchExpr),
    Block(BlockExpr),
    MethodCall(MethodCall),
    /// enum のバリアントの構築。
    ///
    /// 構文の上では関数呼び出し・構造体リテラル・変数参照のいずれかだが、
    /// パスがバリアントに解決されたものは lowering でこれになる。
    VariantCtor(VariantCtor),
}

#[derive(Debug, Clone)]
pub struct BlockExpr {
    pub stmts: Vec<Stmt>,
    pub expr: Box<Expr>,
    pub span: Span,
}

/// enum のバリアントの構築。
///
/// 親の enum とフィールドの宣言順は **型推論が埋める** (`resolved`)。
/// 外部パッケージの enum は lowering の時点ではまだ HIR に載っていない
/// (`TyCtx` が `.biwameta` から遅延ロードする) ので、
/// 自パッケージと外部で経路を分けないためにここで解決を遅らせている。
#[derive(Debug, Clone)]
pub struct VariantCtor {
    pub variant: VariantDefId,
    /// 書かれたとおりの実引数。
    pub fields: VariantCtorFields,
    /// 書き方。宣言と一致しているかを型推論が検査する。
    pub shape: VariantShape,
    pub span: Span,
    pub resolved: OnceCell<ResolvedVariant>,
}

#[derive(Debug, Clone)]
pub enum VariantCtorFields {
    /// `Color::Red`
    Unit,
    /// `Color::Rgb(a, b, c)` — 宣言順の位置で対応する
    Positional(Vec<Expr>),
    /// `Color::Named { name = x }` — 名前で対応する
    Named(Vec<(Ident, Expr)>),
}

/// 型推論が解決したバリアントの素性。
#[derive(Debug, Clone)]
pub struct ResolvedVariant {
    pub enum_def_id: TyDefId,
    /// 宣言順の添字。そのままタグの値になる。
    pub index: u32,
    /// 宣言順のフィールド名。
    pub field_names: Vec<InternedIdent>,
}

/// `match` の式形。
#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchExprArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchExprArm {
    pub pattern: Pattern,
    pub body: BlockExpr,
    pub span: Span,
}

/// 名前解決済みのパターン。
///
/// 今回はネストを入れないので、バリアントのフィールドに対しては
/// 「束縛する」か「無視する」かのどちらかしかない。
#[derive(Debug, Clone)]
pub enum Pattern {
    /// `_`。何にでも当たり、何も束縛しない。
    Wildcard(Span),
    /// バリアントに解決されなかった単独の識別子。何にでも当たり、値を束縛する。
    Binding(VarId, Span),
    Variant(VariantPattern),
}

impl Pattern {
    pub fn span(&self) -> Span {
        match self {
            Self::Wildcard(span) => span.clone(),
            Self::Binding(_, span) => span.clone(),
            Self::Variant(v) => v.span.clone(),
        }
    }

    /// このパターンが必ず当たるか。網羅性の判定に使う。
    pub fn is_irrefutable(&self) -> bool {
        matches!(self, Self::Wildcard(_) | Self::Binding(_, _))
    }
}

/// `VariantCtor` と同じ理由で、解決は型推論に任せる。
#[derive(Debug, Clone)]
pub struct VariantPattern {
    pub variant: VariantDefId,
    pub fields: PatternFields,
    pub shape: VariantShape,
    pub span: Span,
    pub resolved: OnceCell<ResolvedVariant>,
}

#[derive(Debug, Clone)]
pub enum PatternFields {
    Unit,
    /// `Color::Rgb(r, _, b)`
    Positional(Vec<FieldBinding>),
    /// `Color::Named { name = n, alpha }`
    Named(Vec<(Ident, FieldBinding)>),
}

/// バリアントのフィールド 1 つに対する束縛。
///
/// ネストしたパターンは入れないので、束縛か無視のどちらかしかない。
#[derive(Debug, Clone)]
pub enum FieldBinding {
    /// `_`
    Ignore(Span),
    Bind(VarId, Span),
}

impl FieldBinding {
    pub fn var_id(&self) -> Option<VarId> {
        match self {
            Self::Ignore(_) => None,
            Self::Bind(var_id, _) => Some(*var_id),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Ignore(span) | Self::Bind(_, span) => span.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then: BlockExpr,
    // pub else_ifs: Vec<(Expr, BlockExpr)>,
    pub els: BlockExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub id: VarIdKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarIdKind {
    Local(VarId),
    Global(ValDefId),
}

impl Primary {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(l) => l.span(),
            Self::Variable(v) => v.span.clone(),
            Self::FnCall(f) => f.span.clone(),
            Self::MemberAccess(m) => m.span.clone(),
            Self::IfExpr(i) => i.span.clone(),
            Self::Match(m) => m.span.clone(),
            Self::VariantCtor(v) => v.span.clone(),
            Self::Block(b) => b.span.clone(),
            Self::MethodCall(m) => m.span.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Literal {
    Integer(IntegerLiteral),
    Float(FloatLiteral),
    String(StringLiteral),
    Bool(BoolLiteral),
    Struct(StructLiteral),
}

impl Literal {
    pub fn span(&self) -> Span {
        match self {
            Self::Integer(i) => i.span.clone(),
            Self::Float(f) => f.span.clone(),
            Self::String(s) => s.span.clone(),
            Self::Bool(b) => b.span.clone(),
            Self::Struct(s) => s.span.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructLiteral {
    pub tid: TyDefId,
    pub members: Vec<(Ident, Expr)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnCall {
    pub callee: Callee,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Callee {
    Var(VarId),
    Fn(ValDefId),
    /// 型を通した関連関数の呼び出し (`Character::new(..)`)。
    ///
    /// `self_ty` は**呼び出し位置に書かれた型**である。
    /// `Callee::Fn` と違ってこれを残すのは、型エイリアスが
    /// 型引数を書き込んでいることがあるからである。
    ///
    /// ```text
    /// type CharacterBiwa = Character[BiwaCharacterProps];
    /// CharacterBiwa::new(..)   // self_ty = Character[BiwaCharacterProps]
    /// ```
    ///
    /// パスの最後のセグメントだけを見ると `Character::new` に潰れてしまい、
    /// `BiwaCharacterProps` がどこにも残らない。
    /// エイリアスの展開 (`alias_expansion`) はこの `self_ty` にも及ぶので、
    /// 推論の時点では右辺に置き換わっている。
    ///
    /// なお呼び出し位置に型引数を書く構文はまだ無いので、
    /// エイリアスを経由しない `Character::new(..)` の `self_ty` は
    /// 型引数が空のままである。使えるかどうかは推論側が判断する。
    AssocFn {
        def_id: ValDefId,
        self_ty: Ty,
    },
}

#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub left: Box<Expr>,
    pub member: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodCall {
    pub left: Box<Expr>,
    pub method: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
    pub def_id: OnceCell<ValDefId>,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinOperator,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl BinaryExpr {
    pub fn span(&self) -> Span {
        Span::merge(&self.left.span(), &self.right.span())
    }
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnOperator,
    pub right: Box<Expr>,
    pub span: Span,
}
