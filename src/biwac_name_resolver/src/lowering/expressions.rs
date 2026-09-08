use std::{cell::OnceCell, collections::HashMap};

use biwac_ast::{PathSegmentResolution, VariantShape};
use biwac_span::{DefIdKind, VarId};

use biwac_hir::{
    BinaryExpr, BlockExpr, Callee, DecledVar, DefinedTy, Expr, ExprId, ExprVal, FnCall, Ident,
    IfExpr, Literal, MatchExpr, MatchExprArm, MemberAccess, MethodCall, Primary, Stmt,
    StructLiteral, Ty, TyKind, UnaryExpr, VarIdKind, Variable, VariantCtor, VariantCtorFields,
};

use crate::ResolveError;

use super::{
    TyDefIdKind, def_id_kind_from_path, patterns::lower_pattern, ty_def_id_kind_from_path,
};

pub(crate) struct ExprLowerCtx {
    // self_ty: Option<TyKind>,
    next_expr_id: usize,
    vars: HashMap<VarId, DecledVar>,
    self_var_id: Option<VarId>,
}

impl ExprLowerCtx {
    pub(crate) fn new() -> Self {
        Self {
            next_expr_id: 0,
            vars: HashMap::new(),
            self_var_id: None,
        }
    }

    pub(crate) fn alloc_expr_id(&mut self) -> ExprId {
        let id = ExprId::new(self.next_expr_id);
        self.next_expr_id += 1;
        id
    }

    pub(crate) fn declare_var(&mut self, id: VarId, var: DecledVar) {
        if var.id.id == biwac_base::InternedIdent::SELF {
            self.self_var_id = Some(id);
        }
        self.vars.insert(id, var);
    }

    pub(crate) fn into_vars(self) -> HashMap<VarId, DecledVar> {
        self.vars
    }

    fn self_var_id(&self) -> Option<VarId> {
        self.self_var_id
    }
}

pub(crate) fn lower_expr(
    ctx: &mut ExprLowerCtx,
    expr: &biwac_ast::Exprs,
    errors: &mut Vec<ResolveError>,
) -> Option<Expr> {
    let expr_val = match expr {
        biwac_ast::Exprs::Primary(prim) => ExprVal::Primary(lower_primary(ctx, prim, errors)?),
        biwac_ast::Exprs::Unary(u) => {
            let right = Box::new(lower_expr(ctx, &u.right, errors)?);
            ExprVal::Unary(UnaryExpr {
                op: u.op,
                right,
                span: u.span.clone(),
            })
        }
        biwac_ast::Exprs::Binary(b) => {
            let left = Box::new(lower_expr(ctx, &b.left, errors)?);
            let right = Box::new(lower_expr(ctx, &b.right, errors)?);
            ExprVal::Binary(BinaryExpr {
                op: b.op,
                left,
                right,
            })
        }
    };
    let id = ctx.alloc_expr_id();
    Some(Expr { expr: expr_val, id })
}

