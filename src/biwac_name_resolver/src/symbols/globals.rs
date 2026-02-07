use std::collections::HashMap;

use biwac_base::ModPath;
use biwac_parser::{VarDecl, types::TypDecl};

use crate::{
    AbsId, Exprs, ModuleLevelTryResolve, ResolveError, RsvResult, Stmt, Typ, context::ModLvlRslvCtx,
};

#[derive(Debug, Clone)]
pub struct GlobalVarDecl {
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

impl ModuleLevelTryResolve<biwac_parser::FnDef> for FnDefContent {
    fn try_resolve<'pctx>(
        value: biwac_parser::FnDef,
        mctx: &ModLvlRslvCtx<'pctx>,
    ) -> RsvResult<Self> {
        todo!()
        // Ok(Self {
        //     args: value
        //         .args
        //         .into_iter()
        //         .map(
        //             |(atyp, aid)| match Typ::try_resolve(atyp, imports, modpath) {
        //                 Ok(typ) => Ok((typ, aid)),
        //                 Err(e) => Err(e),
        //             },
        //         )
        //         .collect::<Result<Vec<(Typ, String)>, ResolveError>>()?,
        //     stmts: value
        //         .stmts
        //         .into_iter()
        //         .map(|stmt| Stmt::try_resolve(stmt, imports, modpath))
        //         .collect::<Result<Vec<Stmt>, ResolveError>>()?,
        //     rtype: value
        //         .rtype
        //         .map(|typ| Typ::try_resolve(typ, imports, modpath))
        //         .transpose()?,
        // })
    }
}

impl ModuleLevelTryResolve<biwac_parser::StructDef> for StructDefContent {
    fn try_resolve<'pctx>(
        value: biwac_parser::StructDef,
        mctx: &ModLvlRslvCtx<'pctx>,
    ) -> RsvResult<Self> {
        todo!()
    }
    // fn try_resolve(
    //     value: biwac_parser::StructDef,
    //     imports: &[biwac_parser::QualifiedId],
    //     modpath: &ModPath,
    // ) -> Result<(AbsId, Self), ResolveError> {
    //     Ok((
    //         AbsId::new(modpath.clone().into(), value.id),
    //         Self {
    //             members: value
    //                 .members
    //                 .into_iter()
    //                 .map(|(id, (typ, index))| {
    //                     Typ::try_resolve(typ, imports, modpath).map(|typ| (id, (typ, index)))
    //                 })
    //                 .collect::<Result<HashMap<String, (Typ, usize)>, ResolveError>>()?,
    //         },
    //     ))
    // }
}

impl ModuleLevelTryResolve<VarDecl> for GlobalVarDecl {
    fn try_resolve<'pctx>(value: VarDecl, mctx: &ModLvlRslvCtx<'pctx>) -> RsvResult<Self> {
        todo!()
    }
    // fn try_resolve(
    //     value: VarDecl,
    //     imports: &[biwac_parser::QualifiedId],
    //     modpath: &ModPath,
    // ) -> Result<(AbsId, Self), ResolveError> {
    //     let typ = match value.typ {
    //         TypDecl::Any => None,
    //         TypDecl::Typ(t) => Some(Typ::try_resolve(t, imports, modpath)?),
    //     };
    //
    //     Ok((
    //         AbsId::new(modpath.clone().into(), value.name),
    //         Self {
    //             typ,
    //             init: Exprs::try_resolve(value.init, imports, modpath)?,
    //         },
    //     ))
    // }
}
