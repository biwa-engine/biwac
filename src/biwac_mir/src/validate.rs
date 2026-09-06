//! MIR の不変条件を検査する。
//!
//! ここで検出されるのはすべてコンパイラのバグであり、ユーザのコードの誤りではない。
//! それでも黙って壊れた MIR を後段に渡すよりは、
//! 作った直後に落ちたほうが原因を追いやすい。
//!
//! 構築の直後だけでなく、MIR→MIR のパスを掛けたあとにも走らせる。
//! 「表現が満たすべき条件」なのでここ (表現の crate) に置いてある。

use std::fmt;

use crate::{
    BasicBlock, Body, Callee, Local, Mir, MirItem, Operand, Place, PlaceElem, Rvalue,
    StatementKind, TerminatorKind,
};
use biwac_hir::TyKind;
use biwac_span::ValDefId;

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub def_id: ValDefId,
    pub kind: ValidationErrorKind,
}

#[derive(Debug, Clone)]
pub enum ValidationErrorKind {
    /// 存在しない local を参照している。
    UndefinedLocal(Local),

    /// 存在しないブロックへ飛んでいる。
    UndefinedBlock(BasicBlock),

    /// 入口から到達できないブロックがある。
    UnreachableBlock(BasicBlock),

    /// 戻り値スロット `_0` が無い。
    MissingReturnLocal,

    /// 引数の個数が local の個数を超えている。
    ArgCountOutOfRange { arg_count: usize, locals: usize },

    /// 型推論が終わっているのに推論待ちの型が残っている。
    UnresolvedTy(Local),

    /// CFG が簡約可能でない。
    ///
    /// biwa には goto が無く、lowering が作る CFG は構造化されているので
    /// 本来起こらない。WASM への構造化変換がこの前提に乗るので、
    /// 崩れていないことをここで確かめる。
    IrreducibleCfg(BasicBlock),

    /// `Downcast` の直後がフィールドの射影になっていない。
    ///
    /// downcast は「この値をこのバリアントとして見る」という印でしかなく、
    /// 単体では値にならない。
    DanglingDowncast(Place),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "val#{}: ", self.def_id.value())?;
        match &self.kind {
            ValidationErrorKind::UndefinedLocal(l) => write!(f, "undefined local _{}", l.value()),
            ValidationErrorKind::UndefinedBlock(b) => write!(f, "undefined block bb{}", b.value()),
            ValidationErrorKind::UnreachableBlock(b) => {
                write!(f, "block bb{} is not reachable from bb0", b.value())
            }
            ValidationErrorKind::MissingReturnLocal => write!(f, "return slot _0 is missing"),
            ValidationErrorKind::ArgCountOutOfRange { arg_count, locals } => {
                write!(f, "arg_count {arg_count} does not fit in {locals} locals")
            }
            ValidationErrorKind::UnresolvedTy(l) => {
                write!(f, "local _{} still has an inference type", l.value())
            }
            ValidationErrorKind::IrreducibleCfg(b) => write!(
                f,
                "control flow graph is irreducible: bb{} is entered from outside its loop",
                b.value()
            ),
            ValidationErrorKind::DanglingDowncast(p) => write!(
                f,
                "downcast on _{} is not followed by a field projection",
                p.local.value()
            ),
        }
    }
}

/// パッケージ 1 つ分の MIR を検査する。
pub fn validate(mir: &Mir) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for item in mir.items.values() {
        if let MirItem::Body(body) = item {
            validate_body(body, &mut errors);
        }
    }
    errors
}

/// 関数 1 つ分を検査する。パスの後始末の確認に使う。
pub fn validate_body(body: &Body, errors: &mut Vec<ValidationError>) {
    let mut push = |kind| {
        errors.push(ValidationError {
            def_id: body.def_id,
            kind,
        })
    };

    if body.locals.is_empty() {
        push(ValidationErrorKind::MissingReturnLocal);
        return;
    }
    if body.arg_count + 1 > body.locals.len() {
        push(ValidationErrorKind::ArgCountOutOfRange {
            arg_count: body.arg_count,
            locals: body.locals.len(),
        });
    }

    for (idx, decl) in body.locals.iter().enumerate() {
        if matches!(decl.ty.kind, TyKind::Infer(_)) {
            push(ValidationErrorKind::UnresolvedTy(Local::new(idx as u32)));
        }
    }

    let local_count = body.locals.len();
    let block_count = body.blocks.len();

    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign(place, rvalue) = &stmt.kind;
            check_place(place, local_count, &mut push);
            check_rvalue(rvalue, local_count, &mut push);
        }

        match &block.term.kind {
            TerminatorKind::SwitchInt { discr, .. } => check_operand(discr, local_count, &mut push),
            TerminatorKind::Call {
                callee, args, dest, ..
            } => {
                if let Callee::Indirect(op) = callee {
                    check_operand(op, local_count, &mut push);
                }
                for arg in args {
                    check_operand(arg, local_count, &mut push);
                }
                check_place(dest, local_count, &mut push);
            }
            TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Unreachable => {}
        }

        for succ in block.term.successors() {
            if succ.index() >= block_count {
                push(ValidationErrorKind::UndefinedBlock(succ));
            }
        }
    }

    check_cfg(body, errors);
}

