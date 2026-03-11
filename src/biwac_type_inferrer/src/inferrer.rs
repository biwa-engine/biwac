use std::collections::{HashMap, HashSet, hash_map::Entry};

pub(crate) mod context;

use biwac_hir::{
    BlockExpr, BlockStmt, Callee, DefinedTy, Expr, ExprVal, FnTy, GenTyId, Hir, InferTy, Literal,
    LocGenTyId, Primary, Stmt, Ty, TyDefContentKind, TyVar, ValDefContentKind, VarIdKind,
};
use biwac_parser::{BinOperator, Ident, UnOperator};

use crate::{
    TyCtx, TyError, TyResult,
    inferrer::context::{FnTyCtx, TyInfo},
};

#[derive(Default)]
struct CallCtx {
    gen_assigns: HashMap<LocGenTyId, Ty>,
}

#[derive(Default)]
struct DefinedTyCtx {
    gen_assigns: HashMap<GenTyId, Ty>,
}

impl<'tctx> FnTyCtx<'tctx> {
    fn apply(&mut self, t: Ty) -> Ty {
        match t {
            Ty::Infer(i) => match i {
                InferTy::Var(v) => {
                    if let Some(t2) = self.substitutions.get(&v) {
                        self.apply(t2.clone())
                    } else {
                        Ty::Infer(i)
                    }
                }
                InferTy::Unknown => self.fresh(),
            },
            Ty::Fn(f) => Ty::Fn(FnTy {
                args: f.args.into_iter().map(|a| self.apply(a)).collect(),
                rty: Box::new(self.apply(*f.rty)),
                genargs: f.genargs,
            }),
            x => x,
        }
    }

