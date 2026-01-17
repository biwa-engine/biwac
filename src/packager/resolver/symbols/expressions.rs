use crate::{
    packager::{
        resolver::{ResolveError, TryResolve},
        ModulePath,
    },
    parser::{
        self,
        symbols::{
            expressions::{BinOperator, UnOperator},
            QualifiedId,
        },
    },
    validator::AbsoluteId,
};

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
    // LanglibfnCall(LanglibfnCall),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Uint(u32),
    // Float(f64),
    String(String),
    Bool(bool),
    Struct(AbsoluteId, Vec<(String, Box<Exprs>)>),
}

#[derive(Debug, Clone)]
pub struct FnCall {
    pub absid: AbsoluteId,
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

// #[derive(Debug, Clone)]
// pub struct LanglibfnCall {
//     pub id: String,
//     pub args: Vec<Exprs>,
// }

impl TryResolve<parser::symbols::expressions::Primary> for Primary {
    fn try_resolve(
        value: parser::symbols::expressions::Primary,
        imports: &[QualifiedId],
        modpath: &ModulePath,
    ) -> Result<Self, ResolveError> {
        match value {
            parser::symbols::expressions::Primary::Literal(l) => {
                Ok(Self::Literal(Literal::try_resolve(l, imports, modpath)?))
            }
            parser::symbols::expressions::Primary::Variable(v) => Ok(Self::Variable(v)),
            parser::symbols::expressions::Primary::FnCall(f) => {
                Ok(Self::FnCall(FnCall::try_resolve(f, imports, modpath)?))
            }
            parser::symbols::expressions::Primary::MemberAccess(m) => Ok(Self::MemberAccess(
                MemberAccess::try_resolve(m, imports, modpath)?,
            )),
            _ => todo!(),
        }
    }
}

impl TryResolve<parser::symbols::expressions::FnCall> for FnCall {
    fn try_resolve(
        value: parser::symbols::expressions::FnCall,
        imports: &[QualifiedId],
        modpath: &ModulePath,
    ) -> Result<Self, ResolveError> {
        Ok(Self {
            absid: AbsoluteId::try_resolve(value.qualed_id, imports, modpath)?,
            args: value
                .args
                .into_iter()
                .map(|expr| Exprs::try_resolve(expr, imports, modpath))
                .collect::<Result<Vec<Exprs>, ResolveError>>()?,
        })
    }
}

impl TryResolve<parser::symbols::expressions::MemberAccess> for MemberAccess {
    fn try_resolve(
        value: parser::symbols::expressions::MemberAccess,
        imports: &[QualifiedId],
        modpath: &ModulePath,
    ) -> Result<Self, ResolveError> {
        Ok(Self {
            left: Box::new(Exprs::try_resolve(*value.left, imports, modpath)?),
            member: value.member,
        })
    }
}

impl TryResolve<parser::symbols::expressions::Literal> for Literal {
    fn try_resolve(
        value: parser::symbols::expressions::Literal,
        imports: &[QualifiedId],
        modpath: &ModulePath,
    ) -> Result<Self, ResolveError> {
        match value {
            parser::symbols::expressions::Literal::Uint(u) => Ok(Self::Uint(u)),
            parser::symbols::expressions::Literal::String(s) => Ok(Self::String(s)),
            parser::symbols::expressions::Literal::Bool(b) => Ok(Self::Bool(b)),
            parser::symbols::expressions::Literal::Struct(qualid, mems) => Ok(Self::Struct(
                AbsoluteId::try_resolve(qualid, imports, modpath)?,
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

impl TryResolve<parser::symbols::expressions::Exprs> for Exprs {
    fn try_resolve(
        value: parser::symbols::expressions::Exprs,
        imports: &[QualifiedId],
        modpath: &ModulePath,
    ) -> Result<Self, ResolveError> {
        match value {
            parser::symbols::expressions::Exprs::Primary(prim) => {
                Ok(Self::Primary(Primary::try_resolve(prim, imports, modpath)?))
            }
            parser::symbols::expressions::Exprs::Unary(op, expr) => Ok(Self::Unary(
                op,
                Box::new(Self::try_resolve(*expr, imports, modpath)?),
            )),
            parser::symbols::expressions::Exprs::Binary(op, left, right) => Ok(Self::Binary(
                op,
                Box::new(Self::try_resolve(*left, imports, modpath)?),
                Box::new(Self::try_resolve(*right, imports, modpath)?),
            )),
        }
    }
}