fn check_place(place: &Place, local_count: usize, push: &mut impl FnMut(ValidationErrorKind)) {
    if place.local.index() >= local_count {
        push(ValidationErrorKind::UndefinedLocal(place.local));
    }

    // downcast はバリアントのフィールドを見るためだけのものなので、
    // 直後に必ずフィールドの射影が続く。
    for (i, elem) in place.projection.iter().enumerate() {
        if matches!(elem, PlaceElem::Downcast(_))
            && !matches!(place.projection.get(i + 1), Some(PlaceElem::Field(_, _)))
        {
            push(ValidationErrorKind::DanglingDowncast(place.clone()));
        }
    }
}

fn check_operand(
    operand: &Operand,
    local_count: usize,
    push: &mut impl FnMut(ValidationErrorKind),
) {
    if let Operand::Place(p) = operand {
        check_place(p, local_count, push);
    }
}

fn check_rvalue(rvalue: &Rvalue, local_count: usize, push: &mut impl FnMut(ValidationErrorKind)) {
    match rvalue {
        Rvalue::Use(op) => check_operand(op, local_count, push),
        Rvalue::UnaryOp(_, op) => check_operand(op, local_count, push),
        Rvalue::BinaryOp(_, l, r) => {
            check_operand(l, local_count, push);
            check_operand(r, local_count, push);
        }
        Rvalue::Aggregate(_, members) => {
            for (_, op) in members {
                check_operand(op, local_count, push);
            }
        }
        Rvalue::Discriminant(place) => check_place(place, local_count, push),
    }
}

/// 到達性と簡約可能性をまとめて見る。
///
/// 深さ優先で辿り、後方辺 (今たどっている経路の上にあるブロックへ戻る辺) を見つけたら、
/// その行き先が経路上にあること = ループの頭であることを確かめる。
/// 経路上に無い訪問済みブロックへの辺は横断辺で、これは簡約可能性を壊さない。
///
/// 「ループの頭以外から入ってくる」形は、biwa の構文からは作れない。
/// 作れてしまったらそれは lowering かパスのバグである。
fn check_cfg(body: &Body, errors: &mut Vec<ValidationError>) {
    let block_count = body.blocks.len();
    if block_count == 0 {
        return;
    }

    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Unseen,
        OnPath,
        Done,
    }

    let mut state = vec![State::Unseen; block_count];
    // 再帰にしないのは、深いネストで stack を食い潰さないため。
    let mut stack: Vec<(BasicBlock, usize)> = vec![(BasicBlock::START, 0)];
    state[0] = State::OnPath;

    while let Some((bb, next_succ)) = stack.pop() {
        let succs = body.block(bb).term.successors();
        if next_succ >= succs.len() {
            state[bb.index()] = State::Done;
            continue;
        }
        stack.push((bb, next_succ + 1));

        let succ = succs[next_succ];
        if succ.index() >= block_count {
            // 行き先が範囲外なのは別に報告済み。
            continue;
        }
        match state[succ.index()] {
            State::Unseen => {
                state[succ.index()] = State::OnPath;
                stack.push((succ, 0));
            }
            // 経路上へ戻る = 後方辺。ループの頭なので簡約可能。
            State::OnPath => {}
            // 訪問済みで経路上に無い = 横断辺。
            State::Done => {}
        }
    }

    for (idx, s) in state.iter().enumerate() {
        if *s == State::Unseen {
            errors.push(ValidationError {
                def_id: body.def_id,
                kind: ValidationErrorKind::UnreachableBlock(BasicBlock::new(idx as u32)),
            });
        }
    }
}
