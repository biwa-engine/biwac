use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;

use biwac_base::{InternedIdent, PackageId};
use biwac_hir::{Ty, TyKind};
use biwac_span::{GenDefId, LocalGenDefId, TyDefId, ValDefId};

use crate::codec::{BIWAC_MIR_FORMAT_VERSION, EncodeCtx};
use crate::{
    AggregateKind, BinOp, Body, Callee, Const, GenArgs, Mir, MirItem, NativeItem, Operand, Place,
    PlaceElem, Rvalue, StatementKind, TerminatorKind, UnOp,
};

/// 型表の 1 行。パッケージ別名を振る前なので、生の [`PackageId`] を持つ。
#[derive(PartialEq, Eq, Hash)]
enum TyEntry {
    Int,
    Float,
    Bool,
    Void,
    Def {
        pkg: u32,
        sym: u32,
        args: Vec<usize>,
    },
    Gen {
        pkg: u32,
        owner: u32,
        ord: u32,
    },
    LocGen {
        pkg: u32,
        owner: u32,
        ord: u32,
    },
    Fn {
        args: Vec<usize>,
        rty: usize,
    },
}

/// ジェネリック引数の参照。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GenArgRef {
    pkg: u32,
    owner: u32,
    ord: u32,
}

type GaEntry = Vec<(GenArgRef, usize)>;

struct Encoder<'a> {
    ctx: &'a EncodeCtx<'a>,
    /// いま書いている項目のシンボル索引。
    ///
    /// ローカルジェネリック引数の id は「どの関数から見たものか」で決まるので、
    /// 型を表に入れるときにこれが要る。
    /// 呼び出しのジェネリック引数だけは呼び先の索引で解決する。
    owner: Option<u32>,
    /// PackageId の生値 → 別名。自パッケージが 0、残りは id 昇順。
    alias: HashMap<u32, u32>,
    tys: Vec<TyEntry>,
    ty_index: HashMap<TyEntry, usize>,
    gas: Vec<GaEntry>,
    ga_index: HashMap<GaEntry, usize>,
}

pub(super) fn encode(mir: &Mir, ctx: &EncodeCtx) -> String {
    // パッケージ別名は、行を書き始める前に決まっていなければならない。
    // 型表にもシンボル参照が出るので、先に軽い走査で集める。
    let mut pkgs = BTreeSet::new();
    collect_packages(mir, ctx, &mut pkgs);
    pkgs.remove(&ctx.pkg_id.value());

    let mut alias = HashMap::new();
    alias.insert(ctx.pkg_id.value(), 0u32);
    let mut ordered = vec![(0u32, ctx.pkg_id.value())];
    for (i, pkg) in pkgs.iter().enumerate() {
        let a = i as u32 + 1;
        alias.insert(*pkg, a);
        ordered.push((a, *pkg));
    }

    let mut enc = Encoder {
        ctx,
        owner: None,
        alias,
        tys: Vec::new(),
        ty_index: HashMap::new(),
        gas: Vec::new(),
        ga_index: HashMap::new(),
    };

    // 本体を先に組み立てる。型表とジェネリック引数表はこの過程で埋まる。
    //
    // items は **写像後のシンボル索引順** に出す。
    // メモリ上のキー順 (自パッケージなら collector の連番順) ではない。
    let mut items: Vec<(u32, &MirItem)> = mir
        .items
        .iter()
        .map(|(def_id, item)| (enc.val_sym(def_id).1, item))
        .collect();
    items.sort_by_key(|(sym, _)| *sym);

    let mut body = String::new();
    for (_, item) in &items {
        body.push('\n');
        match item {
            MirItem::Body(b) => {
                enc.owner = Some(enc.val_sym(&b.def_id).1);
                enc.emit_body(&mut body, b);
            }
            MirItem::Native(n) => {
                enc.owner = Some(enc.val_sym(&n.def_id).1);
                enc.emit_native(&mut body, n);
            }
        }
    }

    // ヘッダと各表を前に置く。
    let mut out = String::new();
    let _ = writeln!(out, "biwamir {BIWAC_MIR_FORMAT_VERSION}");
    for (a, pkg) in &ordered {
        let name = if *pkg == ctx.pkg_id.value() {
            mir.pkg_name.value().to_string()
        } else {
            "-".to_string()
        };
        let _ = writeln!(out, "pkg {a} {name} {pkg}");
    }
    let _ = writeln!(out, "meta-svh {:016x}", ctx.meta_svh.as_u64());

    if !mir.module_natives.is_empty() {
        out.push('\n');
        for native in &mir.module_natives {
            let _ = writeln!(out, "modnative {native:?}");
        }
    }

    if !mir.strings.is_empty() {
        out.push('\n');
        for (id, s) in mir.strings.iter() {
            let _ = writeln!(out, "str {} {:?}", id.value(), s);
        }
    }

    if !enc.tys.is_empty() {
        out.push('\n');
        for (i, entry) in enc.tys.iter().enumerate() {
            let _ = writeln!(out, "ty {i} {}", enc.render_ty_entry(entry));
        }
    }

    if !enc.gas.is_empty() {
        out.push('\n');
        for (i, entry) in enc.gas.iter().enumerate() {
            let mut line = format!("ga {i}");
            for (g, ty) in entry {
                let _ = write!(line, " {} {}", enc.render_genarg(g), ty);
            }
            let _ = writeln!(out, "{line}");
        }
    }

    out.push_str(&body);
    out
}

