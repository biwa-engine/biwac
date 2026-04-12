mod expression;
mod globals;
mod statement;
mod types;

use std::{cell::Cell, collections::HashMap};

use biwac_hir::{
    ExprId, Hir, ImplValDefContentKind, LocVarId, Ty, TyDefContentKind, TyId, TyKind,
    ValDefContentKind, ValId,
};
use oxc_allocator::FromIn;

pub fn generate(hir: &Hir) -> String {
    let allocator = oxc_allocator::Allocator::default();

    // ライフタイムが長い必要がある
    let native_tys = hir
        .tys
        .iter()
        .flat_map(
            |(tid, ty_impl)| match &ty_impl.ty_content.expect_completed() {
                TyDefContentKind::Struct(_) => None,
                TyDefContentKind::TypeAlias(_) => None,
                TyDefContentKind::NativeTypeAlias(native) => Some((
                    tid.clone(),
                    // 型単体をパースできないため、文にする
                    (&**native, format!("type X = {};", &native.native)),
                )),
            },
        )
        .collect::<HashMap<TyId, (&_, String)>>();

    // module global native code は、必ず先頭に展開される
    let mut body = oxc_allocator::Vec::new_in(&allocator);
    for native in hir
        .module_global_natives
        .iter()
        .flat_map(|(_modpath, natives)| natives.iter())
    {
        body.extend(native.into_oxc(&allocator, hir));
    }

    // 依存する外部パッケージのシンボルをimportとして展開
    body.extend(oxc_allocator::Vec::from_iter_in(
        hir.deps_recorder.borrow().depended_tys().iter().map(|tid| {
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
                                                allocator.alloc_str(&tid.mangled()),
                                            ),
                                        },
                                    ),
                                    local: oxc_ast::ast::BindingIdentifier {
                                        span: span(),
                                        name: oxc_span::Ident::new_const(
                                            allocator.alloc_str(&tid.mangled()),
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
                            &format!("./{}.ts", tid.pkg().name().value()),
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
            .map(|vid| {
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
                                                    allocator.alloc_str(&vid.mangled()),
                                                ),
                                            },
                                        ),
                                        local: oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&vid.mangled()),
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
                                &format!("./{}.ts", vid.pkg().name().value()),
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
            .flat_map(
                |(tid, ty_impl)| match &ty_impl.ty_content.expect_completed() {
                    TyDefContentKind::Struct(struct_) => {
                        if tid.pkg().name() == &hir.pkg_name {
                            Some(struct_.as_oxc_global(tid, &allocator, hir))
                        } else {
                            // 外部パッケージの型定義は生成しないガード
                            None
                        }
                    }
                    TyDefContentKind::TypeAlias(_) => None, // 型のエイリアスを生成する必要はない
                    TyDefContentKind::NativeTypeAlias(_) => Some(
                        native_tys
                            .get(tid)
                            .unwrap()
                            .as_oxc_global(tid, &allocator, hir),
                    ),
                },
            )
            .chain(hir.tys.iter().flat_map(|(tid, ty_impl)| {
                ty_impl.vals.iter().flat_map(|(val_name, impl_list)| {
                    impl_list
                        .vals
                        .iter()
                        .map(|(impl_valid, impl_)| match &impl_.val_content {
                            ImplValDefContentKind::Fn(f) => f.as_oxc_global(
                                &(&tid.clone(), val_name.as_str(), impl_valid),
                                &allocator,
                                hir,
                            ),
                            ImplValDefContentKind::Method(m) => m.as_oxc_global(
                                &(&tid.clone(), val_name.as_str(), impl_valid),
                                &allocator,
                                hir,
                            ),
                            ImplValDefContentKind::NativeFn(f) => f.as_oxc_global(
                                &(&tid.clone(), val_name.as_str(), impl_valid),
                                &allocator,
                                hir,
                            ),
                            ImplValDefContentKind::NativeMethod(m) => m.as_oxc_global(
                                &(&tid.clone(), val_name.as_str(), impl_valid),
                                &allocator,
                                hir,
                            ),
                        })
                })
            }))
            .chain(hir.special_ty_impls.iter().flat_map(|(ty, ty_impl)| {
                ty_impl.vals.iter().map(|(val_name, val)| match val {
                    ImplValDefContentKind::Fn(f) => {
                        f.as_oxc_global(&(&ty.clone(), val_name.as_str()), &allocator, hir)
                    }
                    ImplValDefContentKind::Method(m) => {
                        m.as_oxc_global(&(&ty.clone(), val_name.as_str()), &allocator, hir)
                    }
                    ImplValDefContentKind::NativeFn(f) => {
                        f.as_oxc_global(&(&ty.clone(), val_name.as_str()), &allocator, hir)
                    }
                    ImplValDefContentKind::NativeMethod(m) => {
                        m.as_oxc_global(&(&ty.clone(), val_name.as_str()), &allocator, hir)
                    }
                })
            }))
            .chain(hir.vals.iter().flat_map(|(vid, val)| match val {
                ValDefContentKind::Fn(f) => Some(f.as_oxc_global(vid, &allocator, hir)),
                ValDefContentKind::Native(f) => Some(f.as_oxc_global(vid, &allocator, hir)),
                ValDefContentKind::NovelScene(n) => Some(n.as_oxc_global(vid, &allocator, hir)),
                ValDefContentKind::ExternalFn(_) => None,
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

struct FnAstBuildEnv<'a> {
    pub(super) expr_tys: &'a HashMap<ExprId, Ty>,
    pub(super) var_tys: &'a HashMap<LocVarId, Ty>,
    pub(super) stmts: Vec<oxc_ast::ast::Statement<'a>>,
}

// struct TyInfo {
//     pub(super) expr_tys: HashMap<ExprId, Ty>,
// }

trait AsOxcGlobal<'a, O, I: Mangled> {
    fn as_oxc_global(&'a self, id: &I, allocator: &'a oxc_allocator::Allocator, hir: &Hir) -> O;
}

trait AsOxc<'a, O> {
    fn as_oxc(
        &'a self,
        env: &mut FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
        hir: &Hir,
    ) -> O;
}

trait IntoOxc<'a, O> {
    fn into_oxc(self, allocator: &'a oxc_allocator::Allocator, hir: &Hir) -> O;
}

trait Mangled {
    fn mangled(&self) -> String;
}

impl Mangled for TyId {
    fn mangled(&self) -> String {
        let mut result = String::from("_Z");

        // ネストがある場合は N ... E で囲む
        result.push('N');

        let pkg_name_str = self.pkg().name().value();
        result.push_str(&format!("{}{}", pkg_name_str.len(), pkg_name_str));

        for q in self.quals() {
            result.push_str(&format!("{}{}", q.len(), q));
        }

        result.push_str(&format!("{}{}", self.id().len(), self.id()));
        result.push('E');

        result
    }
}

impl Mangled for ValId {
    fn mangled(&self) -> String {
        let mut result = String::from("_Z");

        // ネストがある場合は N ... E で囲む
        result.push('N');

        let pkg_name_str = self.pkg().name().value();
        result.push_str(&format!("{}{}", pkg_name_str.len(), pkg_name_str));

        for q in self.quals() {
            result.push_str(&format!("{}{}", q.len(), q));
        }

        result.push_str(&format!("{}{}", self.id().len(), self.id()));
        result.push('E');

        result
    }
}

impl Mangled for (&TyKind, &str) {
    fn mangled(&self) -> String {
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
                panic!("compiler bug: must use (&TyId, &str, &ImplValId)")
            }
        }
    }
}
