//! 制御フローグラフを、WASM の構造化制御フローに変換する。
//!
//! WASM には任意の goto が無く、`block` / `loop` / `if` / `br` / `br_table` しかない。
//! MIR は基本ブロックのグラフなので、そのままでは出せない。
//!
//! # アルゴリズム
//!
//! Ramsey 2022 "Beyond Relooper: Recursive Translation of Unstructured
//! Control Flow to Structured Control Flow" の支配木ベースの変換である。
//! Relooper より単純で、簡約可能な CFG なら節点分割なしで必ず構造化できる。
//!
//! 手順は次のとおり。
//!
//! 1. 逆後行順 (reverse postorder) を取る。合流点の順序づけに使う。
//! 2. 支配木を作る。あるブロックが支配するブロックは、そのブロックの中に置ける。
//! 3. 入口から再帰的に木を組む。
//!    - **ループの頭** (後方辺で戻られるブロック) は `loop` で包む。
//!      `loop` への `br` は「先頭に戻る」意味になる。
//!    - **合流点** (先行辺が 2 本以上あるブロック) は、
//!      それを支配するブロックの直前に `block` で包む。
//!      `block` への `br` は「その先へ抜ける」意味になる。
//!    - それ以外の行き先は、その場に埋め込む。
//!
//! biwa には goto が無く、lowering が作る CFG は必ず簡約可能である
//! (`biwac_mir::validate` が不変条件として検査している)。
//! したがって節点分割は起きない。
//!
//! # ここで扱わないもの
//!
//! 型にも値にも触らない。各ブロックの中身は [`Structured::Simple`] に
//! [`BasicBlock`] の番号として残るだけで、命令への変換は呼び出し側が行う。
//! したがってこの変換は ABI の決定に依存しない。

use std::collections::HashSet;

use biwac_mir::{BasicBlock, Body, TerminatorKind};

/// 構造化された制御フロー。
///
/// `br` の深さは、この木の上で自分を囲む
/// [`Structured::Block`] と [`Structured::Loop`] を内側から数えたものである
/// (WASM の相対深さと同じ規則)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Structured {
    /// `block ... end`。中身を順に実行する。
    /// ここへの `br` は **末尾へ抜ける**。
    Block(Vec<Structured>),

    /// `loop ... end`。
    /// ここへの `br` は **先頭へ戻る**。
    Loop(Box<Structured>),

    /// 基本ブロックの中身 (文の列と、分岐でない終端子)。
    ///
    /// 制御の移り先は続く要素が表す。
    Simple(BasicBlock),

    /// `if ... else ... end`。
    /// 条件は `bb` の終端子 (`SwitchInt`) が持っている。
    If {
        /// 条件を持つブロック。
        bb: BasicBlock,
        then: Vec<Structured>,
        els: Vec<Structured>,
    },

    /// `br <depth>` / `br_if`。相対深さで囲みを指す。
    Br(u32),

    /// `return`。
    Return,

    /// `unreachable`。
    Unreachable,
}

#[derive(Debug)]
pub enum StructureError {
    /// 簡約可能でない CFG。
    ///
    /// biwa の構文からは作れないので、起きたら lowering かパスのバグである。
    Irreducible(BasicBlock),
}

impl std::fmt::Display for StructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Irreducible(bb) => write!(
                f,
                "control flow graph is irreducible at bb{}; it cannot be structured",
                bb.value()
            ),
        }
    }
}

/// 関数 1 つ分の CFG を構造化する。
pub fn structure(body: &Body) -> Result<Structured, StructureError> {
    let cfg = Cfg::of(body);
    cfg.check_reducible()?;
    let mut builder = Builder {
        cfg: &cfg,
        // 囲みの並び。内側が末尾。`br` の深さはここから数える。
        context: Vec::new(),
    };
    Ok(Structured::Block(builder.do_tree(BasicBlock::START)))
}

/// 構造化に要るグラフの情報。
struct Cfg {
    succs: Vec<Vec<BasicBlock>>,
    preds: Vec<Vec<BasicBlock>>,
    /// 逆後行順での順位。小さいほど手前。
    rpo_index: Vec<usize>,
    /// 直接支配ブロック。入口だけ `None`。
    idom: Vec<Option<BasicBlock>>,
    /// 支配木の子。逆後行順に並ぶ。
    dom_children: Vec<Vec<BasicBlock>>,
    /// 後方辺で戻られるブロック (= ループの頭)。
    loop_headers: HashSet<BasicBlock>,
    terminators: Vec<TerminatorKind>,
}