/// 別名を振るために、現れる [`PackageId`] だけを集める。
fn collect_packages(mir: &Mir, ctx: &EncodeCtx, out: &mut BTreeSet<u32>) {
    let push_ty = |ty: &Ty, out: &mut BTreeSet<u32>| {
        collect_ty_packages(ty, ctx, out);
    };

    for (def_id, item) in &mir.items {
        out.insert(resolve_pkg(def_id.pkg(), ctx));
        match item {
            MirItem::Native(n) => {
                if let Some(t) = &n.self_ty {
                    push_ty(t, out);
                }
                for t in &n.args {
                    push_ty(t, out);
                }
                push_ty(&n.rty, out);
                for g in &n.genargs {
                    out.insert(resolve_pkg(g.pkg(), ctx));
                }
            }
            MirItem::Body(b) => {
                for l in &b.locals {
                    push_ty(&l.ty, out);
                }
                for g in &b.genargs {
                    out.insert(resolve_pkg(g.pkg(), ctx));
                }
                for block in &b.blocks {
                    for stmt in &block.stmts {
                        let StatementKind::Assign(place, rvalue) = &stmt.kind;
                        collect_place_packages(place, ctx, out);
                        collect_rvalue_packages(rvalue, ctx, out);
                    }
                    match &block.term.kind {
                        TerminatorKind::SwitchInt { discr, .. } => {
                            collect_operand_packages(discr, ctx, out)
                        }
                        TerminatorKind::Call {
                            callee, args, dest, ..
                        } => {
                            match callee {
                                Callee::Direct { def_id, genargs } => {
                                    out.insert(resolve_pkg(def_id.pkg(), ctx));
                                    collect_genargs_packages(genargs, ctx, out);
                                }
                                Callee::TraitAssoc {
                                    assoc,
                                    self_ty,
                                    genargs,
                                } => {
                                    out.insert(resolve_pkg(assoc.pkg(), ctx));
                                    collect_ty_packages(self_ty, ctx, out);
                                    collect_genargs_packages(genargs, ctx, out);
                                }
                                Callee::Indirect(op) => collect_operand_packages(op, ctx, out),
                            }
                            for a in args {
                                collect_operand_packages(a, ctx, out);
                            }
                            collect_place_packages(dest, ctx, out);
                        }
                        TerminatorKind::Goto { .. }
                        | TerminatorKind::Return
                        | TerminatorKind::Unreachable => {}
                    }
                }
            }
        }
    }
}

fn resolve_pkg(pkg: PackageId, ctx: &EncodeCtx) -> u32 {
    if pkg.is_self() {
        ctx.pkg_id.value()
    } else {
        pkg.value()
    }
}

fn collect_ty_packages(ty: &Ty, ctx: &EncodeCtx, out: &mut BTreeSet<u32>) {
    match &ty.kind {
        TyKind::Defined(dt) => {
            out.insert(resolve_pkg(dt.def_id.pkg(), ctx));
            for g in &dt.genargs {
                collect_ty_packages(g, ctx, out);
            }
        }
        TyKind::Gen(g) => {
            out.insert(resolve_pkg(g.pkg(), ctx));
        }
        TyKind::LocGen(g) => {
            out.insert(resolve_pkg(g.pkg(), ctx));
        }
        TyKind::Fn(f) => {
            for a in &f.args {
                collect_ty_packages(a, ctx, out);
            }
            collect_ty_packages(&f.rty, ctx, out);
        }
        TyKind::Int | TyKind::Float | TyKind::Bool | TyKind::Void | TyKind::Infer(_) => {}
    }
}

