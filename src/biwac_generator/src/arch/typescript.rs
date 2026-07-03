mod context;
mod expression;
mod globals;
mod statement;
mod types;

use std::{cell::Cell, collections::HashMap};

use biwac_base::{IdentInterner, SourceHolder};
use biwac_hir::{AssocValDefKind, Hir, TyDefKind, TyKind, ValDefKind};
use biwac_span::{TyDefId, ValDefId};
use oxc_allocator::FromIn;

use crate::arch::typescript::{
    context::{AstBuildCtx, FnAstBuildCtx},
    globals::native_code_as_oxc,
};

pub fn generate(hir: &Hir, interner: &IdentInterner, srcs: &SourceHolder) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let ctx = AstBuildCtx::new(hir, interner, srcs, &allocator);

    // ライフタイムが長い必要がある
    let native_tys = hir
        .tys
        .iter()
        .flat_map(|(def_id, ty_impl)| match &ty_impl.ty_content {
            Some(TyDefKind::NativeTypeAlias(native)) => Some((
                *def_id,
                // 型単体をパースできないため、文にする
                (&**native, format!("type X = {};", &native.native)),
            )),
            _ => None,
        })
        .collect::<HashMap<TyDefId, (&_, String)>>();

    // module global native code は、必ず先頭に展開される
    let mut body = oxc_allocator::Vec::new_in(&allocator);
    for native in hir
        .module_global_natives
        .values()
        .flat_map(|natives| natives.iter())
    {
        body.extend(native_code_as_oxc(native, &ctx));
    }

    // 依存する外部パッケージのシンボルをimportとして展開
    body.extend(oxc_allocator::Vec::from_iter_in(
        hir.deps_recorder
            .borrow()
            .depended_tys()
            .iter()
            .map(|def_id| {
                oxc_ast::ast::Statement::ImportDeclaration(oxc_allocator::Box::new_in(
                    oxc_ast::ast::ImportDeclaration {
                        span: span(),
                        specifiers: Some(oxc_allocator::Vec::from_iter_in(
                            [oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(
                                oxc_allocator::Box::new_in(
                                    oxc_ast::ast::ImportSpecifier {
                                        span: span(),
                                        imported: oxc_ast::ast::ModuleExportName::IdentifierName(
                                            oxc_ast::ast::IdentifierName {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    allocator
                                                        .alloc_str(&ctx.get_type_mangled(def_id)),
                                                ),
                                            },
                                        ),
                                        local: oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&ctx.get_type_mangled(def_id)),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        import_kind: oxc_ast::ast::ImportOrExportKind::Value,
                                    },
                                    &allocator,
                                ),
                            )],
                            &allocator,
                        )),
                        source: oxc_ast::ast::StringLiteral {
                            span: span(),
                            value: oxc_ast::ast::Atom::from_in(
                                &format!("./{}.ts", ctx.get_package_name_of_type(def_id)),
                                &allocator,
                            ),
                            raw: None,
                            lone_surrogates: false,
                        },
                        phase: None,
                        with_clause: None,
                        import_kind: oxc_ast::ast::ImportOrExportKind::Type,
                    },
                    &allocator,
                ))
            }),
        &allocator,
    ));

    body.extend(oxc_allocator::Vec::from_iter_in(
        hir.deps_recorder
            .borrow()
            .depended_vals()
            .iter()
            .map(|def_id| {
                oxc_ast::ast::Statement::ImportDeclaration(oxc_allocator::Box::new_in(
                    oxc_ast::ast::ImportDeclaration {
                        span: span(),
                        specifiers: Some(oxc_allocator::Vec::from_iter_in(
                            [oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(
                                oxc_allocator::Box::new_in(
                                    oxc_ast::ast::ImportSpecifier {
                                        span: span(),
                                        imported: oxc_ast::ast::ModuleExportName::IdentifierName(
                                            oxc_ast::ast::IdentifierName {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    allocator
                                                        .alloc_str(&ctx.get_value_mangled(def_id)),
                                                ),
                                            },
                                        ),
                                        local: oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&ctx.get_value_mangled(def_id)),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        import_kind: oxc_ast::ast::ImportOrExportKind::Value,
                                    },
                                    &allocator,
                                ),
                            )],
                            &allocator,
                        )),
                        source: oxc_ast::ast::StringLiteral {
                            span: span(),
                            value: oxc_ast::ast::Atom::from_in(
                                &format!("./{}.ts", ctx.get_package_name_of_value(def_id)),
                                &allocator,
                            ),
                            raw: None,
                            lone_surrogates: false,
                        },
                        phase: None,
                        with_clause: None,
                        import_kind: oxc_ast::ast::ImportOrExportKind::Value,
                    },
                    &allocator,
                ))
            }),
        &allocator,
    ));

    body.extend(oxc_allocator::Vec::from_iter_in(
        hir.tys
            .iter()
            .flat_map(|(def_id, ty_impl)| match &ty_impl.ty_content {
                Some(TyDefKind::Struct(struct_)) => {
                    if def_id.pkg().is_self() {
                        Some(struct_.as_oxc_global(ctx.get_type_mangled(def_id), &ctx))
                    } else {
                        // 外部パッケージの型定義は生成しないガード
                        None
                    }
                }
                Some(TyDefKind::NativeTypeAlias(_)) => Some(
                    native_tys
                        .get(def_id)
                        .unwrap()
                        .as_oxc_global(ctx.get_type_mangled(def_id), &ctx),
                ),
                None => None,
            })
            .chain(hir.tys.values().flat_map(|ty_impl| {
                ty_impl.vals.values().flat_map(|impl_list| {
                    impl_list
                        .vals
                        .iter()
                        .map(|(def_id, impl_)| match &impl_.val_content {
                            AssocValDefKind::Fn(f) => {
                                f.as_oxc_global(ctx.get_value_mangled(def_id), &ctx)
                            }
                            AssocValDefKind::NativeFn(f) => {
                                f.as_oxc_global(ctx.get_value_mangled(def_id), &ctx)
                            }
                        })
                })
            }))
            // TODO:
            // .chain(hir.special_ty_impls.iter().flat_map(|(ty, ty_impl)| {
            //     ty_impl.vals.iter().map(|(val_name, val)| match val {
            //         AssocValDefKind::Fn(f) => {
            //             f.as_oxc_global(&(&ty.clone(), val_name.as_str()), &allocator, hir)
            //         }
            //         AssocValDefKind::NativeFn(f) => {
            //             f.as_oxc_global(&(&ty.clone(), val_name.as_str()), &allocator, hir)
            //         }
            //     })
            // }))
            .chain(hir.vals.iter().flat_map(|(def_id, val)| {
                let id = ctx.get_value_mangled(def_id);
                match val {
                    ValDefKind::Fn(f) => Some(f.as_oxc_global(id, &ctx)),
                    ValDefKind::Native(f) => Some(f.as_oxc_global(id, &ctx)),
                    ValDefKind::NovelScene(n) => Some(n.as_oxc_global(id, &ctx)),
                    ValDefKind::ExternalFn(_) => None,
                }
            })),
        &allocator,
    ));

    let oxc_ast = oxc_ast::ast::Program {
        span: span(),
        source_type: oxc_ast::ast::SourceType::ts(), // TypeScript
        body,
        directives: oxc_allocator::Vec::new_in(&allocator),
        hashbang: None,
        source_text: "",
        comments: oxc_allocator::Vec::new_in(&allocator),
        scope_id: Cell::new(None),
    };

    let result = oxc_codegen::Codegen::new().build(&oxc_ast);

    result.code
}

