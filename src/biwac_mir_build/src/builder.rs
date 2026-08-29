//! 関数 1 つ分の HIR を CFG に崩す。
//!
//! rustc の `expr_into_dest` と同じく、
//! 「この場所に値を書け」という形 (destination-passing) を主軸にする。
//! `if` 式やブロック式が値を返すのは、これで自然に扱える。
//!
//! 各 `lower_*` は「続きを書くべきブロック」を返す。

use std::collections::HashMap;

use biwac_ast::{BinOperator, UnOperator};
use biwac_hir::{
    BlockExpr, Callee as HirCallee, DecledVar, DefinedTy, Expr, ExprId, ExprVal, FnBody, FnDef,
    FnSignature, Literal, NativeFnDef, NovelSceneDef, Primary, Stmt, Ty, TyKind, VarIdKind,
};
use biwac_lang_item::{LangItem, LangItemTable};
use biwac_mir::{
    BasicBlock, BasicBlockData, BinOp, Body, Callee, Const, GenArgs, Local, LocalDecl, MirItem,
    NativeItem, Operand, Place, Rvalue, StatementKind, StringPool, SwitchTargets, TerminatorKind,
    UnOp,
};
use biwac_span::{Span, ValDefId, VarId};

pub(crate) fn build_fn(
    lang_items: &LangItemTable,
    strings: &mut StringPool,
    def_id: ValDefId,
    f: &FnDef,
) -> MirItem {
    let genargs = f
        .impl_genargs
        .iter()
        .chain(f.signature.genargs.iter())
        .map(|(_, lgid)| *lgid)
        .collect();

    MirItem::Body(
        BodyBuilder::new(
            lang_items,
            strings,
            &f.expr_tys,
            &f.var_tys,
            &f.call_genargs,
            &f.body.vars,
        )
        .build(def_id, &f.signature, &f.body, genargs),
    )
}

pub(crate) fn build_scene(
    lang_items: &LangItemTable,
    strings: &mut StringPool,
    def_id: ValDefId,
    s: &NovelSceneDef,
) -> MirItem {
    // scene も普通の関数と同じ経路で落ちる。
    // 中断は同期的なホスト呼び出しの向こう側の話であり、MIR には現れない。
    let genargs = s.signature.genargs.iter().map(|(_, lgid)| *lgid).collect();

    MirItem::Body(
        BodyBuilder::new(
            lang_items,
            strings,
            &s.expr_tys,
            &s.var_tys,
            &s.call_genargs,
            &s.body.vars,
        )
        .build(def_id, &s.signature, &s.body, genargs),
    )
}

pub(crate) fn build_native_fn(def_id: ValDefId, n: &NativeFnDef) -> MirItem {
    MirItem::Native(NativeItem {
        def_id,
        self_ty: n.signature.self_ty.clone(),
        args: n.signature.args.iter().map(|a| a.ty.clone()).collect(),
        rty: n.signature.rty.clone(),
        genargs: n
            .impl_genargs
            .iter()
            .chain(n.signature.genargs.iter())
            .map(|(_, lgid)| *lgid)
            .collect(),
        native_body: n.native_body.clone(),
        native_span: n.native_span.clone(),
        span: n.span.clone(),
    })
}

struct BodyBuilder<'a> {
    lang_items: &'a LangItemTable,
    strings: &'a mut StringPool,

    expr_tys: &'a HashMap<ExprId, Ty>,
    var_tys: &'a HashMap<VarId, Ty>,
    call_genargs: &'a HashMap<ExprId, GenArgs>,

    locals: Vec<LocalDecl>,
    blocks: Vec<BasicBlockData>,

    /// 関数内で宣言された変数の一覧 (引数と self を含む)。
    vars: &'a HashMap<VarId, DecledVar>,

    /// HIR の変数と MIR の local の対応。
    /// 引数も宣言された変数も同じ表に入る。
    var_map: HashMap<VarId, Local>,
}

impl<'a> BodyBuilder<'a> {
    fn new(
        lang_items: &'a LangItemTable,
        strings: &'a mut StringPool,
        expr_tys: &'a HashMap<ExprId, Ty>,
        var_tys: &'a HashMap<VarId, Ty>,
        call_genargs: &'a HashMap<ExprId, GenArgs>,
        vars: &'a HashMap<VarId, DecledVar>,
    ) -> Self {
        Self {
            lang_items,
            strings,
            expr_tys,
            var_tys,
            call_genargs,
            vars,
            locals: Vec::new(),
            blocks: Vec::new(),
            var_map: HashMap::new(),
        }
    }

