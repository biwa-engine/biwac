use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    str::FromStr,
};

pub(crate) mod context;

use biwac_ast::{BinOperator, UnOperator};
use biwac_base::PackageName;
use biwac_hir::{
    BlockExpr, BlockStmt, Callee, DefinedTy, Expr, ExprVal, FnDefContentBody,
    FnDefContentSignature, FnTy, GenTyId, Hir, Ident, ImplValDefContentKind, InferTy, Literal,
    LocGenTyId, MemberAccess, PkgId, Primary, Stmt, StructLiteral, Ty, TyDefContentKind, TyId,
    TyKind, TyVar, ValDefContentKind, VarIdKind,
};

use crate::{
    TyCtx, TyError, TyResult,
    inferrer::context::{FnTyCtx, TyInfo},
};

#[derive(Debug, Default)]
struct CallCtx {
    gen_assigns: HashMap<LocGenTyId, Ty>,
}

#[derive(Debug, Default)]
struct DefinedTyCtx {
    gen_assigns: HashMap<GenTyId, Ty>,
}

impl<'tctx> FnTyCtx<'tctx> {
    fn apply(&mut self, t: TyKind) -> TyKind {
        match t {
            TyKind::Infer(i) => match i {
                InferTy::Var(v) => {
                    if let Some(t2) = self.substitutions.get(&v) {
                        self.apply(t2.kind.clone())
                    } else {
                        TyKind::Infer(i)
                    }
                }
                InferTy::Unknown => self.fresh(),
            },
            TyKind::Fn(f) => TyKind::Fn(FnTy {
                args: f
                    .args
                    .into_iter()
                    .map(|a| Ty::new(self.apply(a.kind), a.span))
                    .collect(),
                rty: Box::new(Ty::new(self.apply(f.rty.kind), f.rty.span)),
                genargs: f.genargs,
            }),
            _ => t,
        }
    }

    fn apply_ty(&mut self, t: Ty) -> Ty {
        Ty::new(self.apply(t.kind), t.span)
    }

