use std::collections::{HashMap, HashSet, hash_map::Entry};

pub(crate) mod context;

use biwac_ast::{BinOperator, UnOperator};
use biwac_base::InternedIdent;
use biwac_hir::{
    AssocValDefKind, BlockExpr, BlockStmt, Callee, DefinedTy, Expr, ExprId, ExprVal, FnBody,
    FnSignature, FnTy, Hir, Ident, InferTy, Literal, MemberAccess, Pattern, PatternFields, Primary,
    ResolvedVariant, Stmt, StructLiteral, Ty, TyDefKind, TyKind, TyVar, ValDefKind, VarIdKind,
    VariantCtor, VariantCtorFields,
};
use biwac_lang_item::LangItem;
use biwac_span::{GenDefId, LocalGenDefId, Span, TyDefId, VarId};

use crate::{
    TyCtx, TyError, TyErrorReport, TyResult,
    inferrer::context::{FnTyCtx, TyInfo},
};

#[derive(Debug, Default)]
struct CallCtx {
    gen_assigns: HashMap<LocalGenDefId, Ty>,
}

#[derive(Debug, Default)]
struct DefinedTyCtx {
    gen_assigns: HashMap<GenDefId, Ty>,
}

impl<'tctx, 'a> FnTyCtx<'tctx, 'a> {
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

