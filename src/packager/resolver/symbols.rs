pub(crate) mod expressions;
pub(crate) mod globals;
pub(crate) mod statements;

use crate::{
    packager::resolver::{
        symbols::globals::{FnDefContent, GlobalVarDec, TypeDefContent},
        TryResolveWithId,
    },
    parser,
    validator::AbsoluteId,
};

#[derive(Debug)]
pub enum ModuleSymbols {
    // LanglibfnDec(LanglibfnDec),
    FnDef(FnDefContent),
    VarDec(GlobalVarDec),
    TypeDef(TypeDefContent),
}

impl ModuleSymbols {
    pub(super) fn try_resolve(
        value: parser::symbols::globals::Globals,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
    ) -> Result<Option<(AbsoluteId, Self)>, crate::packager::resolver::ResolveError> {
        match value {
            parser::symbols::globals::Globals::Import(_) => Ok(None),
            parser::symbols::globals::Globals::FnDef(f) => {
                FnDefContent::try_resolve(f, imports, modpath)
                    .map(|(id, f)| Some((id, Self::FnDef(f))))
            }
            parser::symbols::globals::Globals::TypeDef(t) => {
                TypeDefContent::try_resolve(t, imports, modpath)
                    .map(|(id, t)| Some((id, Self::TypeDef(t))))
            }
            parser::symbols::globals::Globals::VarDec(v) => {
                GlobalVarDec::try_resolve(v, imports, modpath)
                    .map(|(id, v)| Some((id, Self::VarDec(v))))
            }
            _ => todo!(),
        }
    }
}
