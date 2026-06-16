use std::cell::Cell;

use biwac_hir::{
    FnDef, Hir, NativeCode, NativeFnDef, NativeTypeAliasDef, NovelSceneDef, StructDef,
};

use biwac_span::VarId;
use oxc_allocator::CloneIn;

use crate::arch::typescript::{
    AsOxc, AsOxcGlobal, AstBuildCtx, FnAstBuildEnv, IntoOxc, Mangled, span,
};

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for StructDef {
    fn as_oxc_global(&'a self, id: String, ctx: &AstBuildCtx) -> oxc_ast::ast::Statement<'a> {
        oxc_ast::ast::Statement::TSTypeAliasDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::TSTypeAliasDeclaration {
                span: span(),
                id: oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(&id)),
                    symbol_id: Cell::new(None),
                },
                type_parameters: if !self.genargs.is_empty() {
                    Some(oxc_allocator::Box::new_in(
                        oxc_ast::ast::TSTypeParameterDeclaration {
                            span: span(),
                            params: oxc_allocator::Vec::from_iter_in(
                                self.genargs
                                    .iter()
                                    .map(|gid| oxc_ast::ast::TSTypeParameter {
                                        span: span(),
                                        name: oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                ctx.allocator.alloc_str(&gid.mangled()),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        constraint: None,
                                        default: None,
                                        r#in: false,
                                        out: false,
                                        r#const: false,
                                    }),
                                &ctx.allocator,
                            ),
                        },
                        &ctx.allocator,
                    ))
                } else {
                    None
                },
                type_annotation: oxc_ast::ast::TSType::TSTypeLiteral(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeLiteral {
                        span: span(),
                        members: oxc_allocator::Vec::from_iter_in(
                            self.members.iter().map(|(id, ty)| {
                                oxc_ast::ast::TSSignature::TSPropertySignature(
                                    oxc_allocator::Box::new_in(
                                        oxc_ast::ast::TSPropertySignature {
                                            span: span(),
                                            computed: false,
                                            optional: false,
                                            readonly: false,
                                            key: oxc_ast::ast::PropertyKey::StaticIdentifier(
                                                oxc_allocator::Box::new_in(
                                                    oxc_ast::ast::IdentifierName {
                                                        span: span(),
                                                        name: oxc_span::Ident::new_const(
                                                            &ctx.allocator
                                                                .alloc_str(ctx.str_of(id)),
                                                        ),
                                                    },
                                                    &ctx.allocator,
                                                ),
                                            ),
                                            type_annotation: Some(oxc_allocator::Box::new_in(
                                                oxc_ast::ast::TSTypeAnnotation {
                                                    span: span(),
                                                    type_annotation: ty
                                                        .kind
                                                        .clone()
                                                        .into_oxc(allocator, hir),
                                                },
                                                &ctx.allocator,
                                            )),
                                        },
                                        &ctx.allocator,
                                    ),
                                )
                            }),
                            &ctx.allocator,
                        ),
                    },
                    &ctx.allocator,
                )),
                scope_id: Cell::new(None),
                declare: false,
            },
            &ctx.allocator,
        ))
    }
}

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for FnDef {
    fn as_oxc_global(&'a self, id: String, ctx: &'a AstBuildCtx) -> oxc_ast::ast::Statement<'a> {
        let mut env = FnAstBuildEnv {
            expr_tys: &self.expr_tys,
            var_tys: &self.var_tys,
            stmts: vec![],
        };

        let fn_body = &self.body;

        for stmt in &fn_body.stmts {
            let oxc_stmt = stmt.as_oxc(&mut env, allocator, hir);
            env.stmts.push(oxc_stmt);
        }

        if let Some(expr) = &fn_body.expr {
            let oxc_return = oxc_ast::ast::Statement::ReturnStatement(oxc_allocator::Box::new_in(
                oxc_ast::ast::ReturnStatement {
                    span: span(),
                    argument: Some(expr.as_oxc(&mut env, allocator, hir)),
                },
                allocator,
            ));
            env.stmts.push(oxc_return);
        }

        oxc_ast::ast::Statement::FunctionDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::Function {
                span: span(),
                id: Some(oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(allocator.alloc_str(&id.mangled())),
                    symbol_id: Cell::new(None),
                }),
                generator: false,
                r#type: oxc_ast::ast::FunctionType::FunctionDeclaration,
                r#async: false,
                pure: false,
                pife: false,
                declare: false,
                scope_id: Cell::new(None),
                this_param: None,
                params: oxc_allocator::Box::new_in(
                    oxc_ast::ast::FormalParameters {
                        span: span(),
                        kind: oxc_ast::ast::FormalParameterKind::FormalParameter,
                        items: oxc_allocator::Vec::from_iter_in(
                            self.signature
                                .self_ty
                                .iter()
                                .map(|ty| oxc_ast::ast::FormalParameter {
                                    span: span(),
                                    decorators: oxc_allocator::Vec::new_in(allocator),
                                    pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                        oxc_allocator::Box::new_in(
                                            oxc_ast::ast::BindingIdentifier {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    allocator
                                                        .alloc_str(&VarId::SELF_VARIABLE.mangled()),
                                                ),
                                                symbol_id: Cell::new(None),
                                            },
                                            allocator,
                                        ),
                                    ),
                                    type_annotation: Some(oxc_allocator::Box::new_in(
                                        oxc_ast::ast::TSTypeAnnotation {
                                            span: span(),
                                            type_annotation: ty
                                                .kind
                                                .as_oxc(&mut env, allocator, hir),
                                        },
                                        allocator,
                                    )),
                                    initializer: None,
                                    optional: false,
                                    accessibility: None,
                                    readonly: false,
                                    r#override: false,
                                })
                                .into_iter()
                                .chain(self.signature.args.iter().map(|arg| {
                                    oxc_ast::ast::FormalParameter {
                                        span: span(),
                                        decorators: oxc_allocator::Vec::new_in(allocator),
                                        pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                            oxc_allocator::Box::new_in(
                                                oxc_ast::ast::BindingIdentifier {
                                                    span: span(),
                                                    name: oxc_span::Ident::new_const(
                                                        allocator.alloc_str(&arg.var_id.mangled()),
                                                    ),
                                                    symbol_id: Cell::new(None),
                                                },
                                                allocator,
                                            ),
                                        ),
                                        type_annotation: Some(oxc_allocator::Box::new_in(
                                            oxc_ast::ast::TSTypeAnnotation {
                                                span: span(),
                                                type_annotation: arg
                                                    .ty
                                                    .kind
                                                    .as_oxc(&mut env, allocator, hir),
                                            },
                                            allocator,
                                        )),
                                        initializer: None,
                                        optional: false,
                                        accessibility: None,
                                        readonly: false,
                                        r#override: false,
                                    }
                                })),
                            allocator,
                        ),
                        rest: None,
                    },
                    allocator,
                ),
                body: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::FunctionBody {
                        span: span(),
                        directives: oxc_allocator::Vec::new_in(allocator),
                        statements: oxc_allocator::Vec::from_iter_in(env.stmts, allocator),
                    },
                    allocator,
                )),
                type_parameters: if !self.signature.genargs.is_empty()
                    || !self.impl_genargs.is_empty()
                {
                    Some(oxc_allocator::Box::new_in(
                        oxc_ast::ast::TSTypeParameterDeclaration {
                            span: span(),
                            params: oxc_allocator::Vec::from_iter_in(
                                self.impl_genargs
                                    .iter()
                                    .map(|(_, lgid)| oxc_ast::ast::TSTypeParameter {
                                        span: span(),
                                        name: oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&lgid.mangled()),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        constraint: None,
                                        default: None,
                                        r#in: false,
                                        out: false,
                                        r#const: false,
                                    })
                                    .chain(self.signature.genargs.iter().map(|(_, lgid)| {
                                        oxc_ast::ast::TSTypeParameter {
                                            span: span(),
                                            name: oxc_ast::ast::BindingIdentifier {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    allocator.alloc_str(&lgid.mangled()),
                                                ),
                                                symbol_id: Cell::new(None),
                                            },
                                            constraint: None,
                                            default: None,
                                            r#in: false,
                                            out: false,
                                            r#const: false,
                                        }
                                    })),
                                allocator,
                            ),
                        },
                        allocator,
                    ))
                } else {
                    None
                },
                return_type: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeAnnotation {
                        span: span(),
                        type_annotation: self.signature.rty.kind.clone().into_oxc(allocator, hir),
                    },
                    allocator,
                )),
            },
            allocator,
        ))
    }
}

