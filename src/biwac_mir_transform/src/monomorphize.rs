//! 単相化 (monomorphization)。
//!
//! ジェネリックな MIR から、実際に使われる型で実体化した
//! **プログラム全体** の MIR を作る。
//!
//! # なぜプログラム全体なのか
//!
//! biwa には可視性修飾子が無く、ライブラリ単体では
//! どの型で実体化されるかが決まらない。`std::Option::none[T]` の `T` を知っているのは
//! 呼ぶ側だけである。したがって
//!
//! - ライブラリの `.biwamir` は **ジェネリックなまま** 出荷するしかない
//! - 実体化できるのは根を持つパッケージ、つまり `scene main` を持つ
//!   playable パッケージだけである
//! - `Pair::new[Int, Int]` のような上流のシンボルの実体は、
//!   実際に依存している下流のこの結果に入る
//!
//! # id 空間
//!
//! 自パッケージのシンボルは `SELF`、依存のシンボルは
//! `(本当の PackageId, .biwameta のシンボル索引)` である。
//! `SELF` は 0、依存はハッシュ由来で 2 以上なので衝突しない。
//! 定義を引くときだけ `is_self()` で分岐する。

use std::collections::{HashMap, HashSet};

use biwac_base::{IdentInterner, InternedIdent, PackageId};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{DefinedTy, FnTy, Hir, Ty, TyDefKind, TyKind};
use biwac_mir::{
    Body, Callee, Const, GenArgs, InstanceKey, Mir, MirItem, MonoInstance, MonoMir, MonoTyDef,
    MonoTyDefKind, NativeItem, Operand, Place, PlaceElem, Rvalue, StatementKind, StringPool,
    TerminatorKind, TyInstanceKey,
};
use biwac_span::{LocalGenDefId, TyDefId, ValDefId};

/// 実体が無限に増えるのを止める上限。
///
/// `fn f[T]() { f[Vec[T]]() }` のような再帰は型が毎回変わるので、
/// 見たものを覚えるだけでは止まらない。
const INSTANCE_LIMIT: usize = 10_000;

/// 単相化の入力。
pub struct MonoInput<'a> {
    /// 自パッケージの HIR。自パッケージの型定義を引くのに使う。
    pub hir: &'a Hir,

    /// 自パッケージの MIR。シンボルは `SELF`。
    pub own: &'a Mir,

    /// 依存パッケージの `.biwameta` と `.biwamir`。
    pub deps: &'a [(PackageId, &'a DepMetadata, &'a Mir)],

    /// 根。playable パッケージの `scene main` など。
    pub roots: &'a [ValDefId],

    /// 依存の型定義を復元するのに要る。
    pub interner: &'a mut IdentInterner,
}

#[derive(Debug)]
pub enum MonoError {
    /// 根が 1 つも無い。ライブラリパッケージを単相化しようとした。
    NoRoots,
    /// 呼び先の本体が見つからない。依存の `.biwamir` が古いか欠けている。
    MissingBody { def_id: ValDefId },
    /// 型定義が見つからない。
    MissingTyDef { def_id: TyDefId },
    /// 呼び出し位置でジェネリック引数が決まらなかった。
    UnresolvedGenericArg {
        caller: ValDefId,
        callee: ValDefId,
        param: LocalGenDefId,
    },
    /// 間接呼び出しはまだ実体化できない。
    IndirectCall { caller: ValDefId },
    /// 実体が増えすぎた。
    TooManyInstances { limit: usize },
}

impl std::fmt::Display for MonoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRoots => write!(
                f,
                "nothing to monomorphize: this package has no entry point \
                 (only a playable package can be monomorphized)"
            ),
            Self::MissingBody { def_id } => write!(
                f,
                "the body of val#{} is missing (a dependency's .biwamir may be stale)",
                def_id.value()
            ),
            Self::MissingTyDef { def_id } => {
                write!(f, "the definition of ty#{} is missing", def_id.value())
            }
            Self::UnresolvedGenericArg {
                caller,
                callee,
                param,
            } => write!(
                f,
                "val#{}: the generic argument T{} of val#{} was not determined at the call site",
                caller.value(),
                param.value(),
                callee.value()
            ),
            Self::IndirectCall { caller } => write!(
                f,
                "val#{}: indirect calls cannot be monomorphized yet",
                caller.value()
            ),
            Self::TooManyInstances { limit } => write!(
                f,
                "more than {limit} instances were produced; \
                 a generic function probably instantiates itself with a growing type"
            ),
        }
    }
}