impl Cfg {
    fn of(body: &Body) -> Self {
        let n = body.blocks.len();
        let succs: Vec<Vec<BasicBlock>> = body.blocks.iter().map(|b| b.term.successors()).collect();

        let mut preds = vec![Vec::new(); n];
        for (i, ss) in succs.iter().enumerate() {
            for s in ss {
                preds[s.index()].push(BasicBlock::new(i as u32));
            }
        }

        let rpo = reverse_postorder(&succs);
        let mut rpo_index = vec![usize::MAX; n];
        for (i, bb) in rpo.iter().enumerate() {
            rpo_index[bb.index()] = i;
        }

        let idom = dominators(&preds, &rpo, &rpo_index);

        let mut dom_children = vec![Vec::new(); n];
        for bb in &rpo {
            if let Some(p) = idom[bb.index()] {
                dom_children[p.index()].push(*bb);
            }
        }
        for children in &mut dom_children {
            children.sort_by_key(|c| rpo_index[c.index()]);
        }

        // 後方辺: 行き先が自分を支配しているもの。
        let mut loop_headers = HashSet::new();
        for (i, ss) in succs.iter().enumerate() {
            let from = BasicBlock::new(i as u32);
            if rpo_index[i] == usize::MAX {
                continue;
            }
            for s in ss {
                if dominates(*s, from, &idom) {
                    loop_headers.insert(*s);
                }
            }
        }

        Self {
            succs,
            preds,
            rpo_index,
            idom,
            dom_children,
            loop_headers,
            terminators: body.blocks.iter().map(|b| b.term.kind.clone()).collect(),
        }
    }

    fn reachable(&self, bb: BasicBlock) -> bool {
        self.rpo_index[bb.index()] != usize::MAX
    }

    /// 合流点か。到達可能な先行辺が 2 本以上あるもの。
    ///
    /// 合流点は「囲んでから抜ける」形にしないと、複数の経路から書けない。
    fn is_merge(&self, bb: BasicBlock) -> bool {
        self.preds[bb.index()]
            .iter()
            .filter(|p| self.reachable(**p))
            .count()
            > 1
    }

    fn is_loop_header(&self, bb: BasicBlock) -> bool {
        self.loop_headers.contains(&bb)
    }

    /// 後方辺か。ループの頭へ戻る辺。
    fn is_backward(&self, from: BasicBlock, to: BasicBlock) -> bool {
        dominates(to, from, &self.idom)
    }

    /// 簡約可能性。
    ///
    /// 「戻る辺の行き先が必ず自分を支配している」ことを確かめる。
    /// 支配していないのに逆後行順で手前へ戻る辺があれば、
    /// ループの頭以外から入るループがあるということで、簡約可能でない。
    fn check_reducible(&self) -> Result<(), StructureError> {
        for (i, ss) in self.succs.iter().enumerate() {
            let from = BasicBlock::new(i as u32);
            if !self.reachable(from) {
                continue;
            }
            for s in ss {
                if self.rpo_index[s.index()] <= self.rpo_index[from.index()]
                    && !dominates(*s, from, &self.idom)
                {
                    return Err(StructureError::Irreducible(*s));
                }
            }
        }
        Ok(())
    }
}

/// 囲みの種類。`br` の深さを数えるのに使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Enclosing {
    /// `loop`。ここへ `br` すると先頭へ戻る。
    Loop(BasicBlock),
    /// `block`。ここへ `br` すると、そのブロックの手前へ抜ける。
    Block(BasicBlock),
    /// `if` の枝。`br` の対象にはならないが、深さは 1 つ増える。
    If,
}

struct Builder<'a> {
    cfg: &'a Cfg,
    context: Vec<Enclosing>,
}

