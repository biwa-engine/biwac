use std::collections::HashMap;

use biwac_ast::{ArgDeclList, RetTypRepr, TypeDef, VariantFieldsDecl};
use biwac_base::InternedIdent;
use biwac_hir::{
    AssocValDefKind, DefinedTy, DefinedTyImpl, EnumDef, FnArgDecl, FnBody, FnDef, FnSignature,
    Ident, NativeCode, NativeFnDef, NativeTypeAliasDef, StructDef, TraitDef, TraitItemDef, Ty,
    TyDefKind, TyKind, TyValImplGenargsContentPair, TyValImplList, TypeAliasDef, ValDefKind,
    VariantDef,
};
use biwac_span::{GenDefId, LocalGenDefId, Span, TraitDefId, TyDefId, ValDefId, VarId};

use crate::{ResolveError, resolving::def_collector::ImplCollector};

use super::{
    ExprLowerCtx, alias_expansion::expand_ty, expressions::lower_expr, statements::lower_stmt,
    ty_from_typ_repr,
};

/// 関数シグネチャを HIR に落とす。
///
/// `self_ty` は `Self` を型として解決するために使う。
/// impl block の中であれば関連関数にもメソッドにも必要になる。
///
/// 一方 `has_self` はレシーバを取るか (メソッドか) を表し、
/// `FnSignature::self_ty` に反映される。
/// この 2 つを混同すると関連関数にもレシーバがあることになり、
/// codegen が余分な第一引数を出力してしまう。
pub(super) fn build_fn_signature(
    args: &ArgDeclList,
    rtype: &RetTypRepr,
    self_ty: Option<TyKind>,
    has_self: bool,
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

    // impl ブロックの対象型は関連関数でも要るので、
    // レシーバの有無で絞る前に控えておく。
    let impl_self_ty_hir = self_ty.clone().map(|k| Ty::new(k, span.clone()));
    let self_ty_hir = self_ty
        .filter(|_| has_self)
        .map(|k| Ty::new(k, span.clone()));

    FnSignature {
        args: hir_args,
        self_ty: self_ty_hir,
        impl_self_ty: impl_self_ty_hir,
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

pub(crate) fn collect_impl_block_genargs_map(
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
    fn_def: &biwac_ast::FnDef,
    impl_genargs: Vec<(Ident, LocalGenDefId)>,
    errors: &mut Vec<ResolveError>,
) -> (ValDefId, ValDefKind) {
    let val_def_id = *fn_def
        .def_id
        .get()
        .expect("compiler bug: def_id not assigned before lowering");

    let signature = build_fn_signature(
        &fn_def.args,
        &fn_def.rtype,
        None,
        false,
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

    (
        val_def_id,
        ValDefKind::Fn(Box::new(FnDef::new(
            fn_def.id.clone().into(),
            signature,
            body,
            impl_genargs,
        ))),
    )
}

pub(super) fn lower_native_fn_def(
    fn_def: &biwac_ast::NativeFnDef,
    impl_genargs: Vec<(Ident, LocalGenDefId)>,
    _errors: &mut Vec<ResolveError>,
) -> (ValDefId, ValDefKind) {
    let val_def_id = *fn_def
        .def_id
        .get()
        .expect("compiler bug: def_id not assigned before lowering");

    let signature = build_fn_signature(
        &fn_def.args,
        &fn_def.rtype,
        None,
        false,
        &fn_def.genargs,
        fn_def.span.clone(),
    );

    (
        val_def_id,
        ValDefKind::Native(Box::new(NativeFnDef::new(
            fn_def.id.clone().into(),
            fn_def.native_span.clone(),
            fn_def.span.clone(),
            fn_def.native.clone(),
            signature,
            impl_genargs,
        ))),
    )
}

/// 型定義を lower する。
///
/// 型 alias は `tys` ではなく `aliases` 側に入る。
/// alias は独立した型ではなく別名にすぎず、
/// [`crate::lowering::alias_expansion`] が使用箇所を右辺で置き換えるためである。
pub(crate) fn lower_type_def(
    type_def: &TypeDef,
    tys: &mut Vec<(TyDefId, DefinedTyImpl)>,
    aliases: &mut Vec<(TyDefId, TypeAliasDef)>,
    errors: &mut Vec<ResolveError>,
) {
    match type_def {
        TypeDef::Struct(s) => {
            tys.push(lower_struct_def(s, errors));
        }
        TypeDef::Enum(e) => {
            tys.push(lower_enum_def(e, errors));
        }
        TypeDef::NativeTypeAlias(n) => {
            tys.push(lower_native_type_alias(n));
        }
        TypeDef::TypeAlias(a) => {
            aliases.push(lower_type_alias(a, errors));
        }
    }
}

fn lower_struct_def(
    struct_def: &biwac_ast::StructDef,
    _errors: &mut Vec<ResolveError>,
) -> (TyDefId, DefinedTyImpl) {
    let ty_def_id = *struct_def
        .def_id
        .get()
        .expect("compiler bug: def_id not assigned before lowering");

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

    (
        ty_def_id,
        DefinedTyImpl {
            ty_content: Some(ty_content.clone()),
            vals: HashMap::new(),
            trait_impls: Vec::new(),
        },
    )
}

/// enum を lower する。
///
/// タプル形式のフィールドは `_0`, `_1` に正規化する。
/// 名前を持つ形に揃えておけば、MIR も backend も struct と同じ経路を通れる。
fn lower_enum_def(
    enum_def: &biwac_ast::EnumDef,
    _errors: &mut Vec<ResolveError>,
) -> (TyDefId, DefinedTyImpl) {
    let ty_def_id = *enum_def
        .def_id
        .get()
        .expect("compiler bug: def_id not assigned before lowering");

    let genargs: Vec<GenDefId> = enum_def
        .genargs
        .as_ref()
        .map(|gd| {
            gd.genargs
                .iter()
                .map(|item| {
                    *item
                        .def_id
                        .get()
                        .expect("compiler bug: enum genarg def_id not set")
                })
                .collect()
        })
        .unwrap_or_default();

    // 宣言順のまま。添字がそのままタグの値になる。
    let variants = enum_def
        .variants
        .iter()
        .map(|variant| {
            let fields = match &variant.fields {
                VariantFieldsDecl::Unit => Vec::new(),
                // タプル形式も `_0`, `_1` の名前を持つ形にパーサが揃えてある。
                VariantFieldsDecl::Tuple(fields) | VariantFieldsDecl::Struct(fields) => fields
                    .iter()
                    .map(|(ident, typ)| (ident.clone().into(), ty_from_typ_repr(typ, None)))
                    .collect(),
            };

            VariantDef {
                name: variant.id.clone().into(),
                def_id: *variant
                    .def_id
                    .get()
                    .expect("compiler bug: variant def_id not assigned before lowering"),
                shape: variant.fields.shape(),
                fields,
            }
        })
        .collect();

    let ty_content = TyDefKind::Enum(Box::new(EnumDef {
        name: enum_def.id.clone().into(),
        variants,
        genargs,
    }));

    (
        ty_def_id,
        DefinedTyImpl {
            ty_content: Some(ty_content),
            vals: HashMap::new(),
            trait_impls: Vec::new(),
        },
    )
}

fn lower_type_alias(
    alias_def: &biwac_ast::TypeAlias,
    _errors: &mut Vec<ResolveError>,
) -> (TyDefId, TypeAliasDef) {
    let ty_def_id = *alias_def
        .def_id
        .get()
        .expect("compiler bug: def_id not assigned before lowering");

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

    (
        ty_def_id,
        TypeAliasDef {
            name: alias_def.ident.clone().into(),
            genargs,
            right,
        },
    )
}

fn lower_native_type_alias(native_def: &biwac_ast::NativeTypeAlias) -> (TyDefId, DefinedTyImpl) {
    let ty_def_id = *native_def
        .def_id
        .get()
        .expect("compiler bug: def_id not assigned before lowering");

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

    (
        ty_def_id,
        DefinedTyImpl {
            ty_content: Some(ty_content.clone()),
            vals: HashMap::new(),
            trait_impls: Vec::new(),
        },
    )
}

pub(super) fn lower_impl_block(
    tys: &mut HashMap<TyDefId, DefinedTyImpl>,
    impl_block: &biwac_ast::ImplBlock,
    impl_collector: &ImplCollector,
    ty_aliases: &HashMap<TyDefId, TypeAliasDef>,
    errors: &mut Vec<ResolveError>,
) {
    let impl_id = impl_block.impl_id.get().unwrap();

    // 実装対象が型エイリアスで書かれていたら、ここで右辺に展開する。
    //
    // 名前解決はエイリアスを型の位置で潰さない
    // (潰すと `type C = Character[P]` の `[P]` が失われる) ので、
    // `impl_self_tys` にはエイリアス自身の `TyDefId` が入っている。
    // それをそのまま実装の置き場所にすると、
    // エイリアスは `.biwameta` のシンボルにならないため、
    // 関連関数が「シンボル表に無い型」にぶら下がってしまう。
    //
    // 展開は `alias_expansion` (Pass 5) が式や シグニチャに対して行うが、
    // `tys` の **キー** は書き換えられない。だからここで展開しておく必要がある。
    let self_ty_kind = expand_ty(
        Ty::new(
            impl_collector.impl_self_tys.get(impl_id).unwrap().clone(),
            impl_block.self_typ.span.clone(),
        ),
        ty_aliases,
    )
    .kind;

    // trait impl なら、その項目は直接の関連アイテムとしては見えない。
    // 印を付けておかないと import 無しで引けてしまう。
    let trait_impl_of = impl_collector.impl_traits.get(impl_id);
    let trait_of = trait_impl_of.map(|(def_id, _)| *def_id);
    let impl_genargs = collect_impl_genargs(impl_block);
    let impl_block_genargs_map = collect_impl_block_genargs_map(impl_block);

    let ty_genargs: Vec<Ty> = if let TyKind::Defined(DefinedTy { ref genargs, .. }) = self_ty_kind {
        genargs.clone()
    } else {
        vec![]
    };

    // trait impl の索引を、展開後の型の下に張る。
    //
    // def collection 側の索引 (`ImplCollector::trait_impls`) は
    // 名前解決のフォールバック専用で、エイリアスの型引数を持っていない。
    // 特殊化まで見るメソッド解決はこちらを読む。
    if let Some((trait_def_id, trait_genargs)) = trait_impl_of {
        let mut vals: HashMap<InternedIdent, ValDefId> = HashMap::new();
        for f in &impl_block.assoc_fns {
            vals.insert(f.id.id, *f.def_id.get().unwrap());
        }
        for m in &impl_block.methods {
            vals.insert(m.id.id, *m.def_id.get().unwrap());
        }
        for f in &impl_block.native_assoc_fns {
            vals.insert(f.id.id, *f.def_id.get().unwrap());
        }
        for m in &impl_block.native_methods {
            vals.insert(m.id.id, *m.def_id.get().unwrap());
        }

        if let Some(target) = self_ty_kind.def_id() {
            tys.entry(target)
                .or_insert_with(|| DefinedTyImpl {
                    ty_content: None,
                    vals: HashMap::new(),
                    trait_impls: Vec::new(),
                })
                .trait_impls
                .push(biwac_hir::TyTraitImpl {
                    trait_def_id: *trait_def_id,
                    impl_block_genargs: impl_block_genargs_map.clone(),
                    ty_genargs: ty_genargs.clone(),
                    trait_genargs: trait_genargs.clone(),
                    vals,
                    span: impl_block.span.clone(),
                });
        }
    }

    for fn_def in &impl_block.assoc_fns {
        let signature = build_fn_signature(
            &fn_def.args,
            &fn_def.rtype,
            Some(self_ty_kind.clone()),
            false,
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
            tys,
            *fn_def.def_id.get().unwrap(),
            &self_ty_kind,
            fn_def.id.id,
            impl_block_genargs_map.clone(),
            ty_genargs.clone(),
            AssocValDefKind::Fn(Box::new(hir_fn)),
            trait_of,
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
            true,
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
            tys,
            *method_def.def_id.get().unwrap(),
            &self_ty_kind,
            method_def.id.id,
            impl_block_genargs_map.clone(),
            ty_genargs.clone(),
            AssocValDefKind::Fn(Box::new(hir_fn)),
            trait_of,
        );
    }

    for fn_def in &impl_block.native_assoc_fns {
        let signature = build_fn_signature(
            &fn_def.args,
            &fn_def.rtype,
            Some(self_ty_kind.clone()),
            false,
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
            tys,
            *fn_def.def_id.get().unwrap(),
            &self_ty_kind,
            fn_def.id.id,
            impl_block_genargs_map.clone(),
            ty_genargs.clone(),
            AssocValDefKind::NativeFn(Box::new(hir_fn)),
            trait_of,
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
            true,
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
            tys,
            *method_def.def_id.get().unwrap(),
            &self_ty_kind,
            method_def.id.id,
            impl_block_genargs_map.clone(),
            ty_genargs.clone(),
            AssocValDefKind::NativeFn(Box::new(hir_fn)),
            trait_of,
        );
    }
}

/// 関連アイテム 1 つを型の実装表に登録する。
///
/// `trait_of` は省略できない。
/// 付け忘れると trait impl の項目が直接の関連アイテムとして見えてしまい、
/// import 規則が効かなくなる。呼ぶ側に必ず書かせる。
#[allow(clippy::too_many_arguments)]
fn register_impl_val(
    tys: &mut HashMap<TyDefId, DefinedTyImpl>,
    def_id: ValDefId,
    self_ty_kind: &TyKind,
    name: InternedIdent,
    impl_block_genargs: HashMap<InternedIdent, (LocalGenDefId, Span)>,
    genargs: Vec<Ty>,
    val_content: AssocValDefKind,
    trait_of: Option<TraitDefId>,
) {
    let pair = TyValImplGenargsContentPair {
        impl_block_genargs,
        genargs,
        val_content,
        trait_of,
    };

    match self_ty_kind {
        TyKind::Defined(defined_ty) => {
            // 外部パッケージの型への trait impl では、
            // その型はまだ `tys` に居ない。中身の無い入れ物を作る
            // (型の定義は依存メタデータの側にある)。
            let entry = tys
                .entry(defined_ty.def_id)
                .or_insert_with(|| DefinedTyImpl {
                    ty_content: None,
                    vals: HashMap::new(),
                    trait_impls: Vec::new(),
                });
            entry
                .vals
                .entry(name)
                .or_insert_with(|| TyValImplList {
                    vals: HashMap::new(),
                })
                .vals
                .insert(def_id, pair);
        }
        other => {
            if let Some(prim_def_id) = other.def_id() {
                let entry = tys.entry(prim_def_id).or_insert_with(|| DefinedTyImpl {
                    ty_content: None,
                    vals: HashMap::new(),
                    trait_impls: Vec::new(),
                });
                entry
                    .vals
                    .entry(name)
                    .or_insert_with(|| TyValImplList {
                        vals: HashMap::new(),
                    })
                    .vals
                    .insert(def_id, pair);
            }
            // Fn/Gen/LocGen/Infer は impl 対象外
        }
    }
}

/// trait の宣言を lower する。
///
/// 項目は本体を持たないのでシグニチャだけを作る。
/// 宣言の中の `Self` は [`TraitDef::self_gen`] の `TyKind::Gen` になる。
pub(crate) fn lower_trait_def(trait_def: &biwac_ast::TraitDef) -> (TraitDefId, TraitDef) {
    let def_id = *trait_def
        .def_id
        .get()
        .expect("compiler bug: trait def_id not assigned before lowering");
    let self_gen = *trait_def
        .self_gen
        .get()
        .expect("compiler bug: trait self_gen not assigned before lowering");
    let self_ty_kind = TyKind::Gen(self_gen);

    let genargs: Vec<GenDefId> = trait_def
        .genargs
        .as_ref()
        .map(|gd| {
            gd.genargs
                .iter()
                .map(|item| {
                    *item
                        .def_id
                        .get()
                        .expect("compiler bug: trait genarg def_id not set")
                })
                .collect()
        })
        .unwrap_or_default();

    let items = trait_def
        .items
        .iter()
        .map(|item| {
            let (args, is_method) = match &item.args {
                biwac_ast::TraitItemArgs::Assoc(a) => (a, false),
                biwac_ast::TraitItemArgs::Method(a) => (
                    &ArgDeclList {
                        args: a.args.clone(),
                        span: a.span.clone(),
                    },
                    true,
                ),
            };

            let signature = build_fn_signature(
                args,
                &item.rtype,
                Some(self_ty_kind.clone()),
                is_method,
                &item.genargs,
                item.span.clone(),
            );

            TraitItemDef {
                name: item.id.clone().into(),
                def_id: *item
                    .def_id
                    .get()
                    .expect("compiler bug: trait item def_id not assigned"),
                signature,
            }
        })
        .collect();

    (
        def_id,
        TraitDef {
            name: trait_def.id.clone().into(),
            self_gen,
            items,
            genargs,
        },
    )
}

/// trait impl が宣言と一致しているかを検査する。
///
/// 索引 (`DefinedTyImpl::trait_impls`) と実体 (`DefinedTyImpl::vals`) は
/// [`lower_impl_block`] が既に入れてある。ここでやるのは検査だけである。
///
/// 検査をここまで遅らせるのは、def collection の段では
/// trait の項目の型がまだ解決されていないからである。
pub(crate) fn check_trait_impls(
    tys: &HashMap<TyDefId, DefinedTyImpl>,
    traits: &HashMap<TraitDefId, TraitDef>,
    ext_pkgs: &[biwac_dependency_metadata::ExternalPackage],
    interner: &mut biwac_base::IdentInterner,
    errors: &mut Vec<ResolveError>,
) {
    // 走査の順序を固定する。エラーの並びがビルドごとに変わらないようにするため。
    let mut targets: Vec<(&TyDefId, &Vec<biwac_hir::TyTraitImpl>)> = tys
        .iter()
        .filter(|(_, ty_impl)| !ty_impl.trait_impls.is_empty())
        .map(|(ty_def_id, ty_impl)| (ty_def_id, &ty_impl.trait_impls))
        .collect();
    targets.sort_by_key(|(ty_def_id, _)| ty_def_id.value());

    for (ty_def_id, impls) in targets {
        for imp in impls {
            // 外部パッケージの trait は `.biwameta` から復元する。
            let ext_trait_def = if imp.trait_def_id.pkg().is_self() {
                None
            } else {
                ext_pkgs
                    .iter()
                    .find(|p| p.pkg_id == imp.trait_def_id.pkg())
                    .and_then(|p| {
                        p.meta.get_ext_trait_def(
                            imp.trait_def_id.local_idx(),
                            imp.trait_def_id.pkg(),
                            interner,
                        )
                    })
            };

            if let Some(trait_def) = traits.get(&imp.trait_def_id).or(ext_trait_def.as_ref()) {
                check_trait_impl(*ty_def_id, imp, trait_def, tys, errors);
            }
        }
    }
}

fn check_trait_impl(
    ty_def_id: TyDefId,
    imp: &biwac_hir::TyTraitImpl,
    trait_def: &TraitDef,
    tys: &HashMap<TyDefId, DefinedTyImpl>,
    errors: &mut Vec<ResolveError>,
) {
    // 宣言の `Self` と trait のジェネリック引数を、この impl のもので置き換える。
    let mut assigns: HashMap<GenDefId, TyKind> = HashMap::new();
    assigns.insert(
        trait_def.self_gen,
        TyKind::Defined(DefinedTy {
            def_id: ty_def_id,
            genargs: imp.ty_genargs.clone(),
        }),
    );
    for (gid, ty) in trait_def.genargs.iter().zip(&imp.trait_genargs) {
        assigns.insert(*gid, ty.kind.clone());
    }

    // 宣言された項目がすべて実装されているか。
    for item in &trait_def.items {
        let Some(val_def_id) = imp.vals.get(&item.name.id) else {
            errors.push(ResolveError::MissingTraitItem {
                name: item.name.id,
                span: imp.span.clone(),
            });
            continue;
        };

        let Some(actual) = find_signature(tys, ty_def_id, item.name.id, val_def_id) else {
            continue;
        };

        let expected = substitute_signature(&item.signature, &assigns);
        if let Some(detail) = signature_mismatch(&expected, actual) {
            errors.push(ResolveError::TraitItemSignatureMismatch {
                name: item.name.id,
                span: actual.span.clone(),
                decl_span: item.signature.span.clone(),
                detail,
            });
        }
    }

    // 宣言に無い項目が実装されていないか。
    for name in imp.vals.keys() {
        if trait_def.item(name).is_none() {
            errors.push(ResolveError::UnknownTraitItem {
                name: *name,
                span: imp.span.clone(),
            });
        }
    }
}

fn find_signature<'a>(
    tys: &'a HashMap<TyDefId, DefinedTyImpl>,
    ty_def_id: TyDefId,
    name: InternedIdent,
    val_def_id: &ValDefId,
) -> Option<&'a FnSignature> {
    match &tys
        .get(&ty_def_id)?
        .vals
        .get(&name)?
        .vals
        .get(val_def_id)?
        .val_content
    {
        AssocValDefKind::Fn(f) => Some(&f.signature),
        AssocValDefKind::NativeFn(f) => Some(&f.signature),
    }
}

fn substitute_signature(sig: &FnSignature, assigns: &HashMap<GenDefId, TyKind>) -> FnSignature {
    FnSignature {
        args: sig
            .args
            .iter()
            .map(|a| FnArgDecl {
                id: a.id.clone(),
                ty: a.ty.clone().embody_by_gen_ty_id(assigns),
                var_id: a.var_id,
            })
            .collect(),
        self_ty: sig.self_ty.clone().map(|t| t.embody_by_gen_ty_id(assigns)),
        impl_self_ty: sig
            .impl_self_ty
            .clone()
            .map(|t| t.embody_by_gen_ty_id(assigns)),
        rty: sig.rty.clone().embody_by_gen_ty_id(assigns),
        genargs: sig.genargs.clone(),
        span: sig.span.clone(),
    }
}

/// 食い違っていれば、その説明を返す。
fn signature_mismatch(expected: &FnSignature, actual: &FnSignature) -> Option<String> {
    if expected.self_ty.is_some() != actual.self_ty.is_some() {
        return Some(if expected.self_ty.is_some() {
            "the trait declares this as a method taking `self`".to_string()
        } else {
            "the trait declares this as an associated function without `self`".to_string()
        });
    }

    if expected.args.len() != actual.args.len() {
        return Some(format!(
            "expected {} argument(s), found {}",
            expected.args.len(),
            actual.args.len()
        ));
    }

    for (i, (e, a)) in expected.args.iter().zip(&actual.args).enumerate() {
        if e.ty.kind != a.ty.kind {
            return Some(format!("argument {} has a different type", i + 1));
        }
    }

    if expected.rty.kind != actual.rty.kind {
        return Some("the return type is different".to_string());
    }

    None
}

pub(super) fn lower_native_code(native: &biwac_ast::NativeCode) -> NativeCode {
    NativeCode::from(native)
}
