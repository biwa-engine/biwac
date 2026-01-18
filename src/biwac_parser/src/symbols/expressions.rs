pub mod arithmetic;
pub mod equality;
pub mod multiplication;
pub mod postfix;
pub mod primary;
pub mod relational;
pub mod unary;

use crate::{ParseError, parser::TokenStream, symbols::QualifiedId};

#[derive(Debug, Clone)]
pub enum Primary {
    Literal(Literal),
    Variable(String),
    FnCall(FnCall),
    MemberAccess(MemberAccess),
    // MethodCall(MethodCall),
    LanglibfnCall(LanglibfnCall),
}

#[derive(Debug, Clone)]
pub struct LanglibfnCall {
    pub id: String,
    pub args: Vec<Exprs>,
}

#[derive(Debug, Clone)]
pub struct FnCall {
    pub qualed_id: QualifiedId,
    pub args: Vec<Exprs>,
}

// id . (member | methodcall ) . (member | methodcall) . ...
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

#[derive(Debug, Clone)]
pub enum Literal {
    Integer(u64),
    // Float(f64),
    String(String),
    Bool(bool),
    Struct(QualifiedId, Vec<(String, Box<Exprs>)>),
}

#[derive(Debug, Clone)]
pub enum Exprs {
    Primary(Primary),
    Unary(UnOperator, Box<Exprs>),
    Binary(BinOperator, Box<Exprs>, Box<Exprs>),
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum UnOperator {
    Neg, // -
         // Not, // !
}

impl<'t> TokenStream<'t> {
    pub(crate) fn consume_expression(&mut self) -> Result<Exprs, ParseError> {
        self.consume_equality_expression()
    }
}