    fn build(
        mut self,
        def_id: ValDefId,
        signature: &FnSignature,
        body: &FnBody,
        genargs: Vec<biwac_span::LocalGenDefId>,
    ) -> Body {
        // _0 は戻り値スロット。
        self.new_local(signature.rty.clone(), signature.rty.span.clone());

        // _1 ..= arg_count が引数。メソッドなら _1 が self。
        let mut arg_count = 0;
        if let Some(self_ty) = &signature.self_ty {
            let var_id = body.self_var_id.unwrap_or(VarId::SELF_VARIABLE);
            let ty = self.var_ty(var_id, self_ty);
            let local = self.new_local(ty, self_ty.span.clone());
            self.var_map.insert(var_id, local);
            arg_count += 1;
        }
        for arg in &signature.args {
            let ty = self.var_ty(arg.var_id, &arg.ty);
            let local = self.new_local(ty, arg.id.span.clone());
            self.var_map.insert(arg.var_id, local);
            arg_count += 1;
        }

        let start = self.new_block();
        let bb = self.lower_stmts(start, &body.stmts);

        // 末尾の式があればそれが戻り値。
        // 無ければ Void を返す関数か、すべての経路が return で終わっている。
        let bb = if let Some(expr) = &body.expr {
            let bb = self.lower_expr_into(bb, Place::from_local(Local::RETURN), expr);
            self.terminate(bb, TerminatorKind::Return, expr.span());
            bb
        } else {
            self.terminate(bb, TerminatorKind::Return, signature.span.clone());
            bb
        };
        let _ = bb;

        self.prune_unreachable_blocks();

        Body {
            def_id,
            arg_count,
            locals: self.locals,
            blocks: self.blocks,
            genargs,
            span: signature.span.clone(),
        }
    }

    /// 入口から到達できないブロックを落とす。
    ///
    /// 構築の都合で必ず出る。`return` の後ろには文が続きうるので
    /// 行き先のブロックを開けておくが、実際には続かないことのほうが多い。
    /// 最適化ではなく後片付けなので、パスにはせずここで済ませる。
    fn prune_unreachable_blocks(&mut self) {
        let count = self.blocks.len();
        let mut reachable = vec![false; count];
        let mut stack = vec![BasicBlock::START];
        reachable[0] = true;
        while let Some(bb) = stack.pop() {
            for succ in self.blocks[bb.index()].term.successors() {
                if !reachable[succ.index()] {
                    reachable[succ.index()] = true;
                    stack.push(succ);
                }
            }
        }

        if reachable.iter().all(|r| *r) {
            return;
        }

        // 元の並び順は保つ。番号だけ詰める。
        let mut remap = vec![None; count];
        let mut next = 0u32;
        for (idx, r) in reachable.iter().enumerate() {
            if *r {
                remap[idx] = Some(BasicBlock::new(next));
                next += 1;
            }
        }

        let map = |bb: BasicBlock| {
            remap[bb.index()].expect("compiler bug: reachable block points at a pruned block")
        };

        let mut blocks = Vec::with_capacity(next as usize);
        for (idx, mut block) in std::mem::take(&mut self.blocks).into_iter().enumerate() {
            if !reachable[idx] {
                continue;
            }
            block.term.kind = match block.term.kind {
                TerminatorKind::Goto { target } => TerminatorKind::Goto {
                    target: map(target),
                },
                TerminatorKind::SwitchInt { discr, targets } => {
                    let (values, dests): (Vec<_>, Vec<_>) =
                        targets.iter().map(|(v, t)| (v, map(t))).unzip();
                    TerminatorKind::SwitchInt {
                        discr,
                        targets: SwitchTargets::new(values, dests, map(targets.otherwise())),
                    }
                }
                TerminatorKind::Call {
                    callee,
                    args,
                    dest,
                    target,
                } => TerminatorKind::Call {
                    callee,
                    args,
                    dest,
                    target: map(target),
                },
                other => other,
            };
            blocks.push(block);
        }
        self.blocks = blocks;
    }

    // ---- local / block の管理 ----

    fn new_local(&mut self, ty: Ty, span: Span) -> Local {
        let local = Local::new(self.locals.len() as u32);
        self.locals.push(LocalDecl { ty, span });
        local
    }

