use std::cell::Cell;

use biwac_hir::{Hir, Stmt};

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
            Stmt::Expr(_) => todo!(),
            Stmt::If(_) => todo!(),
            Stmt::Block(_) => todo!(),
            Stmt::While(_) => todo!(),
            Stmt::Assign(_) => todo!(),
        }
    }
}