fn collect_place_packages(place: &Place, ctx: &EncodeCtx, out: &mut BTreeSet<u32>) {
    for elem in &place.projection {
        // downcast は型を持たない。直後の Field が型を持っている。
        if let PlaceElem::Field(_, ty) = elem {
            collect_ty_packages(ty, ctx, out);
        }
    }
}

fn collect_operand_packages(op: &Operand, ctx: &EncodeCtx, out: &mut BTreeSet<u32>) {
    match op {
        Operand::Place(p) => collect_place_packages(p, ctx, out),
        Operand::Const(Const::FnDef(def_id, genargs)) => {
            out.insert(resolve_pkg(def_id.pkg(), ctx));
            collect_genargs_packages(genargs, ctx, out);
        }
        Operand::Const(_) => {}
    }
}

fn collect_rvalue_packages(rvalue: &Rvalue, ctx: &EncodeCtx, out: &mut BTreeSet<u32>) {
    match rvalue {
        Rvalue::Use(op) | Rvalue::UnaryOp(_, op) => collect_operand_packages(op, ctx, out),
        Rvalue::BinaryOp(_, l, r) => {
            collect_operand_packages(l, ctx, out);
            collect_operand_packages(r, ctx, out);
        }
        Rvalue::Aggregate(kind, members) => {
            out.insert(resolve_pkg(kind.def_id().pkg(), ctx));
            for (_, op) in members {
                collect_operand_packages(op, ctx, out);
            }
        }
        Rvalue::Discriminant(place) => collect_place_packages(place, ctx, out),
    }
}

fn collect_genargs_packages(genargs: &GenArgs, ctx: &EncodeCtx, out: &mut BTreeSet<u32>) {
    for (g, ty) in genargs {
        out.insert(resolve_pkg(g.pkg(), ctx));
        collect_ty_packages(ty, ctx, out);
    }
}