impl Builder<'_> {
    /// 支配木を辿って、`bb` とそれが支配する範囲を組み立てる。
    fn do_tree(&mut self, bb: BasicBlock) -> Vec<Structured> {
        if self.cfg.is_loop_header(bb) {
            self.context.push(Enclosing::Loop(bb));
            let inner = self.node_within(bb);
            self.context.pop();
            vec![Structured::Loop(Box::new(Structured::Block(inner)))]
        } else {
            self.node_within(bb)
        }
    }

    /// `bb` が支配する合流点を `block` で包んでから、`bb` 自身を組み立てる。
    ///
    /// 合流点は「自分より後ろ」に置き、そこへの分岐を `br` で表す。
    /// 包む順序は逆後行順の**逆**である。
    /// 内側ほど手前の合流点になり、`br` の深さが自然に合う。
    fn node_within(&mut self, bb: BasicBlock) -> Vec<Structured> {
        let mut merges: Vec<BasicBlock> = self.cfg.dom_children[bb.index()]
            .iter()
            .copied()
            .filter(|c| self.cfg.is_merge(*c))
            .collect();
        merges.sort_by_key(|c| self.cfg.rpo_index[c.index()]);

        self.node_within_inner(bb, &merges)
    }

    fn node_within_inner(&mut self, bb: BasicBlock, merges: &[BasicBlock]) -> Vec<Structured> {
        let Some((last, rest)) = merges.split_last() else {
            return self.do_node(bb);
        };

        // 一番後ろの合流点を一番外側の `block` にする。
        self.context.push(Enclosing::Block(*last));
        let inner = self.node_within_inner(bb, rest);
        self.context.pop();

        let mut out = vec![Structured::Block(inner)];
        out.extend(self.do_tree(*last));
        out
    }

    /// `bb` の中身と、その終端子が示す続きを組み立てる。
    fn do_node(&mut self, bb: BasicBlock) -> Vec<Structured> {
        let mut out = vec![Structured::Simple(bb)];

        match self.cfg.terminators[bb.index()].clone() {
            TerminatorKind::Return => out.push(Structured::Return),
            TerminatorKind::Unreachable => out.push(Structured::Unreachable),
            TerminatorKind::Goto { target } => out.extend(self.do_branch(bb, target)),
            // 呼び出しは制御を移すが、戻ってくるので分岐と同じ扱いでよい。
            TerminatorKind::Call { target, .. } => out.extend(self.do_branch(bb, target)),
            TerminatorKind::SwitchInt { targets, .. } => {
                // MIR の switch は必ず 2 分岐である。
                // `if` と `while` の条件だけでなく、`match` も
                // 「タグと 1 つの値を比べる」形の連鎖に落としてある
                // (lowering の `lower_match` を参照)。
                //
                // 多分岐をそのまま持てるようにするなら、
                // ここを `br_table` に落とす形へ足すことになる。
                let arms: Vec<(u128, BasicBlock)> = targets.iter().collect();
                let otherwise = targets.otherwise();

                if arms.len() == 1 {
                    // 値が一致したら arms[0]、しなければ otherwise。
                    self.context.push(Enclosing::If);
                    let then = self.do_branch(bb, arms[0].1);
                    let els = self.do_branch(bb, otherwise);
                    self.context.pop();
                    out.push(Structured::If { bb, then, els });
                } else {
                    panic!(
                        "compiler bug: a switch with {} arms is not supported yet",
                        arms.len()
                    );
                }
            }
        }

        out
    }

    /// `from` から `to` への辺を組み立てる。
    ///
    /// - 後方辺、または合流点への辺 → 囲みへの `br`
    /// - それ以外 → その場に埋め込む
    fn do_branch(&mut self, from: BasicBlock, to: BasicBlock) -> Vec<Structured> {
        if self.cfg.is_backward(from, to) || self.cfg.is_merge(to) {
            vec![Structured::Br(self.depth_of(to))]
        } else {
            self.do_tree(to)
        }
    }

    /// `to` を指す囲みまでの相対深さ。
    fn depth_of(&self, to: BasicBlock) -> u32 {
        for (i, enclosing) in self.context.iter().rev().enumerate() {
            let matched = match enclosing {
                Enclosing::Loop(bb) | Enclosing::Block(bb) => *bb == to,
                Enclosing::If => false,
            };
            if matched {
                return i as u32;
            }
        }
        panic!(
            "compiler bug: no enclosing block for a branch to bb{}",
            to.value()
        );
    }
}

/// 逆後行順。到達可能なブロックだけが並ぶ。
fn reverse_postorder(succs: &[Vec<BasicBlock>]) -> Vec<BasicBlock> {
    let n = succs.len();
    let mut visited = vec![false; n];
    let mut post = Vec::with_capacity(n);

    // 再帰にしないのは、深いネストで stack を食い潰さないため。
    let mut stack = vec![(BasicBlock::START, 0usize)];
    visited[0] = true;
    while let Some((bb, next)) = stack.pop() {
        if next < succs[bb.index()].len() {
            stack.push((bb, next + 1));
            let s = succs[bb.index()][next];
            if !visited[s.index()] {
                visited[s.index()] = true;
                stack.push((s, 0));
            }
        } else {
            post.push(bb);
        }
    }

    post.reverse();
    post
}

