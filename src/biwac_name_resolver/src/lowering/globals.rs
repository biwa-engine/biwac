use std::collections::HashMap;

use biwac_ast::{ArgDeclList, RetTypRepr, TypeDef};
use biwac_base::{InternedIdent, ModPath};
use biwac_hir::{
    AssocValDefKind, DefinedTy, DefinedTyImpl, FnArgDecl, FnBody, FnDef, FnSignature, Hir, Ident,
    NativeCode, NativeFnDef, NativeTypeAliasDef, StructDef, Ty, TyDefKind, TyKind,
    TyValImplGenargsContentPair, TyValImplList, TypeAliasDef, ValDefKind,
};
use biwac_span::{GenDefId, LocalGenDefId, Span, ValDefId, VarId};

use crate::ResolveError;

use super::{
    ExprLowerCtx, alias_expansion, expressions::lower_expr, statements::lower_stmt,
    ty_from_typ_repr,
};

pub(super) fn build_fn_signature(
    args: &ArgDeclList,
    rtype: &RetTypRepr,
    self_ty: Option<TyKind>,
    genargs_decl: &Option<biwac_ast::symbols::globals::GenArgsDecl<LocalGenDefId>>,
    span: Span,
) -> FnSignature {
    let self_ty_opt_kind = self_ty.clone();
    let hir_args: Vec<FnArgDecl> = args
        .args
        .iter()
        .map(|arg| {
            let ty = ty_from_typ_repr(&arg.typ, self_ty_opt_kind.as_ref());
            FnArgDecl {
                id: Ident::from(arg.id.clone()),
                ty,
                var_id: *arg.var_id.get().unwrap(),
            }
        })
        .collect();

    let rty = match rtype {
        RetTypRepr::Typ(typ) => ty_from_typ_repr(typ, self_ty_opt_kind.as_ref()),
        RetTypRepr::Void(sp) => Ty::new(TyKind::Void, sp.clone()),
    };

    let genargs: Vec<(Ident, LocalGenDefId)> = genargs_decl
        .as_ref()
        .map(|gd| {
            gd.genargs
                .iter()
                .map(|item| {
                    (
                        Ident::from(item.id.clone()),
                        *item
                            .def_id
                            .get()
                            .expect("compiler bug: fn genarg def_id not set"),
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    let self_ty_hir = self_ty.map(|k| Ty::new(k, span.clone()));

    FnSignature {
        args: hir_args,
        self_ty: self_ty_hir,
        rty,
        genargs,
        span,
    }
}

fn build_fn_body(
    args: &ArgDeclList,
    stmts: &[biwac_ast::Stmt],
    expr: Option<&biwac_ast::Exprs>,
    has_self: bool,
    self_ty: Option<&TyKind>,
    signature: &FnSignature,
    errors: &mut Vec<ResolveError>,
) -> FnBody {
    let self_var_id = has_self.then_some(VarId::SELF_VARIABLE);

    let mut ctx = ExprLowerCtx::new();

    // self
    if let (Some(svid), Some(sty)) = (self_var_id, self_ty) {
        let self_span = signature.self_ty.as_ref().unwrap().span.clone();
        ctx.declare_var(
            svid,
            biwac_hir::DecledVar {
                id: Ident {
                    id: InternedIdent::SELF,
                    span: self_span.clone(),
                },
                ty: Ty::new(sty.clone(), self_span),
            },
        );
    }

    // explicit args
    for (arg, sarg) in args.args.iter().zip(signature.args.iter()) {
        let ty = sarg.ty.clone();
        ctx.declare_var(
            *arg.var_id.get().unwrap(),
            biwac_hir::DecledVar {
                id: Ident::from(arg.id.clone()),
                ty,
            },
        );
    }

    let lowered_stmts = stmts
        .iter()
        .filter_map(|s| lower_stmt(&mut ctx, s, errors))
        .collect();
    let lowered_expr = expr.and_then(|e| lower_expr(&mut ctx, e, errors));

    FnBody {
        stmts: lowered_stmts,
        expr: lowered_expr,
        self_var_id,
        vars: ctx.into_vars(),
    }
}

fn collect_impl_genargs(impl_block: &biwac_ast::ImplBlock) -> Vec<(Ident, LocalGenDefId)> {
    impl_block
        .genargs_decl
        .as_ref()
        .map(|gd| {
            gd.genargs
                .iter()
                .map(|item| {
                    (
                        Ident::from(item.id.clone()),
                        *item
                            .def_id
                            .get()
                            .expect("compiler bug: impl genarg def_id not set"),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn collect_impl_block_genargs_map(
    impl_block: &biwac_ast::ImplBlock,
) -> HashMap<InternedIdent, (LocalGenDefId, Span)> {
    impl_block
        .genargs_decl
        .as_ref()
        .map(|gd| {
            gd.genargs
                .iter()
                .map(|item| {
                    (
                        item.id.id,
                        (
                            *item
                                .def_id
                                .get()
                                .expect("compiler bug: impl genarg def_id not set"),
                            item.id.span.clone(),
                        ),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn lower_fn_def(
    hir: &mut Hir,
    fn_def: &biwac_ast::FnDef,
    impl_genargs: Vec<(Ident, LocalGenDefId)>,
    errors: &mut Vec<ResolveError>,
) {
    let val_def_id = match fn_def.def_id.get() {
        Some(id) => *id,
        None => return,
    };

    let signature = build_fn_signature(
        &fn_def.args,
        &fn_def.rtype,
        None,
        &fn_def.genargs,
        fn_def.span.clone(),
    );

    let body = build_fn_body(
        &fn_def.args,
        &fn_def.stmts,
        fn_def.expr.as_ref(),
        false,
        None,
        &signature,
        errors,
    );

    hir.vals.insert(
        val_def_id,
        ValDefKind::Fn(Box::new(FnDef::new(
            fn_def.id.clone().into(),
            signature,
            body,
            impl_genargs,
        ))),
    );
}

pub(super) fn lower_native_fn_def(
    hir: &mut Hir,
    fn_def: &biwac_ast::NativeFnDef,
    impl_genargs: Vec<(Ident, LocalGenDefId)>,
    _errors: &mut Vec<ResolveError>,
) {
    let val_def_id = match fn_def.def_id.get() {
        Some(id) => *id,
        None => return,
    };

    let signature = build_fn_signature(
        &fn_def.args,
        &fn_def.rtype,
        None,
        &fn_def.genargs,
        fn_def.span.clone(),
    );

    hir.vals.insert(
        val_def_id,
        ValDefKind::Native(Box::new(NativeFnDef::new(
            fn_def.id.clone().into(),
            fn_def.native_span.clone(),
            fn_def.span.clone(),
            fn_def.native.clone(),
            signature,
            impl_genargs,
        ))),
    );
}

pub(crate) fn lower_type_def(hir: &mut Hir, type_def: &TypeDef, errors: &mut Vec<ResolveError>) {
    match type_def {
        TypeDef::Struct(s) => lower_struct_def(hir, s, errors),
        TypeDef::TypeAlias(a) => lower_type_alias(hir, a, errors),
        TypeDef::NativeTypeAlias(n) => lower_native_type_alias(hir, n),
    }
}

fn lower_struct_def(
    hir: &mut Hir,
    struct_def: &biwac_ast::StructDef,
    _errors: &mut Vec<ResolveError>,
) {
    let ty_def_id = match struct_def.def_id.get() {
        Some(id) => *id,
        None => return,
    };

    let genargs: Vec<GenDefId> = struct_def
        .genargs
        .as_ref()
        .map(|gd| {
            gd.genargs
                .iter()
                .map(|item| {
                    *item
                        .def_id
                        .get()
                        .expect("compiler bug: struct genarg def_id not set")
                })
                .collect()
        })
        .unwrap_or_default();

    let members: HashMap<InternedIdent, Ty> = struct_def
        .members
        .iter()
        .map(|(ident, typ)| (ident.id, ty_from_typ_repr(typ, None))) // TODO: Some(self_ty)
        .collect();

    let ty_content = TyDefKind::Struct(Box::new(StructDef {
        name: struct_def.id.clone().into(),
        members,
        genargs,
    }));
    let fallback = DefinedTyImpl {
        ty_content: Some(ty_content.clone()),
        vals: HashMap::new(),
    };
    hir.tys
        .entry(ty_def_id)
        .or_insert_with(|| fallback)
        .ty_content = Some(ty_content);
}

fn lower_type_alias(
    hir: &mut Hir,
    alias_def: &biwac_ast::TypeAlias,
    _errors: &mut Vec<ResolveError>,
) {
    let ty_def_id = match alias_def.def_id.get() {
        Some(id) => *id,
        None => return,
    };

    let genargs: Vec<GenDefId> = alias_def
        .genargs
        .as_ref()
        .map(|gd| {
            gd.genargs
                .iter()
                .map(|item| {
                    *item
                        .def_id
                        .get()
                        .expect("compiler bug: alias genarg def_id not set")
                })
                .collect()
        })
        .unwrap_or_default();

    let right = ty_from_typ_repr(&alias_def.right, None);

    hir.ty_aliases.insert(
        ty_def_id,
        TypeAliasDef {
            name: alias_def.ident.clone().into(),
            genargs,
            right,
        },
    );
}

fn lower_native_type_alias(hir: &mut Hir, native_def: &biwac_ast::NativeTypeAlias) {
    let ty_def_id = match native_def.def_id.get() {
        Some(id) => *id,
        None => return,
    };

    let genargs: Vec<Ident> = native_def
        .genargs
        .as_ref()
        .map(|gd| {
            gd.genargs
                .iter()
                .map(|item| Ident::from(item.id.clone()))
                .collect()
        })
        .unwrap_or_default();

    let ty_content = TyDefKind::NativeTypeAlias(Box::new(NativeTypeAliasDef {
        name: native_def.ident.clone().into(),
        genargs,
        native: native_def.native.clone(),
        native_span: native_def.native_span.clone(),
    }));
    let fallback = DefinedTyImpl {
        ty_content: Some(ty_content.clone()),
        vals: HashMap::new(),
    };
    hir.tys
        .entry(ty_def_id)
        .or_insert_with(|| fallback)
        .ty_content = Some(ty_content);
}

pub(super) fn lower_impl_block(
    hir: &mut Hir,
    impl_block: &biwac_ast::ImplBlock,
    errors: &mut Vec<ResolveError>,
) {
    // Expand type aliases so we register under the canonical type, not the alias.
    let raw_self_ty = ty_from_typ_repr(&impl_block.self_typ, None);
    let aliases = hir.ty_aliases.clone();
    let self_ty_kind = alias_expansion::expand_ty(raw_self_ty, &aliases).kind;
    let impl_genargs = collect_impl_genargs(impl_block);
    let impl_block_genargs_map = collect_impl_block_genargs_map(impl_block);

    let ty_genargs: Vec<Ty> = if let TyKind::Defined(DefinedTy { ref genargs, .. }) = self_ty_kind {
        genargs.clone()
    } else {
        vec![]
    };

    for fn_def in &impl_block.assoc_fns {
        let signature = build_fn_signature(
            &fn_def.args,
            &fn_def.rtype,
            Some(self_ty_kind.clone()),
            &fn_def.genargs,
            fn_def.span.clone(),
        );
        let body = build_fn_body(
            &fn_def.args,
            &fn_def.stmts,
            fn_def.expr.as_ref(),
            false,
            Some(&self_ty_kind),
            &signature,
            errors,
        );
        let hir_fn = FnDef::new(
            fn_def.id.clone().into(),
            signature,
            body,
            impl_genargs.clone(),
        );
        register_impl_val(
            hir,
            *fn_def.def_id.get().unwrap(),
            &self_ty_kind,
            fn_def.id.id,
            impl_block_genargs_map.clone(),
            ty_genargs.clone(),
            AssocValDefKind::Fn(Box::new(hir_fn)),
        );
    }

    for method_def in &impl_block.methods {
        let args_list = ArgDeclList {
            args: method_def.args.args.clone(),
            span: method_def.args.span.clone(),
        };
        let signature = build_fn_signature(
            &args_list,
            &method_def.rtype,
            Some(self_ty_kind.clone()),
            &method_def.genargs,
            method_def.span.clone(),
        );
        let body = build_fn_body(
            &args_list,
            &method_def.stmts,
            method_def.expr.as_ref(),
            true,
            Some(&self_ty_kind),
            &signature,
            errors,
        );
        let hir_fn = FnDef::new(
            method_def.id.clone().into(),
            signature,
            body,
            impl_genargs.clone(),
        );
        register_impl_val(
            hir,
            *method_def.def_id.get().unwrap(),
            &self_ty_kind,
            method_def.id.id,
            impl_block_genargs_map.clone(),
            ty_genargs.clone(),
            AssocValDefKind::Fn(Box::new(hir_fn)),
        );
    }

    for fn_def in &impl_block.native_assoc_fns {
        let signature = build_fn_signature(
            &fn_def.args,
            &fn_def.rtype,
            Some(self_ty_kind.clone()),
            &fn_def.genargs,
            fn_def.span.clone(),
        );
        let hir_fn = NativeFnDef::new(
            fn_def.id.clone().into(),
            fn_def.native_span.clone(),
            fn_def.span.clone(),
            fn_def.native.clone(),
            signature,
            impl_genargs.clone(),
        );
        register_impl_val(
            hir,
            *fn_def.def_id.get().unwrap(),
            &self_ty_kind,
            fn_def.id.id,
            impl_block_genargs_map.clone(),
            ty_genargs.clone(),
            AssocValDefKind::NativeFn(Box::new(hir_fn)),
        );
    }

    for method_def in &impl_block.native_methods {
        let args_list = ArgDeclList {
            args: method_def.args.args.clone(),
            span: method_def.args.span.clone(),
        };
        let signature = build_fn_signature(
            &args_list,
            &method_def.rtype,
            Some(self_ty_kind.clone()),
            &method_def.genargs,
            method_def.span.clone(),
        );
        let hir_fn = NativeFnDef::new(
            method_def.id.clone().into(),
            method_def.native_span.clone(),
            method_def.span.clone(),
            method_def.native.clone(),
            signature,
            impl_genargs.clone(),
        );
        register_impl_val(
            hir,
            *method_def.def_id.get().unwrap(),
            &self_ty_kind,
            method_def.id.id,
            impl_block_genargs_map.clone(),
            ty_genargs.clone(),
            AssocValDefKind::NativeFn(Box::new(hir_fn)),
        );
    }
}

fn register_impl_val(
    hir: &mut Hir,
    def_id: ValDefId,
    self_ty_kind: &TyKind,
    name: InternedIdent,
    impl_block_genargs: HashMap<InternedIdent, (LocalGenDefId, Span)>,
    genargs: Vec<Ty>,
    val_content: AssocValDefKind,
) {
    match self_ty_kind {
        TyKind::Defined(defined_ty) => {
            let entry = hir.tys.get_mut(&defined_ty.def_id).unwrap();
            entry
                .vals
                .entry(name)
                .or_insert_with(|| TyValImplList {
                    vals: HashMap::new(),
                })
                .vals
                .insert(
                    def_id,
                    TyValImplGenargsContentPair {
                        impl_block_genargs,
                        genargs,
                        val_content,
                    },
                );
        }
        other => {
            if let Some(prim_def_id) = other.def_id() {
                let entry = hir.tys.entry(prim_def_id).or_insert_with(|| DefinedTyImpl {
                    ty_content: None,
                    vals: HashMap::new(),
                });
                entry
                    .vals
                    .entry(name)
                    .or_insert_with(|| TyValImplList {
                        vals: HashMap::new(),
                    })
                    .vals
                    .insert(
                        def_id,
                        TyValImplGenargsContentPair {
                            impl_block_genargs,
                            genargs,
                            val_content,
                        },
                    );
            }
            // Fn/Gen/LocGen/Infer は impl 対象外
        }
    }
}

pub(super) fn lower_native_code(hir: &mut Hir, modpath: &ModPath, native: &biwac_ast::NativeCode) {
    hir.module_global_natives
        .entry(modpath.clone())
        .or_default()
        .push(NativeCode::from(native));
}