    fn new_temp(&mut self, ty: Ty, span: Span) -> Place {
        Place::from_local(self.new_local(ty, span))
    }

    /// 終端子が未確定のブロックを作る。
    ///
    /// 置いておく [`TerminatorKind::Unreachable`] は
    /// 後で [`Self::terminate`] が差し替える。
    /// 差し替えられずに残るのは、`return` の直後のように
    /// 本当に到達しないブロックだけである。
    fn new_block(&mut self) -> BasicBlock {
        let bb = BasicBlock::new(self.blocks.len() as u32);
        self.blocks.push(BasicBlockData::new(
            TerminatorKind::Unreachable.with_span(Span::dummy()),
        ));
        bb
    }

    fn push_assign(&mut self, bb: BasicBlock, dest: Place, rvalue: Rvalue, span: Span) {
        self.blocks[bb.index()]
            .stmts
            .push(StatementKind::Assign(dest, rvalue).with_span(span));
    }

    fn terminate(&mut self, bb: BasicBlock, kind: TerminatorKind, span: Span) {
        self.blocks[bb.index()].term = kind.with_span(span);
    }

    // ---- 型の引き方 ----

    /// 変数の型。推論結果があればそちら、無ければ宣言された型。
    fn var_ty(&self, var_id: VarId, declared: &Ty) -> Ty {
        self.var_tys
            .get(&var_id)
            .cloned()
            .unwrap_or_else(|| declared.clone())
    }

    /// 式の型。型推論が終わっていれば必ずある。
    fn expr_ty(&self, expr: &Expr) -> Ty {
        self.expr_tys.get(&expr.id).cloned().unwrap_or_else(|| {
            panic!(
                "compiler bug: type of expression {:?} is missing after inference",
                expr.id
            )
        })
    }

    fn genargs_of(&self, expr_id: ExprId) -> GenArgs {
        self.call_genargs.get(&expr_id).cloned().unwrap_or_default()
    }

    // ---- 文 ----

    fn lower_stmts(&mut self, mut bb: BasicBlock, stmts: &[Stmt]) -> BasicBlock {
        for stmt in stmts {
            bb = self.lower_stmt(bb, stmt);
        }
        bb
    }

