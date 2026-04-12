use biwac_hir::{
    BinaryExpr, BlockExpr, Callee, Expr, ExprVal, FnCall, IfExpr, Literal, MemberAccess,
    MethodCall, Primary, Stmt, StructLiteral, TyKind, UnaryExpr, Variable,
};

use crate::{RsvResult, TryResolve, context::val_phase::ResolvedValue};

impl TryResolve<&biwac_ast::Primary> for Primary {
    fn try_resolve<'mctx>(
        value: &biwac_ast::Primary,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match &value {
            biwac_ast::Primary::Literal(l) => {
                Ok(Self::Literal(Literal::try_resolve(l, fctx, hir)?))
            }
            biwac_ast::Primary::Variable(v) => Ok(Self::Variable(Variable {
                id: fctx.try_resolve_variable(v, hir)?,
                span: v.span.clone(),
            })),
            biwac_ast::Primary::FnCall(f) => Ok(Self::FnCall(FnCall::try_resolve(f, fctx, hir)?)),
            biwac_ast::Primary::MemberAccess(m) => {
                Ok(Self::MemberAccess(MemberAccess::try_resolve(m, fctx, hir)?))
            }
            biwac_ast::Primary::MethodCall(m) => {
                Ok(Self::MethodCall(MethodCall::try_resolve(m, fctx, hir)?))
            }
            biwac_ast::Primary::IfExpr(i) => Ok(Self::IfExpr(IfExpr::try_resolve(i, fctx, hir)?)),
            biwac_ast::Primary::Block(b) => Ok(Self::Block(BlockExpr::try_resolve(b, fctx, hir)?)),
        }
    }
}

impl TryResolve<&biwac_ast::IfExpr> for IfExpr {
    fn try_resolve<'mctx>(
        value: &biwac_ast::IfExpr,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            cond: Box::new(Expr::try_resolve(&value.cond, fctx, hir)?),
            then: BlockExpr::try_resolve(&value.then, fctx, hir)?,
            els: BlockExpr::try_resolve(&value.els, fctx, hir)?,
            span: value.span.clone(),
        })
    }
}

impl TryResolve<&biwac_ast::BlockExpr> for BlockExpr {
    fn try_resolve<'mctx>(
        value: &biwac_ast::BlockExpr,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            stmts: value
                .stmts
                .iter()
                .map(|stmt| Stmt::try_resolve(stmt, fctx, hir))
                .collect::<RsvResult<_>>()?,
            expr: Box::new(Expr::try_resolve(&value.expr, fctx, hir)?),
            span: value.span.clone(),
        })
    }
}

impl TryResolve<&biwac_ast::FnCall> for FnCall {
    fn try_resolve<'mctx>(
        value: &biwac_ast::FnCall,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            // TODO: 関数呼び出し側にもジェネリック型注釈を導入
            callee: match fctx.try_resolve_value(&value.qualed_id, hir)? {
                ResolvedValue::Local(var_id) => Callee::Var(var_id),
                ResolvedValue::Global(vid) => Callee::Fn(vid),
                ResolvedValue::Assoc(assoc_callee) => Callee::Assoc(assoc_callee),
            },
            args: value
                .args
                .iter()
                .map(|expr| Expr::try_resolve(expr, fctx, hir))
                .collect::<RsvResult<Vec<Expr>>>()?,
            span: value.span.clone(),
        })
    }
}

impl TryResolve<&biwac_ast::MemberAccess> for MemberAccess {
    fn try_resolve<'mctx>(
        value: &biwac_ast::MemberAccess,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            span: value.span(),
            left: Box::new(Expr::try_resolve(&value.left, fctx, hir)?),
            member: value.member.clone().into(),
        })
    }
}

impl TryResolve<&biwac_ast::MethodCall> for MethodCall {
    fn try_resolve<'mctx>(
        value: &biwac_ast::MethodCall,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            span: value.span.clone(),
            left: Box::new(Expr::try_resolve(&value.left, fctx, hir)?),
            method: value.method.clone().into(),
            args: value
                .args
                .iter()
                .map(|a| Expr::try_resolve(a, fctx, hir))
                .collect::<RsvResult<_>>()?,
        })
    }
}

impl TryResolve<&biwac_ast::Literal> for Literal {
    fn try_resolve<'mctx>(
        value: &biwac_ast::Literal,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match value {
            biwac_ast::Literal::Integer(u) => Ok(Self::Integer(u.clone())),
            biwac_ast::Literal::String(s) => Ok(Self::String(s.clone())),
            biwac_ast::Literal::Bool(b) => Ok(Self::Bool(b.clone())),
            biwac_ast::Literal::Struct(s) => Ok(Self::Struct(StructLiteral {
                // NOTE: struct リテラルにはジェネリック型注釈が必要か否か
                tid: match fctx
                    .try_resolve_defined_ty(
                        &biwac_ast::DefTyp {
                            qualid: s.qualid.clone(),
                            genargs: None, // ジェネリック引数列が明示されていない
                        },
                        hir,
                    )?
                    .kind
                {
                    TyKind::Defined(defined_ty) => defined_ty.tid,
                    _ => panic!("compiler bug: not a user-defined type"),
                },
                members: s
                    .members
                    .iter()
                    .map(|(ident, expr)| match Expr::try_resolve(expr, fctx, hir) {
                        Ok(expr) => Ok((ident.clone().into(), expr)),
                        Err(e) => Err(e),
                    })
                    .collect::<RsvResult<Vec<(biwac_hir::Ident, Expr)>>>()?,
                span: s.span.clone(),
            })),
        }
    }
}

impl TryResolve<&biwac_ast::Exprs> for Expr {
    fn try_resolve<'mctx>(
        value: &biwac_ast::Exprs,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match value {
            biwac_ast::Exprs::Primary(prim) => Ok(Self {
                expr: ExprVal::Primary(Primary::try_resolve(prim, fctx, hir)?),
                id: fctx.new_expr_id(),
            }),
            biwac_ast::Exprs::Unary(u) => Ok(Self {
                expr: ExprVal::Unary(UnaryExpr {
                    op: u.op,
                    right: Box::new(Self::try_resolve(&u.right, fctx, hir)?),
                    span: u.span.clone(),
                }),
                id: fctx.new_expr_id(),
            }),
            biwac_ast::Exprs::Binary(b) => Ok(Self {
                expr: ExprVal::Binary(BinaryExpr {
                    op: b.op,
                    left: Box::new(Self::try_resolve(&b.left, fctx, hir)?),
                    right: Box::new(Self::try_resolve(&b.right, fctx, hir)?),
                }),
                id: fctx.new_expr_id(),
            }),
        }
    }
}
