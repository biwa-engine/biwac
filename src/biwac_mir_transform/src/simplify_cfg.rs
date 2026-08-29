//! 制御フローグラフを畳む。
//!
//! lowering は合流のたびにブロックを開けるので、
//! `bb2: { goto 3 }` のような中身の無いブロックが大量に残る。
//! 意味は変わらないが、構造化制御フローへの変換 (WASM の `block` / `loop` / `br`) の
//! 出力品質に直結するので、ここで畳んでおく。
//!
//! 3 つを、変化がなくなるまで回す:
//!
//! 1. **goto の短絡** — 文が無く `goto` だけのブロックへの辺を、その行き先に付け替える
//! 2. **単一先行辺の併合** — `A: goto B` で `B` の先行が `A` だけなら `B` を `A` に取り込む
//! 3. **到達不能の除去** — bb0 から辿れないブロックを落として番号を詰める

use biwac_mir::{BasicBlock, Body, SwitchTargets, TerminatorKind};

use crate::MirPass;

pub struct SimplifyCfg;

impl MirPass for SimplifyCfg {
    fn name(&self) -> &'static str {
        "simplify_cfg"
    }

    fn run(&self, body: &mut Body) {
        // 各段が次の段の対象を生むので、収束するまで回す。
        // 1 回で落ち着くことがほとんどだが、上限は念のため置く。
        for _ in 0..16 {
            let mut changed = false;
            changed |= thread_gotos(body);
            changed |= collapse_uniform_switches(body);
            changed |= merge_single_predecessor(body);
            changed |= remove_unreachable(body);
            if !changed {
                break;
            }
        }
    }
}

/// 文が無く `goto` だけのブロックを飛ばして、その行き先を直接指す。
fn thread_gotos(body: &mut Body) -> bool {
    let targets: Vec<Option<BasicBlock>> = body
        .blocks
        .iter()
        .map(|b| match (&b.stmts[..], &b.term.kind) {
            ([], TerminatorKind::Goto { target }) => Some(*target),
            _ => None,
        })
        .collect();

    // 空ブロックの連なりを辿る。自己ループがあると止まらないので回数で切る。
    let limit = body.blocks.len();
    let resolve = |mut bb: BasicBlock| {
        for _ in 0..limit {
            match targets.get(bb.index()).copied().flatten() {
                Some(next) if next != bb => bb = next,
                _ => break,
            }
        }
        bb
    };

    let mut changed = false;
    for block in &mut body.blocks {
        let new_kind = match &block.term.kind {
            TerminatorKind::Goto { target } => {
                let t = resolve(*target);
                (t != *target).then(|| TerminatorKind::Goto { target: t })
            }
            TerminatorKind::Call {
                callee,
                args,
                dest,
                target,
            } => {
                let t = resolve(*target);
                (t != *target).then(|| TerminatorKind::Call {
                    callee: callee.clone(),
                    args: args.clone(),
                    dest: dest.clone(),
                    target: t,
                })
            }
            TerminatorKind::SwitchInt { discr, targets: sw } => {
                let (values, dests): (Vec<_>, Vec<_>) =
                    sw.iter().map(|(v, t)| (v, resolve(t))).unzip();
                let otherwise = resolve(sw.otherwise());
                let same = dests
                    .iter()
                    .zip(sw.iter())
                    .all(|(new, (_, old))| *new == old)
                    && otherwise == sw.otherwise();
                (!same).then(|| TerminatorKind::SwitchInt {
                    discr: discr.clone(),
                    targets: SwitchTargets::new(values, dests, otherwise),
                })
            }
            TerminatorKind::Return | TerminatorKind::Unreachable => None,
        };

        if let Some(kind) = new_kind {
            block.term.kind = kind;
            changed = true;
        }
    }
    changed
}

/// 行き先がすべて同じ `switch` を `goto` にする。
///
/// 条件の評価そのものは終端子の外 (文) で済んでいるので、落としてよい。
fn collapse_uniform_switches(body: &mut Body) -> bool {
    let mut changed = false;
    for block in &mut body.blocks {
        let TerminatorKind::SwitchInt { targets, .. } = &block.term.kind else {
            continue;
        };
        let otherwise = targets.otherwise();
        if targets.iter().all(|(_, t)| t == otherwise) {
            block.term.kind = TerminatorKind::Goto { target: otherwise };
            changed = true;
        }
    }
    changed
}

/// `A: goto B` で `B` の先行が `A` だけなら、`B` を `A` に取り込む。
fn merge_single_predecessor(body: &mut Body) -> bool {
    let preds = predecessor_counts(body);

    let mut changed = false;
    for i in 0..body.blocks.len() {
        let TerminatorKind::Goto { target } = body.blocks[i].term.kind else {
            continue;
        };
        // 自分自身へのループは畳めない。
        if target.index() == i || preds[target.index()] != 1 {
            continue;
        }

        let taken = std::mem::replace(
            &mut body.blocks[target.index()],
            biwac_mir::BasicBlockData::new(
                TerminatorKind::Unreachable.with_span(biwac_span::Span::dummy()),
            ),
        );
        body.blocks[i].stmts.extend(taken.stmts);
        body.blocks[i].term = taken.term;
        changed = true;

        // 取り込んだ結果また `goto` になっているかもしれないが、
        // 先行数の表が古くなるので、この周では次のブロックへ進む。
        // 収束するまで呼び直されるので取りこぼさない。
    }
    changed
}

fn predecessor_counts(body: &Body) -> Vec<usize> {
    let mut preds = vec![0usize; body.blocks.len()];
    for block in &body.blocks {
        for succ in block.term.successors() {
            if let Some(c) = preds.get_mut(succ.index()) {
                *c += 1;
            }
        }
    }
    preds
}

/// 入口から辿れないブロックを落として、番号を詰める。
fn remove_unreachable(body: &mut Body) -> bool {
    let count = body.blocks.len();
    let mut reachable = vec![false; count];
    let mut stack = vec![BasicBlock::START];
    reachable[0] = true;
    while let Some(bb) = stack.pop() {
        for succ in body.blocks[bb.index()].term.successors() {
            if !reachable[succ.index()] {
                reachable[succ.index()] = true;
                stack.push(succ);
            }
        }
    }

    if reachable.iter().all(|r| *r) {
        return false;
    }

    let mut remap = vec![None; count];
    let mut next = 0u32;
    for (idx, r) in reachable.iter().enumerate() {
        if *r {
            remap[idx] = Some(BasicBlock::new(next));
            next += 1;
        }
    }
    let map = |bb: BasicBlock| {
        remap[bb.index()].expect("compiler bug: a reachable block points at a removed block")
    };

    let mut blocks = Vec::with_capacity(next as usize);
    for (idx, mut block) in std::mem::take(&mut body.blocks).into_iter().enumerate() {
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
    body.blocks = blocks;
    true
}
