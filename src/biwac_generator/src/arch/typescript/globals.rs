use std::cell::Cell;

use biwac_name_resolver::{AbsId, StructDefContent};
use biwac_type_inferrer::{FnDefContent, NativeFnDefContent, Ty};

use crate::arch::typescript::{AsOxc, AsOxcGlobal, FnAstBuildEnv, IntoOxc, Mangled, span};

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for StructDefContent {
    fn as_oxc_global(
        &'a self,
        id: &AbsId,
        allocator: &'a oxc_allocator::Allocator,
    ) -> oxc_ast::ast::Statement<'a> {
        oxc_ast::ast::Statement::TSTypeAliasDeclaration(oxc_allocator::Box::new_in(
            oxc_ast::ast::TSTypeAliasDeclaration {
                span: span(),
                id: oxc_ast::ast::BindingIdentifier {
                    span: span(),
                    name: oxc_span::Ident::new_const(allocator.alloc_str(&id.mangled())),
                    symbol_id: Cell::new(None),
                },
                type_parameters: None,
                type_annotation: oxc_ast::ast::TSType::TSTypeLiteral(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeLiteral {
                        span: span(),
                        members: oxc_allocator::Vec::from_iter_in(
                            self.members.iter().map(|(ident, typ)| {
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
                                                            allocator.alloc_str(&ident.id),
                                                        ),
                                                    },
                                                    allocator,
                                                ),
                                            ),
                                            type_annotation: Some(oxc_allocator::Box::new_in(
                                                oxc_ast::ast::TSTypeAnnotation {
                                                    span: span(),
                                                    type_annotation: Ty::from(typ.clone())
                                                        .into_oxc(allocator),
                                                },
                                                allocator,
                                            )),
                                        },
                                        allocator,
                                    ),
                                )
                            }),
                            allocator,
                        ),
                    },
                    allocator,
                )),
                scope_id: Cell::new(None),
                declare: false,
            },
            allocator,
        ))
    }
}

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for FnDefContent {
    fn as_oxc_global(
        &'a self,
        id: &AbsId,
        allocator: &'a oxc_allocator::Allocator,
    ) -> oxc_ast::ast::Statement<'a> {
        let mut env = FnAstBuildEnv {
            ty_info: &self.ty_info,
            stmts: vec![],
        };

        for stmt in &self.stmts {
            let oxc_stmt = stmt.as_oxc(&mut env, allocator);
            env.stmts.push(oxc_stmt);
        }

        if let Some(expr) = &self.expr {
            let oxc_return = oxc_ast::ast::Statement::ReturnStatement(oxc_allocator::Box::new_in(
                oxc_ast::ast::ReturnStatement {
                    span: span(),
                    argument: Some(expr.as_oxc(&mut env, allocator)),
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
                            self.args.iter().map(|a| oxc_ast::ast::FormalParameter {
                                span: span(),
                                decorators: oxc_allocator::Vec::new_in(allocator),
                                pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                    oxc_allocator::Box::new_in(
                                        oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&a.id.mangled()),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        allocator,
                                    ),
                                ),
                                type_annotation: Some(oxc_allocator::Box::new_in(
                                    oxc_ast::ast::TSTypeAnnotation {
                                        span: span(),
                                        type_annotation: self
                                            .ty_info
                                            .unwrap_type_of_variable(&a.id)
                                            .as_oxc(&mut env, allocator),
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
                type_parameters: None,
                return_type: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeAnnotation {
                        span: span(),
                        type_annotation: self.rty.clone().into_oxc(allocator),
                    },
                    allocator,
                )),
            },
            allocator,
        ))
    }
}

impl<'a> AsOxcGlobal<'a, oxc_ast::ast::Statement<'a>> for NativeFnDefContent {
    fn as_oxc_global(
        &'a self,
        id: &AbsId,
        allocator: &'a oxc_allocator::Allocator,
    ) -> oxc_ast::ast::Statement<'a> {
        // TODO: そもそもnativeのターゲットがTSかチェック

        // TSをパースして取り込む
        let ts =
            oxc_parser::Parser::new(allocator, &self.native, oxc_span::SourceType::ts()).parse();

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
                            self.args.iter().map(|a| oxc_ast::ast::FormalParameter {
                                span: span(),
                                decorators: oxc_allocator::Vec::new_in(allocator),
                                pattern: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                    oxc_allocator::Box::new_in(
                                        oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&a.id.id),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        allocator,
                                    ),
                                ),
                                type_annotation: Some(oxc_allocator::Box::new_in(
                                    oxc_ast::ast::TSTypeAnnotation {
                                        span: span(),
                                        type_annotation: a.ty.clone().into_oxc(allocator),
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
                type_parameters: None,
                return_type: Some(oxc_allocator::Box::new_in(
                    oxc_ast::ast::TSTypeAnnotation {
                        span: span(),
                        type_annotation: self.rty.clone().into_oxc(allocator),
                    },
                    allocator,
                )),
            },
            allocator,
        ))
    }
}