    // TODO:
    //  型の位置を
    //  ExprTy {
    //      ty: Ty,
    //      span: Span,
    //  }
    //  などとして渡すべき
    fn unify(&mut self, t1: Ty, t2: Ty) -> TyResult<Ty> {
        let t1 = self.apply(t1);
        let t2 = self.apply(t2);

        // FIXME: inefficient clone to return Err
        match (t1.clone(), t2.clone()) {
            (Ty::Infer(i), t) | (t, Ty::Infer(i)) => {
                let t = self.apply(t);
                let v = self.ty_var_of_infer_ty(i); // 型変数を取得(なければ新規割り当て)
                if t == Ty::Infer(InferTy::Var(v)) {
                    Ok(t)
                } else if occurs(&v, &t) {
                    Err(TyError::OccursCheckFailed(v, t))
                } else {
                    self.substitutions.insert(v, t.clone());

                    Ok(t)
                }
            }
            //  NOTE: FnTy について
            //  unify() では
            //  ```
            //  let f: (Int, Int -> Int) = (x, y) -> { x + y };
            //  ```
            //  など、単に型が等しい必要がある箇所について検査する
            (Ty::Fn(fty1), Ty::Fn(fty2)) => {
                if fty1.args.len() != fty2.args.len() {
                    Err(TyError::FnArgLenMismatched(fty1, fty2))
                } else if fty1.genargs.len() != fty2.genargs.len() {
                    Err(TyError::FnGenArgLenMismatched(fty1, fty2))
                } else {
                    let args = fty1
                        .args
                        .into_iter()
                        .zip(fty2.args.into_iter())
                        .map(|(a1, a2)| self.unify(a1, a2))
                        .collect::<TyResult<_>>()?;

                    let rty = self.unify(*fty1.rty, *fty2.rty)?;

                    Ok(Ty::Fn(FnTy {
                        args,
                        rty: Box::new(rty),
                        genargs: fty2.genargs,
                        // genargs は caller の値をそのまま使用する
                    }))
                }
            }
            (Ty::Defined(defined_ty1), Ty::Defined(defined_ty2)) => {
                if defined_ty1.tid == defined_ty2.tid {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs.into_iter())
                            .map(|(g1, g2)| self.unify(g1, g2))
                            .collect::<TyResult<_>>()?;

                        Ok(Ty::Defined(DefinedTy {
                            tid: defined_ty1.tid,
                            genargs,
                        }))
                    } else {
                        // それぞれ定義と検査済みであるため等しいはず
                        panic!("compiler bug: generic argument length mismatched")
                    }
                } else {
                    Err(TyError::TypeConfliced(t1, t2))
                }
            }
            (Ty::Gen(_), _) | (_, Ty::Gen(_)) => {
                // Ty::Gen(GenTyId) は型定義しにしか現れない
                panic!("compiler bug: unresolved generic type found")
            }
            (x, y) => {
                if x == y {
                    Ok(x)
                } else {
                    Err(TyError::TypeConfliced(t1, t2))
                }
            }
        }
    }

    fn call_unify(&mut self, callee_ty: Ty, caller_ty: Ty, ctx: &mut CallCtx) -> TyResult<Ty> {
        let callee_ty = self.apply(callee_ty);
        let caller_ty = self.apply(caller_ty);

        match (callee_ty.clone(), caller_ty.clone()) {
            (Ty::Infer(i), t) | (t, Ty::Infer(i)) => {
                let t = self.apply(t);
                let v = self.ty_var_of_infer_ty(i); // 型変数を取得(なければ新規割り当て)
                if t == Ty::Infer(InferTy::Var(v)) {
                    Ok(t)
                } else if occurs(&v, &t) {
                    Err(TyError::OccursCheckFailed(v, t))
                } else {
                    self.substitutions.insert(v, t.clone());

                    Ok(t)
                }
            }
            // NOTE:
            // callee_fty.genargs は 関数定義側でのVec<LocGenTyId>が記録されており、
            // caller_fty.genargs は 空(vec![]) であるものとする
            // また、caller に現れる LocGenTyId は impl block やその関数で宣言された
            // ジェネリック型であり、
            // callee に現れる LocGenTyId とは別であるので注意が必要
            (Ty::Fn(callee_fty), Ty::Fn(caller_fty)) => {
                if callee_fty.args.len() != caller_fty.args.len() {
                    Err(TyError::FnArgLenMismatched(callee_fty, caller_fty))
                } else {
                    // LocGenTyId -> Ty の割り当てが計算できるため、
                    // それによりできるだけ具体の型を計算して返す
                    let args = callee_fty
                        .args
                        .into_iter()
                        .zip(caller_fty.args.into_iter())
                        .map(|(a1, a2)| self.call_unify(a1, a2, ctx))
                        .collect::<TyResult<_>>()?;

                    let rty = self.call_unify(*callee_fty.rty, *caller_fty.rty, ctx)?;
                    // genargs に登場する LocGenTyId が必ず引数または戻り値に現れるという前提のもと、
                    // この時点で gen_assigns にはすべての LocGenTyId に対する Ty
                    // の割り当てが計算されている
                    //
                    // NOTE: FnTyに割り当てを記録しても良い

                    Ok(Ty::Fn(FnTy {
                        args,
                        rty: Box::new(rty),
                        genargs: caller_fty.genargs,
                        // genargs は caller の値をそのまま使用する
                    }))
                }
            }
            (Ty::Defined(defined_ty1), Ty::Defined(defined_ty2)) => {
                if defined_ty1.tid == defined_ty2.tid {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs.into_iter())
                            .map(|(g1, g2)| self.call_unify(g1, g2, ctx))
                            .collect::<TyResult<_>>()?;

                        Ok(Ty::Defined(DefinedTy {
                            tid: defined_ty1.tid,
                            genargs,
                        }))
                    } else {
                        // それぞれ定義と検査済みであるため等しいはず
                        panic!("compiler bug: generic argument length mismatched")
                    }
                } else {
                    Err(TyError::TypeConfliced(callee_ty, caller_ty))
                }
            }
            (Ty::LocGen(lgid), caller_ty) => match ctx.gen_assigns.entry(lgid) {
                Entry::Vacant(e) => {
                    e.insert(caller_ty.clone());

                    Ok(caller_ty)
                }
                Entry::Occupied(mut e) => {
                    // 両方 caller 由来の型であるため、unify() でよい
                    let t = self.unify(e.get().clone(), caller_ty)?;
                    e.insert(t.clone());

                    Ok(t)
                }
            },
            (Ty::Gen(_), _) | (_, Ty::Gen(_)) => {
                // Ty::Gen(GenTyId) は型定義しにしか現れない
                panic!("compiler bug: unresolved generic type found")
            }
            (x, y) => {
                if x == y {
                    // WARN: really?
                    Ok(x)
                } else {
                    Err(TyError::TypeConfliced(callee_ty, caller_ty))
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
    ) -> TyResult<Ty> {
        let definition_ty = self.apply(definition_ty);
        let user_ty = self.apply(user_ty);

        // FIXME: inefficient clone to return Err
        match (definition_ty.clone(), user_ty.clone()) {
            (Ty::Infer(i), t) | (t, Ty::Infer(i)) => {
                let t = self.apply(t);
                let v = self.ty_var_of_infer_ty(i); // 型変数を取得(なければ新規割り当て)
                if t == Ty::Infer(InferTy::Var(v)) {
                    Ok(t)
                } else if occurs(&v, &t) {
                    Err(TyError::OccursCheckFailed(v, t))
                } else {
                    self.substitutions.insert(v, t.clone());

                    Ok(t)
                }
            }
            //  NOTE: FnTy について
            //  defined_ty_unify() では
            //  ```
            //  let f: (Int, Int -> Int) = (x, y) -> { x + y };
            //  ```
            //  など、単に型が等しい必要がある箇所について検査する
            (Ty::Fn(fty1), Ty::Fn(fty2)) => {
                if fty1.args.len() != fty2.args.len() {
                    Err(TyError::FnArgLenMismatched(fty1, fty2))
                } else if fty1.genargs.len() != fty2.genargs.len() {
                    Err(TyError::FnGenArgLenMismatched(fty1, fty2))
                } else {
                    let args = fty1
                        .args
                        .into_iter()
                        .zip(fty2.args.into_iter())
                        .map(|(a1, a2)| self.defined_ty_unify(a1, a2, ctx))
                        .collect::<TyResult<_>>()?;

                    let rty = self.defined_ty_unify(*fty1.rty, *fty2.rty, ctx)?;

                    Ok(Ty::Fn(FnTy {
                        args,
                        rty: Box::new(rty),
                        genargs: fty2.genargs,
                        // genargs は caller の値をそのまま使用する
                    }))
                }
            }
            (Ty::Defined(defined_ty1), Ty::Defined(defined_ty2)) => {
                if defined_ty1.tid == defined_ty2.tid {
                    if defined_ty1.genargs.len() == defined_ty2.genargs.len() {
                        let genargs = defined_ty1
                            .genargs
                            .into_iter()
                            .zip(defined_ty2.genargs.into_iter())
                            .map(|(g1, g2)| self.defined_ty_unify(g1, g2, ctx))
                            .collect::<TyResult<_>>()?;

                        Ok(Ty::Defined(DefinedTy {
                            tid: defined_ty1.tid,
                            genargs,
                        }))
                    } else {
                        // それぞれ定義と検査済みであるため等しいはず
                        panic!("compiler bug: generic argument length mismatched")
                    }
                } else {
                    Err(TyError::TypeConfliced(definition_ty, user_ty))
                }
            }
            (Ty::Gen(gid), user_ty) => match ctx.gen_assigns.entry(gid) {
                Entry::Vacant(e) => {
                    e.insert(user_ty.clone());

                    Ok(user_ty)
                }
                Entry::Occupied(mut e) => {
                    // 両方 user 由来の型であるため、unify() でよい
                    let t = self.unify(e.get().clone(), user_ty)?;
                    e.insert(t.clone());

                    Ok(t)
                }
            },
            (x, y) => {
                if x == y {
                    Ok(x)
                } else {
                    Err(TyError::TypeConfliced(definition_ty, user_ty))
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
                        match ty {
                            Ty::Infer(_) | Ty::Int | Ty::Float => Ok(ty),
                            _ => Err(TyError::InvalidUnaryOperationForType {
                                ty,
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

                    let ty = self.unify(left.clone(), right.clone())?;

                    // NOTE: traitによる演算子オーバーロードが可能になれば
                    // このチェックは要らない
                    match ty {
                        Ty::Infer(_) | Ty::Int | Ty::Float => Ok(ty),
                        _ => Err(TyError::InvalidBinaryOperationForType {
                            ty,
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

                    let ty = self.unify(left.clone(), right.clone())?;

                    // NOTE: traitによる演算子オーバーロードが可能になれば
                    // このチェックは要らない
                    match ty {
                        Ty::Infer(_) | Ty::Int | Ty::Float => Ok(Ty::Bool),
                        _ => Err(TyError::InvalidBinaryOperationForType {
                            ty,
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

                    let ty = self.unify(left.clone(), right.clone())?;

                    // NOTE: traitによる演算子オーバーロードが可能になれば
                    // このチェックは要らない
                    match ty {
                        Ty::Infer(_) | Ty::Int | Ty::Float | Ty::Bool => Ok(Ty::Bool),
                        _ => Err(TyError::InvalidBinaryOperationForType {
                            ty,
                            op: b.op,
                            expr: Box::new(expr.clone()),
                        }),
                    }
                }
            }, // Expr::Lambda(l) => {
               //     let mut local_ctx = self.clone();
               //
               //     let args: Vec<Ty> = l
               //         .args
               //         .iter()
               //         .map(|a| {
               //             let tv = self.fresh();
               //             local_ctx.schemes.insert(
               //                 a.clone(),
               //                 Scheme {
               //                     vars: vec![],
               //                     typ: tv.clone(),
               //                 },
               //             );
               //             tv
               //         })
               //         .collect();
               //
               //     let body = self.infer_expr(&mut local_env, &l.ret)?;
               //
               //     Ok(Ty::Fn(FnTy {
               //         args,
               //         ret: Box::new(body),
               //     }))
               // }
               // Expr::MethodCall(m) => {
               //     let left = self.infer_expr( &m.left)?;
               //
               //     match left.clone() {
               //         Ty::Var(_, _) => Err(TyError::InsufficientContext),
               //         // NOTE: leftの具体の型が判明するならmethodをimplしているかどうか判定できる
               //         Ty::Int | Ty::Float | Ty::Bool | Ty::Fn(_) | Ty::Struct(_) => {
               //             let f = env
               //                 .method_impls
               //                 .get(&left)
               //                 .unwrap()
               //                 .get(&m.method)
               //                 .ok_or(TyError::MethodNotImpled(left, m.method.clone()))
               //                 .cloned()?;
               //
               //             let args = m
               //                 .args
               //                 .iter()
               //                 .map(|a| self.infer_expr(a))
               //                 .collect::<Result<_, _>>()?;
               //
               //             let ret = (*f.ret).clone();
               //
               //             let tv = self.fresh();
               //             self.unify(
               //                 Ty::Fn(f),
               //                 Ty::Fn(FnTy {
               //                     args,
               //                     ret: Box::new(tv),
               //                 }),
               //             )?;
               //
               //             Ok(ret)
               //         }
               //     }
               // }
        };

        self.exprs.insert(expr.id, res.clone()?);

        res
    }

    fn infer_primary_expr(&mut self, primary: &Primary) -> TyResult<Ty> {
        match primary {
            Primary::Literal(l) => match l {
                Literal::Integer(_) => Ok(Ty::Int),
                // Literal::Float(_) => Ok(Ty::Float),
                Literal::Bool(_) => Ok(Ty::Bool),
                Literal::String(_) => todo!(),
                Literal::Struct(s) => {
                    // TODO: member type check

                    match self.tctx.hir.get_type_definition(&s.tid).unwrap() {
                        TyDefContentKind::Struct(struct_) => {
                            let mut members = HashMap::<&str, (&Ident, &Expr)>::new();
                            for (ident, expr) in &s.members {
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
                                if let Some((definition_ty, _)) = struct_.members.get(*id).cloned()
                                {
                                    let user_ty = self.infer_expr(expr)?;
                                    self.defined_ty_unify(definition_ty, user_ty, &mut dtctx)?;
                                    // メンバを取り除いていく
                                    member_ids.remove(id);
                                } else {
                                    return Err(TyError::StructLiteralAssignToInexsistentMember {
                                        tid: Box::new(s.tid.clone()),
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
                                Ok(Ty::Defined(DefinedTy {
                                    tid: s.tid.clone(),
                                    genargs,
                                }))
                            } else {
                                // メンバ名の集合に残されているものが、
                                // 初期化されていないメンバ
                                Err(TyError::StructLiteralMemberInsufficient {
                                    sliteral: Box::new(s.clone()),
                                    insufficient_members: member_ids
                                        .into_iter()
                                        .map(|m| m.to_string())
                                        .collect(),
                                })
                            }
                        }
                    }
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
                    let val = self.tctx.hir.vals.get(vid).unwrap();
                    let callee_fty = match &val {
                        ValDefContentKind::Fn(f) => FnTy::from(&f.signature),
                        ValDefContentKind::Native(f) => FnTy::from(&f.signature),
                    };

                    let callee_rty = callee_fty.rty.clone();

                    let args = c
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<Result<_, _>>()?;
                    let rty = self.fresh();

                    // NOTE: caller は genargs は 空 vec![] でよい
                    // unify で計算する
                    let mut cctx = CallCtx::default();
                    self.call_unify(
                        Ty::Fn(callee_fty),
                        Ty::Fn(FnTy {
                            args,
                            rty: Box::new(rty),
                            genargs: vec![],
                        }),
                        &mut cctx,
                    )?;

                    Ok(*callee_rty)
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
                    let rty = self.fresh();

                    // NOTE: caller は genargs は 空 vec![] でよい
                    // unify で計算する
                    let mut cctx = CallCtx::default();
                    self.call_unify(
                        callee_fty,
                        Ty::Fn(FnTy {
                            args,
                            rty: Box::new(rty.clone()),
                            genargs: vec![],
                        }),
                        &mut cctx,
                    )?;

                    Ok(rty)
                }
                Callee::Assoc(assoc_callee) => {
                    let callee_fty = self.tctx.hir.get_assoc_of_type(assoc_callee)?;

                    let callee_rty = callee_fty.rty.clone();

                    let args = c
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<Result<_, _>>()?;
                    let rty = self.fresh();

                    // NOTE: caller は genargs は 空 vec![] でよい
                    // unify で計算する
                    let mut cctx = CallCtx::default();
                    self.call_unify(
                        Ty::Fn(callee_fty),
                        Ty::Fn(FnTy {
                            args,
                            rty: Box::new(rty),
                            genargs: vec![],
                        }),
                        &mut cctx,
                    )?;

                    Ok(*callee_rty)
                }
            },
            Primary::MemberAccess(m) => {
                let left = self.infer_expr(&m.left)?;

                match left.clone() {
                    Ty::Int | Ty::Float | Ty::Bool | Ty::Fn(_) | Ty::Void => {
                        Err(TyError::ExprNotHasMember {
                            ty: left,
                            access: Box::new(m.clone()),
                        })
                    }
                    Ty::Infer(_) => Err(TyError::InsufficientContext),
                    Ty::Defined(defined_ty) => {
                        match self.tctx.hir.get_type_definition(&defined_ty.tid).unwrap() {
                            TyDefContentKind::Struct(struct_) => {
                                let ty = struct_
                                    .members
                                    .get(&m.member.id)
                                    .ok_or(TyError::StructNotHasMember {
                                        tid: defined_ty.tid.clone(),
                                        access: Box::new(m.clone()),
                                    })
                                    .map(|(ty, _)| ty)
                                    .cloned()?;

                                // NOTE: ジェネリック型 Ty::Gen(GenTyId) の場合、
                                // ジェネリック引数列の位置から GenTyId -> Ty を割り当て
                                if let Ty::Gen(gid) = &ty {
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
                        }
                    }
                    Ty::Gen(_) => {
                        // Ty::Gen(GenTyId) は型定義にしか現れないため、
                        // メンバアクセスの左辺地に現れる場合はバグ
                        panic!("compiler bug: generic type not resolved")
                    }
                    Ty::LocGen(_) => {
                        // impl block や 関数 でローカルに宣言されたジェネリック型
                        // これがメンバアクセス可能性を満たすことは判定できないため
                        // コンパイルエラー
                        Err(TyError::ExprNotHasMember {
                            ty: left,
                            access: Box::new(m.clone()),
                        })
                    }
                }
            }
            Primary::IfExpr(if_expr) => {
                let cond = self.infer_expr(&if_expr.cond)?;
                self.unify(cond, Ty::Bool)?;

                // TODO: else if に対応
                let then_ty = self.infer_block_expr(&if_expr.then)?;
                let els_ty = self.infer_block_expr(&if_expr.els)?;

                self.unify(then_ty, els_ty)
            }
            Primary::Block(block) => self.infer_block_expr(block),
            Primary::MethodCall(m) => {
                let left = self.infer_expr(&m.left)?;

                // 左辺値の型のメソッド実装からメソッド名をキーにメソッドを取得
                if let Some(callee_fty) = self.tctx.hir.get_method_of_type(&left, &m.method)? {
                    let args = m
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<TyResult<_>>()?;

                    let rty = self.fresh();

                    // NOTE: caller は genargs は 空 vec![] でよい
                    // unify で計算する
                    let mut cctx = CallCtx::default();
                    self.call_unify(
                        Ty::Fn(callee_fty),
                        Ty::Fn(FnTy {
                            args,
                            rty: Box::new(rty.clone()),
                            genargs: vec![],
                        }),
                        &mut cctx,
                    )?;

                    Ok(rty)
                } else {
                    Err(TyError::MethodNotImplemented {
                        ty: left,
                        method: Box::new(m.method.clone()),
                    })
                }
            }
        }
    }

    fn infer_block_expr(&mut self, block: &BlockExpr) -> TyResult<Ty> {
        for stmt in &block.stmts {
            self.infer_stmt(stmt)?;
        }

        self.infer_expr(&block.expr)
    }

    fn infer_stmt(&mut self, stmt: &Stmt) -> TyResult<Ty> {
        match stmt {
            Stmt::VarDecl(v) => {
                let ty = self.infer_expr(&v.init)?;
                let ty = self.apply(ty);

                self.vars.insert(v.id, ty);

                Ok(Ty::Void)
            }
            Stmt::If(if_stmt) => {
                let cond = self.infer_expr(&if_stmt.cond)?;
                self.unify(cond, Ty::Bool)?;

                // TODO: else if に対応
                let then_ty = self.infer_block_stmt(&if_stmt.then)?;

                if let Some(els) = &if_stmt.els {
                    let els_ty = self.infer_block_stmt(els)?;

                    min_of_ty(&then_ty, &els_ty)
                } else {
                    // elseが無いということは分岐の1つは何も返さない(= Voidを返す)ということである
                    // したがって、thenの型に関係なく、全体の形はVoid
                    Ok(Ty::Void)
                }
            }
            Stmt::Block(block) => self.infer_block_stmt(block),
            Stmt::Expr(expr) => {
                self.infer_expr(&expr.expr)?;

                Ok(Ty::Void)
            }
            Stmt::Return(ret) => {
                let rty = self.infer_expr(&ret.expr)?;

                self.unify(rty, self.rty.clone())
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

                Ok(Ty::Void)
            }
            Stmt::While(while_stmt) => {
                let cond = self.infer_expr(&while_stmt.cond)?;
                self.unify(cond, Ty::Bool)?;

                self.infer_block_stmt(&while_stmt.stmts)?;

                Ok(Ty::Void)
            }
        }
    }

    fn infer_block_stmt(&mut self, block: &BlockStmt) -> TyResult<Ty> {
        let mut ty = Ty::Void;
        for stmt in &block.stmts {
            let t = self.infer_stmt(stmt)?;

            if t != Ty::Void {
                ty = t;
            }

            // TODO: warn unreachable code after return
        }

        Ok(ty)
    }
}

fn occurs(v: &TyVar, t: &Ty) -> bool {
    match t {
        Ty::Infer(i) => match i {
            InferTy::Var(v2) => v == v2,
            InferTy::Unknown => false, // WARN: really?
        },
        Ty::Fn(f) => f.args.iter().any(|a| occurs(v, a)) || occurs(v, &f.rty),
        _ => false,
    }
}

fn min_of_ty(t1: &Ty, t2: &Ty) -> TyResult<Ty> {
    match (t1, t2) {
        (Ty::Void, _) => Ok(Ty::Void),
        (_, Ty::Void) => Ok(Ty::Void),
        (x, y) => {
            if x == y {
                Ok(x.clone())
            } else {
                Err(TyError::TypeConfliced(x.clone(), y.clone()))
            }
        }
    }
}

impl TyCtx {
    pub fn infer(mut self) -> TyResult<Hir> {
        // 型推論し、その結果を一時的に保持
        let mut fn_ty_infos = vec![];
        for (vid, val) in &self.hir.vals {
            match &val {
                ValDefContentKind::Fn(f) => match &f.body {
                    biwac_hir::Progressive::NotYet(_) => {
                        panic!("compiler bug: name resolution not completed")
                    }
                    biwac_hir::Progressive::Completed(fn_body) => {
                        let mut fctx = FnTyCtx::new(&self, f.signature.rty.clone());

                        // TODO: 引数を決定済みの型として文脈に記録
                        // fn_body.vars から取得
                        //
                        // for arg in &f.signature.args {
                        //     fctx.vars.insert(
                        //         arg.id,
                        //         f.vars.get(&arg.id).unwrap().typ.clone().unwrap().into(),
                        //     );
                        // }

                        // 式で終わっている場合、その式の型が戻り値の型と一致することを検査すれば良い
                        // 文のみの場合、最後の文のすべての分岐でreturn文があり、正しい型を返していることを検査する必要がある
                        // 文は
                        // - 基本的にVoidを返すものとし、
                        // - return文はその式の型、
                        // - 分岐文はすべての分岐で一致すればその型、そうでなければVoidとする
                        // これにより、最後の文の型の一致を検査可能になる
                        // また、早期returnの型を検査するために、TyCtxに戻り値の型を含める
                        let mut stmt_last_ty = Ty::Void;
                        for stmt in &fn_body.stmts {
                            stmt_last_ty = fctx.infer_stmt(stmt)?;
                        }

                        // 最後の式があれば検査
                        let rty = if let Some(expr) = &fn_body.expr {
                            fctx.infer_expr(expr)?
                        } else {
                            stmt_last_ty
                        };

                        // 戻り値の型の一致を検査
                        fctx.unify(rty, fctx.rty.clone())?;

                        // 計算した型を記録
                        fn_ty_infos.push((
                            vid.clone(),
                            TyInfo {
                                expr_tys: fctx.exprs,
                                var_tys: fctx.vars,
                            },
                        ));
                    }
                },
                ValDefContentKind::Native(_) => {
                    // nothing to do
                }
            }
        }

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
                ValDefContentKind::Native(_) => {
                    // nothing to do
                }
            }
        }

        todo!()
    }
}

// pub fn infer(pkg: ResolvedPkg) -> TyResult<TypedPkg> {
//     let pctx = PkgTyCtx::new(&pkg)?;
//     let mut syms = HashMap::new();
//
//     for (id, sym) in pkg.syms {
//         match sym {
//             ModSym::FnDef(f) => {
//                 // 戻り値の型を文脈に記録
//                 let rty = if let Some(typ) = &f.rtype {
//                     typ.clone().into()
//                 } else {
//                     Ty::Void
//                 };
//                 let mut fctx = FnTyCtx::new(&pctx, rty.clone());
//
//                 // 引数を決定済みの型として文脈に記録
//                 for arg in &f.args {
//                     fctx.vars.insert(
//                         arg.id,
//                         f.vars.get(&arg.id).unwrap().typ.clone().unwrap().into(),
//                     );
//                 }
//
//                 // 式で終わっている場合、その式の型が戻り値の型と一致することを検査すれば良い
//                 // 文のみの場合、最後の文のすべての分岐でreturn文があり、正しい型を返していることを検査する必要がある
//                 // 文は
//                 // - 基本的にVoidを返すものとし、
//                 // - return文はその式の型、
//                 // - 分岐文はすべての分岐で一致すればその型、そうでなければVoidとする
//                 // これにより、最後の文の型の一致を検査可能になる
//                 // また、早期returnの型を検査するために、TyCtxに戻り値の型を含める
//                 let mut stmt_last_ty = Ty::Void;
//                 for stmt in &f.stmts {
//                     stmt_last_ty = fctx.infer_stmt(stmt)?;
//                 }
//
//                 // 最後の式があれば検査
//                 let ret_ty = if let Some(expr) = &f.expr {
//                     fctx.infer_expr(expr)?
//                 } else {
//                     stmt_last_ty
//                 };
//
//                 // 戻り値の型の一致を検査
//                 fctx.unify(ret_ty, fctx.rty.clone())?;
//
//                 // ---- 以降は結果の組み立て ----
//                 let mut vars = HashMap::new();
//                 for (id, ty) in fctx.vars {
//                     let ty = match ty {
//                         Ty::Var(tv) => fctx
//                             .substitutions
//                             .get(&tv)
//                             .expect("not found, error!!!!!")
//                             .clone(),
//                         x => x,
//                     };
//                     vars.insert(id, ty);
//                 }
//
//                 let mut exprs = HashMap::new();
//                 for (id, ty) in fctx.exprs {
//                     let ty = match ty {
//                         Ty::Var(tv) => fctx
//                             .substitutions
//                             .get(&tv)
//                             .expect("not found, error!!!!!")
//                             .clone(),
//                         x => x,
//                     };
//                     exprs.insert(id, ty);
//                 }
//
//                 syms.insert(
//                     id,
//                     Sym::FnDef(FnDefContent {
//                         args: f.args,
//                         stmts: f.stmts,
//                         expr: f.expr,
//                         rty,
//                         vars: f.vars,
//                         ty_info: TyInfo { vars, exprs },
//                     }),
//                 );
//             }
//             ModSym::NativeFnDef(f) => {
//                 syms.insert(
//                     id,
//                     Sym::NativeFnDef(NativeFnDefContent {
//                         args: f
//                             .args
//                             .into_iter()
//                             .map(|arg| NativeFnArg {
//                                 ty: arg.typ.into(),
//                                 id: arg.id,
//                                 span: arg.span,
//                             })
//                             .collect(),
//                         rty: match f.rtype {
//                             Some(typ) => typ.into(),
//                             None => Ty::Void,
//                         },
//                         native: f.native,
//                         native_span: f.native_span,
//                         span: f.span,
//                     }),
//                 );
//             }
//             ModSym::VarDecl(v) => {
//                 syms.insert(id, Sym::VarDecl(v));
//             }
//             ModSym::TypeDef(t) => {
//                 syms.insert(id, Sym::TypeDef(t));
//             }
//             ModSym::MethodDef(f) => {
//                 // 戻り値の型を文脈に記録
//                 let rty = if let Some(typ) = &f.rtype {
//                     typ.clone().into()
//                 } else {
//                     Ty::Void
//                 };
//                 let mut fctx = TyCtx::new(&pctx, rty.clone());
//
//                 // selfを決定済みの型として文脈に記録
//                 fctx.vars.insert(
//                     f.self_id,
//                     f.vars.get(&f.self_id).unwrap().typ.clone().unwrap().into(),
//                 );
//
//                 // 引数を決定済みの型として文脈に記録
//                 for arg in &f.args {
//                     fctx.vars.insert(
//                         arg.id,
//                         f.vars.get(&arg.id).unwrap().typ.clone().unwrap().into(),
//                     );
//                 }
//
//                 // 式で終わっている場合、その式の型が戻り値の型と一致することを検査すれば良い
//                 // 文のみの場合、最後の文のすべての分岐でreturn文があり、正しい型を返していることを検査する必要がある
//                 // 文は
//                 // - 基本的にVoidを返すものとし、
//                 // - return文はその式の型、
//                 // - 分岐文はすべての分岐で一致すればその型、そうでなければVoidとする
//                 // これにより、最後の文の型の一致を検査可能になる
//                 // また、早期returnの型を検査するために、TyCtxに戻り値の型を含める
//                 let mut stmt_last_ty = Ty::Void;
//                 for stmt in &f.stmts {
//                     stmt_last_ty = fctx.infer_stmt(stmt)?;
//                 }
//
//                 // 最後の式があれば検査
//                 let ret_ty = if let Some(expr) = &f.expr {
//                     fctx.infer_expr(expr)?
//                 } else {
//                     stmt_last_ty
//                 };
//
//                 // 戻り値の型の一致を検査
//                 fctx.unify(ret_ty, fctx.rty.clone())?;
//
//                 // ---- 以降は結果の組み立て ----
//                 let mut vars = HashMap::new();
//                 for (id, ty) in fctx.vars {
//                     let ty = match ty {
//                         Ty::Var(tv) => fctx
//                             .substitutions
//                             .get(&tv)
//                             .expect("not found, error!!!!!")
//                             .clone(),
//                         x => x,
//                     };
//                     vars.insert(id, ty);
//                 }
//
//                 let mut exprs = HashMap::new();
//                 for (id, ty) in fctx.exprs {
//                     let ty = match ty {
//                         Ty::Var(tv) => fctx
//                             .substitutions
//                             .get(&tv)
//                             .expect("not found, error!!!!!")
//                             .clone(),
//                         x => x,
//                     };
//                     exprs.insert(id, ty);
//                 }
//
//                 // メソッドは第一引数がselfである関数に解決される
//                 let mut args = vec![DecledArg { id: f.self_id }];
//                 args.extend(f.args);
//
//                 syms.insert(
//                     id,
//                     Sym::FnDef(FnDefContent {
//                         args,
//                         stmts: f.stmts,
//                         expr: f.expr,
//                         rty,
//                         vars: f.vars,
//                         ty_info: TyInfo { vars, exprs },
//                     }),
//                 );
//             }
//         }
//     }
//
//     Ok(TypedPkg { syms })
// }
