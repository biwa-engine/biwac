//! MIR のテキスト表現。
//!
//! コンパイラを触るときに中身を見るためのもので、
//! 機械が読み戻すことは想定していない。
//! 出力順は [`crate::Mir::items`] の順 ([`biwac_span::ValDefId`] 順) で固定される。

use std::fmt::Write;

use biwac_base::{IdentInterner, InternedIdent};
use biwac_hir::{Hir, Ty, TyDefKind, TyKind, ValDefKind};
use biwac_span::{TyDefId, ValDefId};

use crate::{
    BasicBlock, BinOp, Body, Callee, Const, Mir, MirItem, NativeItem, Operand, Place, PlaceElem,
    Rvalue, StatementKind, TerminatorKind, UnOp,
};

/// ダンプに必要な、MIR の外にある情報。
///
/// MIR はシンボルを id でしか持たないので、
/// 名前を出すには定義表が要る。
pub struct DumpCtx<'a> {
    pub hir: &'a Hir,
    pub interner: &'a IdentInterner,

    /// 依存パッケージのシンボルの名前を引くもの。
    ///
    /// HIR には依存パッケージのシンボルが載っていないので、
    /// これが無いと id のまま出る。読みやすさのためだけの仕組みである。
    pub externals: Option<&'a dyn ExternalNames>,
}

/// 依存パッケージのシンボルの名前を引く。
///
/// メタデータの読み方を知っているのは呼び出し側なので、
/// [`biwac_mir`](crate) はこの形だけ決めて中身は持たない。
pub trait ExternalNames {
    fn ty_name(&self, def_id: TyDefId) -> Option<String>;
    fn val_name(&self, def_id: ValDefId) -> Option<String>;
}

pub fn dump(mir: &Mir, ctx: &DumpCtx) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "// package: {}", mir.pkg_name.value());

    if !mir.strings.is_empty() {
        let _ = writeln!(out);
        for (id, s) in mir.strings.iter() {
            let _ = writeln!(out, "str{} = {:?}", id.value(), s);
        }
    }

    for item in mir.items.values() {
        let _ = writeln!(out);
        match item {
            MirItem::Body(body) => dump_body(&mut out, body, ctx),
            MirItem::Native(native) => dump_native(&mut out, native, ctx),
        }
    }

    out
}

fn dump_body(out: &mut String, body: &Body, ctx: &DumpCtx) {
    let args = body
        .arg_locals()
        .map(|l| format!("_{}: {}", l.value(), ty(&body.local_decl(l).ty, ctx)))
        .collect::<Vec<_>>()
        .join(", ");

    let _ = writeln!(
        out,
        "fn {}{}({}) -> {} {{",
        val_name(&body.def_id, ctx),
        genargs_decl(&body.genargs),
        args,
        ty(body.return_ty(), ctx),
    );

    // 引数以外の local を宣言として並べる。引数はシグニチャに出ているので省く。
    for (idx, decl) in body.locals.iter().enumerate() {
        if idx != 0 && idx <= body.arg_count {
            continue;
        }
        let _ = writeln!(out, "    let _{}: {};", idx, ty(&decl.ty, ctx));
    }

    for (idx, block) in body.blocks.iter().enumerate() {
        let _ = writeln!(out);
        let _ = writeln!(out, "    bb{}: {{", idx);
        for stmt in &block.stmts {
            match &stmt.kind {
                StatementKind::Assign(place, rvalue) => {
                    let _ = writeln!(
                        out,
                        "        {} = {};",
                        self::place(place, ctx),
                        self::rvalue(rvalue, ctx)
                    );
                }
            }
        }
        let _ = writeln!(out, "        {};", terminator(&block.term.kind, ctx));
        let _ = writeln!(out, "    }}");
    }

    let _ = writeln!(out, "}}");
}