    fn lower_stmt(&mut self, bb: BasicBlock, stmt: &Stmt) -> BasicBlock {
        match stmt {
            // ブロックは名前解決の時点で用途を終えているので、そのまま並べる。
            Stmt::Block(b) => self.lower_stmts(bb, &b.stmts),

            Stmt::Expr(e) => {
                // 値は捨てるが、副作用のために評価はする。
                let ty = self.expr_ty(&e.expr);
                let tmp = self.new_temp(ty, e.span.clone());
                self.lower_expr_into(bb, tmp, &e.expr)
            }

            Stmt::VarDecl(v) => {
                let decl = self.vars.get(&v.id).unwrap_or_else(|| {
                    panic!(
                        "compiler bug: declared variable {:?} is not in the body",
                        v.id
                    )
                });
                let ty = self.var_ty(v.id, &decl.ty);
                let span = decl.id.span.clone();
                let local = self.new_local(ty, span);
                self.var_map.insert(v.id, local);
                self.lower_expr_into(bb, Place::from_local(local), &v.init)
            }

            Stmt::Assign(a) => {
                let (bb, dest) = self.lower_assign_dst(bb, &a.dst, &a.src);
                self.lower_expr_into(bb, dest, &a.src)
            }

            Stmt::Return(r) => {
                let bb = self.lower_expr_into(bb, Place::from_local(Local::RETURN), &r.expr);
                self.terminate(bb, TerminatorKind::Return, r.span.clone());
                // return の後ろに文が続いていても行き先が要るので、
                // 到達しないブロックを開けておく。
                self.new_block()
            }

            Stmt::If(i) => {
                let (bb, cond) = self.lower_operand(bb, &i.cond);

                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let join_bb = self.new_block();

                self.terminate(
                    bb,
                    TerminatorKind::SwitchInt {
                        discr: cond,
                        targets: SwitchTargets::if_bool(then_bb, else_bb),
                    },
                    i.cond.span(),
                );

                let then_end = self.lower_stmts(then_bb, &i.then.stmts);
                self.terminate(
                    then_end,
                    TerminatorKind::Goto { target: join_bb },
                    i.then.span.clone(),
                );

                let else_end = match &i.els {
                    Some(els) => {
                        let end = self.lower_stmts(else_bb, &els.stmts);
                        self.terminate(
                            end,
                            TerminatorKind::Goto { target: join_bb },
                            els.span.clone(),
                        );
                        end
                    }
                    None => {
                        self.terminate(
                            else_bb,
                            TerminatorKind::Goto { target: join_bb },
                            i.cond.span(),
                        );
                        else_bb
                    }
                };
                let _ = else_end;

                join_bb
            }

            Stmt::While(w) => {
                // ループ頭を独立したブロックにする。
                // 条件式が呼び出しを含むこともあるので、
                // 条件の評価は頭のブロックから始めて、終わったところで分岐する。
                let head_bb = self.new_block();
                self.terminate(bb, TerminatorKind::Goto { target: head_bb }, w.cond.span());

                let (cond_end, cond) = self.lower_operand(head_bb, &w.cond);

                let body_bb = self.new_block();
                let exit_bb = self.new_block();
                self.terminate(
                    cond_end,
                    TerminatorKind::SwitchInt {
                        discr: cond,
                        targets: SwitchTargets::if_bool(body_bb, exit_bb),
                    },
                    w.cond.span(),
                );

                let body_end = self.lower_stmts(body_bb, &w.stmts.stmts);
                // 後方辺。ここだけが CFG に循環を作る。
                self.terminate(
                    body_end,
                    TerminatorKind::Goto { target: head_bb },
                    w.stmts.span.clone(),
                );

                exit_bb
            }

            // novel 文はエンジンへのシステムコールの発行である。
            //
            // 発行の仕方は「lang item の関数を呼ぶ」だけで、
            // 呼んだ先が同期的にブロックするのか、積んで即座に返るのかは
            // エンジン側の都合であってゲスト側のコードには現れない。
            // したがって MIR にも中断は現れず、普通の呼び出しになる。
            Stmt::NovelWrite(w) => {
                let msg = self.strings.intern(&w.msg);
                self.lower_syscall(
                    bb,
                    LangItem::Write,
                    vec![Operand::Const(Const::Str(msg))],
                    w.span.clone(),
                )
            }
            Stmt::NovelWait(w) => {
                self.lower_syscall(bb, LangItem::Wait, Vec::new(), w.span.clone())
            }
        }
    }

    fn lower_syscall(
        &mut self,
        bb: BasicBlock,
        item: LangItem,
        args: Vec<Operand>,
        span: Span,
    ) -> BasicBlock {
        let def_id = ValDefId::new(self.lang_items.get(&item).unwrap_or_else(|| {
            panic!(
                "compiler bug: lang item `{}` is missing at MIR building",
                item.key()
            )
        }));

        // 記述子は捨てる。捨てる先にも型が要るので syscall 型の一時変数を作る。
        let syscall_ty = self.lang_item_ty(LangItem::Syscall, span.clone());
        let dest = self.new_temp(syscall_ty, span.clone());

        let next = self.new_block();
        self.terminate(
            bb,
            TerminatorKind::Call {
                callee: Callee::Direct {
                    def_id,
                    genargs: Vec::new(),
                },
                args,
                dest,
                target: next,
            },
            span,
        );
        next
    }

    fn lang_item_ty(&self, item: LangItem, span: Span) -> Ty {
        let def_id = self.lang_items.get(&item).unwrap_or_else(|| {
            panic!(
                "compiler bug: lang item `{}` is missing at MIR building",
                item.key()
            )
        });
        Ty::new(
            TyKind::Defined(DefinedTy {
                def_id: biwac_span::TyDefId::new(def_id),
                genargs: Vec::new(),
            }),
            span,
        )
    }

    // ---- 場所 ----

    /// 代入文の左辺を場所にする。
    ///
    /// 左辺には [`ExprId`] が振られていないので、
    /// メンバの型は右辺の型から取る (単一化済みで一致する)。
    fn lower_assign_dst(
        &mut self,
        bb: BasicBlock,
        dst: &Primary,
        src: &Expr,
    ) -> (BasicBlock, Place) {
        match dst {
            Primary::Variable(v) => match v.id {
                VarIdKind::Local(var_id) => (bb, Place::from_local(self.local_of(var_id))),
                VarIdKind::Global(_) => {
                    unreachable!("global variables are not supported yet (see type inferrer)")
                }
            },
            Primary::MemberAccess(m) => {
                let (bb, base) = self.lower_place(bb, &m.left);
                let ty = self.expr_ty(src);
                (bb, base.field(m.member.id, ty))
            }
            _ => panic!("compiler bug: assignment destination must be a variable or a member"),
        }
    }