    // TODO:
    //  型の位置を
    //  ExprTy {
    //      ty: Ty,
    //      span: Span,
    //  }
    //  などとして渡すべき
    fn unify(&mut self, t1: Ty, t2: Ty) -> TyResult<TyKind> {
        let t1 = self.apply_ty(t1);
        let t2 = self.apply_ty(t2);

        // FIXME: inefficient clone to return Err
        match (t1.clone().kind, t2.clone().kind) {
            (TyKind::Infer(i), tk) | (tk, TyKind::Infer(i)) => {
                let tk = self.apply(tk);
                let v = self.ty_var_of_infer_ty(i); // 型変数を取得(なければ新規割り当て)
                let ty = Ty::new(tk.clone(), t1.span.clone());
                if tk == TyKind::Infer(InferTy::Var(v)) {
                    Ok(tk)
                } else if occurs(&v, &tk) {
                    Err(TyError::OccursCheckFailed {
                        tv: Box::new(v),
                        ty: Box::new(ty),
                    })
                } else {
                    self.substitutions.insert(v, ty.clone());

                    Ok(tk)
                }
            }
            //  NOTE: FnTy について
            //  unify() では
            //  ```
            //  let f: (Int, Int -> Int) = (x, y) -> { x + y };
            //  ```
            //  など、単に型が等しい必要がある箇所について検査する
            (TyKind::Fn(fty1), TyKind::Fn(fty2)) => {
                if fty1.args.len() != fty2.args.len() {
                    Err(TyError::FnArgLenMismatched(fty1, fty2))
                } else if fty1.genargs.len() != fty2.genargs.len() {
                    Err(TyError::FnGenArgLenMismatched(fty1, fty2))
                } else {
                    let args = fty1
                        .args
                        .into_iter()
                        .zip(fty2.args.into_iter())
                        .map(|(a1, a2)| {
                            let a1_span = a1.span.clone();
                            Ok(Ty::new(self.unify(a1, a2)?, a1_span))
                        })
                        .collect::<TyResult<_>>()?;

                    let rty_span = fty1.rty.span.clone();
                    let rty = Ty::new(self.unify(*fty1.rty, *fty2.rty)?, rty_span);

                    Ok(TyKind::Fn(FnTy {
                        args,
                        rty: Box::new(rty),
                        genargs: fty2.genargs,
                        // genargs は caller の値をそのまま使用する
                    }))
                }
            }
            (TyKind::Defined(defined_ty1), TyKind::Defined(defined_ty2)) => {
                if defined_ty1.tid == defined_ty2.tid {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs.into_iter())
                            .map(|(g1, g2)| {
                                let g1_span = g1.span.clone();
                                Ok(Ty::new(self.unify(g1, g2)?, g1_span))
                            })
                            .collect::<TyResult<_>>()?;

                        Ok(TyKind::Defined(DefinedTy {
                            tid: defined_ty1.tid,
                            genargs,
                        }))
                    } else {
                        // それぞれ定義と検査済みであるため等しいはず
                        panic!("compiler bug: generic argument length mismatched")
                    }
                } else {
                    Err(TyError::TypeConfliced {
                        t1: Box::new(t1),
                        t2: Box::new(t2),
                    })
                }
            }
            (TyKind::Gen(_), _) | (_, TyKind::Gen(_)) => {
                // Ty::Gen(GenTyId) は型定義しにしか現れない
                panic!("compiler bug: unresolved generic type found")
            }
            (x, y) => {
                if x == y {
                    Ok(x)
                } else {
                    Err(TyError::TypeConfliced {
                        t1: Box::new(t1),
                        t2: Box::new(t2),
                    })
                }
            }
        }
    }

    // callee の型を具体化する
    fn call_embody(&self, callee_ty: TyKind, ctx: &mut CallCtx) -> TyKind {
        match callee_ty {
            TyKind::LocGen(lgid) => {
                if let Some(ty) = ctx.gen_assigns.get(&lgid) {
                    ty.kind.clone()
                } else {
                    callee_ty
                }
            }
            TyKind::Defined(defined_ty) => TyKind::Defined(DefinedTy {
                tid: defined_ty.tid,
                genargs: defined_ty
                    .genargs
                    .into_iter()
                    .map(|ty| Ty::new(self.call_embody(ty.kind, ctx), ty.span))
                    .collect(),
            }),
            TyKind::Fn(fty) => TyKind::Fn(FnTy {
                args: fty
                    .args
                    .into_iter()
                    .map(|ty| Ty::new(self.call_embody(ty.kind, ctx), ty.span))
                    .collect(),
                rty: Box::new(Ty::new(self.call_embody(fty.rty.kind, ctx), fty.rty.span)),
                genargs: fty.genargs,
            }),
            TyKind::Int
            | TyKind::Float
            | TyKind::Bool
            | TyKind::Void
            | TyKind::Gen(_)
            | TyKind::Infer(_) => callee_ty,
        }
    }

    fn call_unify(&mut self, callee_ty: Ty, caller_ty: Ty, ctx: &mut CallCtx) -> TyResult<TyKind> {
        let callee_ty = self.apply_ty(callee_ty);
        let caller_ty = self.apply_ty(caller_ty);

        match (callee_ty.kind.clone(), caller_ty.kind.clone()) {
            (TyKind::Infer(i), tk) | (tk, TyKind::Infer(i)) => {
                let tk = self.apply(tk);
                let tk = self.call_embody(tk, ctx);
                let ty = Ty::new(tk.clone(), caller_ty.span.clone()); // caller 側のspanをとる
                let v = self.ty_var_of_infer_ty(i); // 型変数を取得(なければ新規割り当て)
                if tk == TyKind::Infer(InferTy::Var(v)) {
                    Ok(tk)
                } else if occurs(&v, &tk) {
                    Err(TyError::OccursCheckFailed {
                        tv: Box::new(v),
                        ty: Box::new(ty),
                    })
                } else {
                    self.substitutions.insert(v, ty.clone());

                    Ok(tk)
                }
            }
            // NOTE:
            // callee_fty.genargs は 関数定義側でのVec<LocGenTyId>が記録されており、
            // caller_fty.genargs は 空(vec![]) であるものとする
            // また、caller に現れる LocGenTyId は impl block やその関数で宣言された
            // ジェネリック型であり、
            // callee に現れる LocGenTyId とは別であるので注意が必要
            (TyKind::Fn(callee_fty), TyKind::Fn(caller_fty)) => {
                if callee_fty.args.len() != caller_fty.args.len() {
                    Err(TyError::FnArgLenMismatched(callee_fty, caller_fty))
                } else {
                    // LocGenTyId -> Ty の割り当てが計算できるため、
                    // それによりできるだけ具体の型を計算して返す
                    let args = callee_fty
                        .args
                        .into_iter()
                        .zip(caller_fty.args.into_iter())
                        .map(|(a1, a2)| {
                            let a2_span = a2.span.clone();
                            Ok(Ty::new(self.call_unify(a1, a2, ctx)?, a2_span))
                        })
                        .collect::<TyResult<_>>()?;

                    let rty = Ty::new(
                        self.call_unify(*callee_fty.rty, *caller_fty.rty, ctx)?,
                        caller_ty.span,
                    );
                    // genargs に登場する LocGenTyId が必ず引数または戻り値に現れるという前提のもと、
                    // この時点で gen_assigns にはすべての LocGenTyId に対する Ty
                    // の割り当てが計算されている
                    //
                    // NOTE: FnTyに割り当てを記録しても良い

                    Ok(TyKind::Fn(FnTy {
                        args,
                        rty: Box::new(rty),
                        genargs: caller_fty.genargs,
                        // genargs は caller の値をそのまま使用する
                    }))
                }
            }
            (TyKind::Defined(defined_ty1), TyKind::Defined(defined_ty2)) => {
                if defined_ty1.tid == defined_ty2.tid {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs.into_iter())
                            .map(|(g1, g2)| {
                                let g2_span = g2.span.clone();
                                Ok(Ty::new(self.call_unify(g1, g2, ctx)?, g2_span))
                            })
                            .collect::<TyResult<_>>()?;

                        Ok(TyKind::Defined(DefinedTy {
                            tid: defined_ty1.tid,
                            genargs,
                        }))
                    } else {
                        // それぞれ定義と検査済みであるため等しいはず
                        panic!("compiler bug: generic argument length mismatched")
                    }
                } else {
                    Err(TyError::TypeConfliced {
                        t1: Box::new(callee_ty),
                        t2: Box::new(caller_ty),
                    })
                }
            }
            (TyKind::LocGen(lgid), _) => match ctx.gen_assigns.entry(lgid) {
                Entry::Vacant(e) => {
                    e.insert(caller_ty.clone());

                    Ok(caller_ty.kind)
                }
                Entry::Occupied(mut e) => {
                    // 両方 caller 由来の型であるため、unify() でよい
                    let caller_ty_span = caller_ty.span.clone();
                    let ty = Ty::new(self.unify(e.get().clone(), caller_ty)?, caller_ty_span);
                    e.insert(ty.clone());

                    Ok(ty.kind)
                }
            },
            (TyKind::Gen(_), _) | (_, TyKind::Gen(_)) => {
                // Ty::Gen(GenTyId) は型定義にしか現れない
                panic!("compiler bug: unresolved generic type found")
            }
            (x, y) => {
                if x == y {
                    // WARN: really?
                    Ok(self.call_embody(x, ctx))
                } else {
                    Err(TyError::TypeConfliced {
                        t1: Box::new(callee_ty),
                        t2: Box::new(caller_ty),
                    })
                }
            }
        }
    }

    // 型定義を使用する箇所(struct, enum リテラル)でのunify
    // definition_ty: 型定義側
    // user_ty: 型使用側
    fn defined_ty_unify(
        &mut self,
        definition_ty: Ty,
        user_ty: Ty,
        ctx: &mut DefinedTyCtx,
    ) -> TyResult<TyKind> {
        let definition_ty = self.apply_ty(definition_ty);
        let user_ty = self.apply_ty(user_ty);

        // FIXME: inefficient clone to return Err
        match (definition_ty.kind.clone(), user_ty.kind.clone()) {
            (TyKind::Infer(i), tk) | (tk, TyKind::Infer(i)) => {
                let tk = self.apply(tk);
                let v = self.ty_var_of_infer_ty(i); // 型変数を取得(なければ新規割り当て)
                let ty = Ty::new(tk.clone(), user_ty.span.clone()); // user 側のspanをとる
                if tk == TyKind::Infer(InferTy::Var(v)) {
                    Ok(tk)
                } else if occurs(&v, &tk) {
                    Err(TyError::OccursCheckFailed {
                        tv: Box::new(v),
                        ty: Box::new(ty),
                    })
                } else {
                    self.substitutions.insert(v, ty.clone());

                    Ok(tk)
                }
            }
            //  NOTE: FnTy について
            //  defined_ty_unify() では
            //  ```
            //  let f: (Int, Int -> Int) = (x, y) -> { x + y };
            //  ```
            //  など、単に型が等しい必要がある箇所について検査する
            (TyKind::Fn(fty1), TyKind::Fn(fty2)) => {
                if fty1.args.len() != fty2.args.len() {
                    Err(TyError::FnArgLenMismatched(fty1, fty2))
                } else if fty1.genargs.len() != fty2.genargs.len() {
                    Err(TyError::FnGenArgLenMismatched(fty1, fty2))
                } else {
                    let args = fty1
                        .args
                        .into_iter()
                        .zip(fty2.args.into_iter())
                        .map(|(a1, a2)| {
                            let a2_span = a2.span.clone();
                            Ok(Ty::new(self.defined_ty_unify(a1, a2, ctx)?, a2_span))
                        })
                        .collect::<TyResult<_>>()?;

                    let rty = Ty::new(
                        self.defined_ty_unify(*fty1.rty, *fty2.rty, ctx)?,
                        user_ty.span,
                    );

                    Ok(TyKind::Fn(FnTy {
                        args,
                        rty: Box::new(rty),
                        genargs: fty2.genargs,
                        // genargs は caller の値をそのまま使用する
                    }))
                }
            }
            (TyKind::Defined(defined_ty1), TyKind::Defined(defined_ty2)) => {
                if defined_ty1.tid == defined_ty2.tid {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs.into_iter())
                            .map(|(g1, g2)| {
                                let g1_span = g1.span.clone();
                                Ok(Ty::new(self.defined_ty_unify(g1, g2, ctx)?, g1_span))
                            })
                            .collect::<TyResult<_>>()?;

                        Ok(TyKind::Defined(DefinedTy {
                            tid: defined_ty1.tid,
                            genargs,
                        }))
                    } else {
                        // それぞれ定義と検査済みであるため等しいはず
                        panic!("compiler bug: generic argument length mismatched")
                    }
                } else {
                    Err(TyError::TypeConfliced {
                        t1: Box::new(definition_ty),
                        t2: Box::new(user_ty),
                    })
                }
            }
            (TyKind::Gen(gid), _) => match ctx.gen_assigns.entry(gid) {
                Entry::Vacant(e) => {
                    e.insert(user_ty.clone());

                    Ok(user_ty.kind)
                }
                Entry::Occupied(mut e) => {
                    // 両方 user 由来の型であるため、unify() でよい
                    let user_ty_span = user_ty.span.clone();
                    let ty = Ty::new(self.unify(e.get().clone(), user_ty)?, user_ty_span);
                    e.insert(ty.clone());

                    Ok(ty.kind)
                }
            },
            (x, y) => {
                if x == y {
                    Ok(x)
                } else {
                    Err(TyError::TypeConfliced {
                        t1: Box::new(definition_ty),
                        t2: Box::new(user_ty),
                    })
                }
            }
        }
    }

    // fn instantiate(&mut self, s: &Scheme) -> Ty {
    //     let mut m = HashMap::new();
    //
    //     for v in &s.vars {
    //         m.insert(*v, self.fresh());
    //     }
    //
    //     fn go(t: &Ty, m: &HashMap<TyVar, Ty>) -> Ty {
    //         match t {
    //             Ty::Var(v) => m.get(v).cloned().unwrap_or(Ty::Var(*v)),
    //             Ty::Fn(f) => Ty::Fn(FnTy {
    //                 args: f.args.iter().map(|a| go(a, m)).collect(),
    //                 ret: Box::new(go(&f.ret, m)),
    //             }),
    //             x => x.clone(),
    //         }
    //     }
    //
    //     go(&s.ty, &m)
    // }

    fn infer_expr(&mut self, expr: &Expr) -> TyResult<Ty> {
        let res = match &expr.expr {
            ExprVal::Primary(primary) => self.infer_primary_expr(primary),
            ExprVal::Unary(u) => {
                match &u.op {
                    // T: Int, Uint, Float のいずれかに対して適用可能で
                    // T -> T である演算子
                    UnOperator::Neg => {
                        let ty = self.infer_expr(&u.right)?;
                        // NOTE: traitによる演算子オーバーロードが可能になれば
                        // このチェックは要らない
                        match ty.kind {
                            TyKind::Infer(_) | TyKind::Int | TyKind::Float => {
                                Ok(Ty::new(ty.kind, expr.span().into()))
                            }
                            _ => Err(TyError::InvalidUnaryOperationForType {
                                ty: Box::new(ty),
                                op: u.op,
                                expr: Box::new(expr.clone()),
                            }),
                        }
                    }
                }
            }
            ExprVal::Binary(b) => match &b.op {
                // T: Int, Uint, Float に対して適用可能で
                // (T, T) -> T である演算子
                BinOperator::Add
                | BinOperator::Sub
                | BinOperator::Mul
                | BinOperator::Div
                | BinOperator::Mod => {
                    let left = self.infer_expr(&b.left)?;
                    let right = self.infer_expr(&b.right)?;

                    let tk = self.unify(left.clone(), right.clone())?;

                    // NOTE: traitによる演算子オーバーロードが可能になれば
                    // このチェックは要らない
                    match tk {
                        TyKind::Infer(_) | TyKind::Int | TyKind::Float => {
                            Ok(Ty::new(tk, expr.span().into()))
                        }
                        _ => Err(TyError::InvalidBinaryOperationForType {
                            ty: Box::new(Ty::new(tk, left.span)),
                            op: b.op,
                            expr: Box::new(expr.clone()),
                        }),
                    }
                }
                // T: Int, Uint, Float に対して適用可能で
                // (T, T) -> Bool である演算子
                BinOperator::Gt | BinOperator::Lt | BinOperator::Ge | BinOperator::Le => {
                    let left = self.infer_expr(&b.left)?;
                    let right = self.infer_expr(&b.right)?;

                    let tk = self.unify(left.clone(), right.clone())?;

                    // NOTE: traitによる演算子オーバーロードが可能になれば
                    // このチェックは要らない
                    match tk {
                        TyKind::Infer(_) | TyKind::Int | TyKind::Float => {
                            Ok(Ty::new(TyKind::Bool, expr.span().into()))
                        }
                        _ => Err(TyError::InvalidBinaryOperationForType {
                            ty: Box::new(Ty::new(tk, left.span)),
                            op: b.op,
                            expr: Box::new(expr.clone()),
                        }),
                    }
                }
                // T: Int, Uint, Float, Bool に対して適用可能で
                // (T, T) -> Bool である演算子
                BinOperator::Eq | BinOperator::Ne => {
                    let left = self.infer_expr(&b.left)?;
                    let right = self.infer_expr(&b.right)?;

                    let tk = self.unify(left.clone(), right.clone())?;

                    // NOTE: traitによる演算子オーバーロードが可能になれば
                    // このチェックは要らない
                    match tk {
                        TyKind::Infer(_) | TyKind::Int | TyKind::Float | TyKind::Bool => {
                            Ok(Ty::new(TyKind::Bool, expr.span().into()))
                        }
                        _ => Err(TyError::InvalidBinaryOperationForType {
                            ty: Box::new(Ty::new(tk, left.span)),
                            op: b.op,
                            expr: Box::new(expr.clone()),
                        }),
                    }
                }
            },
        };

        self.exprs.insert(expr.id, res.clone()?);

        res
    }

    fn infer_struct_literal(&mut self, tid: &TyId, struct_literal: &StructLiteral) -> TyResult<Ty> {
        match self.tctx.hir.get_type_definition(tid).unwrap() {
            TyDefContentKind::Struct(struct_) => {
                let mut members = HashMap::<&str, (&Ident, &Expr)>::new();
                for (ident, expr) in &struct_literal.members {
                    match members.entry(&ident.id) {
                        Entry::Vacant(e) => {
                            e.insert((ident, expr));
                        }
                        Entry::Occupied(e) => {
                            return Err(TyError::StructLiteralMemberConfliced {
                                member1: Box::new(e.get().0.clone()),
                                member2: Box::new(ident.clone()),
                            });
                        }
                    }
                }

                // 構造体に定義されているメンバ名の集合
                let mut member_ids = struct_
                    .members
                    .keys()
                    .map(|m| m.as_str())
                    .collect::<HashSet<&str>>();

                let mut dtctx = DefinedTyCtx::default();
                for (id, (ident, expr)) in &members {
                    if let Some(definition_ty) = struct_.members.get(*id).cloned() {
                        let user_ty = self.infer_expr(expr)?;
                        self.defined_ty_unify(
                            Ty::new(definition_ty.kind, ident.span.clone()),
                            user_ty,
                            &mut dtctx,
                        )?;
                        // メンバを取り除いていく
                        member_ids.remove(id);
                    } else {
                        return Err(TyError::StructLiteralAssignToInexsistentMember {
                            tid: Box::new(tid.clone()),
                            member: Box::new(ident.to_owned().clone()),
                        });
                    }
                }

                // 定義型のジェネリック引数宣言に登場するジェネリック型が
                // そのメンバなどに必ず使用されることが保証されているなら、
                // dtctx.gen_assigns にはこの時点で必ず GenTyId -> Ty の割り当てがある
                // その割り当てを収集して返す
                let genargs = struct_
                    .genargs
                    .iter()
                    .map(|gid| dtctx.gen_assigns.get(gid).unwrap().clone())
                    .collect();

                if member_ids.is_empty() {
                    Ok(Ty::new(
                        TyKind::Defined(DefinedTy {
                            tid: tid.clone(),
                            genargs,
                        }),
                        struct_literal.span.clone().into(),
                    ))
                } else {
                    // メンバ名の集合に残されているものが、
                    // 初期化されていないメンバ
                    Err(TyError::StructLiteralMemberInsufficient {
                        sliteral: Box::new(struct_literal.clone()),
                        insufficient_members: member_ids
                            .into_iter()
                            .map(|m| m.to_string())
                            .collect(),
                    })
                }
            }
            TyDefContentKind::TypeAlias(alias) => match &alias.right.kind {
                TyKind::Int | TyKind::Float | TyKind::Bool | TyKind::Fn(_) | TyKind::Void => {
                    Err(TyError::InvalidStructLiteralOnAliasType {
                        ty: Box::new(alias.right.clone()),
                        sliteral: Box::new(struct_literal.clone()),
                    })
                }
                TyKind::Infer(_) => panic!("compiler bug: type alias on unknown type"),
                TyKind::Gen(_) => {
                    // TyKind::Gen(GenTyId) は型定義にしか現れないため、
                    // メンバアクセスの左辺地に現れる場合はバグ
                    panic!("compiler bug: generic type not resolved")
                }
                TyKind::LocGen(_) => {
                    // impl block や 関数 でローカルに宣言されたジェネリック型
                    // これがグローバルな type alias で現れることはないのでバグ
                    panic!("compiler bug: type alias on generic type")
                }
                TyKind::Defined(defined_ty) => {
                    // alias された TyId で再試行
                    self.infer_struct_literal(&defined_ty.tid, struct_literal)
                }
            },
            TyDefContentKind::NativeTypeAlias(alias) => {
                // native type alias を構造体のように初期化することは出来ない
                Err(TyError::InvalidStructLiteralOnAliasType {
                    ty: Box::new(Ty::new(
                        TyKind::Defined(DefinedTy {
                            tid: tid.clone(),
                            genargs: vec![
                                Ty::new(
                                    TyKind::Infer(InferTy::Unknown),
                                    struct_literal.span.clone().into() // 正しくないが、エラー表示には使われないため、良しとする
                                );
                                alias.genargs.len()
                            ],
                        }),
                        struct_literal.span.clone().into(),
                    )),
                    sliteral: Box::new(struct_literal.clone()),
                })
            }
        }
    }

    fn infer_primary_expr(&mut self, primary: &Primary) -> TyResult<Ty> {
        match primary {
            Primary::Literal(l) => match l {
                Literal::Integer(_) => Ok(Ty::new(TyKind::Int, primary.span().into())),
                // Literal::Float(_) => Ok(Ty::Float),
                Literal::Bool(_) => Ok(Ty::new(TyKind::Bool, primary.span().into())),
                Literal::String(_) => Ok(Ty::new(
                    TyKind::Defined(DefinedTy {
                        tid: TyId::new(
                            PkgId::new(PackageName::from_str("std").unwrap()),
                            vec!["types".into(), "string".into()],
                            "String".into(),
                        ),
                        genargs: Vec::new(),
                    }),
                    primary.span().into(),
                )),
                Literal::Struct(struct_literal) => {
                    self.infer_struct_literal(&struct_literal.tid, struct_literal)
                }
            },
            Primary::Variable(v) => {
                match v.id {
                    VarIdKind::Global(_) => todo!(),
                    VarIdKind::Local(var_id) => {
                        // 名前解決済みなので存在は保証されている
                        Ok(self.vars.get(&var_id).unwrap().clone())
                    }
                }
            }
            Primary::FnCall(c) => match &c.callee {
                Callee::Fn(vid) => {
                    let callee_ty = self.tctx.hir.get_fn_sign(vid).unwrap().as_ty();

                    let args = c
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<Result<_, _>>()?;
                    let rty = self.fresh();

                    // NOTE: caller は genargs は 空 vec![] でよい
                    // unify で計算する
                    let mut cctx = CallCtx::default();
                    let unified_ty = self.call_unify(
                        callee_ty,
                        Ty::new(
                            TyKind::Fn(FnTy {
                                args,
                                rty: Box::new(Ty::new(rty, primary.span().into())),
                                genargs: vec![],
                            }),
                            primary.span().into(),
                        ),
                        &mut cctx,
                    )?;

                    let unified_fty = if let TyKind::Fn(fty) = unified_ty {
                        fty
                    } else {
                        panic!("compiler bug: 2 Ty::Fn unification must be Ty::Fn")
                    };

                    Ok(self.fresh_loc_gen_ty(*unified_fty.rty))
                }
                Callee::Var(v) => {
                    // 変数は名前解決済みであるため、先に型推論されているはず
                    // TODO: callee を式に対応させる
                    let callee_fty = self.vars.get(v).unwrap().clone();
                    // let f = self.infer_expr(&expr)?;

                    let args = c
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<Result<_, _>>()?;
                    let rty = Ty::new(self.fresh(), primary.span().into());

                    // NOTE: caller は genargs は 空 vec![] でよい
                    // unify で計算する
                    let mut cctx = CallCtx::default();
                    self.call_unify(
                        callee_fty,
                        Ty::new(
                            TyKind::Fn(FnTy {
                                args,
                                rty: Box::new(rty.clone()),
                                genargs: vec![],
                            }),
                            primary.span().into(),
                        ),
                        &mut cctx,
                    )?;

                    Ok(rty)
                }
                Callee::Assoc(assoc_callee) => {
                    let callee_ty = self.tctx.hir.get_assoc_of_type(assoc_callee)?;

                    let args = c
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<Result<_, _>>()?;
                    let rty = Ty::new(self.fresh(), primary.span().into());

                    // NOTE: caller は genargs は 空 vec![] でよい
                    // unify で計算する
                    let mut cctx = CallCtx::default();
                    let unified_ty = self.call_unify(
                        callee_ty,
                        Ty::new(
                            TyKind::Fn(FnTy {
                                args,
                                rty: Box::new(rty),
                                genargs: vec![],
                            }),
                            primary.span().into(),
                        ),
                        &mut cctx,
                    )?;
                    let unified_fty = if let TyKind::Fn(fty) = unified_ty {
                        fty
                    } else {
                        panic!("compiler bug: 2 Ty::Fn unification must be Ty::Fn")
                    };

                    Ok(self.fresh_loc_gen_ty(*unified_fty.rty))
                }
            },
            Primary::MemberAccess(m) => {
                let left = self.infer_expr(&m.left)?;

                self.infer_member_access(left, m)
            }
            Primary::IfExpr(if_expr) => {
                let cond = self.infer_expr(&if_expr.cond)?;
                self.unify(cond, Ty::new(TyKind::Bool, primary.span().into()))?;

                // TODO: else if に対応
                let then_ty = self.infer_block_expr(&if_expr.then)?;
                let els_ty = self.infer_block_expr(&if_expr.els)?;

                Ok(Ty::new(self.unify(then_ty, els_ty)?, primary.span().into()))
            }
            Primary::Block(block) => self.infer_block_expr(block),
            Primary::MethodCall(m) => {
                let left = self.infer_expr(&m.left)?;

                // 左辺値の型のメソッド実装からメソッド名をキーにメソッドを取得
                if let Some(callee_ty) = self.tctx.hir.get_method_of_type(&left.kind, &m.method)? {
                    let args = m
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<TyResult<_>>()?;

                    let rty = Ty::new(self.fresh(), primary.span().into());

                    // NOTE: caller は genargs は 空 vec![] でよい
                    // unify で計算する
                    let mut cctx = CallCtx::default();
                    let caller_ty = Ty::new(
                        TyKind::Fn(FnTy {
                            args,
                            rty: Box::new(rty.clone()),
                            genargs: vec![],
                        }),
                        primary.span().into(),
                    );
                    let unified_ty = self.call_unify(callee_ty, caller_ty, &mut cctx)?;

                    let unified_fty = if let TyKind::Fn(fty) = unified_ty {
                        fty
                    } else {
                        panic!("compiler bug: 2 Ty::Fn unification must be Ty::Fn")
                    };

                    Ok(self.fresh_loc_gen_ty(*unified_fty.rty))
                } else {
                    Err(TyError::MethodNotImplemented {
                        ty: Box::new(left),
                        method: Box::new(m.method.clone()),
                    })
                }
            }
        }
    }

    // 関数や型などの定義に存在するジェネリック型について、
    // 呼び出して使用する際に未確定の場合、
    // 推論が必要なものとして型変数を割り当てる
    #[allow(dead_code)]
    fn fresh_gen_ty(&mut self, ty: Ty) -> Ty {
        match ty.kind {
            TyKind::Int
            | TyKind::Float
            | TyKind::Bool
            | TyKind::Infer(_)
            | TyKind::Void
            | TyKind::LocGen(_) => ty,
            TyKind::Fn(fty) => Ty::new(
                TyKind::Fn(FnTy {
                    args: fty
                        .args
                        .into_iter()
                        .map(|a| self.fresh_loc_gen_ty(a))
                        .collect(),
                    rty: Box::new(self.fresh_loc_gen_ty(*fty.rty)),
                    genargs: fty.genargs,
                }),
                ty.span,
            ),
            TyKind::Defined(defined_ty) => Ty::new(
                TyKind::Defined(DefinedTy {
                    tid: defined_ty.tid,
                    genargs: defined_ty
                        .genargs
                        .into_iter()
                        .map(|g| self.fresh_loc_gen_ty(g))
                        .collect(),
                }),
                ty.span,
            ),
            TyKind::Gen(_) => Ty::new(self.fresh(), ty.span),
        }
    }

    // 関数や型などの定義に存在するジェネリック型について、
    // 呼び出して使用する際に未確定の場合、
    // 推論が必要なものとして型変数を割り当てる
    fn fresh_loc_gen_ty(&mut self, ty: Ty) -> Ty {
        match ty.kind {
            TyKind::Int
            | TyKind::Float
            | TyKind::Bool
            | TyKind::Infer(_)
            | TyKind::Void
            | TyKind::Gen(_) => ty,
            TyKind::Fn(fty) => Ty::new(
                TyKind::Fn(FnTy {
                    args: fty
                        .args
                        .into_iter()
                        .map(|a| self.fresh_loc_gen_ty(a))
                        .collect(),
                    rty: Box::new(self.fresh_loc_gen_ty(*fty.rty)),
                    genargs: fty.genargs,
                }),
                ty.span,
            ),
            TyKind::Defined(defined_ty) => Ty::new(
                TyKind::Defined(DefinedTy {
                    tid: defined_ty.tid,
                    genargs: defined_ty
                        .genargs
                        .into_iter()
                        .map(|g| self.fresh_loc_gen_ty(g))
                        .collect(),
                }),
                ty.span,
            ),
            TyKind::LocGen(_) => Ty::new(self.fresh(), ty.span),
        }
    }

    fn infer_member_access(&mut self, left_ty: Ty, member_access: &MemberAccess) -> TyResult<Ty> {
        match left_ty.kind {
            TyKind::Int | TyKind::Float | TyKind::Bool | TyKind::Fn(_) | TyKind::Void => {
                Err(TyError::ExprNotHasMember {
                    ty: Box::new(left_ty),
                    access: Box::new(member_access.clone()),
                })
            }
            TyKind::Infer(_) => Err(TyError::InsufficientContext),
            TyKind::Defined(defined_ty) => {
                match self.tctx.hir.get_type_definition(&defined_ty.tid).unwrap() {
                    TyDefContentKind::Struct(struct_) => {
                        let ty = struct_
                            .members
                            .get(&member_access.member.id)
                            .ok_or(TyError::StructNotHasMember {
                                tid: defined_ty.tid.clone(),
                                access: Box::new(member_access.clone()),
                            })
                            .cloned()?;

                        // NOTE: ジェネリック型 TyKind::Gen(GenTyId) の場合、
                        // ジェネリック引数列の位置から GenTyId -> TyKind を割り当て
                        if let TyKind::Gen(gid) = &ty.kind {
                            let idx = struct_.genargs.iter().position(|g| g == gid).expect(
                                "compiler bug: undefined generic type found in struct member",
                            );

                            if defined_ty.genargs.len() == struct_.genargs.len() {
                                Ok(defined_ty.genargs.get(idx).unwrap().clone())
                            } else {
                                panic!("compiler bug: generic argument length mismatched")
                            }
                        } else {
                            Ok(ty)
                        }
                    }
                    TyDefContentKind::TypeAlias(alias) => {
                        // alias の右辺の型で再度試行
                        self.infer_member_access(alias.right.clone(), member_access)
                    }
                    TyDefContentKind::NativeTypeAlias(_) => {
                        // native type alias にはメンバアクセスできない
                        Err(TyError::ExprNotHasMember {
                            ty: Box::new(Ty::new(TyKind::Defined(defined_ty), left_ty.span)),
                            access: Box::new(member_access.clone()),
                        })
                    }
                }
            }
            TyKind::Gen(_) => {
                // TyKind::Gen(GenTyId) は型定義にしか現れないため、
                // メンバアクセスの左辺地に現れる場合はバグ
                panic!("compiler bug: generic type not resolved")
            }
            TyKind::LocGen(_) => {
                // impl block や 関数 でローカルに宣言されたジェネリック型
                // これがメンバアクセス可能性を満たすことは判定できないため
                // コンパイルエラー
                Err(TyError::ExprNotHasMember {
                    ty: Box::new(left_ty),
                    access: Box::new(member_access.clone()),
                })
            }
        }
    }

    fn infer_block_expr(&mut self, block: &BlockExpr) -> TyResult<Ty> {
        for stmt in &block.stmts {
            self.infer_stmt(stmt)?;
        }

        self.infer_expr(&block.expr)
    }

    fn infer_stmt(&mut self, stmt: &Stmt) -> TyResult<Option<Ty>> {
        match stmt {
            Stmt::VarDecl(v) => {
                let ty = self.infer_expr(&v.init)?;
                let ty = self.apply_ty(ty);

                // 変数に付いた型は、ターゲットによっては依存に含まれる
                self.tctx.hir.deps_recorder.borrow_mut().depends_on_ty(&ty);

                self.vars.insert(v.id, ty);

                Ok(None)
            }
            Stmt::If(if_stmt) => {
                let cond = self.infer_expr(&if_stmt.cond)?;
                let bool_ty = Ty::new(TyKind::Bool, cond.span.clone());
                self.unify(cond, bool_ty)?;

                // TODO: else if に対応
                let then_ty = self.infer_block_stmt(&if_stmt.then)?;

                if let Some(els) = &if_stmt.els {
                    let els_ty = self.infer_block_stmt(els)?;

                    min_of_ty(&then_ty, &els_ty)
                } else {
                    // elseが無いということは分岐の1つは何も返さない(= Voidを返す)ということである
                    // したがって、thenの型に関係なく、全体の型はVoid
                    Ok(None)
                }
            }
            Stmt::Block(block) => self.infer_block_stmt(block),
            Stmt::Expr(expr) => {
                self.infer_expr(&expr.expr)?;

                Ok(None)
            }
            Stmt::Return(ret) => {
                let rty = self.infer_expr(&ret.expr)?;

                Ok(Some(Ty::new(
                    self.unify(rty, self.rty.clone())?,
                    ret.expr.span().into(),
                )))
            }
            Stmt::Assign(ass) => {
                match &ass.dst {
                    Primary::Variable(_) | Primary::MemberAccess(_) => {
                        let dst = self.infer_primary_expr(&ass.dst)?;
                        let src = self.infer_expr(&ass.src)?;

                        self.unify(dst, src)?;
                    }
                    _ => {
                        return Err(TyError::InvalidAssignOperation {
                            ass: Box::new(ass.clone()),
                        });
                    }
                }

                Ok(None)
            }
            Stmt::While(while_stmt) => {
                let cond = self.infer_expr(&while_stmt.cond)?;
                let bool_ty = Ty::new(TyKind::Bool, cond.span.clone());
                self.unify(cond, bool_ty)?;

                self.infer_block_stmt(&while_stmt.stmts)?;

                Ok(None)
            }
        }
    }

    fn infer_block_stmt(&mut self, block: &BlockStmt) -> TyResult<Option<Ty>> {
        let mut opt_ty = None;
        for stmt in &block.stmts {
            let t = self.infer_stmt(stmt)?;

            if t.is_some() {
                opt_ty = t;
            }

            // TODO: warn unreachable code after return
        }

        Ok(opt_ty)
    }
}

