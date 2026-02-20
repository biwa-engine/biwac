use std::cell::Cell;

use biwac_name_resolver::{
    BlockExpr, Callee, Expr, ExprVal, Literal, LocVarId, Primary, ResolvedIdent,
};
use biwac_parser::{BinOperator, UnOperator};
use oxc_allocator::FromIn;

use crate::arch::typescript::{AsOxc, Mangled, span};

impl Mangled for ResolvedIdent {
    fn mangled(&self) -> String {
        match self {
            Self::Abs(absid) => absid.mangled(),
            Self::Var(var_id) => var_id.mangled(),
        }
    }
}

impl Mangled for LocVarId {
    fn mangled(&self) -> String {
        format!("__lv{}", self.value())
    }
}

impl<'a> AsOxc<'a, oxc_span::Ident<'a>> for Callee {
    fn as_oxc(
        &'a self,
        _env: &mut super::FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
    ) -> oxc_span::Ident<'a> {
        match self {
            Self::Var(_) => todo!(),
            Self::Abs(absid) => oxc_span::Ident::new_const(allocator.alloc_str(&absid.mangled())),
        }
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::Expression<'a>> for BlockExpr {
    fn as_oxc(
        &'a self,
        env: &mut super::FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
    ) -> oxc_ast::ast::Expression<'a> {
        let oxc_stmts = self
            .stmts
            .iter()
            .map(|stmt| stmt.as_oxc(env, allocator))
            .collect::<Vec<_>>();

        env.stmts.extend(oxc_stmts);

        self.expr.as_oxc(env, allocator)
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::UnaryOperator> for UnOperator {
    fn as_oxc(
        &'a self,
        _env: &mut super::FnAstBuildEnv<'a>,
        _allocator: &'a oxc_allocator::Allocator,
    ) -> oxc_ast::ast::UnaryOperator {
        match self {
            Self::Neg => oxc_ast::ast::UnaryOperator::UnaryNegation,
        }
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::BinaryOperator> for BinOperator {
    fn as_oxc(
        &'a self,
        _env: &mut super::FnAstBuildEnv<'a>,
        _allocator: &'a oxc_allocator::Allocator,
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
        env: &mut super::FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
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
                            allocator,
                        ))
                    }
                    Literal::Bool(b) => {
                        oxc_ast::ast::Expression::BooleanLiteral(oxc_allocator::Box::new_in(
                            oxc_ast::ast::BooleanLiteral {
                                span: span(),
                                value: b.val,
                            },
                            allocator,
                        ))
                    }
                    Literal::String(s) => {
                        oxc_ast::ast::Expression::StringLiteral(oxc_allocator::Box::new_in(
                            oxc_ast::ast::StringLiteral {
                                span: span(),
                                value: oxc_ast::ast::Atom::from_in(&s.val, allocator),
                                raw: None,
                                lone_surrogates: false,
                            },
                            allocator,
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
                                                                            allocator
                                                                                .alloc(&ident.id),
                                                                        ),
                                                                },
                                                                allocator,
                                                            ),
                                                        ),
                                                    value: expr.as_oxc(env, allocator),
                                                    method: false,
                                                    shorthand: false,
                                                    computed: false,
                                                },
                                                allocator,
                                            ),
                                        )
                                    }),
                                    allocator,
                                ),
                            },
                            allocator,
                        ))
                    }
                },
                Primary::Variable(v) => {
                    oxc_ast::ast::Expression::Identifier(oxc_allocator::Box::new_in(
                        oxc_ast::ast::IdentifierReference {
                            span: span(),
                            name: oxc_span::Ident::new_const(allocator.alloc_str(&v.id.mangled())),
                            reference_id: Cell::new(None),
                        },
                        allocator,
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
                                            allocator.alloc_str(&c.callee.as_oxc(env, allocator)),
                                        ),
                                        reference_id: Cell::new(None),
                                    },
                                    allocator,
                                ),
                            ),
                            type_arguments: None,
                            arguments: oxc_allocator::Vec::from_iter_in(
                                c.args.iter().map(|a| {
                                    oxc_ast::ast::Argument::from(a.as_oxc(env, allocator))
                                }),
                                allocator,
                            ),
                            optional: false,
                            pure: false,
                        },
                        allocator,
                    ))
                }
                Primary::IfExpr(if_expr) => {
                    if if_expr.then.stmts.is_empty() && if_expr.els.stmts.is_empty() {
                        // then と else のブロック式が文を全く含まない場合、
                        // TS三項演算子 ? : を使えば良い
                        oxc_ast::ast::Expression::ConditionalExpression(oxc_allocator::Box::new_in(
                            oxc_ast::ast::ConditionalExpression {
                                span: span(),
                                test: if_expr.cond.as_oxc(env, allocator),
                                consequent: if_expr.then.as_oxc(env, allocator),
                                alternate: if_expr.els.as_oxc(env, allocator),
                            },
                            allocator,
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
                Primary::MemberAccess(_) => todo!(),
                Primary::Block(block) => block.as_oxc(env, allocator),
                Primary::MethodCall(_) => todo!(),
            },
            ExprVal::Unary(u) => {
                oxc_ast::ast::Expression::UnaryExpression(oxc_allocator::Box::new_in(
                    oxc_ast::ast::UnaryExpression {
                        span: span(),
                        operator: u.op.as_oxc(env, allocator),
                        argument: u.right.as_oxc(env, allocator),
                    },
                    allocator,
                ))
            }
            ExprVal::Binary(b) => {
                oxc_ast::ast::Expression::BinaryExpression(oxc_allocator::Box::new_in(
                    oxc_ast::ast::BinaryExpression {
                        span: span(),
                        operator: b.op.as_oxc(env, allocator),
                        left: b.left.as_oxc(env, allocator),
                        right: b.right.as_oxc(env, allocator),
                    },
                    allocator,
                ))
            }
        }
    }
}
