use std::collections::{HashMap, HashSet, hash_map::Entry};

mod context;
pub(crate) mod types;

use biwac_name_resolver::{
    BlockExpr, BlockStmt, Callee, Expr, ExprVal, Literal, ModSym, PkgSymMap, Primary,
    ResolvedIdent, Stmt,
};
use biwac_parser::{BinOperator, Ident, UnOperator};

use crate::{
    FnDefContent, Sym, TyError, TyInfo, TyResult, TypedPkg,
    inferrer::{
        context::{PkgTyCtx, SymTy, TyCtx},
        types::{FnTy, Ty, TyVar},
    },
};

impl<'pctx> TyCtx<'pctx> {
    fn apply(&self, t: Ty) -> Ty {
        match t {
            Ty::Var(v) => {
                if let Some(t2) = self.substitutions.get(&v) {
                    self.apply(t2.clone())
                } else {
                    Ty::Var(v)
                }
            }
            Ty::Fn(f) => Ty::Fn(FnTy {
                args: f.args.into_iter().map(|a| self.apply(a)).collect(),
                ret: Box::new(self.apply(*f.ret)),
            }),
            x => x,
        }
    }

    fn unify(&mut self, t1: Ty, t2: Ty) -> TyResult<Ty> {
        let t1 = self.apply(t1);
        let t2 = self.apply(t2);

        // FIXME: inefficient clone to return Err
        match (t1.clone(), t2.clone()) {
            (Ty::Var(v), t) | (t, Ty::Var(v)) => {
                let t = self.apply(t);
                if t == Ty::Var(v) {
                    // TODO: constraintsも含んで正しく等価計算をする関数を定義
                    Ok(t)
                } else if occurs(&v, &t) {
                    Err(TyError::OccursCheckFailed(v, t))
                } else {
                    self.substitutions.insert(v, t.clone());

                    Ok(t)
                }
            }
            (Ty::Fn(f1), Ty::Fn(f2)) => {
                if f1.args.len() != f2.args.len() {
                    Err(TyError::FnArgLenMismatched(f1, f2))
                } else {
                    let args = f1
                        .args
                        .into_iter()
                        .zip(f2.args.into_iter())
                        .map(|(a1, a2)| self.unify(a1, a2))
                        .collect::<TyResult<_>>()?;

                    let ret = Box::new(self.unify(*f1.ret, *f2.ret)?);

                    Ok(Ty::Fn(FnTy { args, ret }))
                }
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
                            Ty::Var(_) | Ty::Int | Ty::Float => Ok(ty),
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
                        Ty::Var(_) | Ty::Int | Ty::Float => Ok(ty),
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
                        Ty::Var(_) | Ty::Int | Ty::Float => Ok(Ty::Bool),
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
                        Ty::Var(_) | Ty::Int | Ty::Float | Ty::Bool => Ok(Ty::Bool),
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

                    match self.pctx.syms.get(&s.id).unwrap() {
                        SymTy::Struct(sty) => {
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
                            let mut member_ids = sty
                                .members
                                .keys()
                                .map(|m| m.as_str())
                                .collect::<HashSet<&str>>();

                            for (id, (ident, expr)) in &members {
                                if let Some(ty) = sty.members.get(*id).cloned() {
                                    let expr_ty = self.infer_expr(expr)?;
                                    self.unify(ty, expr_ty)?;
                                    // メンバを取り除いていく
                                    member_ids.remove(id);
                                } else {
                                    return Err(TyError::StructLiteralAssignToInexsistentMember {
                                        id: s.id.clone(),
                                        member: Box::new(ident.to_owned().clone()),
                                    });
                                }
                            }

                            if member_ids.is_empty() {
                                Ok(Ty::Struct(s.id.clone()))
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
                        SymTy::Fn(_) => Err(TyError::SymbolNotAStruct {
                            id: s.id.clone(),
                            span: primary.span(),
                        }),
                    }
                }
            },
            Primary::Variable(v) => {
                match v.id {
                    ResolvedIdent::Abs(_) => todo!(),
                    ResolvedIdent::Var(var_id) => {
                        // 名前解決済みなので存在は保証されている
                        Ok(self.vars.get(&var_id).unwrap().clone())
                    }
                }
            }
            Primary::FnCall(c) => match &c.callee {
                Callee::Abs(id) => match self.pctx.syms.get(id) {
                    Some(SymTy::Fn(f)) => {
                        let callee_ret = (f.ret).clone();

                        let args = c
                            .args
                            .iter()
                            .map(|a| self.infer_expr(a))
                            .collect::<Result<_, _>>()?;
                        let ret = self.fresh();

                        self.unify(
                            Ty::Fn(f.clone()),
                            Ty::Fn(FnTy {
                                args,
                                ret: Box::new(ret),
                            }),
                        )?;

                        Ok(*callee_ret)
                    }
                    Some(SymTy::Struct(_)) => Err(TyError::SymbolNotCallable {
                        id: id.clone(),
                        caller: c.span.clone(),
                    }),
                    None => {
                        panic!("compiler bug: symbol not found")
                    }
                },
                Callee::Var(v) => {
                    // 変数は名前解決済みであるため、先に型推論されているはず
                    // TODO: callee を式に対応させる
                    let f = self.vars.get(v).unwrap().clone();
                    // let f = self.infer_expr(&expr)?;

                    let args = c
                        .args
                        .iter()
                        .map(|a| self.infer_expr(a))
                        .collect::<Result<_, _>>()?;

                    let ret = self.fresh();
                    self.unify(
                        f,
                        Ty::Fn(FnTy {
                            args,
                            ret: Box::new(ret.clone()),
                        }),
                    )?;

                    Ok(ret)
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
                    Ty::Var(_) => Err(TyError::InsufficientContext),
                    Ty::Struct(s) => match self.pctx.syms.get(&s).unwrap() {
                        SymTy::Struct(sty) => sty
                            .members
                            .get(&m.member.id)
                            .ok_or(TyError::StructNotHasMember {
                                id: s.clone(),
                                access: Box::new(m.clone()),
                            })
                            .cloned(),
                        SymTy::Fn(_) => Err(TyError::SymbolNotHasMember {
                            id: s.clone(),
                            access: Box::new(m.clone()),
                        }),
                    },
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

                    then_ty.min(&els_ty)
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
        Ty::Var(v2) => v == v2,
        Ty::Fn(f) => f.args.iter().any(|a| occurs(v, a)) || occurs(v, &f.ret),
        _ => false,
    }
}

impl Ty {
    fn min(&self, other: &Self) -> TyResult<Self> {
        match (self, other) {
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
}

pub fn infer(pkg: PkgSymMap) -> TyResult<TypedPkg> {
    let pctx = PkgTyCtx::new(&pkg)?;
    let mut syms = HashMap::new();

    for (id, sym) in pkg.syms {
        match sym {
            ModSym::FnDef(f) => {
                // 戻り値の型を文脈に記録
                let rty = if let Some(typ) = &f.rtype {
                    typ.clone().into()
                } else {
                    Ty::Void
                };
                let mut fctx = TyCtx::new(&pctx, rty);

                // 引数を決定済みの型として文脈に記録
                for arg in &f.args {
                    fctx.vars.insert(
                        arg.id,
                        f.vars.get(&arg.id).unwrap().typ.clone().unwrap().into(),
                    );
                }

                // 式で終わっている場合、その式の型が戻り値の型と一致することを検査すれば良い
                // 文のみの場合、最後の文のすべての分岐でreturn文があり、正しい型を返していることを検査する必要がある
                // 文は
                // - 基本的にVoidを返すものとし、
                // - return文はその式の型、
                // - 分岐文はすべての分岐で一致すればその型、そうでなければVoidとする
                // これにより、最後の文の型の一致を検査可能になる
                // また、早期returnの型を検査するために、TyCtxに戻り値の型を含める
                let mut stmt_last_ty = Ty::Void;
                for stmt in &f.stmts {
                    stmt_last_ty = fctx.infer_stmt(stmt)?;
                }

                // 最後の式があれば検査
                let rty = if let Some(expr) = &f.expr {
                    fctx.infer_expr(expr)?
                } else {
                    stmt_last_ty
                };

                // 戻り値の型の一致を検査
                fctx.unify(rty, fctx.rty.clone())?;

                // ---- 以降は結果の組み立て ----
                let mut vars = HashMap::new();
                for (id, ty) in fctx.vars {
                    let ty = match ty {
                        Ty::Var(tv) => fctx
                            .substitutions
                            .get(&tv)
                            .expect("not found, error!!!!!")
                            .clone(),
                        x => x,
                    };
                    vars.insert(id, ty);
                }

                let mut exprs = HashMap::new();
                for (id, ty) in fctx.exprs {
                    let ty = match ty {
                        Ty::Var(tv) => fctx
                            .substitutions
                            .get(&tv)
                            .expect("not found, error!!!!!")
                            .clone(),
                        x => x,
                    };
                    exprs.insert(id, ty);
                }

                syms.insert(
                    id,
                    Sym::FnDef(FnDefContent {
                        args: f.args,
                        stmts: f.stmts,
                        expr: f.expr,
                        rtype: f.rtype.map(|typ| typ.into()),
                        vars: f.vars,
                        ty_info: TyInfo { vars, exprs },
                    }),
                );
            }
            ModSym::VarDecl(v) => {
                syms.insert(id, Sym::VarDecl(v));
            }
            ModSym::TypeDef(t) => {
                syms.insert(id, Sym::TypeDef(t));
            }
        }
    }

    Ok(TypedPkg { syms })
}
