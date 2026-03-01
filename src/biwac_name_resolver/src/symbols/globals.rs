use std::collections::HashMap;

use biwac_base::Span;
use biwac_parser::{Ident, VarDecl};

use crate::{
    DecledVar, Expr, ImplLevelTryResolve, LocVarId, ModuleLevelTryResolve, ResolveError, RsvResult,
    Stmt, TryResolve, Typ,
    context::{FnLvlRslvCtx, ImplLvlGenTypRslvCtx, ModLvlRslvCtx},
};

#[derive(Debug, Clone)]
pub struct GlobalVarDecl {
    pub typ: Option<Typ>,
    pub init: Expr,
}

#[derive(Debug, Clone)]
pub struct FnDefContent {
    pub args: Vec<DecledArg>,
    pub stmts: Vec<Stmt>,
    pub expr: Option<Expr>,
    pub rtype: Option<Typ>, // None means void
    pub vars: HashMap<LocVarId, DecledVar>,
}

#[derive(Debug, Clone)]
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
pub struct MethodDefContent {
    pub ident: Ident,
    pub self_id: LocVarId,
    pub args: Vec<DecledArg>,
    pub stmts: Vec<Stmt>,
    pub expr: Option<Expr>,
    pub rtype: Option<Typ>, // None means void
    pub vars: HashMap<LocVarId, DecledVar>,
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

impl ImplLevelTryResolve<biwac_parser::FnDef> for FnDefContent {
    fn try_resolve_in_impl<'mctx>(
        value: biwac_parser::FnDef,
        ictx: &crate::context::ImplLvlGenTypRslvCtx<'mctx>,
    ) -> RsvResult<Self> {
        // 関数のベースのスコープも初期化される
        let mut fctx = FnLvlRslvCtx::try_from_ictx(Some(&value.genargs), ictx)?;

        Ok(Self {
            args: value
                .args
                .into_iter()
                .map(|arg| match Typ::try_resolve(&arg.typ, &mut fctx) {
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
                .map(|typ| Typ::try_resolve(&typ, &mut fctx))
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
        // 関数のベースのスコープも初期化される
        let mut fctx = FnLvlRslvCtx::try_from_mctx(Some(&value.genargs), mctx)?;

        Ok(Self {
            args: value
                .args
                .into_iter()
                .map(|arg| {
                    Ok(NativeFnArgDecl {
                        typ: Typ::try_resolve(&arg.typ, &mut fctx)?,
                        span: arg.span,
                        id: arg.id,
                    })
                })
                .collect::<RsvResult<_>>()?,
            rtype: value
                .rtype
                .map(|typ| Typ::try_resolve(&typ, &mut fctx))
                .transpose()?,
            native: value.native,
            native_span: value.native_span,
            span: value.span,
        })
    }
}

impl ImplLevelTryResolve<biwac_parser::MethodDef> for MethodDefContent {
    fn try_resolve_in_impl<'mctx>(
        value: biwac_parser::MethodDef,
        ictx: &crate::context::ImplLvlGenTypRslvCtx<'mctx>,
    ) -> RsvResult<Self> {
        // 関数のベースのスコープも初期化される
        let mut fctx = FnLvlRslvCtx::try_from_ictx(Some(&value.genargs), ictx)?;

        // 変数selfの初期化
        let self_typ = Typ::try_resolve_in_impl(&value.self_typ, ictx)?;
        let self_id = fctx.declare_variable(&value.self_ident, Some(self_typ.clone()))?;

        Ok(Self {
            ident: value.id,
            self_id,
            args: value
                .args
                .into_iter()
                .map(|arg| match Typ::try_resolve_in_impl(&arg.typ, ictx) {
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
                .map(|typ| Typ::try_resolve_in_impl(&typ, ictx))
                .transpose()?,
            vars: fctx.into_vars(), // 関数内で収集した変数宣言を保存
        })
    }
}

impl ModuleLevelTryResolve<biwac_parser::StructDef> for StructDefContent {
    fn try_resolve_in_module<'pctx>(
        value: biwac_parser::StructDef,
        mctx: &ModLvlRslvCtx<'pctx>,
    ) -> RsvResult<Self> {
        // 構造体の定義は
        //  struct Foo[T] {
        //            ^^^
        //      bar: T,
        //      baz: Int,
        //  }
        //  ジェネリック型引数宣言を含む
        //  メンバの型はこれを含めて解決する必要がある
        //
        // FIXME: ictx を流用することにする
        let ictx = ImplLvlGenTypRslvCtx::new(Some(&value.genargs), mctx)?;

        Ok(Self {
            members: value
                .members
                .into_iter()
                .map(|(id, typ)| Typ::try_resolve_in_impl(&typ, &ictx).map(|typ| (id, typ)))
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