impl<'a, I: Mangled> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>, I> for NativeFnDef {
    fn as_oxc_global(
        &'a self,
        id: &I,
        allocator: &'a oxc_allocator::Allocator,
        hir: &Hir,
    ) -> oxc_ast::ast::Statement<'a> {
        // TODO: そもそもnativeのターゲットがTSかチェック

        // TSをパースして取り込む
        let ts = oxc_parser::Parser::new(allocator, &self.native_body, oxc_span::SourceType::ts())
            .parse();

        oxc_ast::ast::Statement::FunctionDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::Function {
                span: span(),
                id: Some(oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(allocator.alloc_str(&id.mangled())),
                    symbol_id: Cell::new(None),
                }),
                generator: false,
                r#type: oxc_ast::ast::FunctionType::FunctionDeclaration,
                r#async: false,
                pure: false,
                pife: false,
                declare: false,
                scope_id: Cell::new(None),
                this_param: None,
                params: oxc_allocator::Box::new_in(
                    oxc_ast::ast::FormalParameters {
                        span: span(),
                        kind: oxc_ast::ast::FormalParameterKind::FormalParameter,
                        items: oxc_allocator::Vec::from_iter_in(
                            self.signature
                                .args
                                .iter()
                                .map(|arg| oxc_ast::ast::FormalParameter {
                                    span: span(),
                                    decorators: oxc_allocator::Vec::new_in(allocator),
                                    pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                        oxc_allocator::Box::new_in(
                                            oxc_ast::ast::BindingIdentifier {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    allocator.alloc_str(&arg.id.id),
                                                ),
                                                symbol_id: Cell::new(None),
                                            },
                                            allocator,
                                        ),
                                    ),
                                    type_annotation: Some(oxc_allocator::Box::new_in(
                                        oxc_ast::ast::TSTypeAnnotation {
                                            span: span(),
                                            type_annotation: arg
                                                .ty
                                                .kind
                                                .clone()
                                                .into_oxc(allocator, hir),
                                        },
                                        allocator,
                                    )),
                                    initializer: None,
                                    optional: false,
                                    accessibility: None,
                                    readonly: false,
                                    r#override: false,
                                }),
                            allocator,
                        ),
                        rest: None,
                    },
                    allocator,
                ),
                body: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::FunctionBody {
                        span: span(),
                        directives: oxc_allocator::Vec::new_in(allocator),
                        statements: ts.program.body,
                    },
                    allocator,
                )),
                type_parameters: if !self.signature.genargs.is_empty()
                    || !self.impl_genargs.is_empty()
                {
                    Some(oxc_allocator::Box::new_in(
                        oxc_ast::ast::TSTypeParameterDeclaration {
                            span: span(),
                            params: oxc_allocator::Vec::from_iter_in(
                                self.impl_genargs
                                    .iter()
                                    .map(|(_, lgid)| oxc_ast::ast::TSTypeParameter {
                                        span: span(),
                                        name: oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&lgid.mangled()),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        constraint: None,
                                        default: None,
                                        r#in: false,
                                        out: false,
                                        r#const: false,
                                    })
                                    .chain(self.signature.genargs.iter().map(|(_, lgid)| {
                                        oxc_ast::ast::TSTypeParameter {
                                            span: span(),
                                            name: oxc_ast::ast::BindingIdentifier {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    allocator.alloc_str(&lgid.mangled()),
                                                ),
                                                symbol_id: Cell::new(None),
                                            },
                                            constraint: None,
                                            default: None,
                                            r#in: false,
                                            out: false,
                                            r#const: false,
                                        }
                                    })),
                                allocator,
                            ),
                        },
                        allocator,
                    ))
                } else {
                    None
                },
                return_type: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeAnnotation {
                        span: span(),
                        type_annotation: self.signature.rty.kind.clone().into_oxc(allocator, hir),
                    },
                    allocator,
                )),
            },
            allocator,
        ))
    }
}

