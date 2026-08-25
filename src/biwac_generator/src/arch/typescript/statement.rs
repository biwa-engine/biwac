use std::cell::Cell;

use biwac_hir::{Primary, Stmt};
use biwac_lang_item::LangItem;
use oxc_allocator::FromIn;

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
            // scene 内の novel statement は lang item の関数呼び出しに展開する。
            //
            // どの関数に落とすかはコンパイラだけが知っており、
            // ユーザは名前でこれらを呼ぶことを想定されていない。
            // 呼び出し先の import は型推論時に deps_recorder へ記録済み。
            Stmt::NovelWrite(write) => novel_call(
                ctx,
                LangItem::Write,
                [oxc_ast::ast::Argument::StringLiteral(
                    oxc_allocator::Box::new_in(
                        oxc_ast::ast::StringLiteral {
                            span: span(),
                            value: oxc_ast::ast::Atom::from_in(write.msg.as_str(), ctx.allocator),
                            raw: None,
                            lone_surrogates: false,
                        },
                        ctx.allocator,
                    ),
                )],
            ),
            Stmt::NovelWait(_) => novel_call(ctx, LangItem::Wait, []),
        }
    }
}

/// novel statement を syscall の発行に展開する。
///
/// lang item の関数 (std) が syscall の記述子を組み立て、
/// それを `yield` することでエンジン (kernel) に制御が渡る。
/// レジスタに引数を積むのが std、syscall 命令が `yield` にあたる。
/// 中断できるのは generator である scene の中だけなので、
/// この展開が現れるのも scene の中だけである。
fn novel_call<'a, const N: usize>(
    ctx: &'a super::AstBuildCtx<'a>,
    item: LangItem,
    args: [oxc_ast::ast::Argument<'a>; N],
) -> oxc_ast::ast::Statement<'a> {
    oxc_ast::ast::Statement::ExpressionStatement(oxc_allocator::Box::new_in(
        oxc_ast::ast::ExpressionStatement {
            span: span(),
            expression: yield_expr(
                oxc_ast::ast::Expression::CallExpression(oxc_allocator::Box::new_in(
                    oxc_ast::ast::CallExpression {
                        span: span(),
                        callee: oxc_ast::ast::Expression::Identifier(oxc_allocator::Box::new_in(
                            oxc_ast::ast::IdentifierReference {
                                span: span(),
                                name: oxc_span::Ident::new_const(
                                    ctx.allocator.alloc_str(&ctx.lang_item_fn_mangled(item)),
                                ),
                                reference_id: Cell::new(None),
                            },
                            ctx.allocator,
                        )),
                        type_arguments: None,
                        arguments: oxc_allocator::Vec::from_iter_in(args, ctx.allocator),
                        optional: false,
                        pure: false,
                    },
                    ctx.allocator,
                )),
                false,
                ctx.allocator,
            ),
        },
        ctx.allocator,
    ))
}
