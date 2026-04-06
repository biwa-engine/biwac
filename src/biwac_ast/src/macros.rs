use crate::{BoolLiteral, Ident, IntegerLiteral, StringLiteral};

#[derive(Debug, Clone)]
pub struct CompilerFlag {
    pub flag: Ident,
    pub args: Vec<CompilerFlagArg>,
}

#[derive(Debug, Clone)]
pub struct CompilerFlagArg {
    pub arg: Ident,
    pub val: Option<CompilerFlagLiteral>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerFlagLiteral {
    Integer(IntegerLiteral),
    // Float(f64),
    String(StringLiteral),
    Bool(BoolLiteral),
}