    /// 推論が終わった後に、記録した型から型変数を取り除く。
    ///
    /// 推論の途中で使う [`Self::apply`] と違い、
    /// 定義型のジェネリック引数の中まで潜る。
    /// また、記録を書き換えるだけなので型変数を新しく作らない。
    ///
    /// 型は式を推論した時点で記録されるが、その時点ではまだ確定していないことがある。
    ///
    /// ```biwa
    /// let aa = Pair::new(x, pair.y()).add();
    ///                       ^^^^^^^^ ここの型が決まるのは Pair::new の単一化の後
    /// ```
    fn resolve_ty(&self, t: &Ty) -> Ty {
        let kind = match &t.kind {
            TyKind::Infer(InferTy::Var(v)) => match self.substitutions.get(v) {
                Some(t2) => return Ty::new(self.resolve_ty(t2).kind, t.span.clone()),
                None => t.kind.clone(),
            },
            TyKind::Defined(dt) => TyKind::Defined(DefinedTy {
                def_id: dt.def_id,
                genargs: dt.genargs.iter().map(|g| self.resolve_ty(g)).collect(),
            }),
            TyKind::Fn(fty) => TyKind::Fn(FnTy {
                args: fty.args.iter().map(|a| self.resolve_ty(a)).collect(),
                rty: Box::new(self.resolve_ty(&fty.rty)),
                genargs: fty.genargs.clone(),
            }),
            _ => t.kind.clone(),
        };
        Ty::new(kind, t.span.clone())
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
                        .zip(fty2.args)
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
                if defined_ty1.def_id == defined_ty2.def_id {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs)
                            .map(|(g1, g2)| {
                                let g1_span = g1.span.clone();
                                Ok(Ty::new(self.unify(g1, g2)?, g1_span))
                            })
                            .collect::<TyResult<_>>()?;

                        Ok(TyKind::Defined(DefinedTy {
                            def_id: defined_ty1.def_id,
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
                // Ty::Gen(GenDefId) は型定義しにしか現れない
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

    /// 呼び出し位置で確定したジェネリック型への割り当てを記録する。
    ///
    /// 単相化するターゲットが必要とする情報で、
    /// 単一化が終わると [`CallCtx`] ごと捨てられてしまうのでここで拾っておく。
    ///
    /// 載るのは
    ///
    ///  - 引数とレシーバの単一化で確定したもの
    ///  - 戻り値にしか現れず、この時点では型変数のままのもの
    ///    (推論の最後に `resolve_ty` が解く)
    ///
    /// の両方である。
    fn record_call_genargs(
        &mut self,
        expr_id: Option<ExprId>,
        assigns: HashMap<LocalGenDefId, Ty>,
    ) {
        let Some(expr_id) = expr_id else {
            return;
        };
        if assigns.is_empty() {
            return;
        }

        let mut assigns: Vec<(LocalGenDefId, Ty)> = assigns
            .into_iter()
            .map(|(lgid, ty)| (lgid, self.apply_ty(ty)))
            .collect();
        // 走査順を固定する。ビルドの決定論のため。
        assigns.sort_by_key(|(lgid, _)| lgid.value());

        self.call_genargs.insert(expr_id, assigns);
    }

    // callee の型を具体化する
    //
    // まだ割り当ての無い callee 側の LocGen には、ここで型変数を割り当てる。
    // callee_ty をそのまま返すと、呼び出し先で宣言されたジェネリック型が
    // 呼び出し側の代入表に流れ込む。
    // 代入表は関数本体を通して生き続けるので、たとえば
    //
    //   let v = Vec::new();  // v: Vec[?1]
    //   v.push(a);           // ?1 := T@push が代入表に残る
    //   v.push(b);           // T@push と Image がぶつかる
    //
    // のように、以降その変数の型が二度と具体化されなくなる。
    // 型変数にしておけば、この呼び出しの中で `Image` に解かれ、
    // 呼び出し側にも `Vec[Image]` として伝わる。
    fn call_embody(&mut self, callee_ty: TyKind, ctx: &mut CallCtx) -> TyKind {
        match callee_ty {
            TyKind::LocGen(lgid) => {
                if let Some(ty) = ctx.gen_assigns.get(&lgid) {
                    ty.kind.clone()
                } else {
                    let assigned = Ty::new(self.fresh(), Span::dummy());
                    ctx.gen_assigns.insert(lgid, assigned.clone());
                    assigned.kind
                }
            }
            TyKind::Defined(defined_ty) => TyKind::Defined(DefinedTy {
                def_id: defined_ty.def_id,
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
            // callee_fty.genargs は 関数定義側でのVec<LocalGenDefId>が記録されており、
            // caller_fty.genargs は 空(vec![]) であるものとする
            // また、caller に現れる LocalGenDefId は impl block やその関数で宣言された
            // ジェネリック型であり、
            // callee に現れる LocalGenDefId とは別であるので注意が必要
            (TyKind::Fn(callee_fty), TyKind::Fn(caller_fty)) => {
                if callee_fty.args.len() != caller_fty.args.len() {
                    Err(TyError::FnArgLenMismatched(callee_fty, caller_fty))
                } else {
                    // LocalGenDefId -> Ty の割り当てが計算できるため、
                    // それによりできるだけ具体の型を計算して返す
                    let args = callee_fty
                        .args
                        .into_iter()
                        .zip(caller_fty.args)
                        .map(|(a1, a2)| {
                            let a2_span = a2.span.clone();
                            Ok(Ty::new(self.call_unify(a1, a2, ctx)?, a2_span))
                        })
                        .collect::<TyResult<_>>()?;

                    let rty = Ty::new(
                        self.call_unify(*callee_fty.rty, *caller_fty.rty, ctx)?,
                        caller_ty.span,
                    );
                    // genargs に登場する LocalGenDefId が必ず引数または戻り値に現れるという前提のもと、
                    // この時点で gen_assigns にはすべての LocalGenDefId に対する Ty
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
                if defined_ty1.def_id == defined_ty2.def_id {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs)
                            .map(|(g1, g2)| {
                                let g2_span = g2.span.clone();
                                Ok(Ty::new(self.call_unify(g1, g2, ctx)?, g2_span))
                            })
                            .collect::<TyResult<_>>()?;

                        Ok(TyKind::Defined(DefinedTy {
                            def_id: defined_ty1.def_id,
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
                // Ty::Gen(GenDefId) は型定義にしか現れない
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
                        .zip(fty2.args)
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
                if defined_ty1.def_id == defined_ty2.def_id {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs)
                            .map(|(g1, g2)| {
                                let g1_span = g1.span.clone();
                                Ok(Ty::new(self.defined_ty_unify(g1, g2, ctx)?, g1_span))
                            })
                            .collect::<TyResult<_>>()?;

                        Ok(TyKind::Defined(DefinedTy {
                            def_id: defined_ty1.def_id,
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
            ExprVal::Primary(primary) => self.infer_primary_expr(Some(expr.id), primary),
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
                                Ok(Ty::new(ty.kind, expr.span()))
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
                            Ok(Ty::new(tk, expr.span()))
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
                            Ok(Ty::new(TyKind::Bool, expr.span()))
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
                            Ok(Ty::new(TyKind::Bool, expr.span()))
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

    fn infer_struct_literal(
        &mut self,
        def_id: &TyDefId,
        struct_literal: &StructLiteral,
    ) -> TyResult<Ty> {
        match self.tctx.get_type_definition(def_id).unwrap() {
            TyDefKind::Struct(struct_) => {
                let mut members = HashMap::<InternedIdent, (&Ident, &Expr)>::new();
                for (ident, expr) in &struct_literal.members {
                    match members.entry(ident.id) {
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
                    .cloned()
                    .collect::<HashSet<InternedIdent>>();

                let mut dtctx = DefinedTyCtx::default();
                for (id, (ident, expr)) in &members {
                    if let Some(definition_ty) = struct_.members.get(id).cloned() {
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
                            def_id: Box::new(*def_id),
                            member: Box::new(ident.to_owned().clone()),
                        });
                    }
                }

                // 定義型のジェネリック引数宣言に登場するジェネリック型が
                // そのメンバなどに必ず使用されることが保証されているなら、
                // dtctx.gen_assigns にはこの時点で必ず GenDefId -> Ty の割り当てがある
                // その割り当てを収集して返す
                let genargs = struct_
                    .genargs
                    .iter()
                    .map(|gid| dtctx.gen_assigns.get(gid).unwrap().clone())
                    .collect();

                if member_ids.is_empty() {
                    Ok(Ty::new(
                        TyKind::Defined(DefinedTy {
                            def_id: *def_id,
                            genargs,
                        }),
                        struct_literal.span.clone(),
                    ))
                } else {
                    // メンバ名の集合に残されているものが、
                    // 初期化されていないメンバ
                    Err(TyError::StructLiteralMemberInsufficient {
                        sliteral: Box::new(struct_literal.clone()),
                        insufficient_members: member_ids.into_iter().collect(),
                    })
                }
            }
            // enum 自体を構造体のようには初期化できない。
            // バリアントを指す `Color::Named { .. }` は lowering で
            // `VariantCtor` になっており、ここには来ない。
            TyDefKind::Enum(enum_def) => Err(TyError::InvalidStructLiteralOnAliasType {
                ty: Box::new(Ty::new(
                    TyKind::Defined(DefinedTy {
                        def_id: *def_id,
                        genargs: vec![
                            Ty::new(
                                TyKind::Infer(InferTy::Unknown),
                                struct_literal.span.clone()
                            );
                            enum_def.genargs.len()
                        ],
                    }),
                    struct_literal.span.clone(),
                )),
                sliteral: Box::new(struct_literal.clone()),
            }),
            TyDefKind::NativeTypeAlias(alias) => {
                // native type alias を構造体のように初期化することは出来ない
                Err(TyError::InvalidStructLiteralOnAliasType {
                    ty: Box::new(Ty::new(
                        TyKind::Defined(DefinedTy {
                            def_id: *def_id,
                            genargs: vec![
                                Ty::new(
                                    TyKind::Infer(InferTy::Unknown),
                                    struct_literal.span.clone() // 正しくないが、エラー表示には使われないため、良しとする
                                );
                                alias.genargs.len()
                            ],
                        }),
                        struct_literal.span.clone(),
                    )),
                    sliteral: Box::new(struct_literal.clone()),
                })
            }
        }
    }

    // ---- enum ----

    /// バリアントの構築を推論する。
    ///
    /// 宣言されたフィールドの型と実引数を順に単一化して、
    /// enum のジェネリック引数を決める。構造体リテラルと同じ考え方である。
    fn infer_variant_ctor(&mut self, ctor: &VariantCtor) -> TyResult<Ty> {
        let (owner, variant) = self
            .tctx
            .get_variant(&ctor.variant)
            .expect("compiler bug: variant not found for a resolved VariantDefId");

        if variant.shape != ctor.shape {
            return Err(TyError::VariantShapeMismatched {
                declared: variant.shape,
                found: ctor.shape,
                span: ctor.span.clone(),
            });
        }

        // codegen が enum の型注釈を出すので、外部パッケージなら import が要る。
        self.tctx
            .hir
            .deps_recorder
            .borrow_mut()
            .depends_on_ty(&Ty::new(
                TyKind::Defined(DefinedTy {
                    def_id: owner.enum_def_id,
                    genargs: Vec::new(),
                }),
                ctor.span.clone(),
            ));

        // 宣言順に (宣言された型, 実引数) を並べる。
        let pairs: Vec<(&Ty, &Expr)> = match &ctor.fields {
            VariantCtorFields::Unit => Vec::new(),
            VariantCtorFields::Positional(args) => {
                if args.len() != variant.fields.len() {
                    return Err(TyError::VariantFieldCountMismatched {
                        expected: variant.fields.len(),
                        found: args.len(),
                        span: ctor.span.clone(),
                    });
                }
                variant
                    .fields
                    .iter()
                    .zip(args)
                    .map(|((_, ty), arg)| (ty, arg))
                    .collect()
            }
            VariantCtorFields::Named(args) => {
                let mut given: HashMap<InternedIdent, &Expr> = HashMap::new();
                for (ident, expr) in args {
                    if !variant.fields.iter().any(|(f, _)| f.id == ident.id) {
                        return Err(TyError::VariantFieldNotFound {
                            field: Box::new(ident.clone()),
                        });
                    }
                    if given.insert(ident.id, expr).is_some() {
                        return Err(TyError::StructLiteralMemberConfliced {
                            member1: Box::new(ident.clone()),
                            member2: Box::new(ident.clone()),
                        });
                    }
                }

                let missing: Vec<InternedIdent> = variant
                    .fields
                    .iter()
                    .filter(|(f, _)| !given.contains_key(&f.id))
                    .map(|(f, _)| f.id)
                    .collect();
                if !missing.is_empty() {
                    return Err(TyError::VariantFieldInsufficient {
                        missing,
                        span: ctor.span.clone(),
                    });
                }

                variant
                    .fields
                    .iter()
                    .map(|(f, ty)| (ty, given[&f.id]))
                    .collect()
            }
        };

        let enum_def = self
            .tctx
            .get_enum_definition(&owner.enum_def_id)
            .expect("compiler bug: enum definition not found")
            .clone();

        let mut dtctx = DefinedTyCtx::default();
        for (declared, arg) in pairs {
            let user_ty = self.infer_expr(arg)?;
            self.defined_ty_unify(
                Ty::new(declared.kind.clone(), arg.span()),
                user_ty,
                &mut dtctx,
            )?;
        }

        // 引数に現れないジェネリック引数は、この時点では決まらない。
        // 型変数を割り当てて、外側の文脈で解かれた結果を最後に拾う
        // (`Option::None` の `T` がこれにあたる)。
        let genargs = enum_def
            .genargs
            .iter()
            .map(|gid| match dtctx.gen_assigns.get(gid) {
                Some(ty) => ty.clone(),
                None => Ty::new(self.fresh(), ctor.span.clone()),
            })
            .collect();

        ctor.resolved
            .set(ResolvedVariant {
                enum_def_id: owner.enum_def_id,
                index: owner.index,
                field_names: variant.fields.iter().map(|(f, _)| f.id).collect(),
            })
            .ok();

        Ok(Ty::new(
            TyKind::Defined(DefinedTy {
                def_id: owner.enum_def_id,
                genargs,
            }),
            ctor.span.clone(),
        ))
    }

    /// アームのパターンを検査し、束縛する変数に型を付ける。
    ///
    /// `scrutinee` は既に `apply_ty` 済みの、対象の enum の型である。
    fn check_pattern(&mut self, pattern: &Pattern, scrutinee: &Ty) -> TyResult<()> {
        let Pattern::Variant(vp) = pattern else {
            // `_` は何も束縛しない。裸の識別子は対象の型そのものを束縛する。
            if let Pattern::Binding(var_id, _) = pattern {
                self.vars.insert(*var_id, scrutinee.clone());
            }
            return Ok(());
        };

        let TyKind::Defined(defined) = &scrutinee.kind else {
            return Err(TyError::MatchOnNonEnum {
                ty: Box::new(scrutinee.clone()),
                span: vp.span.clone(),
            });
        };

        let (owner, variant) = self
            .tctx
            .get_variant(&vp.variant)
            .expect("compiler bug: variant not found for a resolved VariantDefId");

        if owner.enum_def_id != defined.def_id {
            return Err(TyError::VariantOfAnotherEnum {
                ty: Box::new(scrutinee.clone()),
                span: vp.span.clone(),
            });
        }

        if variant.shape != vp.shape {
            return Err(TyError::VariantShapeMismatched {
                declared: variant.shape,
                found: vp.shape,
                span: vp.span.clone(),
            });
        }

        // フィールドの型に現れるジェネリック型を、対象の型引数で置き換える。
        let enum_def = self
            .tctx
            .get_enum_definition(&owner.enum_def_id)
            .expect("compiler bug: enum definition not found")
            .clone();
        let assigns: HashMap<GenDefId, TyKind> = enum_def
            .genargs
            .iter()
            .copied()
            .zip(defined.genargs.iter().map(|t| t.kind.clone()))
            .collect();

        // 宣言順に (フィールド名, 束縛) を並べる。
        let bindings: Vec<(InternedIdent, Option<VarId>)> = match &vp.fields {
            PatternFields::Unit => Vec::new(),
            PatternFields::Positional(binds) => {
                if binds.len() != variant.fields.len() {
                    return Err(TyError::VariantFieldCountMismatched {
                        expected: variant.fields.len(),
                        found: binds.len(),
                        span: vp.span.clone(),
                    });
                }
                variant
                    .fields
                    .iter()
                    .zip(binds)
                    .map(|((f, _), b)| (f.id, b.var_id()))
                    .collect()
            }
            PatternFields::Named(fields) => {
                let mut given: HashMap<InternedIdent, Option<VarId>> = HashMap::new();
                for (ident, bind) in fields {
                    if !variant.fields.iter().any(|(f, _)| f.id == ident.id) {
                        return Err(TyError::VariantFieldNotFound {
                            field: Box::new(ident.clone()),
                        });
                    }
                    given.insert(ident.id, bind.var_id());
                }

                // `..` は入れていないので、すべてのフィールドを書く必要がある。
                let missing: Vec<InternedIdent> = variant
                    .fields
                    .iter()
                    .filter(|(f, _)| !given.contains_key(&f.id))
                    .map(|(f, _)| f.id)
                    .collect();
                if !missing.is_empty() {
                    return Err(TyError::VariantFieldInsufficient {
                        missing,
                        span: vp.span.clone(),
                    });
                }

                variant
                    .fields
                    .iter()
                    .map(|(f, _)| (f.id, given[&f.id]))
                    .collect()
            }
        };

        for ((_, var_id), (_, declared)) in bindings.iter().zip(variant.fields.iter()) {
            if let Some(var_id) = var_id {
                let ty = declared.clone().embody_by_gen_ty_id(&assigns);
                self.vars.insert(*var_id, ty);
            }
        }

        vp.resolved
            .set(ResolvedVariant {
                enum_def_id: owner.enum_def_id,
                index: owner.index,
                field_names: variant.fields.iter().map(|(f, _)| f.id).collect(),
            })
            .ok();

        Ok(())
    }

    /// `match` のアーム全体を検査する。
    ///
    /// 網羅性はネストが無いので集合の被覆判定で済む。
    fn check_match_arms(
        &mut self,
        scrutinee: &Ty,
        patterns: &[&Pattern],
        span: &Span,
    ) -> TyResult<()> {
        let mut covered: HashSet<u32> = HashSet::new();
        let mut has_catch_all = false;

        for pattern in patterns {
            if has_catch_all {
                return Err(TyError::UnreachableMatchArm {
                    span: pattern.span(),
                });
            }

            self.check_pattern(pattern, scrutinee)?;

            match pattern {
                Pattern::Wildcard(_) | Pattern::Binding(_, _) => has_catch_all = true,
                Pattern::Variant(vp) => {
                    let index = vp
                        .resolved
                        .get()
                        .expect("compiler bug: pattern was not resolved")
                        .index;
                    if !covered.insert(index) {
                        return Err(TyError::UnreachableMatchArm {
                            span: pattern.span(),
                        });
                    }
                }
            }
        }

        if has_catch_all {
            return Ok(());
        }

        let TyKind::Defined(defined) = &scrutinee.kind else {
            return Err(TyError::MatchOnNonEnum {
                ty: Box::new(scrutinee.clone()),
                span: span.clone(),
            });
        };
        let Some(enum_def) = self.tctx.get_enum_definition(&defined.def_id) else {
            return Err(TyError::MatchOnNonEnum {
                ty: Box::new(scrutinee.clone()),
                span: span.clone(),
            });
        };

        let missing: Vec<InternedIdent> = enum_def
            .variants
            .iter()
            .enumerate()
            .filter(|(i, _)| !covered.contains(&(*i as u32)))
            .map(|(_, v)| v.name.id)
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(TyError::NonExhaustiveMatch {
                missing,
                span: span.clone(),
            })
        }
    }

    /// `expr_id` はこの primary を包む [`Expr`] の id。
    /// 呼び出し位置のジェネリック引数を記録するのに使う。
    /// 代入文の左辺のように [`Expr`] に包まれていない primary では `None` になる
    /// (左辺に呼び出しは現れないので記録するものが無い)。
    fn infer_primary_expr(&mut self, expr_id: Option<ExprId>, primary: &Primary) -> TyResult<Ty> {
        match primary {
            Primary::Literal(l) => match l {
                Literal::Integer(_) => Ok(Ty::new(TyKind::Int, primary.span())),
                // Literal::Float(_) => Ok(Ty::Float),
                Literal::Bool(_) => Ok(Ty::new(TyKind::Bool, primary.span())),
                // 文字列リテラルの型は lang item `string` が指す型である。
                // コンパイラは std::types::string::String というパスを知らず、
                // std 側が [[lang="string"]] で名乗り出たものを使う。
                Literal::String(_) => self.tctx.lang_item_ty(LangItem::String, primary.span()),
                Literal::Struct(struct_literal) => {
                    self.infer_struct_literal(&struct_literal.tid, struct_literal)
                }
            },
            Primary::Variable(v) => {
                match v.id {
                    VarIdKind::Global(_) => todo!(),
                    VarIdKind::Local(var_id) => {
                        // 名前解決済みなので存在は保証されている
                        let ty = self.vars.get(&var_id).unwrap().clone();

                        // span は宣言位置ではなくこの使用位置にする。
                        // 型が食い違ったときに指すべきなのは、
                        // 変数を宣言した行ではなく渡した行である。
                        Ok(Ty::new(ty.kind, primary.span()))
                    }
                }
            }
            Primary::FnCall(c) => match &c.callee {
                Callee::Fn(def_id) => {
                    // codegen が呼び出しを出力するので、外部パッケージなら import が要る
                    self.tctx
                        .hir
                        .deps_recorder
                        .borrow_mut()
                        .depends_on_val(def_id);

                    // let callee_ty = self.tctx.hir.get_fn_sign(vid).unwrap().as_ty();
                    let callee_ty = self.tctx.get_value_ty(def_id).unwrap();

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
                                rty: Box::new(Ty::new(rty, primary.span())),
                                genargs: vec![],
                            }),
                            primary.span(),
                        ),
                        &mut cctx,
                    )?;

                    let unified_fty = if let TyKind::Fn(fty) = unified_ty {
                        fty
                    } else {
                        panic!("compiler bug: 2 Ty::Fn unification must be Ty::Fn")
                    };

                    // 戻り値にしか現れないジェネリック型は、この時点ではまだ決まっていない。
                    // 型変数を割り当てておき、外側の文脈で解かれた結果を
                    // 推論の最後 (`resolve_ty`) に拾う。
                    let mut subst = cctx.gen_assigns.clone();
                    let rty = self.fresh_loc_gen_ty(*unified_fty.rty, &mut subst);
                    self.record_call_genargs(expr_id, subst);

                    Ok(rty)
                }
                Callee::AssocFn { def_id, self_ty } => {
                    // 同上
                    self.tctx
                        .hir
                        .deps_recorder
                        .borrow_mut()
                        .depends_on_val(def_id);

                    let callee_sign = self.tctx.get_value_signature(def_id).unwrap();

                    let mut args = c
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<TyResult<Vec<_>>>()?;
                    let mut callee_args: Vec<Ty> =
                        callee_sign.args.iter().map(|a| a.ty.clone()).collect();

                    // 呼び出し位置に書かれた型を、レシーバと同じように
                    // 第 1 引数として単一化に混ぜる。
                    //
                    // これで `type CharacterBiwa = Character[BiwaCharacterProps];` の
                    // `CharacterBiwa::new(..)` が `P := BiwaCharacterProps` を決められる。
                    // 引数からしか決まらなかったものが、書かれた型からも決まるようになる。
                    if let Some(impl_self_ty) = &callee_sign.impl_self_ty
                        && call_site_self_ty_is_usable(impl_self_ty, self_ty)
                    {
                        callee_args.insert(0, impl_self_ty.clone());
                        args.insert(0, self_ty.clone());
                    }

                    let callee_ty = Ty::new(
                        TyKind::Fn(FnTy {
                            args: callee_args,
                            rty: Box::new(callee_sign.rty.clone()),
                            genargs: callee_sign.genargs.iter().map(|(_, g)| *g).collect(),
                        }),
                        callee_sign.span.clone(),
                    );

                    let rty = self.fresh();

                    let mut cctx = CallCtx::default();
                    let unified_ty = self.call_unify(
                        callee_ty,
                        Ty::new(
                            TyKind::Fn(FnTy {
                                args,
                                rty: Box::new(Ty::new(rty, primary.span())),
                                genargs: vec![],
                            }),
                            primary.span(),
                        ),
                        &mut cctx,
                    )?;

                    let unified_fty = if let TyKind::Fn(fty) = unified_ty {
                        fty
                    } else {
                        panic!("compiler bug: 2 Ty::Fn unification must be Ty::Fn")
                    };

                    // 同上
                    let mut subst = cctx.gen_assigns.clone();
                    let rty = self.fresh_loc_gen_ty(*unified_fty.rty, &mut subst);
                    self.record_call_genargs(expr_id, subst);

                    Ok(rty)
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
                    let rty = Ty::new(self.fresh(), primary.span());

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
                            primary.span(),
                        ),
                        &mut cctx,
                    )?;

                    Ok(rty)
                }
            },
            Primary::MemberAccess(m) => {
                let left = self.infer_expr(&m.left)?;

                self.infer_member_access(left, m)
            }
            Primary::VariantCtor(ctor) => self.infer_variant_ctor(ctor),

            Primary::Match(m) => {
                let scrutinee = self.infer_expr(&m.scrutinee)?;
                let scrutinee = self.apply_ty(scrutinee);

                let patterns: Vec<&Pattern> = m.arms.iter().map(|a| &a.pattern).collect();
                self.check_match_arms(&scrutinee, &patterns, &m.span)?;

                // すべてのアームは同じ型を返さなければならない。
                let mut result: Option<Ty> = None;
                for arm in &m.arms {
                    let arm_ty = self.infer_block_expr(&arm.body)?;
                    result = Some(match result {
                        None => arm_ty,
                        Some(prev) => {
                            let span = arm_ty.span.clone();
                            Ty::new(self.unify(prev, arm_ty)?, span)
                        }
                    });
                }

                Ok(result.expect("compiler bug: match expression with no arm"))
            }

            Primary::IfExpr(if_expr) => {
                let cond = self.infer_expr(&if_expr.cond)?;
                // 期待している `Bool` はソースに書かれていないので、
                // 位置は条件式のものを借りる。
                let bool_ty = Ty::new(TyKind::Bool, cond.span.clone());
                self.unify(bool_ty, cond)?;

                // TODO: else if に対応
                let then_ty = self.infer_block_expr(&if_expr.then)?;
                let els_ty = self.infer_block_expr(&if_expr.els)?;

                Ok(Ty::new(self.unify(then_ty, els_ty)?, primary.span()))
            }
            Primary::Block(block) => self.infer_block_expr(block),
            Primary::MethodCall(m) => {
                let left = self.infer_expr(&m.left)?;

                // 左辺値の型のメソッド実装からメソッド名をキーにメソッドを取得
                let def_id = self.tctx.get_method_def_id(&left, &m.method, self.module)?;
                m.def_id.set(def_id).unwrap();

                // 同上
                self.tctx
                    .hir
                    .deps_recorder
                    .borrow_mut()
                    .depends_on_val(&def_id);

                let callee_sign = self.tctx.get_value_signature(&def_id).unwrap();

                let mut args = m
                    .args
                    .iter()
                    .map(|a| self.infer_expr(a))
                    .collect::<TyResult<Vec<_>>>()?;

                // レシーバを第 1 引数として単一化に含める。
                //
                // `FnSignature::as_ty` は self を落とすので、
                // それだけで単一化するとレシーバから決まるジェネリック型
                // (`impl[T] Pair[T, U]` の `T`, `U` など) が確定しないまま残り、
                // 呼び出し側の式に呼び先のジェネリック型が漏れてしまう。
                let mut callee_args: Vec<Ty> =
                    callee_sign.args.iter().map(|a| a.ty.clone()).collect();
                if let Some(self_ty) = &callee_sign.self_ty {
                    callee_args.insert(0, self_ty.clone());
                    args.insert(0, left.clone());
                }

                let callee_ty = Ty::new(
                    TyKind::Fn(FnTy {
                        args: callee_args,
                        rty: Box::new(callee_sign.rty.clone()),
                        genargs: callee_sign.genargs.iter().map(|(_, g)| *g).collect(),
                    }),
                    callee_sign.span.clone(),
                );

                let rty = Ty::new(self.fresh(), primary.span());

                // NOTE: caller は genargs は 空 vec![] でよい
                // unify で計算する
                let mut cctx = CallCtx::default();
                let caller_ty = Ty::new(
                    TyKind::Fn(FnTy {
                        args,
                        rty: Box::new(rty.clone()),
                        genargs: vec![],
                    }),
                    primary.span(),
                );
                let unified_ty = self.call_unify(callee_ty, caller_ty, &mut cctx)?;

                let unified_fty = if let TyKind::Fn(fty) = unified_ty {
                    fty
                } else {
                    panic!("compiler bug: 2 Ty::Fn unification must be Ty::Fn")
                };

                // 同上。
                let mut subst = cctx.gen_assigns.clone();
                let rty = self.fresh_loc_gen_ty(*unified_fty.rty, &mut subst);
                self.record_call_genargs(expr_id, subst);

                Ok(rty)
            }
        }
    }

    // 関数や型などの定義に存在するジェネリック型について、
    // 呼び出して使用する際に未確定の場合、
    // 推論が必要なものとして型変数を割り当てる
    #[allow(dead_code)]
    fn fresh_gen_ty(&mut self, ty: Ty, subst: &mut HashMap<LocalGenDefId, Ty>) -> Ty {
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
                        .map(|a| self.fresh_loc_gen_ty(a, subst))
                        .collect(),
                    rty: Box::new(self.fresh_loc_gen_ty(*fty.rty, subst)),
                    genargs: fty.genargs,
                }),
                ty.span,
            ),
            TyKind::Defined(defined_ty) => Ty::new(
                TyKind::Defined(DefinedTy {
                    def_id: defined_ty.def_id,
                    genargs: defined_ty
                        .genargs
                        .into_iter()
                        .map(|g| self.fresh_loc_gen_ty(g, subst))
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
    //
    // `subst` は「このジェネリック型にこの型変数を割り当てた」という記録である。
    // 同じ LocalGenDefId には必ず同じ型変数を割り当てなければならない。
    // 出現ごとに別の変数を作ると
    //  - 引数と戻り値の両方に `T` が出る関数で両者が繋がらない
    //  - 呼び出し位置でどの `T` がどう決まったのかを後から辿れない
    // ことになり、単相化が必要とする割り当てを記録できない。
    fn fresh_loc_gen_ty(&mut self, ty: Ty, subst: &mut HashMap<LocalGenDefId, Ty>) -> Ty {
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
                        .map(|a| self.fresh_loc_gen_ty(a, subst))
                        .collect(),
                    rty: Box::new(self.fresh_loc_gen_ty(*fty.rty, subst)),
                    genargs: fty.genargs,
                }),
                ty.span,
            ),
            TyKind::Defined(defined_ty) => Ty::new(
                TyKind::Defined(DefinedTy {
                    def_id: defined_ty.def_id,
                    genargs: defined_ty
                        .genargs
                        .into_iter()
                        .map(|g| self.fresh_loc_gen_ty(g, subst))
                        .collect(),
                }),
                ty.span,
            ),
            TyKind::LocGen(lgid) => {
                if let Some(assigned) = subst.get(&lgid) {
                    return Ty::new(assigned.kind.clone(), ty.span);
                }
                let assigned = Ty::new(self.fresh(), ty.span.clone());
                subst.insert(lgid, assigned.clone());
                assigned
            }
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
                match self.tctx.get_type_definition(&defined_ty.def_id).unwrap() {
                    TyDefKind::Struct(struct_) => {
                        let ty = struct_
                            .members
                            .get(&member_access.member.id)
                            .ok_or(TyError::StructNotHasMember {
                                def_id: defined_ty.def_id,
                                access: Box::new(member_access.clone()),
                            })
                            .cloned()?;

                        // NOTE: メンバの型に現れるジェネリック型 TyKind::Gen(GenDefId) を、
                        // ジェネリック引数列の位置から実際の型引数に置き換える。
                        //
                        // メンバの型が `P` そのものとは限らない。
                        // `chara: Character[P]` のように別の型の引数として現れることがあるので、
                        // 型の中まで辿る必要がある。
                        // 置き換え漏れがあると Gen のまま単一化に流れ込んで落ちる。
                        Ok(substitute_struct_gens(
                            &ty,
                            &struct_.genargs,
                            &defined_ty.genargs,
                        ))
                    }
                    // enum のフィールドは `match` でしか取り出せない。
                    // どのバリアントか分からないままメンバを引くことはできない。
                    TyDefKind::Enum(_) => Err(TyError::ExprNotHasMember {
                        ty: Box::new(Ty::new(
                            TyKind::Defined(defined_ty.clone()),
                            left_ty.span.clone(),
                        )),
                        access: Box::new(member_access.clone()),
                    }),
                    TyDefKind::NativeTypeAlias(_) => {
                        // native type alias にはメンバアクセスできない
                        Err(TyError::ExprNotHasMember {
                            ty: Box::new(Ty::new(TyKind::Defined(defined_ty), left_ty.span)),
                            access: Box::new(member_access.clone()),
                        })
                    }
                }
            }
            TyKind::Gen(_) => {
                // TyKind::Gen(GenDefId) は型定義にしか現れないため、
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
                self.unify(bool_ty, cond)?;

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
                    self.unify(self.rty.clone(), rty)?,
                    ret.expr.span(),
                )))
            }
            Stmt::Assign(ass) => {
                match &ass.dst {
                    Primary::Variable(_) | Primary::MemberAccess(_) => {
                        let dst = self.infer_primary_expr(None, &ass.dst)?;
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
            Stmt::Match(m) => {
                let scrutinee = self.infer_expr(&m.scrutinee)?;
                let scrutinee = self.apply_ty(scrutinee);

                let patterns: Vec<&Pattern> = m.arms.iter().map(|a| &a.pattern).collect();
                self.check_match_arms(&scrutinee, &patterns, &m.span)?;

                for arm in &m.arms {
                    self.infer_block_stmt(&arm.body)?;
                }

                Ok(None)
            }

            Stmt::While(while_stmt) => {
                let cond = self.infer_expr(&while_stmt.cond)?;
                let bool_ty = Ty::new(TyKind::Bool, cond.span.clone());
                self.unify(bool_ty, cond)?;

                self.infer_block_stmt(&while_stmt.stmts)?;

                Ok(None)
            }
            // scene 内の novel statement は、
            // コンパイラのみが知っている lang item の関数呼び出しとして扱う。
            // ユーザはこの関数を名前で呼ぶことを想定されていない。
            Stmt::NovelWrite(write) => {
                let msg = self
                    .tctx
                    .lang_item_ty(LangItem::String, write.span.clone())?;
                self.check_novel_call(LangItem::Write, &[msg], &write.span)?;

                Ok(None)
            }
            Stmt::NovelWait(wait) => {
                self.check_novel_call(LangItem::Wait, &[], &wait.span)?;

                Ok(None)
            }
        }
    }

    /// novel statement が展開される先の lang item 関数を、
    /// 実引数の型と突き合わせる。
    ///
    /// 呼び出し式そのものは HIR にまだ存在せず (Stmt::NovelWrite のまま)、
    /// codegen が lang item を引いて実際の呼び出しを生成する。
    /// ここでは「その関数が存在し、想定した引数を取る」ことだけを確かめる。
    fn check_novel_call(&mut self, item: LangItem, args: &[Ty], _span: &Span) -> TyResult<()> {
        let def_id = self.tctx.require_val(item)?;

        // codegen はこの関数の呼び出しを出力するので import が必要になる。
        self.tctx
            .hir
            .deps_recorder
            .borrow_mut()
            .depends_on_val(&def_id);

        // 署名が引けないのは依存メタデータが壊れている場合のみ。
        let callee = self
            .tctx
            .get_value_ty(&def_id)
            .ok_or(TyError::MissingLangItem { item })?;

        // 戻り値には触らない。
        //
        // TypeScript では syscall の記述子を返し、scene がそれを yield して
        // エンジンに制御を渡す。wasm ではエンジン呼び出しがそのまま
        // ホスト関数の呼び出しになるので、返すものが無い。
        // arch ごとに native が選ばれるためシグニチャが違ってよく、
        // ここで戻り値を固定してはいけない。
        //
        // どちらにせよ novel statement は結果を捨てるので、
        // 「その関数が存在し、想定した引数を取る」ことだけを確かめれば足りる。
        let TyKind::Fn(callee_fty) = callee.kind else {
            return Err(TyError::MissingLangItem { item });
        };
        if callee_fty.args.len() != args.len() {
            return Err(TyError::MissingLangItem { item });
        }
        for (declared, actual) in callee_fty.args.into_iter().zip(args.iter().cloned()) {
            self.unify(declared, actual)?;
        }

        Ok(())
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

/// 構造体のメンバの型に現れるジェネリック型を、実際の型引数で置き換える。
///
/// メンバの型が型引数そのもの (`props: P`) とは限らず、
/// `chara: Character[P]` のように別の型の引数として現れることがあるので、
/// 型の中を再帰的に辿る。
fn substitute_struct_gens(ty: &Ty, params: &[GenDefId], args: &[Ty]) -> Ty {
    let kind = match &ty.kind {
        TyKind::Gen(gid) => {
            let idx = params
                .iter()
                .position(|g| g == gid)
                .expect("compiler bug: undefined generic type found in struct member");

            if params.len() != args.len() {
                panic!("compiler bug: generic argument length mismatched")
            }

            return Ty::new(args[idx].kind.clone(), ty.span.clone());
        }
        TyKind::Defined(dt) => TyKind::Defined(DefinedTy {
            def_id: dt.def_id,
            genargs: dt
                .genargs
                .iter()
                .map(|g| substitute_struct_gens(g, params, args))
                .collect(),
        }),
        TyKind::Fn(fty) => TyKind::Fn(FnTy {
            args: fty
                .args
                .iter()
                .map(|a| substitute_struct_gens(a, params, args))
                .collect(),
            rty: Box::new(substitute_struct_gens(&fty.rty, params, args)),
            genargs: fty.genargs.clone(),
        }),
        _ => ty.kind.clone(),
    };

    Ty::new(kind, ty.span.clone())
}

/// 呼び出し位置に書かれた self 型を単一化に使ってよいか。
///
/// 使えないのは、書かれた型が**まだ型引数を伴っていない**場合である。
/// 呼び出し位置に型引数を書く構文が無いので、
///
/// ```text
/// Character::new(..)   // self_ty = Character (型引数なし)
/// Vec::new()           // self_ty = Vec       (型引数なし)
/// ```
///
/// のように、エイリアスを経由しないと `genargs` は空のままになる。
/// これをシグネチャ側の `Character[P]` と単一化しようとすると
/// 型引数の個数が合わずに落ちるので、そのときは混ぜずに
/// 引数から推論する従来どおりの動きにする。
///
/// また `type PairIntT[T] = Pair[Int, T];` のように
/// エイリアス自身が型引数を取る場合、展開しても `Gen` が残る。
/// `Gen` は型定義の中にしか現れてはいけないので、これも除く。
fn call_site_self_ty_is_usable(sign_self_ty: &Ty, call_site_self_ty: &Ty) -> bool {
    let (TyKind::Defined(sign), TyKind::Defined(site)) =
        (&sign_self_ty.kind, &call_site_self_ty.kind)
    else {
        return false;
    };

    sign.def_id == site.def_id
        && sign.genargs.len() == site.genargs.len()
        && !contains_gen(&call_site_self_ty.kind)
}

fn contains_gen(tk: &TyKind) -> bool {
    match tk {
        TyKind::Gen(_) => true,
        TyKind::Defined(defined_ty) => defined_ty.genargs.iter().any(|g| contains_gen(&g.kind)),
        TyKind::Fn(fty) => {
            fty.args.iter().any(|a| contains_gen(&a.kind)) || contains_gen(&fty.rty.kind)
        }
        _ => false,
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

impl<'a> TyCtx<'a> {
    fn infer_fn_body(&self, fn_body: &FnBody, fn_signature: &FnSignature) -> TyResult<TyInfo> {
        // trait 越しのメソッド解決は「その関数が置かれているモジュールで
        // どの trait が import されているか」で決まるので、
        // シグニチャの span からモジュールを引いて持ち回る。
        let mut fctx = FnTyCtx::new(self, fn_signature.rty.clone(), fn_signature.span.module());

        // シグネチャに現れる型は codegen が型注釈として出力するため、
        // 外部パッケージのものは import が必要になる。
        {
            let mut deps = self.hir.deps_recorder.borrow_mut();
            if let Some(ty) = &fn_signature.self_ty {
                deps.depends_on_ty(ty);
            }
            for arg in &fn_signature.args {
                deps.depends_on_ty(&arg.ty);
            }
            deps.depends_on_ty(&fn_signature.rty);
        }

        if let Some(ty) = &fn_signature.self_ty {
            fctx.vars.insert(VarId::SELF_VARIABLE, ty.clone());
        }
        // 引数を決定済みの型として文脈に記録
        // arg_var_ids は第一引数がselfのときはそれも含む
        for arg in &fn_signature.args {
            fctx.vars.insert(arg.var_id, arg.ty.clone());
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
            // 単一化の第 1 引数は「要求されている側」に揃えている。
            // 型が食い違ったときの診断が
            // 「`宣言した戻り値` のはずが `実際の型` だった」の向きになる。
            let rty = fctx.infer_expr(expr)?;
            fctx.unify(fctx.rty.clone(), rty)?;
        } else if let Some(rty) = stmt_last_ty {
            fctx.unify(fctx.rty.clone(), rty)?;
        } else if fctx.rty.kind != TyKind::Void {
            return Err(TyError::ReturnTypeRequired {
                rty: Box::new(fctx.rty),
            });
        };

        // 記録した型に残っている型変数を、最後にまとめて解く。
        let expr_tys = fctx
            .exprs
            .iter()
            .map(|(id, ty)| (*id, fctx.resolve_ty(ty)))
            .collect();
        let var_tys = fctx
            .vars
            .iter()
            .map(|(id, ty)| (*id, fctx.resolve_ty(ty)))
            .collect();
        let call_genargs = fctx
            .call_genargs
            .iter()
            .map(|(id, assigns)| {
                (
                    *id,
                    assigns
                        .iter()
                        .map(|(lgid, ty)| (*lgid, fctx.resolve_ty(ty)))
                        .collect(),
                )
            })
            .collect();

        Ok(TyInfo {
            expr_tys,
            var_tys,
            call_genargs,
        })
    }

    /// 型推論を走らせる。
    ///
    /// エラーは [`TyErrorReport`] に包んで返す。
    /// `TyError` は `Ty` を持つが、型の名前は HIR と依存メタデータからしか
    /// 辿れないので、`self` が生きているここで引いておく必要がある。
    pub fn infer(mut self) -> Result<Hir, Box<TyErrorReport>> {
        match self.infer_inner() {
            Ok(()) => Ok(self.hir),
            Err(e) => Err(Box::new(self.report(e))),
        }
    }

    fn infer_inner(&mut self) -> TyResult<()> {
        // 構造体のメンバの型も codegen が型注釈として出力するため、
        // 外部パッケージのものは import が要る
        // (関数のシグネチャだけを見ていると、メンバにしか現れない型を取りこぼす)。
        {
            let mut deps = self.hir.deps_recorder.borrow_mut();
            for ty_impl in self.hir.tys.values() {
                match &ty_impl.ty_content {
                    Some(TyDefKind::Struct(struct_def)) => {
                        for member_ty in struct_def.members.values() {
                            deps.depends_on_ty(member_ty);
                        }
                    }
                    // enum も同じ。バリアントのフィールドの型が
                    // 生成コードの型注釈に出るので import が要る。
                    Some(TyDefKind::Enum(enum_def)) => {
                        for variant in &enum_def.variants {
                            for (_, ty) in &variant.fields {
                                deps.depends_on_ty(ty);
                            }
                        }
                    }
                    Some(TyDefKind::NativeTypeAlias(_)) | None => {}
                }
            }
        }

        // 普通の関数について
        // 型推論し、その結果を一時的に保持
        let mut fn_ty_infos = vec![];
        for (def_id, val) in &self.hir.vals {
            match &val {
                ValDefKind::Fn(f) => {
                    // 計算した型を記録
                    fn_ty_infos.push((*def_id, self.infer_fn_body(&f.body, &f.signature)?));
                }
                ValDefKind::NovelScene(n) => {
                    // 計算した型を記録
                    fn_ty_infos.push((*def_id, self.infer_fn_body(&n.body, &n.signature)?));
                }
                ValDefKind::Native(_) => {
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
                ValDefKind::Fn(f) => {
                    f.expr_tys = ty_info.expr_tys;
                    f.var_tys = ty_info.var_tys;
                    f.call_genargs = ty_info.call_genargs;
                }
                ValDefKind::NovelScene(n) => {
                    n.expr_tys = ty_info.expr_tys;
                    n.var_tys = ty_info.var_tys;
                    n.call_genargs = ty_info.call_genargs;
                }
                ValDefKind::Native(_) => {
                    // nothing to do
                }
            }
        }

        // ユーザ定義型に対する実装(関連関数、メソッド)について
        // 型推論し、その結果を一時的に保持
        let mut impl_fn_ty_infos = vec![];
        for (def_id, ty_impl) in &self.hir.tys {
            for (val_name, impl_list) in &ty_impl.vals {
                for (impl_valid, impl_) in &impl_list.vals {
                    match &impl_.val_content {
                        AssocValDefKind::Fn(f) => {
                            // 計算した型を記録
                            impl_fn_ty_infos.push((
                                *def_id,
                                *val_name,
                                *impl_valid,
                                self.infer_fn_body(&f.body, &f.signature)?,
                            ));
                        }
                        AssocValDefKind::NativeFn(_) => {
                            // nothing to do
                        }
                    }
                }
            }
        }

        // 推論結果を hir に記録
        for (def_id, val_name, impl_valid, ty_info) in impl_fn_ty_infos {
            let ty_impl = self
                .hir
                .tys
                .get_mut(&def_id)
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
                AssocValDefKind::Fn(f) => {
                    f.expr_tys = ty_info.expr_tys;
                    f.var_tys = ty_info.var_tys;
                    f.call_genargs = ty_info.call_genargs;
                }
                AssocValDefKind::NativeFn(_) => {
                    // nothing to do
                }
            }
        }

        Ok(())
    }
}
