use std::cell::Cell;

use biwac_hir::{
    BlockExpr, Callee, Expr, ExprVal, Hir, ImplValId, Literal, LocVarId, Primary, TyId, TyKind,
    VarIdKind,
};
use biwac_parser::{BinOperator, UnOperator};
use oxc_allocator::FromIn;

use crate::arch::typescript::{AsOxc, Mangled, span};

impl Mangled for VarIdKind {
    fn mangled(&self) -> String {
        match self {
            Self::Global(vid) => vid.mangled(),
            Self::Local(var_id) => var_id.mangled(),
        }
    }
}

impl Mangled for LocVarId {
    fn mangled(&self) -> String {
        format!("__lv{}", self.value())
    }
}

impl Mangled for (&TyId, &str, &ImplValId) {
    fn mangled(&self) -> String {
        let mut result = String::from("_ZN");

        for q in self.0.quals() {
            result.push_str(&format!("{}{}", q.len(), q));
        }

        result.push_str(&format!("{}{}", self.0.id().len(), self.0.id()));

        result.push_str(&format!("{}{}", self.1.len(), self.1));

        result.push_str(&format!(
            "{}G{}",
            self.2.value().to_string().len() + 1,
            self.2.value()
        ));

        result.push('E');

        result
    }
}

impl Mangled for (&TyKind, &str, &ImplValId) {
    fn mangled(&self) -> String {
        match self.0 {
            TyKind::Infer(_) => panic!("compiler bug: failed to infer type of expression"),
            TyKind::Void => panic!("compiler bug: Void cannot be implemented method"),
            TyKind::Fn(_) => panic!("compiler bug: function cannot be implemented method"),
            TyKind::Gen(_) => panic!(""),    // ローカルに出現し得ない
            TyKind::LocGen(_) => panic!(""), // ローカルなジェネリック型のメソッドの有効性は判断できないため、呼ばれることはない
            TyKind::Int | TyKind::Float | TyKind::Bool => (self.0, self.1).mangled(),
            TyKind::Defined(defined_ty) => (&defined_ty.tid, self.1, self.2).mangled(),
        }
    }
}

impl<'a> AsOxc<'a, oxc_span::Ident<'a>> for Callee {
    fn as_oxc(
        &'a self,
        _env: &mut super::FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
        _hir: &Hir,
    ) -> oxc_span::Ident<'a> {
        match self {
            Self::Var(_) => todo!(),
            Self::Fn(vid) => oxc_span::Ident::new_const(allocator.alloc_str(&vid.mangled())),
            Self::Assoc(assoc_callee) => oxc_span::Ident::new_const(
                allocator.alloc_str(
                    &(
                        &assoc_callee.ty.kind,
                        assoc_callee.assoc.as_str(),
                        &assoc_callee.impl_vid,
                    )
                        .mangled(),
                ),
            ),
        }
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::Expression<'a>> for BlockExpr {
    fn as_oxc(
        &'a self,
        env: &mut super::FnAstBuildEnv<'a>,
        allocator: &'a oxc_allocator::Allocator,
        hir: &Hir,
    ) -> oxc_ast::ast::Expression<'a> {
        let oxc_stmts = self
            .stmts
            .iter()
            .map(|stmt| stmt.as_oxc(env, allocator, hir))
            .collect::<Vec<_>>();

        env.stmts.extend(oxc_stmts);

        self.expr.as_oxc(env, allocator, hir)
    }
}

