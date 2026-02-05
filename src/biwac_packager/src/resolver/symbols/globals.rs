use std::collections::HashMap;

use biwac_base::ModPath;
use biwac_parser::{VarDec, types::TypDecl};

use crate::{
    AbsId, Exprs, Stmt, Typ,
    resolver::{ResolveError, TryResolve, TryResolveWithId},
};

#[derive(Debug, Clone)]
pub struct GlobalVarDec {
    pub typ: Option<Typ>,
    pub init: Exprs,
}

#[derive(Debug)]
pub struct FnDefContent {
    pub args: Vec<(Typ, String)>,
    pub stmts: Vec<Stmt>,
    pub rtype: Option<Typ>, // None means void
}

#[derive(Debug, Clone)]
pub enum TypeDefContent {
    Struct(StructDefContent),
    // Enum(EnumType),
    // Typedef(Box<Self>),
}

#[derive(Debug, Clone)]
pub struct StructDefContent {
    pub members: HashMap<String, (Typ, usize)>,
}

impl TryResolveWithId<biwac_parser::FnDef> for FnDefContent {
    fn try_resolve(
        value: biwac_parser::FnDef,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<(AbsId, Self), ResolveError> {
        Ok((
            AbsId::new(modpath.clone().into(), value.name),
            Self {
                args: value
                    .args
                    .into_iter()
                    .map(
                        |(atyp, aid)| match Typ::try_resolve(atyp, imports, modpath) {
                            Ok(typ) => Ok((typ, aid)),
                            Err(e) => Err(e),
                        },
                    )
                    .collect::<Result<Vec<(Typ, String)>, ResolveError>>()?,
                stmts: value
                    .stmts
                    .into_iter()
                    .map(|stmt| Stmt::try_resolve(stmt, imports, modpath))
                    .collect::<Result<Vec<Stmt>, ResolveError>>()?,
                rtype: value
                    .rtype
                    .map(|typ| Typ::try_resolve(typ, imports, modpath))
                    .transpose()?,
            },
        ))
    }
}

impl TryResolveWithId<biwac_parser::TypeDef> for TypeDefContent {
    fn try_resolve(
        value: biwac_parser::TypeDef,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<(AbsId, Self), ResolveError> {
        match value {
            biwac_parser::TypeDef::Struct(s) => {
                Ok(StructDefContent::try_resolve(s, imports, modpath)
                    .map(|(id, s)| (id, Self::Struct(s)))?)
            }
        }
    }
}

impl TryResolveWithId<biwac_parser::StructDef> for StructDefContent {
    fn try_resolve(
        value: biwac_parser::StructDef,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<(AbsId, Self), ResolveError> {
        Ok((
            AbsId::new(modpath.clone().into(), value.id),
            Self {
                members: value
                    .members
                    .into_iter()
                    .map(|(id, (typ, index))| {
                        Typ::try_resolve(typ, imports, modpath).map(|typ| (id, (typ, index)))
                    })
                    .collect::<Result<HashMap<String, (Typ, usize)>, ResolveError>>()?,
            },
        ))
    }
}

impl TryResolveWithId<VarDec> for GlobalVarDec {
    fn try_resolve(
        value: VarDec,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<(AbsId, Self), ResolveError> {
        let typ = match value.typ {
            TypDecl::Any => None,
            TypDecl::Typ(t) => Some(Typ::try_resolve(t, imports, modpath)?),
        };

        Ok((
            AbsId::new(modpath.clone().into(), value.name),
            Self {
                typ,
                init: Exprs::try_resolve(value.init, imports, modpath)?,
            },
        ))
    }
}
