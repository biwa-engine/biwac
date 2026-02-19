pub mod arithmetic;
pub mod block;
pub mod equality;
pub mod if_expr;
pub mod multiplication;
pub mod postfix;
pub mod primary;
pub mod relational;
pub mod unary;

use biwac_base::Span;

use crate::{
    BlockExpr, Ident, IfExpr, ParseError,
    parser::TokenStream,
    symbols::{QualifiedId, globals::FnParseCtx},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Primary {
    Literal(Literal),
    Variable(Ident),
    FnCall(FnCall),
    MemberAccess(MemberAccess),
    IfExpr(IfExpr),
    Block(BlockExpr),
    MethodCall(MethodCall),
}

impl Primary {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(l) => l.span(),
            Self::Variable(v) => v.span.clone(),
            Self::FnCall(f) => f.span.clone(),
            Self::MemberAccess(m) => m.span(),
            Self::IfExpr(i) => i.span.clone(),
            Self::Block(b) => b.span.clone(),
            Self::MethodCall(m) => m.span.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnCall {
    pub qualed_id: QualifiedId,
    pub args: Vec<Exprs>,
    pub span: Span,
}

// id . (member | methodcall ) . (member | methodcall) . ...
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccess {
    pub left: Box<Exprs>,
    pub member: Ident,
}

impl MemberAccess {
    pub fn span(&self) -> Span {
        Span::merge(&self.left.span(), &self.member.span)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodCall {
    pub left: Box<Exprs>,
    pub method: Ident,
    pub args: Vec<Exprs>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    Integer(IntegerLiteral),
    // Float(f64),
    String(StringLiteral),
    Bool(BoolLiteral),
    Struct(StructLiteral),
}

impl Literal {
    pub fn span(&self) -> Span {
        match self {
            Self::Integer(i) => i.span.clone(),
            Self::String(s) => s.span.clone(),
            Self::Bool(b) => b.span.clone(),
            Self::Struct(s) => s.span.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegerLiteral {
    pub val: u64,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoolLiteral {
    pub val: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringLiteral {
    pub val: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLiteral {
    pub qualid: QualifiedId,
    pub members: Vec<(Ident, Box<Exprs>)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exprs {
    Primary(Primary),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
}

impl Exprs {
    pub fn span(&self) -> Span {
        match self {
            Self::Primary(p) => p.span(),
            Self::Unary(u) => u.span.clone(),
            Self::Binary(b) => b.span(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOperator {
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    Mod, // %
    Gt,  // >
    Lt,  // <
    Ge,  // >=
    Le,  // <=
    Eq,  // ==
    Ne,  // !=
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryExpr {
    pub op: BinOperator,
    pub left: Box<Exprs>,
    pub right: Box<Exprs>,
}

impl BinaryExpr {
    pub fn span(&self) -> Span {
        Span::merge(&self.left.span(), &self.right.span())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOperator {
    Neg, // -
         // Not, // !
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpr {
    pub op: UnOperator,
    pub right: Box<Exprs>,
    pub span: Span,
}

impl<'t> TokenStream<'t> {
    pub(crate) fn consume_expression(&mut self, ctx: &FnParseCtx) -> Result<Exprs, ParseError> {
        self.consume_equality_expression(ctx)
    }
}
