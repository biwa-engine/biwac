use biwac_base::ModPath;

use crate::{
    FnDefContent, GlobalVarDec, TypeDefContent,
    resolver::{AbsId, ResolveError, TryResolveWithId},
};

pub(crate) mod expressions;
pub(crate) mod globals;
pub(crate) mod statements;

#[derive(Debug)]
pub enum ModSym {
    FnDef(FnDefContent),
    VarDec(GlobalVarDec),
    TypeDef(TypeDefContent),
}

#[derive(Debug)]
pub enum ModuleSymbols {
    FnDef(FnDefContent),
    VarDec(GlobalVarDec),
    TypeDef(TypeDefContent),
}

impl ModSym {
    pub(super) fn try_resolve(
        value: biwac_parser::Globals,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<Option<(AbsId, Self)>, ResolveError> {
        match value {
            biwac_parser::Globals::Import(_) => Ok(None),
            biwac_parser::Globals::FnDef(f) => FnDefContent::try_resolve(f, imports, modpath)
                .map(|(id, f)| Some((id, Self::FnDef(f)))),
            biwac_parser::Globals::TypeDef(t) => TypeDefContent::try_resolve(t, imports, modpath)
                .map(|(id, t)| Some((id, Self::TypeDef(t)))),
            biwac_parser::Globals::VarDec(v) => GlobalVarDec::try_resolve(v, imports, modpath)
                .map(|(id, v)| Some((id, Self::VarDec(v)))),
        }
    }
}