impl<'a, I: Mangled> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>, I>
    for (&'a NativeTypeAliasDef, String)
{
    fn as_oxc_global(
        &'a self,
        id: &I,
        allocator: &'a oxc_allocator::Allocator,
        _hir: &Hir,
    ) -> oxc_ast::ast::Statement<'a> {
        // TODO: そもそもnativeのターゲットがTSかチェック

        // let src = format!("type X = {};", &self.native);

        // TSをパースして取り込む
        let ts = oxc_parser::Parser::new(allocator, &self.1, oxc_span::SourceType::ts()).parse();

        let type_annotation = match &ts.program.body[0] {
            oxc_ast::ast::Statement::TSTypeAliasDeclaration(decl) => {
                decl.type_annotation.clone_in(allocator)
            }
            _ => panic!("unexpected AST"),
        };

        oxc_ast::ast::Statement::TSTypeAliasDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::TSTypeAliasDeclaration {
                span: span(),
                id: oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(allocator.alloc_str(&id.mangled())),
                    symbol_id: Cell::new(None),
                },
                type_parameters: if !self.0.genargs.is_empty() {
                    Some(oxc_allocator::Box::new_in(
                        oxc_ast::ast::TSTypeParameterDeclaration {
                            span: span(),
                            params: oxc_allocator::Vec::from_iter_in(
                                self.0
                                    .genargs
                                    .iter()
                                    .map(|ident| oxc_ast::ast::TSTypeParameter {
                                        span: span(),
                                        name: oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&ident.id),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        constraint: None,
                                        default: None,
                                        r#in: false,
                                        out: false,
                                        r#const: false,
                                    }),
                                allocator,
                            ),
                        },
                        allocator,
                    ))
                } else {
                    None
                },
                type_annotation,
                scope_id: Cell::new(None),
                declare: false,
            },
            allocator,
        ))
    }
}

