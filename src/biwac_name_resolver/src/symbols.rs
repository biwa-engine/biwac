use crate::{FnDefContent, GlobalVarDecl, TypeDefContent};

pub(crate) mod expressions;
pub(crate) mod globals;
pub(crate) mod statements;

#[derive(Debug)]
pub enum ModSym {
    FnDef(FnDefContent),
    VarDecl(GlobalVarDecl),
    TypeDef(TypeDefContent),
}
