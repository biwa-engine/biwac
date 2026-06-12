use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::{ArgDecl, RetTypRepr};

use crate::{
    DefCollector, ResolveError, ResolveErrorHandler,
    context::{
        ResolveCtx, fn_level::FnResolveCtx, impl_level::ImplResolveCtx,
        module_level::ModuleResolveCtx, ty_def_level::TyDefResolveCtx,
    },
    symbols::NameResolve,
};

fn fn_signature_resolve<C: ResolveCtx>(
    ctx: &C,
    args: &[ArgDecl],
    rtype: &RetTypRepr,
) -> Result<(), Vec<ResolveError>> {
    let mut errors = Vec::new();

    for arg in args {
        ctx.resolve_typ(&arg.typ).handle(&mut errors);
    }

    match rtype {
        RetTypRepr::Typ(typ) => {
            ctx.resolve_typ(&typ).handle(&mut errors);
        }
        RetTypRepr::Void(_) => {
            // nothing to do
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

impl<C: ResolveCtx> NameResolve<C> for biwac_ast::FnDef {
    fn resolve(&self, ctx: &C, def_collector: &mut DefCollector) -> Result<(), Vec<ResolveError>> {
        let ctx = FnResolveCtx::new(ctx, &self.genargs, def_collector)?;
        fn_signature_resolve(&ctx, &self.args.args, &self.rtype)
    }
}

impl NameResolve<ImplResolveCtx<'_>> for biwac_ast::MethodDef {
    fn resolve(
        &self,
        ctx: &ImplResolveCtx<'_>,
        def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        let ctx = FnResolveCtx::new(ctx, &self.genargs, def_collector)?;
        fn_signature_resolve(&ctx, &self.args.args, &self.rtype)
    }
}

impl NameResolve<ModuleResolveCtx<'_>> for biwac_ast::StructDef {
    fn resolve(
        &self,
        ctx: &ModuleResolveCtx<'_>,
        def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        let ctx = TyDefResolveCtx::new(ctx, &self.genargs, def_collector)?;
        let mut members = HashMap::new();
        let mut errors = Vec::new();

        for (ident, typ) in &self.members {
            match members.entry(ident.id) {
                Entry::Vacant(e) => {
                    e.insert(&ident.span);
                }
                Entry::Occupied(e) => {
                    errors.push(ResolveError::DuplicatedStructMember {
                        name: ident.id,
                        span1: e.get().to_owned().clone(),
                        span2: ident.span.clone(),
                    });
                }
            }

            ctx.resolve_typ(&typ).handle(&mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl NameResolve<ModuleResolveCtx<'_>> for biwac_ast::ImplBlock {
    fn resolve(
        &self,
        ctx: &ModuleResolveCtx<'_>,
        def_collector: &mut DefCollector,
    ) -> Result<(), Vec<ResolveError>> {
        let ctx = ImplResolveCtx::new(ctx, &self, def_collector)?;
        let mut errors = Vec::new();

        for fn_def in &self.assoc_fns {
            fn_def.resolve(&ctx, def_collector).handle(&mut errors);
        }

        // TODO: others

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// impl TryResolve<(&biwac_ast::FnDef, &biwac_hir::FnDefContentSignature)> for FnDefContentBody {
//     fn try_resolve<'mctx>(
//         (fn_def, fn_signature): (&biwac_ast::FnDef, &biwac_hir::FnDefContentSignature),
//         fctx: &mut FnLevelResolveCtx<'mctx>,
//         hir: &Hir,
//     ) -> RsvResult<Self> {
//         // 関数のベースのスコープも初期化される
//
//         // 引数も変数の宣言として記録
//         let arg_var_ids = fn_signature
//             .args
//             .iter()
//             .map(|(ident, ty)| fctx.declare_variable(ident, ty.clone()))
//             .collect::<RsvResult<_>>()?;
//
//         Ok(Self {
//             stmts: fn_def
//                 .stmts
//                 .iter()
//                 .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
//                 .collect::<Result<Vec<Stmt>, ResolveError>>()?,
//             expr: fn_def
//                 .expr
//                 .as_ref()
//                 .map(|expr| Expr::try_resolve(expr, fctx, hir))
//                 .transpose()?,
//             arg_var_ids,
//             vars: fctx.vars(), // 関数内で収集した変数宣言を保存
//         })
//     }
// }
//
// impl TryResolve<(&biwac_ast::MethodDef, &biwac_hir::FnDefContentSignature)> for FnDefContentBody {
//     fn try_resolve<'mctx>(
//         (method_def, fn_signature): (&biwac_ast::MethodDef, &biwac_hir::FnDefContentSignature),
//         fctx: &mut FnLevelResolveCtx<'mctx>,
//         hir: &Hir,
//     ) -> RsvResult<Self> {
//         // 変数self,
//         // selfは含まない残りの引数も変数の宣言として記録
//         let self_ty = fctx.try_resolve_ty(&method_def.self_typ, hir)?;
//         let arg_var_ids = [
//             vec![fctx.declare_variable(&method_def.self_ident.clone().into(), self_ty)?],
//             fn_signature
//                 .args
//                 .iter()
//                 .map(|(ident, ty)| fctx.declare_variable(ident, ty.clone()))
//                 .collect::<RsvResult<_>>()?,
//         ]
//         .concat();
//
//         Ok(Self {
//             stmts: method_def
//                 .stmts
//                 .iter()
//                 .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
//                 .collect::<Result<Vec<Stmt>, ResolveError>>()?,
//             expr: method_def
//                 .expr
//                 .as_ref()
//                 .map(|expr| Expr::try_resolve(expr, fctx, hir))
//                 .transpose()?,
//             arg_var_ids,
//             vars: fctx.vars(), // 関数内で収集した変数宣言を保存
//         })
//     }
// }
//
// impl ModuleLevelTryResolveTy<&biwac_ast::StructDef> for biwac_hir::StructDefContent {
//     fn try_resolve_in_module(
//         struct_: &biwac_ast::StructDef,
//         mctx: &crate::context::ty_phase::module_level::ModuleLevelTyResolveCtx,
//         hir: &Hir,
//     ) -> crate::RsvResult<Self> {
//         // 構造体の定義は
//         //  struct Foo[T] {
//         //            ^^^
//         //      bar: T,
//         //      baz: Int,
//         //  }
//         //  ジェネリック型引数宣言を含む
//         //  メンバの型はこれを含めて解決する必要がある
//         let tgctx = TyDefLevelTyResolveCtx::new(mctx, &struct_.genargs)?;
//
//         Ok(Self {
//             members: struct_
//                 .members
//                 .iter()
//                 .map(|(id, typ)| tgctx.try_resolve_ty(typ, hir).map(|ty| (id.id.clone(), ty)))
//                 .collect::<Result<HashMap<_, _>, ResolveError>>()?,
//             genargs: tgctx.ty_def_genarg_vec,
//             struct_name_span: struct_.id.span.clone().into(),
//         })
//     }
// }
//
// // impl ModuleLevelTryResolve<VarDecl> for GlobalVarDecl {
// //     fn try_resolve_in_module<'pctx>(
// //         value: VarDecl,
// //         mctx: &ModLvlRslvCtx<'pctx>,
// //     ) -> RsvResult<Self> {
// //         todo!()
// //         // let typ = match value.typ {
// //         //     TypDecl::Any => None,
// //         //     TypDecl::Typ(t) => Some(Typ::try_resolve(t, mctx)?),
// //         // };
// //         //
// //         // Ok(Self {
// //         //     typ,
// //         //     init: Expr::try_resolve(value.init, mctx)?,
// //         // })
// //     }
// // }
//
// impl TryResolveTy<(&biwac_ast::ArgDeclList, &biwac_ast::RetTypRepr)>
//     for biwac_hir::FnDefContentSignature
// {
//     fn try_resolve<'mctx>(
//         value: (&biwac_ast::ArgDeclList, &biwac_ast::RetTypRepr),
//         fctx: &crate::context::ty_phase::fn_level::FnLevelTyResolveCtx<'mctx>,
//         hir: &Hir,
//     ) -> crate::RsvResult<Self> {
//         let args = value
//             .0
//             .args
//             .iter()
//             .map(|arg| Ok((arg.id.clone().into(), fctx.try_resolve_ty(&arg.typ, hir)?)))
//             .collect::<RsvResult<Vec<_>>>()?;
//
//         let (rty, rty_span) = match &value.1 {
//             RetTypRepr::Typ(typ) => (fctx.try_resolve_ty(typ, hir)?, typ.span.clone()),
//             RetTypRepr::Void(span) => (Ty::new(TyKind::Void, span.clone().into()), span.clone()),
//         };
//
//         Ok(biwac_hir::FnDefContentSignature {
//             span: Span::merge(&value.0.span, &rty_span).into(),
//             args,
//             rty,
//             genargs: vec![],
//         })
//     }
// }
//
// impl TryResolve<(&biwac_ast::NovelScene, &biwac_hir::FnDefContentSignature)> for FnDefContentBody {
//     fn try_resolve<'mctx>(
//         (scene_def, fn_signature): (&biwac_ast::NovelScene, &biwac_hir::FnDefContentSignature),
//         fctx: &mut FnLevelResolveCtx<'mctx>,
//         hir: &Hir,
//     ) -> RsvResult<Self> {
//         // 関数のベースのスコープも初期化される
//
//         // 引数も変数の宣言として記録
//         let arg_var_ids = fn_signature
//             .args
//             .iter()
//             .map(|(ident, ty)| fctx.declare_variable(ident, ty.clone()))
//             .collect::<RsvResult<_>>()?;
//
//         Ok(Self {
//             stmts: scene_def
//                 .stmts
//                 .iter()
//                 .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
//                 .collect::<Result<Vec<Stmt>, ResolveError>>()?,
//             expr: None, // scene は式記法で戻り値を返さない
//             arg_var_ids,
//             vars: fctx.vars(), // 関数内で収集した変数宣言を保存
//         })
//     }
// }