pub(crate) fn lower_primary(
    ctx: &mut ExprLowerCtx,
    prim: &biwac_ast::Primary,
    errors: &mut Vec<ResolveError>,
) -> Option<Primary> {
    match prim {
        // `Color::Named { name = x }` は構文の上では構造体リテラルだが、
        // パスがバリアントに解決されていれば構造体形式のバリアント構築である。
        biwac_ast::Primary::Literal(biwac_ast::Literal::Struct(s))
            if matches!(def_id_kind_from_path(&s.path), Ok(DefIdKind::Variant(_))) =>
        {
            let Ok(DefIdKind::Variant(variant)) = def_id_kind_from_path(&s.path) else {
                unreachable!()
            };

            let fields = s
                .members
                .iter()
                .filter_map(|(ident, expr)| {
                    Some((Ident::from(ident.clone()), lower_expr(ctx, expr, errors)?))
                })
                .collect();

            Some(Primary::VariantCtor(VariantCtor {
                variant,
                fields: VariantCtorFields::Named(fields),
                shape: VariantShape::Struct,
                span: s.span.clone(),
                resolved: OnceCell::new(),
            }))
        }

        biwac_ast::Primary::Literal(lit) => {
            Some(Primary::Literal(lower_literal(ctx, lit, errors)?))
        }

        biwac_ast::Primary::Variable(v) => match v {
            biwac_ast::Variable::Path(path) => {
                let span = path.span();
                match def_id_kind_from_path(path) {
                    Ok(DefIdKind::Var(vid)) => Some(Primary::Variable(Variable {
                        id: VarIdKind::Local(vid),
                        span,
                    })),
                    Ok(DefIdKind::Val(vid)) => Some(Primary::Variable(Variable {
                        id: VarIdKind::Global(vid),
                        span,
                    })),
                    // `Color::Red` や、import した `Red`。
                    // 構文の上では変数参照だが、意味は unit バリアントの構築である。
                    Ok(DefIdKind::Variant(variant)) => Some(Primary::VariantCtor(VariantCtor {
                        variant,
                        fields: VariantCtorFields::Unit,
                        shape: VariantShape::Unit,
                        span,
                        resolved: OnceCell::new(),
                    })),
                    // 値の位置にモジュールやパッケージ、ジェネリック引数が来た場合。
                    // 名前は解決できているが値ではないので、
                    // その名前の値は無い、という報告になる。
                    Ok(_) => {
                        errors.push(ResolveError::IdentNotFound {
                            ident: path.segments.last().unwrap().ident.clone(),
                        });
                        None
                    }
                    Err(e) => {
                        errors.push(e);
                        None
                    }
                }
            }
            biwac_ast::Variable::SelfVar(span) => {
                let vid = ctx
                    .self_var_id()
                    .expect("compiler bug: SelfVar reference outside method context");
                Some(Primary::Variable(Variable {
                    id: VarIdKind::Local(vid),
                    span: span.clone(),
                }))
            }
        },

        biwac_ast::Primary::FnCall(fn_call) => {
            // `Color::Rgb(1, 2, 3)` は構文の上では関数呼び出しだが、
            // パスがバリアントに解決されていればタプル形式の構築である。
            if let Ok(DefIdKind::Variant(variant)) = def_id_kind_from_path(&fn_call.path) {
                let args = fn_call
                    .args
                    .iter()
                    .filter_map(|a| lower_expr(ctx, a, errors))
                    .collect();

                return Some(Primary::VariantCtor(VariantCtor {
                    variant,
                    fields: VariantCtorFields::Positional(args),
                    shape: VariantShape::Tuple,
                    span: fn_call.span.clone(),
                    resolved: OnceCell::new(),
                }));
            }

            let callee = lower_callee(&fn_call.path, errors)?;
            let args = fn_call
                .args
                .iter()
                .filter_map(|a| lower_expr(ctx, a, errors))
                .collect();
            Some(Primary::FnCall(FnCall {
                callee,
                args,
                span: fn_call.span.clone(),
            }))
        }

        biwac_ast::Primary::MemberAccess(ma) => {
            let left = Box::new(lower_expr(ctx, &ma.left, errors)?);
            let span = ma.span();
            Some(Primary::MemberAccess(MemberAccess {
                left,
                member: Ident::from(ma.member.clone()),
                span,
            }))
        }

        biwac_ast::Primary::MethodCall(mc) => {
            let left = Box::new(lower_expr(ctx, &mc.left, errors)?);
            let args = mc
                .args
                .iter()
                .filter_map(|a| lower_expr(ctx, a, errors))
                .collect();
            Some(Primary::MethodCall(MethodCall {
                left,
                method: Ident::from(mc.method.clone()),
                args,
                span: mc.span.clone(),
                def_id: OnceCell::new(),
            }))
        }

        biwac_ast::Primary::IfExpr(if_expr) => {
            let cond = Box::new(lower_expr(ctx, &if_expr.cond, errors)?);
            let then = lower_block_expr(ctx, &if_expr.then, errors)?;
            let els = lower_block_expr(ctx, &if_expr.els, errors)?;
            Some(Primary::IfExpr(IfExpr {
                cond,
                then,
                els,
                span: if_expr.span.clone(),
            }))
        }

        biwac_ast::Primary::Match(m) => {
            let scrutinee = Box::new(lower_expr(ctx, &m.scrutinee, errors)?);
            let arms = m
                .arms
                .iter()
                .filter_map(|arm| {
                    Some(MatchExprArm {
                        pattern: lower_pattern(&arm.pattern, errors)?,
                        body: lower_block_expr(ctx, &arm.body, errors)?,
                        span: arm.span.clone(),
                    })
                })
                .collect();

            Some(Primary::Match(MatchExpr {
                scrutinee,
                arms,
                span: m.span.clone(),
            }))
        }

        biwac_ast::Primary::Block(block) => {
            Some(Primary::Block(lower_block_expr(ctx, block, errors)?))
        }
    }
}

