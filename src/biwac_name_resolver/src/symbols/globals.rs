use std::collections::HashMap;

use biwac_hir::{Expr, FnDefContentBody, Hir, Stmt, Ty};

use crate::{
    ModuleLevelTryResolveTy, ResolveError, RsvResult, TryResolve, TryResolveTy,
    context::{
        ty_phase::ty_def_level::TyDefLevelTyResolveCtx, val_phase::fn_level::FnLevelResolveCtx,
    },
};

impl TryResolve<(&biwac_parser::FnDef, &biwac_hir::FnDefContentSignature)> for FnDefContentBody {
    fn try_resolve<'mctx>(
        (fn_def, fn_signature): (&biwac_parser::FnDef, &biwac_hir::FnDefContentSignature),
        fctx: &mut FnLevelResolveCtx<'mctx>,
        hir: &Hir,
    ) -> RsvResult<Self> {
        // 関数のベースのスコープも初期化される

        // 引数も変数の宣言として記録
        let arg_var_ids = fn_signature
            .args
            .iter()
            .map(|(ident, ty)| fctx.declare_variable(ident, ty.clone()))
            .collect::<RsvResult<_>>()?;

        Ok(Self {
            stmts: fn_def
                .stmts
                .iter()
                .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
                .collect::<Result<Vec<Stmt>, ResolveError>>()?,
            expr: fn_def
                .expr
                .as_ref()
                .map(|expr| Expr::try_resolve(expr, fctx, hir))
                .transpose()?,
            arg_var_ids,
            vars: fctx.vars(), // 関数内で収集した変数宣言を保存
        })
    }
}

impl TryResolve<(&biwac_parser::MethodDef, &biwac_hir::FnDefContentSignature)>
    for FnDefContentBody
{
    fn try_resolve<'mctx>(
        (method_def, fn_signature): (&biwac_parser::MethodDef, &biwac_hir::FnDefContentSignature),
        fctx: &mut FnLevelResolveCtx<'mctx>,
        hir: &Hir,
    ) -> RsvResult<Self> {
        // 変数self,
        // selfは含まない残りの引数も変数の宣言として記録
        let self_ty = fctx.try_resolve_ty(&method_def.self_typ, hir)?;
        let arg_var_ids = [
            vec![fctx.declare_variable(&method_def.self_ident, self_ty)?],
            fn_signature
                .args
                .iter()
                .map(|(ident, ty)| fctx.declare_variable(ident, ty.clone()))
                .collect::<RsvResult<_>>()?,
        ]
        .concat();

        Ok(Self {
            stmts: method_def
                .stmts
                .iter()
                .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
                .collect::<Result<Vec<Stmt>, ResolveError>>()?,
            expr: method_def
                .expr
                .as_ref()
                .map(|expr| Expr::try_resolve(expr, fctx, hir))
                .transpose()?,
            arg_var_ids,
            vars: fctx.vars(), // 関数内で収集した変数宣言を保存
        })
    }
}

impl ModuleLevelTryResolveTy<&biwac_parser::StructDef> for biwac_hir::StructDefContent {
    fn try_resolve_in_module(
        struct_: &biwac_parser::StructDef,
        mctx: &crate::context::ty_phase::module_level::ModuleLevelTyResolveCtx,
        hir: &Hir,
    ) -> crate::RsvResult<Self> {
        // 構造体の定義は
        //  struct Foo[T] {
        //            ^^^
        //      bar: T,
        //      baz: Int,
        //  }
        //  ジェネリック型引数宣言を含む
        //  メンバの型はこれを含めて解決する必要がある
        let tgctx = TyDefLevelTyResolveCtx::new(mctx, &struct_.genargs)?;

        Ok(Self {
            members: struct_
                .members
                .iter()
                .map(|(id, typ)| {
                    tgctx
                        .try_resolve_ty(typ, hir)
                        .map(|ty| (id.id.clone(), (ty, id.span.clone())))
                })
                .collect::<Result<HashMap<_, _>, ResolveError>>()?,
            genargs: tgctx.ty_def_genarg_vec,
            struct_name_span: struct_.id.span.clone(),
        })
    }
}

// impl ModuleLevelTryResolve<VarDecl> for GlobalVarDecl {
//     fn try_resolve_in_module<'pctx>(
//         value: VarDecl,
//         mctx: &ModLvlRslvCtx<'pctx>,
//     ) -> RsvResult<Self> {
//         todo!()
//         // let typ = match value.typ {
//         //     TypDecl::Any => None,
//         //     TypDecl::Typ(t) => Some(Typ::try_resolve(t, mctx)?),
//         // };
//         //
//         // Ok(Self {
//         //     typ,
//         //     init: Expr::try_resolve(value.init, mctx)?,
//         // })
//     }
// }

impl TryResolveTy<(&Vec<biwac_parser::ArgDecl>, &Option<biwac_parser::TypRepr>)>
    for biwac_hir::FnDefContentSignature
{
    fn try_resolve<'mctx>(
        value: (&Vec<biwac_parser::ArgDecl>, &Option<biwac_parser::TypRepr>),
        fctx: &crate::context::ty_phase::fn_level::FnLevelTyResolveCtx<'mctx>,
        hir: &Hir,
    ) -> crate::RsvResult<Self> {
        let args = value
            .0
            .iter()
            .map(|arg| Ok((arg.id.clone(), fctx.try_resolve_ty(&arg.typ, hir)?)))
            .collect::<RsvResult<Vec<_>>>()?;

        let rty = if let Some(typ) = &value.1 {
            fctx.try_resolve_ty(typ, hir)?
        } else {
            Ty::Void
        };

        Ok(biwac_hir::FnDefContentSignature {
            args,
            rty,
            genargs: vec![],
        })
    }
}