fn occurs(v: &TyVar, tk: &TyKind) -> bool {
    match &tk {
        TyKind::Infer(i) => match i {
            InferTy::Var(v2) => v == v2,
            InferTy::Unknown => false, // WARN: really?
        },
        TyKind::Fn(f) => f.args.iter().any(|a| occurs(v, &a.kind)) || occurs(v, &f.rty.kind),
        _ => false,
    }
}

fn min_of_ty(t1: &Option<Ty>, t2: &Option<Ty>) -> TyResult<Option<Ty>> {
    match (t1, t2) {
        (None, _) => Ok(None),
        (_, None) => Ok(None),
        (Some(x), Some(y)) => {
            if x == y {
                Ok(Some(x.clone()))
            } else {
                Err(TyError::TypeConfliced {
                    t1: Box::new(x.clone()),
                    t2: Box::new(y.clone()),
                })
            }
        }
    }
}

impl TyCtx {
    fn infer_fn_body(
        &self,
        fn_body: &FnDefContentBody,
        fn_signature: &FnDefContentSignature,
    ) -> TyResult<TyInfo> {
        let mut fctx = FnTyCtx::new(self, fn_signature.rty.clone());

        // 引数を決定済みの型として文脈に記録
        // arg_var_ids は第一引数がselfのときはそれも含む
        for var_id in &fn_body.arg_var_ids {
            fctx.vars
                .insert(*var_id, fn_body.vars.get(var_id).unwrap().ty.clone());
        }

        // 式で終わっている場合、その式の型が戻り値の型と一致することを検査すれば良い
        // 文のみの場合、最後の文のすべての分岐でreturn文があり、正しい型を返していることを検査する必要がある
        // 文は
        // - 基本的にVoidを返すものとし、
        // - return文はその式の型、
        // - 分岐文はすべての分岐で一致すればその型、そうでなければNoneとする
        // これにより、最後の文の型の一致を検査可能になる
        // また、早期returnの型を検査するために、TyCtxに戻り値の型を含める
        let mut stmt_last_ty = None;
        for stmt in &fn_body.stmts {
            stmt_last_ty = fctx.infer_stmt(stmt)?;
        }

        // 最後の式があれば検査
        // 戻り値の型の一致を検査
        if let Some(expr) = &fn_body.expr {
            let rty = fctx.infer_expr(expr)?;
            fctx.unify(rty, fctx.rty.clone())?;
        } else if let Some(rty) = stmt_last_ty {
            fctx.unify(rty, fctx.rty.clone())?;
        } else if fctx.rty.kind != TyKind::Void {
            return Err(TyError::ReturnTypeRequired {
                rty: Box::new(fctx.rty),
            });
        };

        Ok(TyInfo {
            expr_tys: fctx.exprs,
            var_tys: fctx.vars,
        })
    }

