use biwac_base::Span;
use biwac_parser::{BinOperator, BoolLiteral, Ident, IntegerLiteral, StringLiteral, UnOperator};

use crate::{AbsId, LocVarId, RsvResult, TryResolve, context::ResolvedIdent};

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum Primary {
    Literal(Literal),
    Variable(Ident), // TODO: support using external module variables
    FnCall(FnCall),
    MemberAccess(MemberAccess),
    // MethodCall(MethodCall),
}

impl Primary {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal(l) => l.span(),
            Self::Variable(v) => v.span.clone(),
            Self::FnCall(f) => f.span.clone(),
            Self::MemberAccess(m) => m.span.clone(),
        }
    }
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct StructLiteral {
    pub id: AbsId,
    pub members: Vec<(Ident, Exprs)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnCall {
    pub callee: Callee,
    pub args: Vec<Exprs>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Callee {
    Var(LocVarId),
    Abs(AbsId),
}

#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub left: Box<Exprs>,
    pub member: Ident,
    pub span: Span,
}

// #[derive(Debug, Clone)]
// pub struct MethodCall {
//     pub left: Box<Exprs>,
//     pub method: String,
//     pub args: Vec<Exprs>,
// }

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnOperator,
    pub right: Box<Exprs>,
    pub span: Span,
}

impl TryResolve<biwac_parser::Primary> for Primary {
    fn try_resolve<'mctx>(
        value: biwac_parser::Primary,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        match value {
            biwac_parser::Primary::Literal(l) => Ok(Self::Literal(Literal::try_resolve(l, fctx)?)),
            biwac_parser::Primary::Variable(v) => Ok(Self::Variable(v)),
            biwac_parser::Primary::FnCall(f) => Ok(Self::FnCall(FnCall::try_resolve(f, fctx)?)),
            biwac_parser::Primary::MemberAccess(m) => {
                Ok(Self::MemberAccess(MemberAccess::try_resolve(m, fctx)?))
            }
        }
    }
}

impl TryResolve<biwac_parser::FnCall> for FnCall {
    fn try_resolve<'mctx>(
        value: biwac_parser::FnCall,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        Ok(Self {
            callee: match fctx.try_resolve_qualid(&value.qualed_id)? {
                ResolvedIdent::Var(v) => Callee::Var(v),
                ResolvedIdent::Abs(id) => Callee::Abs(id),
            },
            args: value
                .args
                .into_iter()
                .map(|expr| Exprs::try_resolve(expr, fctx))
                .collect::<RsvResult<Vec<Exprs>>>()?,
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
            left: Box::new(Exprs::try_resolve(*value.left, fctx)?),
            member: value.member,
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
                id: fctx.try_resolve_deftyp(&s.qualid)?,
                members: s
                    .members
                    .into_iter()
                    .map(|(id, expr)| match Exprs::try_resolve(*expr, fctx) {
                        Ok(expr) => Ok((id, expr)),
                        Err(e) => Err(e),
                    })
                    .collect::<RsvResult<Vec<(Ident, Exprs)>>>()?,
                span: s.span,
            })),
        }
    }
}

impl TryResolve<biwac_parser::Exprs> for Exprs {
    fn try_resolve<'mctx>(
        value: biwac_parser::Exprs,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        match value {
            biwac_parser::Exprs::Primary(prim) => {
                Ok(Self::Primary(Primary::try_resolve(prim, fctx)?))
            }
            biwac_parser::Exprs::Unary(u) => Ok(Self::Unary(UnaryExpr {
                op: u.op,
                right: Box::new(Self::try_resolve(*u.right, fctx)?),
                span: u.span,
            })),
            biwac_parser::Exprs::Binary(b) => Ok(Self::Binary(BinaryExpr {
                op: b.op,
                left: Box::new(Self::try_resolve(*b.left, fctx)?),
                right: Box::new(Self::try_resolve(*b.right, fctx)?),
            })),
        }
    }
}