impl<'a> AsOxc<'a, oxc_ast::ast::UnaryOperator> for UnOperator {
    fn as_oxc(
        &'a self,
        _env: &mut super::FnAstBuildEnv<'a>,
        _allocator: &'a oxc_allocator::Allocator,
        _hir: &Hir,
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
        _hir: &Hir,
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
        hir: &Hir,
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
                                                    value: expr.as_oxc(env, allocator, hir),
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
                                            allocator
                                                .alloc_str(&c.callee.as_oxc(env, allocator, hir)),
                                        ),
                                        reference_id: Cell::new(None),
                                    },
                                    allocator,
                                ),
                            ),
                            type_arguments: None,
                            arguments: oxc_allocator::Vec::from_iter_in(
                                c.args.iter().map(|a| {
                                    oxc_ast::ast::Argument::from(a.as_oxc(env, allocator, hir))
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
                                test: if_expr.cond.as_oxc(env, allocator, hir),
                                consequent: if_expr.then.as_oxc(env, allocator, hir),
                                alternate: if_expr.els.as_oxc(env, allocator, hir),
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
                Primary::MemberAccess(m) => {
                    oxc_ast::ast::Expression::StaticMemberExpression(oxc_allocator::Box::new_in(
                        oxc_ast::ast::StaticMemberExpression {
                            span: span(),
                            object: m.left.as_oxc(env, allocator, hir),
                            property: oxc_ast::ast::IdentifierName {
                                span: span(),
                                name: oxc_span::Ident::new_const(allocator.alloc(&m.member.id)),
                            },
                            optional: false,
                        },
                        allocator,
                    ))
                }
                Primary::Block(block) => block.as_oxc(env, allocator, hir),
                Primary::MethodCall(m) => {
                    let self_ty = env.expr_tys.get(&m.left.id).unwrap();

                    let callee_mangled_name = match &self_ty.kind {
                        TyKind::Defined(defined_ty) => {
                            let impl_valid = hir
                                .get_impl_value_id_of_type(&self_ty.kind, &m.method.id)
                                .unwrap()
                                .unwrap();

                            (&defined_ty.tid, m.method.id.as_str(), &impl_valid).mangled()
                        }
                        TyKind::Infer(_) => {
                            panic!("compiler bug: failed to infer type of expression")
                        }
                        TyKind::Void => panic!("compiler bug: Void cannot be implemented method"),
                        TyKind::Fn(_) => {
                            panic!("compiler bug: function cannot be implemented method")
                        }
                        TyKind::Gen(_) => panic!(""), // ローカルに出現し得ない
                        TyKind::LocGen(_) => panic!(""), // ローカルなジェネリック型のメソッドの有効性は判断できないため、呼ばれることはない
                        TyKind::Int | TyKind::Float | TyKind::Bool => {
                            (&self_ty.kind, m.method.id.as_str()).mangled()
                        }
                    };

                    // selfは第一引数として与える
                    let mut args = vec![oxc_ast::ast::Argument::from(
                        m.left.as_oxc(env, allocator, hir),
                    )];
                    args.extend(
                        m.args
                            .iter()
                            .map(|a| oxc_ast::ast::Argument::from(a.as_oxc(env, allocator, hir))),
                    );

                    oxc_ast::ast::Expression::CallExpression(oxc_allocator::Box::new_in(
                        oxc_ast::ast::CallExpression {
                            span: span(),
                            callee: oxc_ast::ast::Expression::Identifier(
                                oxc_allocator::Box::new_in(
                                    oxc_ast::ast::IdentifierReference {
                                        span: span(),
                                        name: oxc_span::Ident::new_const(
                                            allocator.alloc_str(&callee_mangled_name),
                                        ),
                                        reference_id: Cell::new(None),
                                    },
                                    allocator,
                                ),
                            ),
                            type_arguments: None,
                            arguments: oxc_allocator::Vec::from_iter_in(args, allocator),
                            optional: false,
                            pure: false,
                        },
                        allocator,
                    ))
                }
            },
            ExprVal::Unary(u) => {
                oxc_ast::ast::Expression::UnaryExpression(oxc_allocator::Box::new_in(
                    oxc_ast::ast::UnaryExpression {
                        span: span(),
                        operator: u.op.as_oxc(env, allocator, hir),
                        argument: u.right.as_oxc(env, allocator, hir),
                    },
                    allocator,
                ))
            }
            ExprVal::Binary(b) => {
                oxc_ast::ast::Expression::BinaryExpression(oxc_allocator::Box::new_in(
                    oxc_ast::ast::BinaryExpression {
                        span: span(),
                        operator: b.op.as_oxc(env, allocator, hir),
                        left: b.left.as_oxc(env, allocator, hir),
                        right: b.right.as_oxc(env, allocator, hir),
                    },
                    allocator,
                ))
            }
        }
    }
}