fn span() -> oxc_span::Span {
    oxc_span::Span::new(0, 0)
}

trait AsOxcGlobal<'a, O> {
    fn as_oxc_global(&'a self, id: String, ctx: &'a AstBuildCtx) -> O;
}

trait AsOxcLocal<'a, O> {
    fn as_oxc_local(&'a self, ctx: &'a AstBuildCtx<'a>, fctx: &mut FnAstBuildCtx<'a>) -> O;
}

trait AsOxc<'a, O> {
    fn as_oxc(&'a self, ctx: &'a AstBuildCtx<'a>) -> O;
}

trait Mangled {
    fn mangled(&self, ctx: &AstBuildCtx) -> String;
}

impl Mangled for ValDefId {
    fn mangled(&self, ctx: &AstBuildCtx) -> String {
        ctx.get_value_mangled(self)
    }
}

impl Mangled for TyDefId {
    fn mangled(&self, ctx: &AstBuildCtx) -> String {
        ctx.get_type_mangled(self)
    }
}

impl Mangled for (&TyKind, &str) {
    fn mangled(&self, _ctx: &AstBuildCtx) -> String {
        match self.0 {
            TyKind::Infer(_) => panic!("compiler bug: failed to infer type of expression"),
            TyKind::Void => panic!("compiler bug: Void cannot be implemented method"),
            TyKind::Fn(_) => panic!("compiler bug: function cannot be implemented method"),
            TyKind::Gen(_) => panic!(""),    // ローカルに出現し得ない
            TyKind::LocGen(_) => panic!(""), // ローカルなジェネリック型のメソッドの有効性は判断できないため、呼ばれることはない
            TyKind::Int => format!("_ZN3Int{}{}E", self.1.len(), &self.1,),
            TyKind::Float => format!("_ZN5Float{}{}E", self.1.len(), &self.1,),
            TyKind::Bool => format!("_ZN4Bool{}{}E", self.1.len(), &self.1,),
            TyKind::Defined(_) => {
                panic!("compiler bug: must use (&TyDefId, &str, &ImplValId)")
            }
        }
    }
}