/// 支配木。Cooper-Harvey-Kennedy の反復法。
fn dominators(
    preds: &[Vec<BasicBlock>],
    rpo: &[BasicBlock],
    rpo_index: &[usize],
) -> Vec<Option<BasicBlock>> {
    let mut idom: Vec<Option<BasicBlock>> = vec![None; preds.len()];
    idom[0] = Some(BasicBlock::START);

    let mut changed = true;
    while changed {
        changed = false;
        for bb in rpo.iter().skip(1) {
            let mut new_idom: Option<BasicBlock> = None;
            for p in &preds[bb.index()] {
                if rpo_index[p.index()] == usize::MAX || idom[p.index()].is_none() {
                    continue;
                }
                new_idom = Some(match new_idom {
                    None => *p,
                    Some(cur) => intersect(*p, cur, &idom, rpo_index),
                });
            }
            if new_idom.is_some() && idom[bb.index()] != new_idom {
                idom[bb.index()] = new_idom;
                changed = true;
            }
        }
    }

    // 入口は誰にも支配されない、という形にしておく。
    idom[0] = None;
    idom
}

fn intersect(
    mut a: BasicBlock,
    mut b: BasicBlock,
    idom: &[Option<BasicBlock>],
    rpo_index: &[usize],
) -> BasicBlock {
    while a != b {
        while rpo_index[a.index()] > rpo_index[b.index()] {
            match idom[a.index()] {
                Some(next) if next != a => a = next,
                _ => return b,
            }
        }
        while rpo_index[b.index()] > rpo_index[a.index()] {
            match idom[b.index()] {
                Some(next) if next != b => b = next,
                _ => return a,
            }
        }
    }
    a
}

/// `a` が `b` を支配するか。
fn dominates(a: BasicBlock, b: BasicBlock, idom: &[Option<BasicBlock>]) -> bool {
    if a == b {
        return true;
    }
    let mut cur = b;
    let mut guard = 0;
    while let Some(next) = idom[cur.index()] {
        if next == a {
            return true;
        }
        if next == cur {
            break;
        }
        cur = next;
        guard += 1;
        if guard > idom.len() {
            break;
        }
    }
    false
}

/// 木に現れる基本ブロックの集合。検証に使う。
pub fn blocks_in(tree: &Structured) -> HashSet<BasicBlock> {
    let mut out = HashSet::new();
    collect_blocks(tree, &mut out);
    out
}

fn collect_blocks(tree: &Structured, out: &mut HashSet<BasicBlock>) {
    match tree {
        Structured::Simple(bb) => {
            out.insert(*bb);
        }
        Structured::Block(items) => {
            for i in items {
                collect_blocks(i, out);
            }
        }
        Structured::Loop(inner) => collect_blocks(inner, out),
        Structured::If { bb, then, els } => {
            out.insert(*bb);
            for i in then.iter().chain(els) {
                collect_blocks(i, out);
            }
        }
        Structured::Br(_) | Structured::Return | Structured::Unreachable => {}
    }
}

