use std::cell::Cell;

use biwac_ast::{BinOperator, UnOperator};
use biwac_hir::{BlockExpr, Callee, Expr, ExprVal, Literal, Primary, VarIdKind};
use biwac_span::VarId;
use oxc_allocator::FromIn;

use crate::arch::typescript::{AsOxc, Mangled, span};

impl Mangled for VarIdKind {
    fn mangled(&self, ctx: &super::AstBuildCtx) -> String {
        match self {
            VarIdKind::Global(def_id) => ctx.get_value_mangled(def_id),
            VarIdKind::Local(var_id) => var_id.mangled(ctx),
        }
    }
}

impl Mangled for VarId {
    fn mangled(&self, _ctx: &super::AstBuildCtx) -> String {
        format!("__lv{}", self.value())
    }
}

impl<'a> AsOxc<'a, oxc_span::Ident<'a>> for Callee {
    fn as_oxc(
        &'a self,
        ctx: &'a super::AstBuildCtx<'a>,
        _fctx: &mut super::FnAstBuildCtx<'a>,
    ) -> oxc_span::Ident<'a> {
        match self {
            Self::Var(var_id) => {
                oxc_span::Ident::new_const(ctx.allocator.alloc_str(&var_id.mangled(ctx)))
            }
            Self::Fn(def_id) => {
                oxc_span::Ident::new_const(ctx.allocator.alloc_str(&def_id.mangled(ctx)))
            }
        }
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::Expression<'a>> for BlockExpr {
    fn as_oxc(
        &'a self,
        ctx: &'a super::AstBuildCtx<'a>,
        fctx: &mut super::FnAstBuildCtx<'a>,
    ) -> oxc_ast::ast::Expression<'a> {
        let oxc_stmts = self
            .stmts
            .iter()
            .map(|stmt| stmt.as_oxc(ctx, fctx))
            .collect::<Vec<_>>();

        fctx.stmts.extend(oxc_stmts);

        self.expr.as_oxc(ctx, fctx)
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::UnaryOperator> for UnOperator {
    fn as_oxc(
        &'a self,
        _ctx: &'a super::AstBuildCtx<'a>,
        _fctx: &mut super::FnAstBuildCtx<'a>,
    ) -> oxc_ast::ast::UnaryOperator {
        match self {
            Self::Neg => oxc_ast::ast::UnaryOperator::UnaryNegation,
        }
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::BinaryOperator> for BinOperator {
    fn as_oxc(
        &'a self,
        _ctx: &'a super::AstBuildCtx<'a>,
        _fctx: &mut super::FnAstBuildCtx<'a>,
    ) -> oxc_ast::ast::BinaryOperator {
        match self {
            Self::Add => oxc_ast::ast::BinaryOperator::Addition, // + TS: "+"
            Self::Sub => oxc_ast::ast::BinaryOperator::Subtraction, // - TS: "-"
            Self::Mul => oxc_ast::ast::BinaryOperator::Multiplication, // * TS: "*"
            Self::Div => oxc_ast::ast::BinaryOperator::Division, // / TS: "/"
            Self::Mod => oxc_ast::ast::BinaryOperator::Remainder, // % TS: "%"
            Self::Gt => oxc_ast::ast::BinaryOperator::GreaterThan, // > TS: ">"
            Self::Lt => oxc_ast::ast::BinaryOperator::LessThan,  // < TS: "<"
            Self::Ge => oxc_ast::ast::BinaryOperator::GreaterEqualThan, // >= TS: ">="
            Self::Le => oxc_ast::ast::BinaryOperator::LessEqualThan, // <= TS: "<="
            Self::Eq => oxc_ast::ast::BinaryOperator::StrictEquality, // == TS: "==="
            Self::Ne => oxc_ast::ast::BinaryOperator::StrictInequality, // != TS: "!=="
        }
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::Expression<'a>> for Expr {
    fn as_oxc(
        &'a self,
        ctx: &'a super::AstBuildCtx<'a>,
        fctx: &mut super::FnAstBuildCtx<'a>,
    ) -> oxc_ast::ast::Expression<'a> {
        match &self.expr {
            ExprVal::Primary(p) => match p {
                Primary::Literal(l) => match l {
                    Literal::Integer(i) => {
                        oxc_ast::ast::Expression::NumericLiteral(oxc_allocator::Box::new_in(
                            oxc_ast::ast::NumericLiteral {
                                span: span(),
                                value: i.val as f64,
                                raw: None,
                                base: oxc_ast::ast::NumberBase::Decimal,
                            },
                            &ctx.allocator,
                        ))
                    }
                    Literal::Bool(b) => {
                        oxc_ast::ast::Expression::BooleanLiteral(oxc_allocator::Box::new_in(
                            oxc_ast::ast::BooleanLiteral {
                                span: span(),
                                value: b.val,
                            },
                            &ctx.allocator,
                        ))
                    }
                    Literal::String(s) => {
                        oxc_ast::ast::Expression::StringLiteral(oxc_allocator::Box::new_in(
                            oxc_ast::ast::StringLiteral {
                                span: span(),
                                value: oxc_ast::ast::Atom::from_in(&s.val, &ctx.allocator),
                                raw: None,
                                lone_surrogates: false,
                            },
                            &ctx.allocator,
                        ))
                    }
                    Literal::Struct(s) => {
                        oxc_ast::ast::Expression::ObjectExpression(oxc_allocator::Box::new_in(
                            oxc_ast::ast::ObjectExpression {
                                span: span(),
                                properties: oxc_allocator::Vec::from_iter_in(
                                    s.members.iter().map(|(ident, expr)| {
                                        oxc_ast::ast::ObjectPropertyKind::ObjectProperty(
                                            oxc_allocator::Box::new_in(
                                                oxc_ast::ast::ObjectProperty {
                                                    span: span(),
                                                    kind: oxc_ast::ast::PropertyKind::Init,
                                                    key:
                                                        oxc_ast::ast::PropertyKey::StaticIdentifier(
                                                            oxc_allocator::Box::new_in(
                                                                oxc_ast::ast::IdentifierName {
                                                                    span: span(),
                                                                    name:
                                                                        oxc_span::Ident::new_const(
                                                                            &ctx.allocator.alloc(
                                                                                ctx.str_of(
                                                                                    &ident.id,
                                                                                ),
                                                                            ),
                                                                        ),
                                                                },
                                                                &ctx.allocator,
                                                            ),
                                                        ),
                                                    value: expr.as_oxc(ctx, fctx),
                                                    method: false,
                                                    shorthand: false,
                                                    computed: false,
                                                },
                                                &ctx.allocator,
                                            ),
                                        )
                                    }),
                                    &ctx.allocator,
                                ),
                            },
                            &ctx.allocator,
                        ))
                    }
                },
                Primary::Variable(v) => {
                    oxc_ast::ast::Expression::Identifier(oxc_allocator::Box::new_in(
                        oxc_ast::ast::IdentifierReference {
                            span: span(),
                            name: oxc_span::Ident::new_const(
                                &ctx.allocator.alloc_str(&v.id.mangled(ctx)),
                            ),
                            reference_id: Cell::new(None),
                        },
                        &ctx.allocator,
                    ))
                }
                Primary::FnCall(c) => {
                    oxc_ast::ast::Expression::CallExpression(oxc_allocator::Box::new_in(
                        oxc_ast::ast::CallExpression {
                            span: span(),
                            callee: oxc_ast::ast::Expression::Identifier(
                                oxc_allocator::Box::new_in(
                                    oxc_ast::ast::IdentifierReference {
                                        span: span(),
                                        name: oxc_span::Ident::new_const(
                                            &ctx.allocator.alloc_str(&c.callee.as_oxc(ctx, fctx)),
                                        ),
                                        reference_id: Cell::new(None),
                                    },
                                    &ctx.allocator,
                                ),
                            ),
                            type_arguments: None,
                            arguments: oxc_allocator::Vec::from_iter_in(
                                c.args
                                    .iter()
                                    .map(|a| oxc_ast::ast::Argument::from(a.as_oxc(ctx, fctx))),
                                &ctx.allocator,
                            ),
                            optional: false,
                            pure: false,
                        },
                        &ctx.allocator,
                    ))
                }
                Primary::IfExpr(if_expr) => {
                    if if_expr.then.stmts.is_empty() && if_expr.els.stmts.is_empty() {
                        // then と else のブロック式が文を全く含まない場合、
                        // TS三項演算子 ? : を使えば良い
                        oxc_ast::ast::Expression::ConditionalExpression(oxc_allocator::Box::new_in(
                            oxc_ast::ast::ConditionalExpression {
                                span: span(),
                                test: if_expr.cond.as_oxc(ctx, fctx),
                                consequent: if_expr.then.as_oxc(ctx, fctx),
                                alternate: if_expr.els.as_oxc(ctx, fctx),
                            },
                            &ctx.allocator,
                        ))
                    } else {
                        // そうでない場合、
                        // let __tmpx;
                        // if (cond) {
                        //   S
                        //   S
                        //   __tmpx = E;
                        // } else {
                        //   S
                        //   __tmpx = E;
                        // }
                        // をenv.stmtsに追加し、
                        // 式の中では
                        // __tmpx
                        // を参照する
                        todo!()
                    }
                }
                Primary::MemberAccess(m) => {
                    oxc_ast::ast::Expression::StaticMemberExpression(oxc_allocator::Box::new_in(
                        oxc_ast::ast::StaticMemberExpression {
                            span: span(),
                            object: m.left.as_oxc(ctx, fctx),
                            property: oxc_ast::ast::IdentifierName {
                                span: span(),
                                name: oxc_span::Ident::new_const(
                                    &ctx.allocator.alloc(ctx.str_of(&m.member.id)),
                                ),
                            },
                            optional: false,
                        },
                        &ctx.allocator,
                    ))
                }
                Primary::Block(block) => block.as_oxc(ctx, fctx),
                Primary::MethodCall(m) => {
                    let callee_mangled_name = ctx.get_value_mangled(m.def_id.get().unwrap());

                    // selfは第一引数として与える
                    let mut args = vec![oxc_ast::ast::Argument::from(m.left.as_oxc(ctx, fctx))];
                    args.extend(
                        m.args
                            .iter()
                            .map(|a| oxc_ast::ast::Argument::from(a.as_oxc(ctx, fctx))),
                    );

                    oxc_ast::ast::Expression::CallExpression(oxc_allocator::Box::new_in(
                        oxc_ast::ast::CallExpression {
                            span: span(),
                            callee: oxc_ast::ast::Expression::Identifier(
                                oxc_allocator::Box::new_in(
                                    oxc_ast::ast::IdentifierReference {
                                        span: span(),
                                        name: oxc_span::Ident::new_const(
                                            &ctx.allocator.alloc_str(&callee_mangled_name),
                                        ),
                                        reference_id: Cell::new(None),
                                    },
                                    &ctx.allocator,
                                ),
                            ),
                            type_arguments: None,
                            arguments: oxc_allocator::Vec::from_iter_in(args, &ctx.allocator),
                            optional: false,
                            pure: false,
                        },
                        &ctx.allocator,
                    ))
                }
            },
            ExprVal::Unary(u) => {
                oxc_ast::ast::Expression::UnaryExpression(oxc_allocator::Box::new_in(
                    oxc_ast::ast::UnaryExpression {
                        span: span(),
                        operator: u.op.as_oxc(ctx, fctx),
                        argument: u.right.as_oxc(ctx, fctx),
                    },
                    &ctx.allocator,
                ))
            }
            ExprVal::Binary(b) => {
                oxc_ast::ast::Expression::BinaryExpression(oxc_allocator::Box::new_in(
                    oxc_ast::ast::BinaryExpression {
                        span: span(),
                        operator: b.op.as_oxc(ctx, fctx),
                        left: b.left.as_oxc(ctx, fctx),
                        right: b.right.as_oxc(ctx, fctx),
                    },
                    &ctx.allocator,
                ))
            }
        }
    }
}