    /// 式を場所にする。
    ///
    /// 変数とメンバアクセスはそのまま場所になる。
    /// それ以外は値でしかないので、一時変数に置いてその場所を返す。
    fn lower_place(&mut self, bb: BasicBlock, expr: &Expr) -> (BasicBlock, Place) {
        if let ExprVal::Primary(primary) = &expr.expr {
            match primary {
                Primary::Variable(v) => match v.id {
                    VarIdKind::Local(var_id) => {
                        return (bb, Place::from_local(self.local_of(var_id)));
                    }
                    VarIdKind::Global(_) => {
                        unreachable!("global variables are not supported yet (see type inferrer)")
                    }
                },
                Primary::MemberAccess(m) => {
                    let (bb, base) = self.lower_place(bb, &m.left);
                    let ty = self.expr_ty(expr);
                    return (bb, base.field(m.member.id, ty));
                }
                _ => {}
            }
        }

        let ty = self.expr_ty(expr);
        let tmp = self.new_temp(ty, expr.span());
        let bb = self.lower_expr_into(bb, tmp.clone(), expr);
        (bb, tmp)
    }

    fn local_of(&self, var_id: VarId) -> Local {
        *self.var_map.get(&var_id).unwrap_or_else(|| {
            panic!("compiler bug: variable {var_id:?} is used before it is declared")
        })
    }

    // ---- 式 ----

    /// 式を評価して、その値をオペランドとして取り出す。
    fn lower_operand(&mut self, bb: BasicBlock, expr: &Expr) -> (BasicBlock, Operand) {
        // 定数と変数は一時変数を挟まずにそのまま使える。
        if let ExprVal::Primary(primary) = &expr.expr {
            match primary {
                Primary::Literal(l) => {
                    if let Some(c) = self.lower_const(l) {
                        return (bb, Operand::Const(c));
                    }
                }
                Primary::Variable(v) => {
                    if let VarIdKind::Local(var_id) = v.id {
                        return (bb, Operand::from_local(self.local_of(var_id)));
                    }
                }
                _ => {}
            }
        }

        let (bb, place) = self.lower_place(bb, expr);
        (bb, Operand::Place(place))
    }

    /// リテラルのうち、そのまま定数になるもの。
    /// struct literal は集約なので定数にならない。
    fn lower_const(&mut self, literal: &Literal) -> Option<Const> {
        match literal {
            Literal::Integer(i) => Some(Const::Int(i.val as i64)),
            Literal::Bool(b) => Some(Const::Bool(b.val)),
            Literal::String(s) => Some(Const::Str(self.strings.intern(&s.val))),
            Literal::Struct(_) => None,
        }
    }

    /// 式を評価して `dest` に書く。
    fn lower_expr_into(&mut self, bb: BasicBlock, dest: Place, expr: &Expr) -> BasicBlock {
        let span = expr.span();
        match &expr.expr {
            ExprVal::Unary(u) => {
                let (bb, operand) = self.lower_operand(bb, &u.right);
                let op = match u.op {
                    UnOperator::Neg => UnOp::Neg,
                };
                self.push_assign(bb, dest, Rvalue::UnaryOp(op, operand), span);
                bb
            }

            ExprVal::Binary(b) => {
                let (bb, left) = self.lower_operand(bb, &b.left);
                let (bb, right) = self.lower_operand(bb, &b.right);
                let op = bin_op(b.op);
                self.push_assign(bb, dest, Rvalue::BinaryOp(op, left, right), span);
                bb
            }

            ExprVal::Primary(primary) => self.lower_primary_into(bb, dest, expr, primary),
        }
    }