pub fn monomorphize(input: MonoInput) -> Result<MonoMir, Vec<MonoError>> {
    if input.roots.is_empty() {
        return Err(vec![MonoError::NoRoots]);
    }
    Collector::new(input).run()
}

struct Collector<'a> {
    hir: &'a Hir,
    own: &'a Mir,
    deps: &'a [(PackageId, &'a DepMetadata, &'a Mir)],
    interner: &'a mut IdentInterner,

    strings: StringPool,
    /// パッケージごとの「元の文字列索引 → 統合後の索引」。
    string_remap: HashMap<(PackageId, u32), biwac_mir::StrId>,

    instances: Vec<MonoInstance>,
    seen_instances: HashSet<InstanceKey>,
    fn_worklist: Vec<InstanceKey>,

    types: Vec<MonoTyDef>,
    seen_types: HashSet<TyInstanceKey>,
    ty_worklist: Vec<TyInstanceKey>,

    errors: Vec<MonoError>,
}

impl<'a> Collector<'a> {
    fn new(input: MonoInput<'a>) -> Self {
        Self {
            hir: input.hir,
            own: input.own,
            deps: input.deps,
            interner: input.interner,
            strings: StringPool::default(),
            string_remap: HashMap::new(),
            instances: Vec::new(),
            seen_instances: HashSet::new(),
            fn_worklist: input
                .roots
                .iter()
                .map(|def_id| InstanceKey::new(*def_id, Vec::new()))
                .collect(),
            types: Vec::new(),
            seen_types: HashSet::new(),
            ty_worklist: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn run(mut self) -> Result<MonoMir, Vec<MonoError>> {
        // 根は必ず先頭に並ぶようにしたいので、逆順に積んで pop する。
        self.fn_worklist.reverse();
        let entry_key = self.fn_worklist.last().cloned();

        while let Some(key) = self.fn_worklist.pop() {
            if !self.seen_instances.insert(key.clone()) {
                continue;
            }
            if self.instances.len() >= INSTANCE_LIMIT {
                self.errors.push(MonoError::TooManyInstances {
                    limit: INSTANCE_LIMIT,
                });
                break;
            }
            self.instantiate(key);
        }

        // 関数から現れた型を閉じるまで辿る。
        while let Some(key) = self.ty_worklist.pop() {
            self.instantiate_ty(key);
        }

        if !self.errors.is_empty() {
            return Err(self.errors);
        }

        let entry = entry_key.and_then(|k| self.instances.iter().position(|i| i.key == k));

        // 実体を提供したパッケージの前置コードだけを集める。
        //
        // 使われないパッケージの import まで並べると、
        // ホストが用意していない関数を要求してインスタンス化に失敗する。
        let mut contributing: Vec<PackageId> = self
            .instances
            .iter()
            .map(|i| i.key.def_id.pkg())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        contributing.sort_by_key(|p| {
            // 自パッケージ (SELF = 0) を最後に置く。
            // 依存が宣言したものを先に並べるほうが読みやすい。
            if p.is_self() { u32::MAX } else { p.value() }
        });

        let mut module_natives = Vec::new();
        for pkg in contributing {
            let natives = if pkg.is_self() {
                &self.own.module_natives
            } else {
                match self.dep_of(pkg) {
                    Some((_, mir)) => &mir.module_natives,
                    None => continue,
                }
            };
            module_natives.extend(natives.iter().cloned());
        }

        Ok(MonoMir {
            instances: self.instances,
            types: self.types,
            strings: self.strings,
            entry,
            module_natives,
        })
    }

    // ---- 定義の引き方 ----
    //
    // 自パッケージは HIR と自分の MIR、依存は `.biwameta` と decode した MIR。

    fn dep_of(&self, pkg: PackageId) -> Option<(&'a DepMetadata, &'a Mir)> {
        self.deps
            .iter()
            .find(|(id, _, _)| *id == pkg)
            .map(|(_, meta, mir)| (*meta, *mir))
    }

    fn item_of(&self, def_id: ValDefId) -> Option<&'a MirItem> {
        if def_id.pkg().is_self() {
            self.own.items.get(&def_id)
        } else {
            self.dep_of(def_id.pkg())
                .and_then(|(_, mir)| mir.items.get(&def_id))
        }
    }

    fn ty_def_of(&mut self, def_id: TyDefId) -> Option<TyDefKind> {
        if def_id.pkg().is_self() {
            return self.hir.tys.get(&def_id).and_then(|t| t.ty_content.clone());
        }
        let (meta, _) = self.dep_of(def_id.pkg())?;
        meta.get_ext_ty_impl(def_id.local_idx(), def_id.pkg(), self.interner)
            .and_then(|t| t.ty_content)
    }

    fn owning_package(&self, def_id: ValDefId) -> PackageId {
        def_id.pkg()
    }

    // ---- 文字列 ----

    fn remap_string(&mut self, owner: PackageId, id: biwac_mir::StrId) -> biwac_mir::StrId {
        if let Some(new) = self.string_remap.get(&(owner, id.value())) {
            return *new;
        }
        let text = self.string_of(owner, id).unwrap_or_default();
        let new = self.strings.intern(&text);
        self.string_remap.insert((owner, id.value()), new);
        new
    }

    fn string_of(&self, owner: PackageId, id: biwac_mir::StrId) -> Option<String> {
        let pool = if owner.is_self() {
            &self.own.strings
        } else {
            &self.dep_of(owner)?.1.strings
        };
        pool.get(id).map(|s| s.to_string())
    }

    // ---- 関数の実体化 ----

    fn instantiate(&mut self, key: InstanceKey) {
        let Some(item) = self.item_of(key.def_id) else {
            self.errors
                .push(MonoError::MissingBody { def_id: key.def_id });
            return;
        };

        let owner = self.owning_package(key.def_id);
        let subst: HashMap<LocalGenDefId, Ty> = key.args.iter().cloned().collect();

        let item = match item {
            MirItem::Native(n) => MirItem::Native(self.subst_native(n, &subst, owner)),
            MirItem::Body(b) => MirItem::Body(self.subst_body(b, &subst, owner, key.def_id)),
        };

        self.instances.push(MonoInstance { key, item });
    }

    fn subst_native(
        &mut self,
        n: &NativeItem,
        subst: &HashMap<LocalGenDefId, Ty>,
        _owner: PackageId,
    ) -> NativeItem {
        NativeItem {
            def_id: n.def_id,
            self_ty: n.self_ty.as_ref().map(|t| self.subst_ty(t, subst)),
            args: n.args.iter().map(|t| self.subst_ty(t, subst)).collect(),
            rty: self.subst_ty(&n.rty, subst),
            // 実体化したので、この本体にジェネリック引数はもう無い。
            genargs: Vec::new(),
            native_body: n.native_body.clone(),
            native_span: n.native_span.clone(),
            span: n.span.clone(),
        }
    }

    fn subst_body(
        &mut self,
        body: &Body,
        subst: &HashMap<LocalGenDefId, Ty>,
        owner: PackageId,
        caller: ValDefId,
    ) -> Body {
        let locals = body
            .locals
            .iter()
            .map(|l| biwac_mir::LocalDecl {
                ty: self.subst_ty(&l.ty, subst),
                span: l.span.clone(),
            })
            .collect();

        let blocks = body
            .blocks
            .iter()
            .map(|block| {
                let stmts = block
                    .stmts
                    .iter()
                    .map(|stmt| {
                        let StatementKind::Assign(place, rvalue) = &stmt.kind;
                        let place = self.subst_place(place, subst);
                        let rvalue = self.subst_rvalue(rvalue, subst, owner);
                        StatementKind::Assign(place, rvalue).with_span(stmt.span.clone())
                    })
                    .collect();

                let term = self
                    .subst_terminator(&block.term.kind, subst, owner, caller)
                    .with_span(block.term.span.clone());

                biwac_mir::BasicBlockData { stmts, term }
            })
            .collect();

        Body {
            def_id: body.def_id,
            arg_count: body.arg_count,
            locals,
            blocks,
            // 実体化したので多相ではなくなる。
            genargs: Vec::new(),
            span: body.span.clone(),
        }
    }

    fn subst_terminator(
        &mut self,
        term: &TerminatorKind,
        subst: &HashMap<LocalGenDefId, Ty>,
        owner: PackageId,
        caller: ValDefId,
    ) -> TerminatorKind {
        match term {
            TerminatorKind::Goto { target } => TerminatorKind::Goto { target: *target },
            TerminatorKind::Return => TerminatorKind::Return,
            TerminatorKind::Unreachable => TerminatorKind::Unreachable,
            TerminatorKind::SwitchInt { discr, targets } => TerminatorKind::SwitchInt {
                discr: self.subst_operand(discr, subst, owner),
                targets: targets.clone(),
            },
            TerminatorKind::Call {
                callee,
                args,
                dest,
                target,
            } => {
                let callee = match callee {
                    Callee::Direct { def_id, genargs } => {
                        let genargs = self.subst_genargs(genargs, subst, caller, *def_id);
                        // 呼び先も実体化する。
                        self.fn_worklist
                            .push(InstanceKey::new(*def_id, genargs.clone()));
                        Callee::Direct {
                            def_id: *def_id,
                            genargs,
                        }
                    }
                    Callee::Indirect(op) => {
                        self.errors.push(MonoError::IndirectCall { caller });
                        Callee::Indirect(self.subst_operand(op, subst, owner))
                    }
                };
                TerminatorKind::Call {
                    callee,
                    args: args
                        .iter()
                        .map(|a| self.subst_operand(a, subst, owner))
                        .collect(),
                    dest: self.subst_place(dest, subst),
                    target: *target,
                }
            }
        }
    }

    /// 呼び出し位置のジェネリック引数を、呼び出し側の置換で具体化する。
    fn subst_genargs(
        &mut self,
        genargs: &GenArgs,
        subst: &HashMap<LocalGenDefId, Ty>,
        caller: ValDefId,
        callee: ValDefId,
    ) -> GenArgs {
        let mut out: GenArgs = genargs
            .iter()
            .map(|(g, ty)| (*g, self.subst_ty(ty, subst)))
            .collect();
        out.sort_by_key(|(g, _)| g.value());

        for (param, ty) in &out {
            if !is_concrete(ty) {
                self.errors.push(MonoError::UnresolvedGenericArg {
                    caller,
                    callee,
                    param: *param,
                });
            }
        }
        out
    }

    fn subst_place(&mut self, place: &Place, subst: &HashMap<LocalGenDefId, Ty>) -> Place {
        Place {
            local: place.local,
            projection: place
                .projection
                .iter()
                .map(|elem| {
                    let PlaceElem::Field(name, ty) = elem;
                    PlaceElem::Field(*name, self.subst_ty(ty, subst))
                })
                .collect(),
        }
    }

    fn subst_rvalue(
        &mut self,
        rvalue: &Rvalue,
        subst: &HashMap<LocalGenDefId, Ty>,
        owner: PackageId,
    ) -> Rvalue {
        match rvalue {
            Rvalue::Use(op) => Rvalue::Use(self.subst_operand(op, subst, owner)),
            Rvalue::UnaryOp(op, o) => Rvalue::UnaryOp(*op, self.subst_operand(o, subst, owner)),
            Rvalue::BinaryOp(op, l, r) => Rvalue::BinaryOp(
                *op,
                self.subst_operand(l, subst, owner),
                self.subst_operand(r, subst, owner),
            ),
            Rvalue::Aggregate(def_id, members) => Rvalue::Aggregate(
                *def_id,
                members
                    .iter()
                    .map(|(name, op)| (*name, self.subst_operand(op, subst, owner)))
                    .collect(),
            ),
        }
    }

    fn subst_operand(
        &mut self,
        op: &Operand,
        subst: &HashMap<LocalGenDefId, Ty>,
        owner: PackageId,
    ) -> Operand {
        match op {
            Operand::Place(p) => Operand::Place(self.subst_place(p, subst)),
            // 文字列はパッケージ相対なので、統合後の索引に付け替える。
            Operand::Const(Const::Str(id)) => {
                Operand::Const(Const::Str(self.remap_string(owner, *id)))
            }
            Operand::Const(Const::FnDef(def_id, genargs)) => {
                let genargs = self.subst_genargs(genargs, subst, *def_id, *def_id);
                self.fn_worklist
                    .push(InstanceKey::new(*def_id, genargs.clone()));
                Operand::Const(Const::FnDef(*def_id, genargs))
            }
            Operand::Const(c) => Operand::Const(c.clone()),
        }
    }

    // ---- 型 ----

    fn subst_ty(&mut self, ty: &Ty, subst: &HashMap<LocalGenDefId, Ty>) -> Ty {
        let kind = match &ty.kind {
            TyKind::LocGen(lgid) => match subst.get(lgid) {
                Some(assigned) => assigned.kind.clone(),
                None => ty.kind.clone(),
            },
            TyKind::Defined(dt) => {
                let genargs: Vec<Ty> = dt.genargs.iter().map(|g| self.subst_ty(g, subst)).collect();
                let key = TyInstanceKey {
                    def_id: dt.def_id,
                    args: genargs.clone(),
                };
                if genargs.iter().all(is_concrete) && self.seen_types.insert(key.clone()) {
                    self.ty_worklist.push(key);
                }
                TyKind::Defined(DefinedTy {
                    def_id: dt.def_id,
                    genargs,
                })
            }
            TyKind::Fn(f) => TyKind::Fn(FnTy {
                args: f.args.iter().map(|a| self.subst_ty(a, subst)).collect(),
                rty: Box::new(self.subst_ty(&f.rty, subst)),
                genargs: f.genargs.clone(),
            }),
            other => other.clone(),
        };
        Ty::new(kind, ty.span.clone())
    }

    /// 具体化された型の定義を作る。
    ///
    /// メンバの型に現れる [`TyKind::Gen`] は、型定義のジェネリック引数宣言の
    /// 位置で `key.args` に対応させて置き換える。
    fn instantiate_ty(&mut self, key: TyInstanceKey) {
        let Some(content) = self.ty_def_of(key.def_id) else {
            // プリミティブ型 (`ty_content` が None) はここに来ない。
            // 来たなら定義が引けていない。
            self.errors
                .push(MonoError::MissingTyDef { def_id: key.def_id });
            return;
        };

        let kind = match content {
            TyDefKind::Struct(s) => {
                let gen_subst: HashMap<biwac_span::GenDefId, Ty> = s
                    .genargs
                    .iter()
                    .zip(key.args.iter())
                    .map(|(g, ty)| (*g, ty.clone()))
                    .collect();

                // 並べ替えは **具体化する前に** 行う。
                // HIR 側の表が HashMap なので、その順に具体化すると
                // 具体化の過程で見つかる型の発見順が実行ごとに変わってしまう。
                let mut raw: Vec<(InternedIdent, Ty)> = s
                    .members
                    .iter()
                    .map(|(name, ty)| (*name, ty.clone()))
                    .collect();
                raw.sort_by_key(|(name, _)| self.member_order(*name));

                let members = raw
                    .into_iter()
                    .map(|(name, ty)| (name, self.subst_ty_gen(&ty, &gen_subst)))
                    .collect();

                MonoTyDefKind::Struct { members }
            }
            TyDefKind::NativeTypeAlias(a) => MonoTyDefKind::Native {
                code: a.native.clone(),
            },
        };

        self.types.push(MonoTyDef { key, kind });
    }

    /// メンバの並び順。名前の文字列で決める。
    fn member_order(&self, name: InternedIdent) -> String {
        self.interner
            .get_str(&name)
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    /// 型定義側のジェネリック引数 ([`TyKind::Gen`]) を置き換える。
    fn subst_ty_gen(&mut self, ty: &Ty, subst: &HashMap<biwac_span::GenDefId, Ty>) -> Ty {
        let kind = match &ty.kind {
            TyKind::Gen(gid) => match subst.get(gid) {
                Some(assigned) => assigned.kind.clone(),
                None => ty.kind.clone(),
            },
            TyKind::Defined(dt) => {
                let genargs: Vec<Ty> = dt
                    .genargs
                    .iter()
                    .map(|g| self.subst_ty_gen(g, subst))
                    .collect();
                let key = TyInstanceKey {
                    def_id: dt.def_id,
                    args: genargs.clone(),
                };
                if genargs.iter().all(is_concrete) && self.seen_types.insert(key.clone()) {
                    self.ty_worklist.push(key);
                }
                TyKind::Defined(DefinedTy {
                    def_id: dt.def_id,
                    genargs,
                })
            }
            TyKind::Fn(f) => TyKind::Fn(FnTy {
                args: f.args.iter().map(|a| self.subst_ty_gen(a, subst)).collect(),
                rty: Box::new(self.subst_ty_gen(&f.rty, subst)),
                genargs: f.genargs.clone(),
            }),
            other => other.clone(),
        };
        Ty::new(kind, ty.span.clone())
    }
}

/// ジェネリック型も推論待ちの型も残っていないか。
fn is_concrete(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Int | TyKind::Float | TyKind::Bool | TyKind::Void => true,
        TyKind::Gen(_) | TyKind::LocGen(_) | TyKind::Infer(_) => false,
        TyKind::Defined(dt) => dt.genargs.iter().all(is_concrete),
        TyKind::Fn(f) => f.args.iter().all(is_concrete) && is_concrete(&f.rty),
    }
}
