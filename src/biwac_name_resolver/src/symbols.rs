use crate::{FnDefContent, GlobalVarDecl, NativeFnDefContent, TypeDefContent};

pub(crate) mod expressions;
pub(crate) mod globals;
pub(crate) mod statements;

#[derive(Debug)]
pub enum ModSym {
    FnDef(FnDefContent),
    VarDecl(GlobalVarDecl),
    TypeDef(TypeDefContent),
    NativeFnDef(NativeFnDefContent),
}