impl Encoder<'_> {
    // ---- id の読み替え ----
    //
    // 自パッケージのシンボルは `.biwameta` の索引に写す。
    // それ以外は既にその空間にいるのでそのまま通す
    // (decode した MIR を書き直すときはすべてこちらを通る)。

    fn alias_of(&self, pkg: PackageId) -> u32 {
        let raw = if pkg.is_self() {
            self.ctx.pkg_id.value()
        } else {
            pkg.value()
        };
        *self
            .alias
            .get(&raw)
            .expect("compiler bug: package alias was not collected before encoding")
    }

    fn symbols(&self) -> &biwac_dependency_metadata::SymbolIndexMap {
        self.ctx.symbols.expect(
            "compiler bug: encoding a MIR that still holds SELF ids requires a SymbolIndexMap",
        )
    }

    fn ty_sym(&self, def_id: &TyDefId) -> (u32, u32) {
        let alias = self.alias_of(def_id.pkg());
        if def_id.pkg().is_self() {
            let sym = self.symbols().ty(def_id).unwrap_or_else(|| {
                panic!("compiler bug: type {def_id:?} is not in the metadata symbol table")
            });
            (alias, sym)
        } else {
            (alias, def_id.local_idx())
        }
    }

    fn val_sym(&self, def_id: &ValDefId) -> (u32, u32) {
        let alias = self.alias_of(def_id.pkg());
        if def_id.pkg().is_self() {
            let sym = self.symbols().val(def_id).unwrap_or_else(|| {
                panic!("compiler bug: value {def_id:?} is not in the metadata symbol table")
            });
            (alias, sym)
        } else {
            (alias, def_id.local_idx())
        }
    }

    fn trait_assoc_sym(&self, def_id: &biwac_span::TraitAssocDefId) -> (u32, u32) {
        let alias = self.alias_of(def_id.pkg());
        if def_id.pkg().is_self() {
            let sym = self.symbols().trait_assoc(def_id).unwrap_or_else(|| {
                panic!("compiler bug: trait item {def_id:?} is not in the metadata symbol table")
            });
            (alias, sym)
        } else {
            (alias, def_id.local_idx())
        }
    }

    fn ty_genarg(&self, def_id: &GenDefId) -> GenArgRef {
        let pkg = self.alias_of(def_id.pkg());
        let (owner, ord) = if def_id.pkg().is_self() {
            self.symbols().ty_genarg(def_id).unwrap_or_else(|| {
                panic!("compiler bug: generic argument {def_id:?} is not in the symbol table")
            })
        } else {
            biwac_dependency_metadata::decompose_genarg_local_idx(def_id.local_idx())
        };
        GenArgRef { pkg, owner, ord }
    }

    /// `owner` は「どの関数から見たジェネリック引数か」。
    /// 項目の中の型なら項目自身、呼び出しのジェネリック引数なら呼び先である。
    fn fn_genarg(&self, def_id: &LocalGenDefId, owner: u32) -> GenArgRef {
        let pkg = self.alias_of(def_id.pkg());
        let (owner, ord) = if def_id.pkg().is_self() {
            let ord = self.symbols().fn_genarg(owner, def_id).unwrap_or_else(|| {
                panic!(
                    "compiler bug: generic argument {def_id:?} is not declared by symbol {owner}"
                )
            });
            (owner, ord)
        } else {
            biwac_dependency_metadata::decompose_genarg_local_idx(def_id.local_idx())
        };
        GenArgRef { pkg, owner, ord }
    }

    /// いま書いている項目のシンボル索引。
    fn owner(&self) -> u32 {
        self.owner
            .expect("compiler bug: encoding a type outside of any item")
    }

    // ---- 表 ----

    fn intern_ty(&mut self, ty: &Ty) -> usize {
        let entry = match &ty.kind {
            TyKind::Int => TyEntry::Int,
            TyKind::Float => TyEntry::Float,
            TyKind::Bool => TyEntry::Bool,
            TyKind::Void => TyEntry::Void,
            TyKind::Defined(dt) => {
                let args: Vec<usize> = dt.genargs.iter().map(|g| self.intern_ty(g)).collect();
                let (pkg, sym) = self.ty_sym(&dt.def_id);
                TyEntry::Def { pkg, sym, args }
            }
            TyKind::Gen(g) => {
                let r = self.ty_genarg(g);
                TyEntry::Gen {
                    pkg: r.pkg,
                    owner: r.owner,
                    ord: r.ord,
                }
            }
            TyKind::LocGen(g) => {
                let r = self.fn_genarg(g, self.owner());
                TyEntry::LocGen {
                    pkg: r.pkg,
                    owner: r.owner,
                    ord: r.ord,
                }
            }
            TyKind::Fn(f) => {
                let args: Vec<usize> = f.args.iter().map(|a| self.intern_ty(a)).collect();
                let rty = self.intern_ty(&f.rty);
                TyEntry::Fn { args, rty }
            }
            TyKind::Infer(_) => {
                panic!(
                    "compiler bug: an inference type reached MIR encoding (owner symbol {:?})",
                    self.owner
                )
            }
        };

        if let Some(i) = self.ty_index.get(&entry) {
            return *i;
        }
        let i = self.tys.len();
        self.ty_index.insert(clone_ty_entry(&entry), i);
        self.tys.push(entry);
        i
    }

    /// `callee` は呼び先のシンボル索引。
    /// ジェネリック引数の id は呼び先から見たものとして解決する
    /// (型のほうは呼び出し側の文脈で解決する)。
    fn intern_genargs(&mut self, genargs: &GenArgs, callee: u32) -> usize {
        let entry: GaEntry = genargs
            .iter()
            .map(|(g, ty)| {
                let ty = self.intern_ty(ty);
                (self.fn_genarg(g, callee), ty)
            })
            .collect();

        if let Some(i) = self.ga_index.get(&entry) {
            return *i;
        }
        let i = self.gas.len();
        self.ga_index.insert(entry.clone(), i);
        self.gas.push(entry);
        i
    }

    // ---- 描画 ----

    fn render_genarg(&self, g: &GenArgRef) -> String {
        format!("p{}:{}#{}", g.pkg, g.owner, g.ord)
    }

    fn render_ty_entry(&self, entry: &TyEntry) -> String {
        match entry {
            TyEntry::Int => "int".to_string(),
            TyEntry::Float => "float".to_string(),
            TyEntry::Bool => "bool".to_string(),
            TyEntry::Void => "void".to_string(),
            TyEntry::Def { pkg, sym, args } => {
                let mut s = format!("def p{pkg}:{sym}");
                for a in args {
                    let _ = write!(s, " {a}");
                }
                s
            }
            TyEntry::Gen { pkg, owner, ord } => format!("gen p{pkg}:{owner}#{ord}"),
            TyEntry::LocGen { pkg, owner, ord } => format!("locgen p{pkg}:{owner}#{ord}"),
            TyEntry::Fn { args, rty } => {
                let mut s = "fn".to_string();
                for a in args {
                    let _ = write!(s, " {a}");
                }
                let _ = write!(s, " -> {rty}");
                s
            }
        }
    }

    fn ident(&self, id: &InternedIdent) -> String {
        if *id == InternedIdent::SELF {
            return "self".to_string();
        }
        self.ctx
            .interner
            .get_str(id)
            .expect("compiler bug: identifier is not in the interner")
            .to_string()
    }

    fn render_place(&mut self, place: &Place) -> String {
        let mut s = format!("_{}", place.local.value());
        for elem in &place.projection {
            match elem {
                PlaceElem::Field(name, ty) => {
                    let ty = self.intern_ty(ty);
                    let _ = write!(s, ".{}@{}", self.ident(name), ty);
                }
                // `.v1` の形。直後に必ず Field が続く。
                PlaceElem::Downcast(index) => {
                    let _ = write!(s, ".v{index}");
                }
            }
        }
        s
    }

    fn render_operand(&mut self, op: &Operand) -> String {
        match op {
            Operand::Place(p) => self.render_place(p),
            Operand::Const(c) => match c {
                Const::Int(v) => format!("int:{v}"),
                Const::Float(v) => format!("float:{v:?}"),
                Const::Bool(v) => format!("bool:{v}"),
                Const::Void => "void".to_string(),
                Const::Str(id) => format!("str:{}", id.value()),
                Const::FnDef(def_id, genargs) => {
                    let (pkg, sym) = self.val_sym(def_id);
                    let ga = self.intern_genargs(genargs, sym);
                    format!("fn:p{pkg}:{sym}/ga{ga}")
                }
            },
        }
    }

    // ---- 項目 ----

    fn emit_native(&mut self, out: &mut String, n: &NativeItem) {
        let (pkg, sym) = self.val_sym(&n.def_id);
        let _ = writeln!(out, "native p{pkg}:{sym}");

        if !n.genargs.is_empty() {
            let mut line = "  genargs".to_string();
            for g in &n.genargs {
                let r = self.fn_genarg(g, sym);
                let _ = write!(line, " {}", self.render_genarg(&r));
            }
            let _ = writeln!(out, "{line}");
        }
        if let Some(t) = &n.self_ty {
            let t = self.intern_ty(t);
            let _ = writeln!(out, "  self {t}");
        }
        let args: Vec<usize> = n.args.iter().map(|t| self.intern_ty(t)).collect();
        let mut line = "  args".to_string();
        for a in &args {
            let _ = write!(line, " {a}");
        }
        let _ = writeln!(out, "{line}");
        let rty = self.intern_ty(&n.rty);
        let _ = writeln!(out, "  rty {rty}");
        let _ = writeln!(out, "  body {:?}", n.native_body);
    }

    fn emit_body(&mut self, out: &mut String, b: &Body) {
        let (pkg, sym) = self.val_sym(&b.def_id);
        let _ = writeln!(out, "fn p{pkg}:{sym}");

        if !b.genargs.is_empty() {
            let mut line = "  genargs".to_string();
            for g in &b.genargs {
                let r = self.fn_genarg(g, sym);
                let _ = write!(line, " {}", self.render_genarg(&r));
            }
            let _ = writeln!(out, "{line}");
        }
        let _ = writeln!(out, "  argc {}", b.arg_count);

        for (i, decl) in b.locals.iter().enumerate() {
            let t = self.intern_ty(&decl.ty);
            let _ = writeln!(out, "  local {i} {t}");
        }

        for (i, block) in b.blocks.iter().enumerate() {
            let _ = writeln!(out, "  bb {i}");
            for stmt in &block.stmts {
                let StatementKind::Assign(place, rvalue) = &stmt.kind;
                let dest = self.render_place(place);
                let rhs = self.render_rvalue(rvalue);
                let _ = writeln!(out, "    {dest} = {rhs}");
            }
            let term = self.render_terminator(&block.term.kind);
            let _ = writeln!(out, "    {term}");
        }
    }

    fn render_rvalue(&mut self, rvalue: &Rvalue) -> String {
        match rvalue {
            Rvalue::Use(op) => self.render_operand(op),
            Rvalue::UnaryOp(op, o) => {
                let o = self.render_operand(o);
                format!("{} {o}", un_op(*op))
            }
            Rvalue::BinaryOp(op, l, r) => {
                let l = self.render_operand(l);
                let r = self.render_operand(r);
                format!("{} {l} {r}", bin_op(*op))
            }
            Rvalue::Aggregate(kind, members) => {
                let def_id = kind.def_id();
                let (pkg, sym) = self.ty_sym(&def_id);
                let mut s = match kind {
                    AggregateKind::Struct(_) => format!("agg p{pkg}:{sym}"),
                    AggregateKind::Enum(_, index) => format!("agg p{pkg}:{sym}/v{index}"),
                };
                for (name, op) in members {
                    let name = self.ident(name);
                    let op = self.render_operand(op);
                    let _ = write!(s, " {name}={op}");
                }
                s
            }
            Rvalue::Discriminant(place) => {
                let place = self.render_place(place);
                format!("discr {place}")
            }
        }
    }

    fn render_terminator(&mut self, term: &TerminatorKind) -> String {
        match term {
            TerminatorKind::Goto { target } => format!("goto {}", target.value()),
            TerminatorKind::SwitchInt { discr, targets } => {
                let discr = self.render_operand(discr);
                let mut s = format!("switch {discr}");
                for (v, t) in targets.iter() {
                    let _ = write!(s, " {v}:{}", t.value());
                }
                let _ = write!(s, " else:{}", targets.otherwise().value());
                s
            }
            TerminatorKind::Call {
                callee,
                args,
                dest,
                target,
            } => {
                let dest = self.render_place(dest);
                let callee = match callee {
                    Callee::Direct { def_id, genargs } => {
                        let (pkg, sym) = self.val_sym(def_id);
                        let ga = self.intern_genargs(genargs, sym);
                        format!("d p{pkg}:{sym} ga{ga}")
                    }
                    Callee::TraitAssoc {
                        assoc,
                        self_ty,
                        genargs,
                    } => {
                        let (pkg, sym) = self.trait_assoc_sym(assoc);
                        // 型は索引をそのまま書く (`ty` の接頭辞は付けない)。
                        let ty = self.intern_ty(self_ty);
                        let ga = self.intern_genargs(genargs, sym);
                        format!("t p{pkg}:{sym} {ty} ga{ga}")
                    }
                    Callee::Indirect(op) => {
                        let op = self.render_operand(op);
                        format!("i {op}")
                    }
                };
                let mut s = format!("call {dest} = {callee}");
                for a in args {
                    let a = self.render_operand(a);
                    let _ = write!(s, " {a}");
                }
                let _ = write!(s, " -> {}", target.value());
                s
            }
            TerminatorKind::Return => "ret".to_string(),
            TerminatorKind::Unreachable => "unreachable".to_string(),
        }
    }
}

fn clone_ty_entry(entry: &TyEntry) -> TyEntry {
    match entry {
        TyEntry::Int => TyEntry::Int,
        TyEntry::Float => TyEntry::Float,
        TyEntry::Bool => TyEntry::Bool,
        TyEntry::Void => TyEntry::Void,
        TyEntry::Def { pkg, sym, args } => TyEntry::Def {
            pkg: *pkg,
            sym: *sym,
            args: args.clone(),
        },
        TyEntry::Gen { pkg, owner, ord } => TyEntry::Gen {
            pkg: *pkg,
            owner: *owner,
            ord: *ord,
        },
        TyEntry::LocGen { pkg, owner, ord } => TyEntry::LocGen {
            pkg: *pkg,
            owner: *owner,
            ord: *ord,
        },
        TyEntry::Fn { args, rty } => TyEntry::Fn {
            args: args.clone(),
            rty: *rty,
        },
    }
}

pub(super) fn bin_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Gt => "gt",
        BinOp::Ge => "ge",
    }
}

pub(super) fn un_op(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg => "neg",
    }
}
