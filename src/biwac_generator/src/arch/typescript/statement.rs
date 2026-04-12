use std::cell::Cell;

use biwac_hir::{Hir, Primary, Stmt};

use crate::arch::typescript::{AsOxc, Mangled, span};

impl<'a> AsOxc<'a, oxc_ast::ast::Statement<'a>> for Stmt {
    fn as_oxc(
        &'a self,
        env: &mut super::FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
        hir: &Hir,
    ) -> oxc_ast::ast::Statement<'a> {
        match &self {
            Stmt::VarDecl(var) => {
                oxc_ast::ast::Statement::VariableDeclaration(oxc_allocator::Box::new_in(
                    oxc_ast::ast::VariableDeclaration {
                        span: span(),
                        kind: oxc_ast::ast::VariableDeclarationKind::Let,
                        declarations: oxc_allocator::Vec::from_iter_in(
                            [var].iter().map(|v| oxc_ast::ast::VariableDeclarator {
                                span: span(),
                                kind: oxc_ast::ast::VariableDeclarationKind::Let,
                                id: oxc_ast::ast::BindingPattern::BindingIdentifier(
                                    oxc_allocator::Box::new_in(
                                        oxc_ast::ast::BindingIdentifier {
                                            span: span(),
                                            name: oxc_span::Ident::new_const(
                                                allocator.alloc_str(&v.id.mangled()),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                        allocator,
                                    ),
                                ),
                                type_annotation: Some(oxc_allocator::Box::new_in(
                                    oxc_ast::ast::TSTypeAnnotation {
                                        span: span(),
                                        type_annotation: env
                                            .var_tys
                                            .get(&v.id)
                                            .unwrap()
                                            .kind
                                            .as_oxc(env, allocator, hir),
                                    },
                                    allocator,
                                )),
                                init: Some(v.init.as_oxc(env, allocator, hir)),
                                definite: false,
                            }),
                            allocator,
                        ),
                        declare: false,
                    },
                    allocator,
                ))
            }
            Stmt::Return(ret) => {
                oxc_ast::ast::Statement::ReturnStatement(oxc_allocator::Box::new_in(
                    oxc_ast::ast::ReturnStatement {
                        span: span(),
                        argument: Some(ret.expr.as_oxc(env, allocator, hir)),
                    },
                    allocator,
                ))
            }
            Stmt::Expr(expr) => {
                oxc_ast::ast::Statement::ExpressionStatement(oxc_allocator::Box::new_in(
                    oxc_ast::ast::ExpressionStatement {
                        span: span(),
                        expression: expr.expr.as_oxc(env, allocator, hir),
                    },
                    allocator,
                ))
            }
            Stmt::If(_) => todo!(),
            Stmt::Block(_) => todo!(),
            Stmt::While(_) => todo!(),
            Stmt::Assign(assign) => {
                oxc_ast::ast::Statement::ExpressionStatement(oxc_allocator::Box::new_in(
                    oxc_ast::ast::ExpressionStatement {
                        span: span(),
                        expression: oxc_ast::ast::Expression::AssignmentExpression(
                            oxc_allocator::Box::new_in(
                                oxc_ast::ast::AssignmentExpression {
                                    span: span(),
                                    operator: oxc_ast::ast::AssignmentOperator::Assign,
                                    left: match &assign.dst {
                                        Primary::Variable(v) => {
                                            oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(
                                                oxc_allocator::Box::new_in(
                                                    oxc_ast::ast::IdentifierReference{
                                                        span: span(),
                                                        name: oxc_span::Ident::new_const(
                                                            allocator.alloc_str(&v.id.mangled())
                                                        ),
                                                        reference_id: Cell::new(None)
                                                    }, allocator)
                                            )
                                        },
                                        Primary::MemberAccess(m) => {
                                            oxc_ast::ast::AssignmentTarget::StaticMemberExpression(
                                                oxc_allocator::Box::new_in(
                                                    oxc_ast::ast::StaticMemberExpression {
                                                        span: span(),
                                                        object: m.left.as_oxc(env, allocator, hir),
                                                        property: oxc_ast::ast::IdentifierName {
                                                            span: span(),
                                                            name: oxc_span::Ident::new_const(allocator.alloc(&m.member.id)),
                                                        },
                                                        optional: false,
                                                    },
                                                    allocator
                                                )
                                            )                        
                                        }
                                        _ => {
                                            panic!(
                                                "compiler bug: other than variable and member access cannot be assigned"
                                            )
                                        }
                                    },
                                    right: assign.src.as_oxc(env, allocator, hir),
                                },
                                allocator,
                            ),
                        ),
                    },
                    allocator,
                ))
            }
        }
    }
}