pub(crate) fn lower_block_expr(
    ctx: &mut ExprLowerCtx,
    block: &biwac_ast::BlockExpr,
    errors: &mut Vec<ResolveError>,
) -> Option<BlockExpr> {
    use super::statements::lower_stmt;
    let stmts: Vec<Stmt> = block
        .stmts
        .iter()
        .filter_map(|s| lower_stmt(ctx, s, errors))
        .collect();
    let expr = lower_expr(ctx, &block.expr, errors)?;
    Some(BlockExpr {
        stmts,
        expr: Box::new(expr),
        span: block.span.clone(),
    })
}

fn lower_callee(path: &biwac_ast::Path, errors: &mut Vec<ResolveError>) -> Option<Callee> {
    match def_id_kind_from_path(path) {
        Ok(DefIdKind::Val(vid)) => match assoc_fn_self_ty(path) {
            Some(self_ty) => Some(Callee::AssocFn {
                def_id: vid,
                self_ty,
            }),
            None => Some(Callee::Fn(vid)),
        },
        Ok(DefIdKind::Var(vid)) => Some(Callee::Var(vid)),
        Ok(DefIdKind::Ty(tid)) => {
            errors.push(ResolveError::ValueNotFoundTypeFound {
                path: Box::new(path.clone()),
                def_id: tid,
            });
            None
        }
        // 同上。呼べる値ではないものが呼び出しの位置に来ている。
        Ok(_) => {
            errors.push(ResolveError::IdentNotFound {
                ident: path.segments.last().unwrap().ident.clone(),
            });
            None
        }
        Err(e) => {
            errors.push(e);
            None
        }
    }
}

/// `Foo::bar(..)` の `Foo` を型として取り出す。
///
/// パスの最後から 2 番目のセグメントが型に解決されていれば、それが
/// 関連関数を持っている型である。これを残さないと、
/// `type CharacterBiwa = Character[BiwaCharacterProps]` のように
/// エイリアスが書き込んだ型引数が推論に届かない。
///
/// 呼び出し位置に型引数を書く構文はまだ無いので `genargs` は空である。
/// エイリアスなら [`super::alias_expansion`] が右辺ごと置き換えるので、
/// そこで型引数が入る。
///
/// `Self::new(..)` は `abs_header` に `Self` が来てセグメントが 1 つしかないため、
/// ここでは `None` になる (従来どおり [`Callee::Fn`] になる)。
fn assoc_fn_self_ty(path: &biwac_ast::Path) -> Option<Ty> {
    let owner = path.segments.get(path.segments.len().checked_sub(2)?)?;

    let Some(PathSegmentResolution::Ok(DefIdKind::Ty(def_id))) = owner.resolved_id.get() else {
        return None;
    };

    Some(Ty::new(
        TyKind::Defined(DefinedTy {
            def_id: *def_id,
            genargs: Vec::new(),
        }),
        owner.span(),
    ))
}

fn lower_literal(
    ctx: &mut ExprLowerCtx,
    lit: &biwac_ast::Literal,
    errors: &mut Vec<ResolveError>,
) -> Option<Literal> {
    match lit {
        biwac_ast::Literal::Integer(i) => Some(Literal::Integer(i.clone())),
        biwac_ast::Literal::Float(f) => Some(Literal::Float(f.clone())),
        biwac_ast::Literal::String(s) => Some(Literal::String(s.clone())),
        biwac_ast::Literal::Bool(b) => Some(Literal::Bool(b.clone())),
        biwac_ast::Literal::Struct(s) => {
            // `Color::Named { name = x }` は構文の上では構造体リテラルだが、
            // パスがバリアントに解決されていれば構造体形式の構築である。
            // ここは `Literal` を返す関数なので、呼び出し側が先に見ている。
            let tid = match ty_def_id_kind_from_path(&s.path) {
                Ok(TyDefIdKind::Ty(tid)) => tid,
                Ok(_) => {
                    errors.push(ResolveError::PathResolutionFailed {
                        path: Box::new(s.path.clone()),
                    });
                    return None;
                }
                Err(e) => {
                    errors.push(e);
                    return None;
                }
            };
            let members: Vec<(Ident, Expr)> = s
                .members
                .iter()
                .filter_map(|(ident, expr)| {
                    let e = lower_expr(ctx, expr, errors)?;
                    Some((Ident::from(ident.clone()), e))
                })
                .collect();
            Some(Literal::Struct(StructLiteral {
                tid,
                members,
                span: s.span.clone(),
            }))
        }
    }
}
