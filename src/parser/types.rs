use crate::parser::symbols::QualifiedId;

#[derive(Debug, Clone)]
pub enum Type {
    Primitive(PrimitiveType),
    List(Box<Type>),
    Defined(QualifiedId),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveType {
    Uint,
    Int,
    // Float,
    Bool,
    // String,
    // Path
}
