use biwac_parser::{Ident, VarDecl};

use crate::{
    Exprs, ModuleLevelTryResolve, ResolveError, RsvResult, Stmt, TryResolve, Typ,
    context::{FnLvlRslvCtx, ModLvlRslvCtx},
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
    pub members: Vec<(Ident, Typ)>,
}

impl ModuleLevelTryResolve<biwac_parser::FnDef> for FnDefContent {
    fn try_resolve_in_module<'pctx>(
        value: biwac_parser::FnDef,
        mctx: &ModLvlRslvCtx<'pctx>,
    ) -> RsvResult<Self> {
        let mut fctx = FnLvlRslvCtx::new(mctx);

        Ok(Self {
            args: value
                .args
                .into_iter()
                .map(|(atyp, aid)| match Typ::try_resolve_in_module(atyp, mctx) {
                    Ok(typ) => Ok((typ, aid)),
                    Err(e) => Err(e),
                })
                .collect::<Result<Vec<(Typ, String)>, ResolveError>>()?,
            stmts: value
                .body
                .stmts
                .into_iter()
                .map(|stmt| Stmt::try_resolve(stmt, &mut fctx))
                .collect::<Result<Vec<Stmt>, ResolveError>>()?,
            rtype: value
                .rtype
                .map(|typ| Typ::try_resolve_in_module(typ, mctx))
                .transpose()?,
        })
    }
}

impl ModuleLevelTryResolve<biwac_parser::StructDef> for StructDefContent {
    fn try_resolve_in_module<'pctx>(
        value: biwac_parser::StructDef,
        mctx: &ModLvlRslvCtx<'pctx>,
    ) -> RsvResult<Self> {
        Ok(Self {
            members: value
                .members
                .into_iter()
                .map(|(id, typ)| Typ::try_resolve_in_module(typ, mctx).map(|typ| (id, typ)))
                .collect::<Result<Vec<_>, ResolveError>>()?,
        })
    }
}

impl ModuleLevelTryResolve<VarDecl> for GlobalVarDecl {
    fn try_resolve_in_module<'pctx>(
        value: VarDecl,
        mctx: &ModLvlRslvCtx<'pctx>,
    ) -> RsvResult<Self> {
        todo!()
        // let typ = match value.typ {
        //     TypDecl::Any => None,
        //     TypDecl::Typ(t) => Some(Typ::try_resolve(t, mctx)?),
        // };
        //
        // Ok(Self {
        //     typ,
        //     init: Exprs::try_resolve(value.init, mctx)?,
        // })
    }
}
