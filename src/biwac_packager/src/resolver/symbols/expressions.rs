use biwac_base::ModPath;
use biwac_parser::{BinOperator, QualifiedId, UnOperator};

use crate::resolver::{AbsId, ResolveError, TryResolve};

#[derive(Debug, Clone)]
pub enum Exprs {
    Primary(Primary),
    Unary(UnOperator, Box<Self>),
    Binary(BinOperator, Box<Self>, Box<Self>),
}

#[derive(Debug, Clone)]
pub enum Primary {
    Literal(Literal),
    Variable(String), // TODO: support using external module variables
    FnCall(FnCall),
    MemberAccess(MemberAccess),
    // MethodCall(MethodCall),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Integer(u64),
    // Float(f64),
    String(String),
    Bool(bool),
    Struct(AbsId, Vec<(String, Box<Exprs>)>),
}

#[derive(Debug, Clone)]
pub struct FnCall {
    pub absid: AbsId,
    pub args: Vec<Exprs>,
}

#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub left: Box<Exprs>,
    pub member: String,
}

// #[derive(Debug, Clone)]
// pub struct MethodCall {
//     pub left: Box<Exprs>,
//     pub method: String,
//     pub args: Vec<Exprs>,
// }

impl TryResolve<biwac_parser::Primary> for Primary {
    fn try_resolve(
        value: biwac_parser::Primary,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        match value {
            biwac_parser::Primary::Literal(l) => {
                Ok(Self::Literal(Literal::try_resolve(l, imports, modpath)?))
            }
            biwac_parser::Primary::Variable(v) => Ok(Self::Variable(v)),
            biwac_parser::Primary::FnCall(f) => {
                Ok(Self::FnCall(FnCall::try_resolve(f, imports, modpath)?))
            }
            biwac_parser::Primary::MemberAccess(m) => Ok(Self::MemberAccess(
                MemberAccess::try_resolve(m, imports, modpath)?,
            )),
            _ => todo!(),
        }
    }
}

impl TryResolve<biwac_parser::FnCall> for FnCall {
    fn try_resolve(
        value: biwac_parser::FnCall,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        Ok(Self {
            absid: AbsId::try_resolve(value.qualed_id, imports, modpath)?,
            args: value
                .args
                .into_iter()
                .map(|expr| Exprs::try_resolve(expr, imports, modpath))
                .collect::<Result<Vec<Exprs>, ResolveError>>()?,
        })
    }
}

impl TryResolve<biwac_parser::MemberAccess> for MemberAccess {
    fn try_resolve(
        value: biwac_parser::MemberAccess,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        Ok(Self {
            left: Box::new(Exprs::try_resolve(*value.left, imports, modpath)?),
            member: value.member,
        })
    }
}

impl TryResolve<biwac_parser::Literal> for Literal {
    fn try_resolve(
        value: biwac_parser::Literal,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        match value {
            biwac_parser::Literal::Integer(u) => Ok(Self::Integer(u)),
            biwac_parser::Literal::String(s) => Ok(Self::String(s)),
            biwac_parser::Literal::Bool(b) => Ok(Self::Bool(b)),
            biwac_parser::Literal::Struct(qualid, mems) => Ok(Self::Struct(
                AbsId::try_resolve(qualid, imports, modpath)?,
                mems.into_iter()
                    .map(
                        |(id, expr)| match Exprs::try_resolve(*expr, imports, modpath) {
                            Ok(expr) => Ok((id, expr)),
                            Err(e) => Err(e),
                        },
                    )
                    .collect::<Result<Vec<(String, Exprs)>, ResolveError>>()?
                    .into_iter()
                    .map(|(id, expr)| (id, Box::new(expr)))
                    .collect(),
            )),
        }
    }
}

impl TryResolve<biwac_parser::Exprs> for Exprs {
    fn try_resolve(
        value: biwac_parser::Exprs,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        match value {
            biwac_parser::Exprs::Primary(prim) => {
                Ok(Self::Primary(Primary::try_resolve(prim, imports, modpath)?))
            }
            biwac_parser::Exprs::Unary(op, expr) => Ok(Self::Unary(
                op,
                Box::new(Self::try_resolve(*expr, imports, modpath)?),
            )),
            biwac_parser::Exprs::Binary(op, left, right) => Ok(Self::Binary(
                op,
                Box::new(Self::try_resolve(*left, imports, modpath)?),
                Box::new(Self::try_resolve(*right, imports, modpath)?),
            )),
        }
    }
}
