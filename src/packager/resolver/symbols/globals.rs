use std::collections::HashMap;

use crate::{
    packager::resolver::{
        symbols::{expressions::Exprs, statements::Stmt},
        ResolveError, TryResolve, TryResolveWithId,
    },
    parser::{self, symbols::statements::vardec::VarDec},
    validator::{types::AbsoluteType, AbsoluteId},
};

// #[derive(Debug)]
// pub struct LanglibfnDec {
//     pub id: String,
//     // pub args: Vec<Type>,
//     pub rtype: Option<Type>, // None means void
// }

#[derive(Debug, Clone)]
pub struct GlobalVarDec {
    pub typ: AbsoluteType,
    pub init: Exprs,
}

#[derive(Debug)]
pub struct FnDefContent {
    pub args: Vec<(AbsoluteType, String)>,
    pub stmts: Vec<Stmt>,
    pub rtype: Option<AbsoluteType>, // None means void
}

#[derive(Debug, Clone)]
pub enum TypeDefContent {
    Struct(StructDefContent),
    // Enum(EnumType),
    // Typedef(Box<Self>),
}

#[derive(Debug, Clone)]
pub struct StructDefContent {
    pub members: HashMap<String, (AbsoluteType, usize)>,
}

impl TryResolveWithId<parser::symbols::globals::FnDef> for FnDefContent {
    fn try_resolve(
        value: parser::symbols::globals::FnDef,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
    ) -> Result<(AbsoluteId, Self), crate::packager::resolver::ResolveError> {
        Ok((
            AbsoluteId::new(modpath.0.clone(), value.name),
            Self {
                args: value
                    .args
                    .into_iter()
                    .map(
                        |(atyp, aid)| match AbsoluteType::try_resolve(atyp, imports, modpath) {
                            Ok(typ) => Ok((typ, aid)),
                            Err(e) => Err(e),
                        },
                    )
                    .collect::<Result<Vec<(AbsoluteType, String)>, ResolveError>>()?,
                stmts: value
                    .stmts
                    .into_iter()
                    .map(|stmt| Stmt::try_resolve(stmt, imports, modpath))
                    .collect::<Result<Vec<Stmt>, ResolveError>>()?,
                rtype: value
                    .rtype
                    .map(|typ| AbsoluteType::try_resolve(typ, imports, modpath))
                    .transpose()?,
            },
        ))
    }
}

impl TryResolveWithId<parser::symbols::globals::TypeDef> for TypeDefContent {
    fn try_resolve(
        value: parser::symbols::globals::TypeDef,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
    ) -> Result<(AbsoluteId, Self), ResolveError> {
        match value {
            parser::symbols::globals::TypeDef::Struct(s) => {
                Ok(StructDefContent::try_resolve(s, imports, modpath)
                    .map(|(id, s)| (id, Self::Struct(s)))?)
            }
        }
    }
}

impl TryResolveWithId<parser::symbols::globals::StructDef> for StructDefContent {
    fn try_resolve(
        value: parser::symbols::globals::StructDef,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
    ) -> Result<(AbsoluteId, Self), ResolveError> {
        Ok((
            AbsoluteId::new(modpath.0.clone(), value.id),
            Self {
                members: value
                    .members
                    .into_iter()
                    .map(|(id, (typ, index))| {
                        AbsoluteType::try_resolve(typ, imports, modpath)
                            .map(|typ| (id, (typ, index)))
                    })
                    .collect::<Result<HashMap<String, (AbsoluteType, usize)>, ResolveError>>()?,
            },
        ))
    }
}

impl TryResolveWithId<VarDec> for GlobalVarDec {
    fn try_resolve(
        value: VarDec,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
    ) -> Result<(AbsoluteId, Self), ResolveError> {
        Ok((
            AbsoluteId::new(modpath.0.clone(), value.name),
            Self {
                typ: AbsoluteType::try_resolve(value.typ, imports, modpath)?,
                init: Exprs::try_resolve(value.init, imports, modpath)?,
            },
        ))
    }
}