impl<'a> IntoOxc<'a, oxc_allocator::Vec<'a, oxc_ast::ast::Statement<'a>>> for &'a NativeCode {
    fn into_oxc(
        self,
        allocator: &'a oxc_allocator::Allocator,
        _hir: &Hir,
    ) -> oxc_allocator::Vec<'a, oxc_ast::ast::Statement<'a>> {
        // TODO: そもそもnativeのターゲットがTSかチェック

        // TSをパースして取り込む
        let ts =
            oxc_parser::Parser::new(allocator, &self.native, oxc_span::SourceType::ts()).parse();
        // TODO: ts.errors をチェック

        ts.program.body
    }
}

impl<'a, I: Mangled> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>, I> for NovelSceneDef {
    fn as_oxc_global(
        &'a self,
        id: &I,
        allocator: &'a oxc_allocator::Allocator,
        hir: &Hir,
    ) -> oxc_ast::ast::Statement<'a> {
        let mut env = FnAstBuildEnv {
            expr_tys: &self.expr_tys,
            var_tys: &self.var_tys,
            stmts: vec![],
        };

        let fn_body = &self.body;

        for stmt in &fn_body.stmts {
            let oxc_stmt = stmt.as_oxc(&mut env, allocator, hir);
            env.stmts.push(oxc_stmt);
        }

        oxc_ast::ast::Statement::FunctionDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::Function {
                span: span(),
                id: Some(oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(allocator.alloc_str(&id.mangled())),
                    symbol_id: Cell::new(None),
                }),
                generator: false,
                r#type: oxc_ast::ast::FunctionType::FunctionDeclaration,
                r#async: false,
                pure: false,
                pife: false,
                declare: false,
                scope_id: Cell::new(None),
                this_param: None,
                params: oxc_allocator::Box::new_in(
                    oxc_ast::ast::FormalParameters {
                        span: span(),
                        kind: oxc_ast::ast::FormalParameterKind::FormalParameter,
                        items: oxc_allocator::Vec::from_iter_in(
                            self.signature
                                .args
                                .iter()
                                .map(|arg| oxc_ast::ast::FormalParameter {
                                    span: span(),
                                    decorators: oxc_allocator::Vec::new_in(allocator),
                                    pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                        oxc_allocator::Box::new_in(
                                            oxc_ast::ast::BindingIdentifier {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    allocator.alloc_str(&arg.var_id.mangled()),
                                                ),
                                                symbol_id: Cell::new(None),
                                            },
                                            allocator,
                                        ),
                                    ),
                                    type_annotation: Some(oxc_allocator::Box::new_in(
                                        oxc_ast::ast::TSTypeAnnotation {
                                            span: span(),
                                            type_annotation: arg
                                                .ty
                                                .kind
                                                .as_oxc(&mut env, allocator, hir),
                                        },
                                        allocator,
                                    )),
                                    initializer: None,
                                    optional: false,
                                    accessibility: None,
                                    readonly: false,
                                    r#override: false,
                                }),
                            allocator,
                        ),
                        rest: None,
                    },
                    allocator,
                ),
                body: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::FunctionBody {
                        span: span(),
                        directives: oxc_allocator::Vec::new_in(allocator),
                        statements: oxc_allocator::Vec::from_iter_in(env.stmts, allocator),
                    },
                    allocator,
                )),
                type_parameters: None, // scene にはジェネリック型引数列はない
                return_type: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeAnnotation {
                        span: span(),
                        type_annotation: self.signature.rty.kind.clone().into_oxc(allocator, hir),
                    },
                    allocator,
                )),
            },
            allocator,
        ))
    }
}
