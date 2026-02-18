use std::collections::HashMap;

use biwac_base::Span;
use biwac_parser::{Ident, VarDecl};

use crate::{
    DecledVar, Expr, LocVarId, ModuleLevelTryResolve, ResolveError, RsvResult, Stmt, TryResolve,
    Typ,
    context::{FnLvlRslvCtx, ModLvlRslvCtx},
};

#[derive(Debug, Clone)]
pub struct GlobalVarDecl {
    pub typ: Option<Typ>,
    pub init: Expr,
}

#[derive(Debug)]
pub struct FnDefContent {
    pub args: Vec<DecledArg>,
    pub stmts: Vec<Stmt>,
    pub expr: Option<Expr>,
    pub rtype: Option<Typ>, // None means void
    pub vars: HashMap<LocVarId, DecledVar>,
}

#[derive(Debug)]
pub struct NativeFnDefContent {
    pub args: Vec<NativeFnArgDecl>,
    pub rtype: Option<Typ>, // None means void
    pub native: String,
    pub native_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFnArgDecl {
    pub typ: Typ,
    pub id: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DecledArg {
    pub id: LocVarId,
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
        // 関数のベースのスコープも初期化される
        let mut fctx = FnLvlRslvCtx::new(mctx);

        Ok(Self {
            args: value
                .args
                .into_iter()
                .map(|arg| match Typ::try_resolve_in_module(arg.typ, mctx) {
                    Ok(typ) => {
                        // 引数も変数の宣言として記録
                        let id = fctx.declare_variable(&arg.id, Some(typ))?;

                        Ok(DecledArg { id })
                    }
                    Err(e) => Err(e),
                })
                .collect::<RsvResult<_>>()?,
            stmts: value
                .stmts
                .into_iter()
                .map(|stmt| Stmt::try_resolve(stmt, &mut fctx))
                .collect::<Result<Vec<Stmt>, ResolveError>>()?,
            expr: value
                .expr
                .map(|expr| Expr::try_resolve(expr, &mut fctx))
                .transpose()?,
            rtype: value
                .rtype
                .map(|typ| Typ::try_resolve_in_module(typ, mctx))
                .transpose()?,
            vars: fctx.into_vars(), // 関数内で収集した変数宣言を保存
        })
    }
}

impl ModuleLevelTryResolve<biwac_parser::NativeFnDef> for NativeFnDefContent {
    fn try_resolve_in_module<'pctx>(
        value: biwac_parser::NativeFnDef,
        mctx: &ModLvlRslvCtx<'pctx>,
    ) -> RsvResult<Self> {
        Ok(Self {
            args: value
                .args
                .into_iter()
                .map(|arg| {
                    Ok(NativeFnArgDecl {
                        typ: Typ::try_resolve_in_module(arg.typ, mctx)?,
                        span: arg.span,
                        id: arg.id,
                    })
                })
                .collect::<RsvResult<_>>()?,
            rtype: value
                .rtype
                .map(|typ| Typ::try_resolve_in_module(typ, mctx))
                .transpose()?,
            native: value.native,
            native_span: value.native_span,
            span: value.span,
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
        //     init: Expr::try_resolve(value.init, mctx)?,
        // })
    }
}
