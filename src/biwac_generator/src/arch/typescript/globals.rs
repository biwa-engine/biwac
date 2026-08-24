use std::cell::Cell;

use biwac_hir::{FnDef, NativeCode, NativeFnDef, NativeTypeAliasDef, NovelSceneDef, StructDef};

use biwac_span::VarId;
use oxc_allocator::CloneIn;

use crate::arch::typescript::{
    AsOxc, AsOxcGlobal, AsOxcLocal, AstBuildCtx, FnAstBuildCtx, Mangled, span,
};

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for StructDef {
    fn as_oxc_global(&'a self, id: String, ctx: &'a AstBuildCtx) -> oxc_ast::ast::Statement<'a> {
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
                                                ctx.allocator.alloc_str(&gid.mangled(ctx)),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        constraint: None,
                                        default: None,
                                        r#in: false,
                                        out: false,
                                        r#const: false,
                                    }),
                                ctx.allocator,
                            ),
                        },
                        ctx.allocator,
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
                                                            ctx.allocator.alloc_str(ctx.str_of(id)),
                                                        ),
                                                    },
                                                    ctx.allocator,
                                                ),
                                            ),
                                            type_annotation: Some(oxc_allocator::Box::new_in(
                                                oxc_ast::ast::TSTypeAnnotation {
                                                    span: span(),
                                                    type_annotation: ty.kind.as_oxc(ctx),
                                                },
                                                ctx.allocator,
                                            )),
                                        },
                                        ctx.allocator,
                                    ),
                                )
                            }),
                            ctx.allocator,
                        ),
                    },
                    ctx.allocator,
                )),
                scope_id: Cell::new(None),
                declare: false,
            },
            ctx.allocator,
        ))
    }
}

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for FnDef {
    fn as_oxc_global(&'a self, id: String, ctx: &'a AstBuildCtx) -> oxc_ast::ast::Statement<'a> {
        let mut fctx = FnAstBuildCtx::new(&self.expr_tys, &self.var_tys);

        let fn_body = &self.body;

        for stmt in &fn_body.stmts {
            let oxc_stmt = stmt.as_oxc_local(ctx, &mut fctx);
            fctx.stmts.push(oxc_stmt);
        }

        if let Some(expr) = &fn_body.expr {
            let oxc_return = oxc_ast::ast::Statement::ReturnStatement(oxc_allocator::Box::new_in(
                oxc_ast::ast::ReturnStatement {
                    span: span(),
                    argument: Some(expr.as_oxc_local(ctx, &mut fctx)),
                },
                ctx.allocator,
            ));
            fctx.stmts.push(oxc_return);
        }

        oxc_ast::ast::Statement::FunctionDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::Function {
                span: span(),
                id: Some(oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(&id)),
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
                                    decorators: oxc_allocator::Vec::new_in(ctx.allocator),
                                    pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                        oxc_allocator::Box::new_in(
                                            oxc_ast::ast::BindingIdentifier {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    ctx.allocator.alloc_str(
                                                        &VarId::SELF_VARIABLE.mangled(ctx),
                                                    ),
                                                ),
                                                symbol_id: Cell::new(None),
                                            },
                                            ctx.allocator,
                                        ),
                                    ),
                                    type_annotation: Some(oxc_allocator::Box::new_in(
                                        oxc_ast::ast::TSTypeAnnotation {
                                            span: span(),
                                            type_annotation: ty.kind.as_oxc(ctx),
                                        },
                                        ctx.allocator,
                                    )),
                                    initializer: None,
                                    optional: false,
                                    accessibility: None,
                                    readonly: false,
                                    r#override: false,
                                })
                                .chain(self.signature.args.iter().map(|arg| {
                                    oxc_ast::ast::FormalParameter {
                                        span: span(),
                                        decorators: oxc_allocator::Vec::new_in(ctx.allocator),
                                        pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                            oxc_allocator::Box::new_in(
                                                oxc_ast::ast::BindingIdentifier {
                                                    span: span(),
                                                    name: oxc_span::Ident::new_const(
                                                        ctx.allocator
                                                            .alloc_str(&arg.var_id.mangled(ctx)),
                                                    ),
                                                    symbol_id: Cell::new(None),
                                                },
                                                ctx.allocator,
                                            ),
                                        ),
                                        type_annotation: Some(oxc_allocator::Box::new_in(
                                            oxc_ast::ast::TSTypeAnnotation {
                                                span: span(),
                                                type_annotation: arg.ty.kind.as_oxc(ctx),
                                            },
                                            ctx.allocator,
                                        )),
                                        initializer: None,
                                        optional: false,
                                        accessibility: None,
                                        readonly: false,
                                        r#override: false,
                                    }
                                })),
                            ctx.allocator,
                        ),
                        rest: None,
                    },
                    ctx.allocator,
                ),
                body: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::FunctionBody {
                        span: span(),
                        directives: oxc_allocator::Vec::new_in(ctx.allocator),
                        statements: oxc_allocator::Vec::from_iter_in(fctx.stmts, ctx.allocator),
                    },
                    ctx.allocator,
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
                                                ctx.allocator.alloc_str(&lgid.mangled(ctx)),
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
                                                    ctx.allocator.alloc_str(&lgid.mangled(ctx)),
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
                                ctx.allocator,
                            ),
                        },
                        ctx.allocator,
                    ))
                } else {
                    None
                },
                return_type: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeAnnotation {
                        span: span(),
                        type_annotation: self.signature.rty.kind.as_oxc(ctx),
                    },
                    ctx.allocator,
                )),
            },
            ctx.allocator,
        ))
    }
}

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for NativeFnDef {
    fn as_oxc_global(&'a self, id: String, ctx: &'a AstBuildCtx) -> oxc_ast::ast::Statement<'a> {
        // TODO: そもそもnativeのターゲットがTSかチェック

        // TSをパースして取り込む
        let ts =
            oxc_parser::Parser::new(ctx.allocator, &self.native_body, oxc_span::SourceType::ts())
                .parse();

        oxc_ast::ast::Statement::FunctionDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::Function {
                span: span(),
                id: Some(oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(&id)),
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
                            // メソッドの場合、レシーバを第一引数として明示する。
                            // ネイティブ実装の本文は引数を書かれたままの名前で参照するので
                            // (他の引数も str_of で生の名前を使っている)、
                            // レシーバも `self` という名前で受ける。
                            self.signature
                                .self_ty
                                .iter()
                                .map(|ty| oxc_ast::ast::FormalParameter {
                                    span: span(),
                                    decorators: oxc_allocator::Vec::new_in(ctx.allocator),
                                    pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                        oxc_allocator::Box::new_in(
                                            oxc_ast::ast::BindingIdentifier {
                                                span: span(),
                                                name: oxc_span::Ident::new_const("self"),
                                                symbol_id: Cell::new(None),
                                            },
                                            ctx.allocator,
                                        ),
                                    ),
                                    type_annotation: Some(oxc_allocator::Box::new_in(
                                        oxc_ast::ast::TSTypeAnnotation {
                                            span: span(),
                                            type_annotation: ty.kind.as_oxc(ctx),
                                        },
                                        ctx.allocator,
                                    )),
                                    initializer: None,
                                    optional: false,
                                    accessibility: None,
                                    readonly: false,
                                    r#override: false,
                                })
                                .chain(self.signature.args.iter().map(|arg| {
                                    oxc_ast::ast::FormalParameter {
                                        span: span(),
                                        decorators: oxc_allocator::Vec::new_in(ctx.allocator),
                                        pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                            oxc_allocator::Box::new_in(
                                                oxc_ast::ast::BindingIdentifier {
                                                    span: span(),
                                                    name: oxc_span::Ident::new_const(
                                                        ctx.allocator
                                                            .alloc_str(ctx.str_of(&arg.id.id)),
                                                    ),
                                                    symbol_id: Cell::new(None),
                                                },
                                                ctx.allocator,
                                            ),
                                        ),
                                        type_annotation: Some(oxc_allocator::Box::new_in(
                                            oxc_ast::ast::TSTypeAnnotation {
                                                span: span(),
                                                type_annotation: arg.ty.kind.as_oxc(ctx),
                                            },
                                            ctx.allocator,
                                        )),
                                        initializer: None,
                                        optional: false,
                                        accessibility: None,
                                        readonly: false,
                                        r#override: false,
                                    }
                                })),
                            ctx.allocator,
                        ),
                        rest: None,
                    },
                    ctx.allocator,
                ),
                body: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::FunctionBody {
                        span: span(),
                        directives: oxc_allocator::Vec::new_in(ctx.allocator),
                        statements: ts.program.body,
                    },
                    ctx.allocator,
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
                                                ctx.allocator.alloc_str(&lgid.mangled(ctx)),
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
                                                    ctx.allocator.alloc_str(&lgid.mangled(ctx)),
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
                                ctx.allocator,
                            ),
                        },
                        ctx.allocator,
                    ))
                } else {
                    None
                },
                return_type: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeAnnotation {
                        span: span(),
                        type_annotation: self.signature.rty.kind.as_oxc(ctx),
                    },
                    ctx.allocator,
                )),
            },
            ctx.allocator,
        ))
    }
}

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for (&'a NativeTypeAliasDef, String) {
    fn as_oxc_global(&'a self, id: String, ctx: &'a AstBuildCtx) -> oxc_ast::ast::Statement<'a> {
        // TODO: そもそもnativeのターゲットがTSかチェック

        // let src = format!("type X = {};", &self.native);

        // TSをパースして取り込む
        let ts =
            oxc_parser::Parser::new(ctx.allocator, &self.1, oxc_span::SourceType::ts()).parse();

        let type_annotation = match &ts.program.body[0] {
            oxc_ast::ast::Statement::TSTypeAliasDeclaration(decl) => {
                decl.type_annotation.clone_in(ctx.allocator)
            }
            _ => panic!("unexpected AST"),
        };

        oxc_ast::ast::Statement::TSTypeAliasDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::TSTypeAliasDeclaration {
                span: span(),
                id: oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(&id)),
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
                                                ctx.allocator.alloc_str(ctx.str_of(&ident.id)),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        constraint: None,
                                        default: None,
                                        r#in: false,
                                        out: false,
                                        r#const: false,
                                    }),
                                ctx.allocator,
                            ),
                        },
                        ctx.allocator,
                    ))
                } else {
                    None
                },
                type_annotation,
                scope_id: Cell::new(None),
                declare: false,
            },
            ctx.allocator,
        ))
    }
}

