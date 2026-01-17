pub mod binary;
pub mod primary;
pub mod unary;

use crate::{
    packager::resolver::symbols::expressions,
    validator::{types::AbsoluteType, AbsoluteId, Env, ValidateError, Variable},
};

#[derive(Debug, Clone)]
pub enum Primary {
    Literal(Literal),
    Variable(Variable),
    FnCall(FnCall),
    LanglibfnCall(LanglibfnCall),
    MemberAccess(MemberAccess),
}

#[derive(Debug, Clone)]
pub struct MemberAccess {
    pub str: (AbsoluteType, Box<Exprs>),
    pub member: (AbsoluteType, usize),
}

#[derive(Debug, Clone)]
pub struct FnCall {
    pub absid: AbsoluteId,
    pub args: Vec<(AbsoluteType, Exprs)>,
    pub rtype: Option<AbsoluteType>,
}

#[derive(Debug, Clone)]
pub struct LanglibfnCall {
    pub id: String,
    pub args: Vec<(AbsoluteType, Exprs)>,
    pub rtype: Option<AbsoluteType>,
}

#[derive(Debug, Clone)]
pub enum AssignableExprs {
    Variable(Variable),
    MemberAccess(MemberAccess),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Uint(u32),
    // Float(f64),
    String(usize),
    Bool(bool),
    Struct(AbsoluteId, Vec<(usize, AbsoluteType, Exprs)>),
}

#[derive(Debug, Clone)]
pub enum Exprs {
    Primary(Primary),
    Unary(UnOperator, Box<Exprs>),
    Binary(BinOperator, Box<Exprs>, Box<Exprs>),
}

#[derive(Debug, Clone)]
pub enum BinOperator {
    Add,
    Sub,
    SMul,
    SDiv,
    SMod,
    UMul,
    UDiv,
    UMod,
    SGt,
    SLt,
    SGe,
    SLe,
    UGt,
    ULt,
    UGe,
    ULe,
    Eq,
    Ne,
}

#[derive(Debug, Clone)]
pub enum UnOperator {
    Neg,
    // Not
    // NOTE: 暗黙的型変換をする演算もここかもしれない
    // Sitofp, // signed int to floating point
    // Uitofp, // unsigned int to floating point
}

impl expressions::Exprs {
    pub fn validate(&self, env: &mut Env) -> Result<(AbsoluteType, Exprs), ValidateError> {
        match self {
            expressions::Exprs::Primary(prim) => primary::validate(prim, env),
            expressions::Exprs::Unary(op, expr) => unary::validate(op, expr, env),
            expressions::Exprs::Binary(op, left, right) => binary::validate(op, left, right, env),
        }
    }
}