    pub fn infer(mut self) -> TyResult<Hir> {
        // 普通の関数について
        // 型推論し、その結果を一時的に保持
        let mut fn_ty_infos = vec![];
        for (vid, val) in &self.hir.vals {
            match &val {
                ValDefContentKind::Fn(f) => {
                    // 計算した型を記録
                    fn_ty_infos.push((
                        vid.clone(),
                        self.infer_fn_body(f.body.expect_completed(), &f.signature)?,
                    ));
                }
                ValDefContentKind::NovelScene(n) => {
                    // 計算した型を記録
                    fn_ty_infos.push((
                        vid.clone(),
                        self.infer_fn_body(n.body.expect_completed(), &n.signature)?,
                    ));
                }
                ValDefContentKind::Native(_) | ValDefContentKind::ExternalFn(_) => {
                    // nothing to do
                }
            }
        }

        // 推論結果を hir に記録
        for (vid, ty_info) in fn_ty_infos {
            let val = self
                .hir
                .vals
                .get_mut(&vid)
                .expect("compiler bug: value not found");
            match val {
                ValDefContentKind::Fn(f) => {
                    f.expr_tys = ty_info.expr_tys;
                    f.var_tys = ty_info.var_tys;
                }
                ValDefContentKind::NovelScene(n) => {
                    n.expr_tys = ty_info.expr_tys;
                    n.var_tys = ty_info.var_tys;
                }
                ValDefContentKind::Native(_) | ValDefContentKind::ExternalFn(_) => {
                    // nothing to do
                }
            }
        }

        // ユーザ定義型に対する実装(関連関数、メソッド)について
        // 型推論し、その結果を一時的に保持
        let mut impl_fn_ty_infos = vec![];
        for (tid, ty_impl) in &self.hir.tys {
            for (val_name, impl_list) in &ty_impl.vals {
                for (impl_valid, impl_) in &impl_list.vals {
                    match &impl_.val_content {
                        ImplValDefContentKind::Fn(f) => {
                            // 計算した型を記録
                            impl_fn_ty_infos.push((
                                tid.clone(),
                                val_name.clone(),
                                *impl_valid,
                                self.infer_fn_body(f.body.expect_completed(), &f.signature)?,
                            ));
                        }
                        ImplValDefContentKind::Method(m) => {
                            // 計算した型を記録
                            impl_fn_ty_infos.push((
                                tid.clone(),
                                val_name.clone(),
                                *impl_valid,
                                self.infer_fn_body(m.body.expect_completed(), &m.signature)?,
                            ));
                        }
                        ImplValDefContentKind::NativeFn(_)
                        | ImplValDefContentKind::NativeMethod(_) => {
                            // nothing to do
                        }
                    }
                }
            }
        }

        // 推論結果を hir に記録
        for (tid, val_name, impl_valid, ty_info) in impl_fn_ty_infos {
            let ty_impl = self
                .hir
                .tys
                .get_mut(&tid)
                .expect("compiler bug: value not found");
            match &mut ty_impl
                .vals
                .get_mut(&val_name)
                .unwrap()
                .vals
                .get_mut(&impl_valid)
                .unwrap()
                .val_content
            {
                ImplValDefContentKind::Fn(f) => {
                    f.expr_tys = ty_info.expr_tys;
                    f.var_tys = ty_info.var_tys;
                }
                ImplValDefContentKind::Method(m) => {
                    m.expr_tys = ty_info.expr_tys;
                    m.var_tys = ty_info.var_tys;
                }
                ImplValDefContentKind::NativeFn(_) | ImplValDefContentKind::NativeMethod(_) => {
                    // nothing to do
                }
            }
        }

        // 特殊型(プリミティブ型など)に対する実装(関連関数、メソッド)について
        // 型推論し、その結果を一時的に保持
        let mut special_impl_fn_ty_infos = vec![];
        for (ty, ty_impl) in &self.hir.special_ty_impls {
            for (val_name, val) in &ty_impl.vals {
                match &val {
                    ImplValDefContentKind::Fn(f) => {
                        // 計算した型を記録
                        special_impl_fn_ty_infos.push((
                            ty.clone(),
                            val_name.clone(),
                            self.infer_fn_body(f.body.expect_completed(), &f.signature)?,
                        ));
                    }
                    ImplValDefContentKind::Method(m) => {
                        // 計算した型を記録
                        special_impl_fn_ty_infos.push((
                            ty.clone(),
                            val_name.clone(),
                            self.infer_fn_body(m.body.expect_completed(), &m.signature)?,
                        ));
                    }
                    ImplValDefContentKind::NativeFn(_) | ImplValDefContentKind::NativeMethod(_) => {
                        // nothing to do
                    }
                }
            }
        }

        // 推論結果を hir に記録
        for (ty, val_name, ty_info) in special_impl_fn_ty_infos {
            let ty_impl = self
                .hir
                .special_ty_impls
                .get_mut(&ty)
                .expect("compiler bug: value not found");
            match &mut ty_impl.vals.get_mut(&val_name).unwrap() {
                ImplValDefContentKind::Fn(f) => {
                    f.expr_tys = ty_info.expr_tys;
                    f.var_tys = ty_info.var_tys;
                }
                ImplValDefContentKind::Method(m) => {
                    m.expr_tys = ty_info.expr_tys;
                    m.var_tys = ty_info.var_tys;
                }
                ImplValDefContentKind::NativeFn(_) | ImplValDefContentKind::NativeMethod(_) => {
                    // nothing to do
                }
            }
        }

        Ok(self.hir)
    }
}