pub(super) fn native_code_as_oxc<'a>(
    native_code: &'a NativeCode,
    ctx: &'a AstBuildCtx,
) -> oxc_allocator::Vec<'a, oxc_ast::ast::Statement<'a>> {
    // TODO: そもそもnativeのターゲットがTSかチェック

    // TSをパースして取り込む
    let ts = oxc_parser::Parser::new(
        ctx.allocator,
        &native_code.native,
        oxc_span::SourceType::ts(),
    )
    .parse();
    // TODO: ts.errors をチェック

    ts.program.body
}

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for NovelSceneDef {
    fn as_oxc_global(&'a self, id: String, ctx: &'a AstBuildCtx) -> oxc_ast::ast::Statement<'a> {
        let mut fctx = FnAstBuildCtx::new(&self.expr_tys, &self.var_tys);

        let fn_body = &self.body;

        for stmt in &fn_body.stmts {
            let oxc_stmt = stmt.as_oxc_local(ctx, &mut fctx);
            fctx.stmts.push(oxc_stmt);
        }

        oxc_ast::ast::Statement::FunctionDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::Function {
                span: span(),
                id: Some(oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(ctx.allocator.alloc_str(&id)),
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
                                    decorators: oxc_allocator::Vec::new_in(ctx.allocator),
                                    pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                        oxc_allocator::Box::new_in(
                                            oxc_ast::ast::BindingIdentifier {
                                                span: span(),
                                                name: oxc_span::Ident::new_const(
                                                    ctx.allocator
                                                        .alloc_str(&arg.var_id.mangled(ctx)),
                                                ),
                                                symbol_id: Cell::new(None),
                                            },
                                            ctx.allocator,
                                        ),
                                    ),
                                    type_annotation: Some(oxc_allocator::Box::new_in(
                                        oxc_ast::ast::TSTypeAnnotation {
                                            span: span(),
                                            type_annotation: arg.ty.kind.as_oxc(ctx),
                                        },
                                        ctx.allocator,
                                    )),
                                    initializer: None,
                                    optional: false,
                                    accessibility: None,
                                    readonly: false,
                                    r#override: false,
                                }),
                            ctx.allocator,
                        ),
                        rest: None,
                    },
                    ctx.allocator,
                ),
                body: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::FunctionBody {
                        span: span(),
                        directives: oxc_allocator::Vec::new_in(ctx.allocator),
                        statements: oxc_allocator::Vec::from_iter_in(fctx.stmts, ctx.allocator),
                    },
                    ctx.allocator,
                )),
                type_parameters: None, // scene にはジェネリック型引数列はない
                return_type: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeAnnotation {
                        span: span(),
                        type_annotation: self.signature.rty.kind.as_oxc(ctx),
                    },
                    ctx.allocator,
                )),
            },
            ctx.allocator,
        ))
    }
}
