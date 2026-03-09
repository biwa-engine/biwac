use biwac_hir::{
    BinaryExpr, BlockExpr, Callee, Expr, ExprVal, FnCall, IfExpr, Literal, MemberAccess,
    MethodCall, Primary, Stmt, StructLiteral, Ty, UnaryExpr, Variable,
};
use biwac_parser::Ident;

use crate::{RsvResult, TryResolve, context::val_phase::ResolvedValue};

impl TryResolve<&biwac_parser::Primary> for Primary {
    fn try_resolve<'mctx>(
        value: &biwac_parser::Primary,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match &value {
            biwac_parser::Primary::Literal(l) => {
                Ok(Self::Literal(Literal::try_resolve(l, fctx, hir)?))
            }
            biwac_parser::Primary::Variable(v) => Ok(Self::Variable(Variable {
                id: fctx.try_resolve_variable(v, hir)?,
                span: v.span.clone(),
            })),
            biwac_parser::Primary::FnCall(f) => {
                Ok(Self::FnCall(FnCall::try_resolve(f, fctx, hir)?))
            }
            biwac_parser::Primary::MemberAccess(m) => {
                Ok(Self::MemberAccess(MemberAccess::try_resolve(m, fctx, hir)?))
            }
            biwac_parser::Primary::MethodCall(m) => {
                Ok(Self::MethodCall(MethodCall::try_resolve(m, fctx, hir)?))
            }
            biwac_parser::Primary::IfExpr(i) => {
                Ok(Self::IfExpr(IfExpr::try_resolve(i, fctx, hir)?))
            }
            biwac_parser::Primary::Block(b) => {
                Ok(Self::Block(BlockExpr::try_resolve(b, fctx, hir)?))
            }
        }
    }
}

impl TryResolve<&biwac_parser::IfExpr> for IfExpr {
    fn try_resolve<'mctx>(
        value: &biwac_parser::IfExpr,
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

impl TryResolve<&biwac_parser::BlockExpr> for BlockExpr {
    fn try_resolve<'mctx>(
        value: &biwac_parser::BlockExpr,
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

impl TryResolve<&biwac_parser::FnCall> for FnCall {
    fn try_resolve<'mctx>(
        value: &biwac_parser::FnCall,
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

impl TryResolve<&biwac_parser::MemberAccess> for MemberAccess {
    fn try_resolve<'mctx>(
        value: &biwac_parser::MemberAccess,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            span: value.span(),
            left: Box::new(Expr::try_resolve(&value.left, fctx, hir)?),
            member: value.member.clone(),
        })
    }
}

impl TryResolve<&biwac_parser::MethodCall> for MethodCall {
    fn try_resolve<'mctx>(
        value: &biwac_parser::MethodCall,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        Ok(Self {
            span: value.span.clone(),
            left: Box::new(Expr::try_resolve(&value.left, fctx, hir)?),
            method: value.method.clone(),
            args: value
                .args
                .iter()
                .map(|a| Expr::try_resolve(a, fctx, hir))
                .collect::<RsvResult<_>>()?,
        })
    }
}

impl TryResolve<&biwac_parser::Literal> for Literal {
    fn try_resolve<'mctx>(
        value: &biwac_parser::Literal,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match value {
            biwac_parser::Literal::Integer(u) => Ok(Self::Integer(u.clone())),
            biwac_parser::Literal::String(s) => Ok(Self::String(s.clone())),
            biwac_parser::Literal::Bool(b) => Ok(Self::Bool(b.clone())),
            biwac_parser::Literal::Struct(s) => Ok(Self::Struct(StructLiteral {
                // NOTE: struct リテラルにはジェネリック型注釈が必要か否か
                tid: match fctx.try_resolve_defined_ty(
                    &biwac_parser::DefTyp {
                        qualid: s.qualid.clone(),
                        genargs: vec![], // TODO:
                    },
                    hir,
                )? {
                    Ty::Defined(defined_ty) => defined_ty.tid,
                    _ => panic!("compiler bug: not a user-defined type"),
                },
                members: s
                    .members
                    .iter()
                    .map(|(ident, expr)| match Expr::try_resolve(expr, fctx, hir) {
                        Ok(expr) => Ok((ident.clone(), expr)),
                        Err(e) => Err(e),
                    })
                    .collect::<RsvResult<Vec<(Ident, Expr)>>>()?,
                span: s.span.clone(),
            })),
        }
    }
}

impl TryResolve<&biwac_parser::Exprs> for Expr {
    fn try_resolve<'mctx>(
        value: &biwac_parser::Exprs,
        fctx: &mut crate::context::val_phase::fn_level::FnLevelResolveCtx<'mctx>,
        hir: &biwac_hir::Hir,
    ) -> RsvResult<Self> {
        match value {
            biwac_parser::Exprs::Primary(prim) => Ok(Self {
                expr: ExprVal::Primary(Primary::try_resolve(prim, fctx, hir)?),
                id: fctx.new_expr_id(),
            }),
            biwac_parser::Exprs::Unary(u) => Ok(Self {
                expr: ExprVal::Unary(UnaryExpr {
                    op: u.op,
                    right: Box::new(Self::try_resolve(&u.right, fctx, hir)?),
                    span: u.span.clone(),
                }),
                id: fctx.new_expr_id(),
            }),
            biwac_parser::Exprs::Binary(b) => Ok(Self {
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