    fn lower_primary_into(
        &mut self,
        bb: BasicBlock,
        dest: Place,
        expr: &Expr,
        primary: &Primary,
    ) -> BasicBlock {
        let span = expr.span();
        match primary {
            Primary::Literal(Literal::Struct(sl)) => {
                let mut members = Vec::with_capacity(sl.members.len());
                let mut bb = bb;
                for (ident, member_expr) in &sl.members {
                    let (next, operand) = self.lower_operand(bb, member_expr);
                    bb = next;
                    members.push((ident.id, operand));
                }
                // 並べ替えない。ここの順序は評価順であり、
                // メンバの初期化式に副作用があれば書いた順に効く必要がある。
                // どのメンバがどこに置かれるかはレイアウトの話で、バックエンドが決める。
                self.push_assign(bb, dest, Rvalue::Aggregate(sl.tid, members), span);
                bb
            }

            Primary::Literal(l) => {
                let c = self
                    .lower_const(l)
                    .expect("compiler bug: struct literal is handled above");
                self.push_assign(bb, dest, Rvalue::Use(Operand::Const(c)), span);
                bb
            }

            Primary::Variable(_) | Primary::MemberAccess(_) => {
                let (bb, place) = self.lower_place(bb, expr);
                self.push_assign(bb, dest, Rvalue::Use(Operand::Place(place)), span);
                bb
            }

            Primary::FnCall(c) => {
                let mut bb = bb;
                let mut args = Vec::with_capacity(c.args.len());
                for arg in &c.args {
                    let (next, operand) = self.lower_operand(bb, arg);
                    bb = next;
                    args.push(operand);
                }

                let callee = match &c.callee {
                    HirCallee::Fn(def_id) => Callee::Direct {
                        def_id: *def_id,
                        genargs: self.genargs_of(expr.id),
                    },
                    HirCallee::Var(var_id) => {
                        Callee::Indirect(Operand::from_local(self.local_of(*var_id)))
                    }
                };

                let next = self.new_block();
                self.terminate(
                    bb,
                    TerminatorKind::Call {
                        callee,
                        args,
                        dest,
                        target: next,
                    },
                    span,
                );
                next
            }

            Primary::MethodCall(m) => {
                // レシーバは第 1 引数として渡す。
                let (mut bb, receiver) = self.lower_operand(bb, &m.left);
                let mut args = Vec::with_capacity(m.args.len() + 1);
                args.push(receiver);
                for arg in &m.args {
                    let (next, operand) = self.lower_operand(bb, arg);
                    bb = next;
                    args.push(operand);
                }

                let def_id = *m
                    .def_id
                    .get()
                    .expect("compiler bug: method is not resolved after inference");

                let next = self.new_block();
                self.terminate(
                    bb,
                    TerminatorKind::Call {
                        callee: Callee::Direct {
                            def_id,
                            genargs: self.genargs_of(expr.id),
                        },
                        args,
                        dest,
                        target: next,
                    },
                    span,
                );
                next
            }

            Primary::IfExpr(i) => {
                let (bb, cond) = self.lower_operand(bb, &i.cond);

                let then_bb = self.new_block();
                let else_bb = self.new_block();
                let join_bb = self.new_block();

                self.terminate(
                    bb,
                    TerminatorKind::SwitchInt {
                        discr: cond,
                        targets: SwitchTargets::if_bool(then_bb, else_bb),
                    },
                    i.cond.span(),
                );

                let then_end = self.lower_block_expr_into(then_bb, dest.clone(), &i.then);
                self.terminate(
                    then_end,
                    TerminatorKind::Goto { target: join_bb },
                    i.then.span.clone(),
                );

                let else_end = self.lower_block_expr_into(else_bb, dest, &i.els);
                self.terminate(
                    else_end,
                    TerminatorKind::Goto { target: join_bb },
                    i.els.span.clone(),
                );

                join_bb
            }

            Primary::Block(b) => self.lower_block_expr_into(bb, dest, b),
        }
    }

    fn lower_block_expr_into(
        &mut self,
        bb: BasicBlock,
        dest: Place,
        block: &BlockExpr,
    ) -> BasicBlock {
        let bb = self.lower_stmts(bb, &block.stmts);
        self.lower_expr_into(bb, dest, &block.expr)
    }
}

fn bin_op(op: BinOperator) -> BinOp {
    match op {
        BinOperator::Add => BinOp::Add,
        BinOperator::Sub => BinOp::Sub,
        BinOperator::Mul => BinOp::Mul,
        BinOperator::Div => BinOp::Div,
        BinOperator::Mod => BinOp::Rem,
        BinOperator::Gt => BinOp::Gt,
        BinOperator::Lt => BinOp::Lt,
        BinOperator::Ge => BinOp::Ge,
        BinOperator::Le => BinOp::Le,
        BinOperator::Eq => BinOp::Eq,
        BinOperator::Ne => BinOp::Ne,
    }
}