fn dump_native(out: &mut String, native: &NativeItem, ctx: &DumpCtx) {
    let args = native
        .self_ty
        .iter()
        .map(|t| format!("self: {}", ty(t, ctx)))
        .chain(native.args.iter().map(|t| ty(t, ctx)))
        .collect::<Vec<_>>()
        .join(", ");

    let _ = writeln!(
        out,
        "native fn {}{}({}) -> {};",
        val_name(&native.def_id, ctx),
        genargs_decl(&native.genargs),
        args,
        ty(&native.rty, ctx),
    );
}

fn genargs_decl(genargs: &[biwac_span::LocalGenDefId]) -> String {
    if genargs.is_empty() {
        return String::new();
    }
    format!(
        "[{}]",
        genargs
            .iter()
            .map(|g| format!("T{}", g.value()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn place(place: &Place, ctx: &DumpCtx) -> String {
    let mut s = format!("_{}", place.local.value());
    for elem in &place.projection {
        match elem {
            PlaceElem::Field(name, _) => {
                let _ = write!(s, ".{}", ident(name, ctx));
            }
        }
    }
    s
}

fn rvalue(rvalue: &Rvalue, ctx: &DumpCtx) -> String {
    match rvalue {
        Rvalue::Use(op) => operand(op, ctx),
        Rvalue::BinaryOp(op, l, r) => {
            format!("{}({}, {})", bin_op(*op), operand(l, ctx), operand(r, ctx))
        }
        Rvalue::UnaryOp(op, o) => format!("{}({})", un_op(*op), operand(o, ctx)),
        Rvalue::Aggregate(def_id, members) => format!(
            "{} {{ {} }}",
            ty_name(def_id, ctx),
            members
                .iter()
                .map(|(name, op)| format!("{}: {}", ident(name, ctx), operand(op, ctx)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn operand(op: &Operand, ctx: &DumpCtx) -> String {
    match op {
        Operand::Place(p) => place(p, ctx),
        Operand::Const(c) => format!("const {}", constant(c, ctx)),
    }
}

fn constant(c: &Const, ctx: &DumpCtx) -> String {
    match c {
        Const::Int(i) => i.to_string(),
        Const::Float(f) => format!("{f}f"),
        Const::Bool(b) => b.to_string(),
        Const::Void => "void".to_string(),
        Const::Str(id) => format!("str{}", id.value()),
        Const::FnDef(def_id, genargs) => {
            format!("{}{}", val_name(def_id, ctx), genargs_use(genargs, ctx))
        }
    }
}

fn terminator(term: &TerminatorKind, ctx: &DumpCtx) -> String {
    match term {
        TerminatorKind::Goto { target } => format!("goto -> {}", bb(*target)),
        TerminatorKind::SwitchInt { discr, targets } => {
            let arms = targets
                .iter()
                .map(|(v, t)| format!("{}: {}", v, bb(t)))
                .chain([format!("otherwise: {}", bb(targets.otherwise()))])
                .collect::<Vec<_>>()
                .join(", ");
            format!("switchInt({}) -> [{}]", operand(discr, ctx), arms)
        }
        TerminatorKind::Call {
            callee,
            args,
            dest,
            target,
        } => {
            let callee = match callee {
                Callee::Direct { def_id, genargs } => {
                    format!("{}{}", val_name(def_id, ctx), genargs_use(genargs, ctx))
                }
                Callee::Indirect(op) => operand(op, ctx),
            };
            format!(
                "{} = call {}({}) -> {}",
                place(dest, ctx),
                callee,
                args.iter()
                    .map(|a| operand(a, ctx))
                    .collect::<Vec<_>>()
                    .join(", "),
                bb(*target)
            )
        }
        TerminatorKind::Return => "return".to_string(),
        TerminatorKind::Unreachable => "unreachable".to_string(),
    }
}

fn bb(bb: BasicBlock) -> String {
    format!("bb{}", bb.value())
}

fn bin_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "Add",
        BinOp::Sub => "Sub",
        BinOp::Mul => "Mul",
        BinOp::Div => "Div",
        BinOp::Rem => "Rem",
        BinOp::Eq => "Eq",
        BinOp::Ne => "Ne",
        BinOp::Lt => "Lt",
        BinOp::Le => "Le",
        BinOp::Gt => "Gt",
        BinOp::Ge => "Ge",
    }
}

fn un_op(op: UnOp) -> &'static str {
    match op {
        UnOp::Neg => "Neg",
    }
}

fn genargs_use(genargs: &[(biwac_span::LocalGenDefId, Ty)], ctx: &DumpCtx) -> String {
    if genargs.is_empty() {
        return String::new();
    }
    format!(
        "[{}]",
        genargs
            .iter()
            .map(|(g, t)| format!("T{} = {}", g.value(), ty(t, ctx)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// 型のジェネリック引数列。こちらは位置で並ぶ。
fn ty_genargs(genargs: &[Ty], ctx: &DumpCtx) -> String {
    if genargs.is_empty() {
        return String::new();
    }
    format!(
        "[{}]",
        genargs
            .iter()
            .map(|t| ty(t, ctx))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn ty(t: &Ty, ctx: &DumpCtx) -> String {
    match &t.kind {
        TyKind::Int => "Int".to_string(),
        TyKind::Float => "Float".to_string(),
        TyKind::Bool => "Bool".to_string(),
        TyKind::Void => "Void".to_string(),
        TyKind::Defined(dt) => {
            format!(
                "{}{}",
                ty_name(&dt.def_id, ctx),
                ty_genargs(&dt.genargs, ctx)
            )
        }
        TyKind::Gen(g) => format!("G{}", g.value()),
        TyKind::LocGen(g) => format!("T{}", g.value()),
        TyKind::Fn(f) => format!(
            "fn({}) -> {}",
            f.args
                .iter()
                .map(|a| ty(a, ctx))
                .collect::<Vec<_>>()
                .join(", "),
            ty(&f.rty, ctx)
        ),
        // 型推論が終わっていれば現れない。
        // ダンプではそのまま見えるようにしておく。
        TyKind::Infer(_) => "?".to_string(),
    }
}

fn ty_name(def_id: &TyDefId, ctx: &DumpCtx) -> String {
    let name = ctx
        .hir
        .tys
        .get(def_id)
        .and_then(|t| t.ty_content.as_ref())
        .map(|content| match content {
            TyDefKind::Struct(s) => s.name.id,
            TyDefKind::NativeTypeAlias(a) => a.name.id,
        })
        .or_else(|| ctx.hir.ty_aliases.get(def_id).map(|a| a.name.id));

    if let Some(name) = name {
        return ident(&name, ctx);
    }

    ctx.externals
        .and_then(|e| e.ty_name(*def_id))
        .unwrap_or_else(|| format!("<ty#{}>", def_id.value()))
}

fn val_name(def_id: &ValDefId, ctx: &DumpCtx) -> String {
    let own = ctx.hir.vals.get(def_id).map(|v| match v {
        ValDefKind::Fn(f) => f.name.id,
        ValDefKind::Native(n) => n.name.id,
        ValDefKind::NovelScene(s) => s.name.id,
    });

    // 関連関数・メソッドは所属する型を前に出す。
    if let Some((ty_def_id, method)) = ctx.hir.assoc_val_map.get(def_id) {
        return format!("{}::{}", ty_name(ty_def_id, ctx), ident(method, ctx));
    }

    if let Some(name) = own {
        return ident(&name, ctx);
    }

    ctx.externals
        .and_then(|e| e.val_name(*def_id))
        .unwrap_or_else(|| format!("<val#{}>", def_id.value()))
}

fn ident(id: &InternedIdent, ctx: &DumpCtx) -> String {
    if *id == InternedIdent::SELF {
        return "self".to_string();
    }
    ctx.interner
        .get_str(id)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<?>".to_string())
}
