use std::cell::Cell;

use biwac_hir::{Primary, Stmt};

use crate::arch::typescript::{AsOxc, AsOxcLocal, Mangled, span, yield_expr};

impl<'a> AsOxcLocal<'a, oxc_ast::ast::Statement<'a>> for Stmt {
    fn as_oxc_local(
        &'a self,
        ctx: &'a super::AstBuildCtx<'a>,
        fctx: &mut super::FnAstBuildCtx<'a>,
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
                                               ctx.allocator.alloc_str(&v.id.mangled(ctx)),
                                            ),
                                            symbol_id: Cell::new(None),
                                        },
                                       ctx.allocator,
                                    ),
                                ),
                                type_annotation: Some(oxc_allocator::Box::new_in(
                                    oxc_ast::ast::TSTypeAnnotation {
                                        span: span(),
                                        type_annotation: fctx
                                            .var_tys
                                            .get(&v.id)
                                            .unwrap()
                                            .kind.as_oxc(ctx)
                                    },
                                   ctx.allocator,
                                )),
                                init: Some(v.init.as_oxc_local(ctx, fctx)),
                                definite: false,
                            }),
                           ctx.allocator,
                        ),
                        declare: false,
                    },
                   ctx.allocator,
                ))
            }
            Stmt::Return(ret) => {
                oxc_ast::ast::Statement::ReturnStatement(oxc_allocator::Box::new_in(
                    oxc_ast::ast::ReturnStatement {
                        span: span(),
                        argument: Some(ret.expr.as_oxc_local(ctx, fctx)),
                    },
                   ctx.allocator,
                ))
            }
            Stmt::Expr(expr) => {
                oxc_ast::ast::Statement::ExpressionStatement(oxc_allocator::Box::new_in(
                    oxc_ast::ast::ExpressionStatement {
                        span: span(),
                        expression: expr.expr.as_oxc_local(ctx, fctx),
                    },
                   ctx.allocator,
                ))
            }
            Stmt::If(if_stmt) => oxc_ast::ast::Statement::IfStatement(oxc_allocator::Box::new_in(
                oxc_ast::ast::IfStatement{
                    span: span(),
                    test: if_stmt.cond.as_oxc_local(ctx, fctx),
                    consequent: oxc_ast::ast::Statement::BlockStatement(oxc_allocator::Box::new_in(
                        oxc_ast::ast::BlockStatement{
                            span: span(),
                            body: oxc_allocator::Vec::from_iter_in(
                                if_stmt.then.stmts.iter().map(|stmt| stmt.as_oxc_local(ctx, fctx)),ctx.allocator),
                            scope_id: Cell::new(None),
                        },ctx.allocator)),
                    alternate: if_stmt.els.as_ref().map(|els| oxc_ast::ast::Statement::BlockStatement(oxc_allocator::Box::new_in(
                        oxc_ast::ast::BlockStatement{
                            span: span(),
                            body: oxc_allocator::Vec::from_iter_in(
                                els.stmts.iter().map(|stmt| stmt.as_oxc_local(ctx, fctx)),ctx.allocator),
                            scope_id: Cell::new(None),
                        },ctx.allocator))),
                },ctx.allocator)),
            Stmt::Match(m) => m.as_oxc_local(ctx, fctx),
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
                                                           ctx.allocator.alloc_str(&v.id.mangled(ctx))
                                                        ),
                                                        reference_id: Cell::new(None)
                                                    }, ctx.allocator)
                                            )
                                        },
                                        Primary::MemberAccess(m) => {
                                            oxc_ast::ast::AssignmentTarget::StaticMemberExpression(
                                                oxc_allocator::Box::new_in(
                                                    oxc_ast::ast::StaticMemberExpression {
                                                        span: span(),
                                                        object: m.left.as_oxc_local(ctx, fctx),
                                                        property: oxc_ast::ast::IdentifierName {
                                                            span: span(),
                                                            name: oxc_span::Ident::new_const(ctx.allocator.alloc(ctx.str_of(&m.member.id))),
                                                        },
                                                        optional: false,
                                                    },
                                                   ctx.allocator
                                                )
                                            )
                                        }
                                        _ => {
                                            panic!(
                                                "compiler bug: other than variable and member access cannot be assigned"
                                            )
                                        }
                                    },
                                    right: assign.src.as_oxc_local(ctx, fctx),
                                },
                               ctx.allocator,
                            ),
                        ),
                    },
                   ctx.allocator,
                ))
            }
            // novel statement は lowering の時点で lang item の呼び出し式になっている。
            //
            // ここでやるのは `yield` を被せることだけである。
            // 中断できるのは generator である scene の中だけで、
            // その位置を決めるのはコンパイラである。
            //
            // NOTE: この形は再検討が要る。`yield` に載るのは
            // 「syscall の記述子」であることを前提にしているが、
            // `content_push` は記述子を返さない普通の関数である。
            // TypeScript ターゲットは制限つきジェネリクスにも未対応で
            // 当面建たないので、`docs/trait.md` の第 3 段とあわせて直す。
            Stmt::NovelSyscall(syscall) => {
                let call = syscall.call.as_oxc_local(ctx, fctx);
                oxc_ast::ast::Statement::ExpressionStatement(oxc_allocator::Box::new_in(
                    oxc_ast::ast::ExpressionStatement {
                        span: span(),
                        expression: yield_expr(call, false, ctx.allocator),
                    },
                    ctx.allocator,
                ))
            }
        }
    }
}
