use biwac_base::Span;
use biwac_parser::{BinOperator, BoolLiteral, Ident, IntegerLiteral, StringLiteral, UnOperator};

use crate::{
    AssocId, ExprId, FnId, LocVarId, ResolvedVar, RsvResult, Stmt, TryResolve, TypId,
    context::ResolvedFn,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    pub expr: ExprVal,
    pub id: ExprId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Primary {
    Literal(Literal),
    Variable(Variable), // TODO: support using external module variables
    FnCall(FnCall),
    MemberAccess(MemberAccess),
    IfExpr(IfExpr),
    Block(BlockExpr),
    MethodCall(MethodCall),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockExpr {
    pub stmts: Vec<Stmt>,
    pub expr: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then: BlockExpr,
    // pub else_ifs: Vec<(Expr, BlockExpr)>,
    pub els: BlockExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub id: ResolvedVar,
    pub span: Span,
}

impl Primary {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(l) => l.span(),
            Self::Variable(v) => v.span.clone(),
            Self::FnCall(f) => f.span.clone(),
            Self::MemberAccess(m) => m.span.clone(),
            Self::IfExpr(i) => i.span.clone(),
            Self::Block(b) => b.span.clone(),
            Self::MethodCall(m) => m.span.clone(),
        }
    }
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
pub struct StructLiteral {
    pub id: TypId,
    pub members: Vec<(Ident, Expr)>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnCall {
    pub callee: Callee,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Callee {
    Var(LocVarId),
    Fn(FnId),
    Assoc(TypId, AssocId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccess {
    pub left: Box<Expr>,
    pub member: Ident,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodCall {
    pub left: Box<Expr>,
    pub method: Ident,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaryExpr {
    pub op: UnOperator,
    pub right: Box<Expr>,
    pub span: Span,
}

impl TryResolve<biwac_parser::Primary> for Primary {
    fn try_resolve<'mctx>(
        value: biwac_parser::Primary,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        match value {
            biwac_parser::Primary::Literal(l) => Ok(Self::Literal(Literal::try_resolve(l, fctx)?)),
            biwac_parser::Primary::Variable(v) => Ok(Self::Variable(Variable {
                id: fctx.try_resolve_variable(&v)?,
                span: v.span,
            })),
            biwac_parser::Primary::FnCall(f) => Ok(Self::FnCall(FnCall::try_resolve(f, fctx)?)),
            biwac_parser::Primary::MemberAccess(m) => {
                Ok(Self::MemberAccess(MemberAccess::try_resolve(m, fctx)?))
            }
            biwac_parser::Primary::MethodCall(m) => {
                Ok(Self::MethodCall(MethodCall::try_resolve(m, fctx)?))
            }
            biwac_parser::Primary::IfExpr(i) => Ok(Self::IfExpr(IfExpr::try_resolve(i, fctx)?)),
            biwac_parser::Primary::Block(b) => Ok(Self::Block(BlockExpr::try_resolve(b, fctx)?)),
        }
    }
}

impl TryResolve<biwac_parser::IfExpr> for IfExpr {
    fn try_resolve<'mctx>(
        value: biwac_parser::IfExpr,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> RsvResult<Self> {
        Ok(Self {
            cond: Box::new(Expr::try_resolve(*value.cond, fctx)?),
            then: BlockExpr::try_resolve(value.then, fctx)?,
            els: BlockExpr::try_resolve(value.els, fctx)?,
            span: value.span,
        })
    }
}

impl TryResolve<biwac_parser::BlockExpr> for BlockExpr {
    fn try_resolve<'mctx>(
        value: biwac_parser::BlockExpr,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> RsvResult<Self> {
        Ok(Self {
            stmts: value
                .stmts
                .into_iter()
                .map(|stmt| Stmt::try_resolve(stmt, fctx))
                .collect::<RsvResult<_>>()?,
            expr: Box::new(Expr::try_resolve(*value.expr, fctx)?),
            span: value.span,
        })
    }
}

impl TryResolve<biwac_parser::FnCall> for FnCall {
    fn try_resolve<'mctx>(
        value: biwac_parser::FnCall,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        Ok(Self {
            // TODO: 関数呼び出し側にもジェネリック型注釈を導入
            callee: match fctx.try_resolve_fn(&value.qualed_id)? {
                ResolvedFn::Var(v) => Callee::Var(v),
                ResolvedFn::Fn(id) => Callee::Fn(id),
                ResolvedFn::Assoc(typid, associd) => Callee::Assoc(typid, associd),
            },
            args: value
                .args
                .into_iter()
                .map(|expr| Expr::try_resolve(expr, fctx))
                .collect::<RsvResult<Vec<Expr>>>()?,
            span: value.span,
        })
    }
}

impl TryResolve<biwac_parser::MemberAccess> for MemberAccess {
    fn try_resolve<'mctx>(
        value: biwac_parser::MemberAccess,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        Ok(Self {
            span: value.span(),
            left: Box::new(Expr::try_resolve(*value.left, fctx)?),
            member: value.member,
        })
    }
}

impl TryResolve<biwac_parser::MethodCall> for MethodCall {
    fn try_resolve<'mctx>(
        value: biwac_parser::MethodCall,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        Ok(Self {
            span: value.span.clone(),
            left: Box::new(Expr::try_resolve(*value.left, fctx)?),
            method: value.method,
            args: value
                .args
                .into_iter()
                .map(|a| Expr::try_resolve(a, fctx))
                .collect::<RsvResult<_>>()?,
        })
    }
}

impl TryResolve<biwac_parser::Literal> for Literal {
    fn try_resolve<'mctx>(
        value: biwac_parser::Literal,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        match value {
            biwac_parser::Literal::Integer(u) => Ok(Self::Integer(u)),
            biwac_parser::Literal::String(s) => Ok(Self::String(s)),
            biwac_parser::Literal::Bool(b) => Ok(Self::Bool(b)),
            biwac_parser::Literal::Struct(s) => Ok(Self::Struct(StructLiteral {
                // NOTE: struct リテラルにはジェネリック型注釈が必要か否か
                id: fctx.try_resolve_deftyp(&s.qualid)?,
                members: s
                    .members
                    .into_iter()
                    .map(|(id, expr)| match Expr::try_resolve(*expr, fctx) {
                        Ok(expr) => Ok((id, expr)),
                        Err(e) => Err(e),
                    })
                    .collect::<RsvResult<Vec<(Ident, Expr)>>>()?,
                span: s.span,
            })),
        }
    }
}

impl TryResolve<biwac_parser::Exprs> for Expr {
    fn try_resolve<'mctx>(
        value: biwac_parser::Exprs,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        match value {
            biwac_parser::Exprs::Primary(prim) => Ok(Self {
                expr: ExprVal::Primary(Primary::try_resolve(prim, fctx)?),
                id: fctx.new_expr_id(),
            }),
            biwac_parser::Exprs::Unary(u) => Ok(Self {
                expr: ExprVal::Unary(UnaryExpr {
                    op: u.op,
                    right: Box::new(Self::try_resolve(*u.right, fctx)?),
                    span: u.span,
                }),
                id: fctx.new_expr_id(),
            }),
            biwac_parser::Exprs::Binary(b) => Ok(Self {
                expr: ExprVal::Binary(BinaryExpr {
                    op: b.op,
                    left: Box::new(Self::try_resolve(*b.left, fctx)?),
                    right: Box::new(Self::try_resolve(*b.right, fctx)?),
                }),
                id: fctx.new_expr_id(),
            }),
        }
    }
}