/// `br` の相対深さが、囲みの数を超えていないか。
///
/// 超えていれば WASM の検証で落ちるので、出す前に見ておく。
pub fn check_branch_depths(tree: &Structured) -> Result<(), u32> {
    fn walk(tree: &Structured, depth: u32) -> Result<(), u32> {
        match tree {
            Structured::Br(d) => {
                if *d >= depth {
                    return Err(*d);
                }
                Ok(())
            }
            Structured::Block(items) => {
                for i in items {
                    walk(i, depth + 1)?;
                }
                Ok(())
            }
            Structured::Loop(inner) => walk(inner, depth + 1),
            Structured::If { then, els, .. } => {
                for i in then.iter().chain(els) {
                    walk(i, depth + 1)?;
                }
                Ok(())
            }
            Structured::Simple(_) | Structured::Return | Structured::Unreachable => Ok(()),
        }
    }
    walk(tree, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use biwac_hir::{Ty, TyKind};
    use biwac_mir::{BasicBlockData, LocalDecl, Operand, SwitchTargets};
    use biwac_span::{DefId, PackageLocalDefId, Span, ValDefId};

    /// 終端子だけを与えて body を組む。
    /// この変換は文にも型にも触らないので、それだけあれば十分である。
    fn body_of(terms: Vec<TerminatorKind>) -> Body {
        Body {
            def_id: ValDefId::new(DefId::new_in_self_pkg(PackageLocalDefId::new(0))),
            arg_count: 0,
            locals: vec![LocalDecl {
                ty: Ty::new(TyKind::Void, Span::dummy()),
                span: Span::dummy(),
            }],
            blocks: terms
                .into_iter()
                .map(|kind| BasicBlockData::new(kind.with_span(Span::dummy())))
                .collect(),
            genargs: Vec::new(),
            span: Span::dummy(),
        }
    }

    fn goto(n: u32) -> TerminatorKind {
        TerminatorKind::Goto {
            target: BasicBlock::new(n),
        }
    }

    /// 偽で `f`、真で `t`。MIR の `if` の形と同じ。
    fn branch(t: u32, f: u32) -> TerminatorKind {
        TerminatorKind::SwitchInt {
            discr: Operand::Const(biwac_mir::Const::Bool(true)),
            targets: SwitchTargets::if_bool(BasicBlock::new(t), BasicBlock::new(f)),
        }
    }

    fn bb(n: u32) -> BasicBlock {
        BasicBlock::new(n)
    }

    /// 出た木が WASM の検証を通せる形か。
    fn well_formed(tree: &Structured, body: &Body) {
        check_branch_depths(tree).expect("branch depth must stay inside its enclosing blocks");

        // 到達可能なブロックがすべて木に現れること。
        let cfg = Cfg::of(body);
        let reachable: HashSet<BasicBlock> = (0..body.blocks.len())
            .map(|i| bb(i as u32))
            .filter(|b| cfg.reachable(*b))
            .collect();
        assert_eq!(
            blocks_in(tree),
            reachable,
            "every reachable block must appear"
        );
    }

    #[test]
    fn straight_line() {
        let body = body_of(vec![goto(1), TerminatorKind::Return]);
        let tree = structure(&body).unwrap();
        well_formed(&tree, &body);
    }

    #[test]
    fn if_else_join() {
        //   bb0 --> bb1 --\
        //       \-> bb2 --> bb3
        let body = body_of(vec![branch(1, 2), goto(3), goto(3), TerminatorKind::Return]);
        let tree = structure(&body).unwrap();
        well_formed(&tree, &body);

        // 合流点 bb3 は `block` で包まれ、両枝からは `br` で抜ける。
        let Structured::Block(items) = &tree else {
            panic!("{tree:?}");
        };
        assert!(
            matches!(items.first(), Some(Structured::Block(_))),
            "the join must be wrapped in a block: {tree:?}"
        );
    }

    #[test]
    fn while_loop() {
        //   bb0 --> bb1(head) --> bb2(body) --> bb1
        //                     \-> bb3(exit)
        let body = body_of(vec![goto(1), branch(2, 3), goto(1), TerminatorKind::Return]);
        let tree = structure(&body).unwrap();
        well_formed(&tree, &body);

        // ループの頭が `loop` で包まれていること。
        fn has_loop(t: &Structured) -> bool {
            match t {
                Structured::Loop(_) => true,
                Structured::Block(items) => items.iter().any(has_loop),
                Structured::If { then, els, .. } => then.iter().chain(els).any(has_loop),
                _ => false,
            }
        }
        assert!(has_loop(&tree), "a back edge must produce a loop: {tree:?}");
    }

    #[test]
    fn nested_if_with_shared_join() {
        //   bb0 -> bb1 -> {bb3, bb4} -> bb5 -> bb6
        //       \-> bb2 -----------------^
        let body = body_of(vec![
            branch(1, 2),
            branch(3, 4),
            goto(5),
            goto(5),
            goto(5),
            goto(6),
            TerminatorKind::Return,
        ]);
        let tree = structure(&body).unwrap();
        well_formed(&tree, &body);
    }

    #[test]
    fn early_return_leaves_no_join() {
        //   bb0 -> bb1(return)
        //       \-> bb2(return)
        let body = body_of(vec![
            branch(1, 2),
            TerminatorKind::Return,
            TerminatorKind::Return,
        ]);
        let tree = structure(&body).unwrap();
        well_formed(&tree, &body);

        // 合流しないので `block` で包む必要が無い。
        let Structured::Block(items) = &tree else {
            panic!("{tree:?}");
        };
        assert!(
            matches!(items.first(), Some(Structured::Simple(_))),
            "no join means no extra block: {tree:?}"
        );
    }

    #[test]
    fn unreachable_blocks_are_ignored() {
        let body = body_of(vec![
            TerminatorKind::Return,
            // 誰からも辿られない。
            goto(0),
        ]);
        let tree = structure(&body).unwrap();
        well_formed(&tree, &body);
        assert_eq!(blocks_in(&tree), HashSet::from([bb(0)]));
    }

    #[test]
    fn loop_with_inner_branch() {
        //   bb0 -> bb1(head) -> bb2 -> {bb3, bb4} -> bb5 -> bb1
        //                    \-> bb6(exit)
        let body = body_of(vec![
            goto(1),
            branch(2, 6),
            branch(3, 4),
            goto(5),
            goto(5),
            goto(1),
            TerminatorKind::Return,
        ]);
        let tree = structure(&body).unwrap();
        well_formed(&tree, &body);
    }
}
