//! 単相化された MIR から WAT (WebAssembly のテキスト形式) を生成する。
//!
//! # なぜテキストか
//!
//! `[[native(arch="wasm")]]` の本体が WAT で書かれているためである。
//! バイナリを直接組み立てると、native の本体を差し込むために
//! WAT のパーサを別途持つことになる。テキストなら同じ土俵で連結できる。
//! TypeScript バックエンドが native の TypeScript をそのまま差し込むのと同じ立て付けである。
//!
//! # 値の表現
//!
//! | biwa | wasm |
//! |---|---|
//! | `Int` | `i32` |
//! | `Float` | `f32` |
//! | `Bool` | `i32` (0/1) |
//! | `Void` | 値なし |
//! | struct | `(ref null $S)` — WasmGC の struct |
//! | native 型 (`String` / `Option` など) | その native 本体に書かれた wasm 型 |
//!
//! 線形メモリは文字列リテラルの定数プールとしてだけ使う。
//! ヒープは WasmGC が持ち、シャドースタックは要らない
//! (biwa に `&` が無く、local のアドレスを取る手段が無いため)。

use std::collections::HashMap;
use std::fmt::Write;

use biwac_base::InternedIdent;
use biwac_hir::{Ty, TyKind};
use biwac_mir::{
    BasicBlock, BinOp, Body, Callee, Const, InstanceKey, Local, MirItem, MonoMir, MonoTyDefKind,
    NativeItem, Operand, Place, PlaceElem, Rvalue, StatementKind, TerminatorKind, TyInstanceKey,
    UnOp,
};
use biwac_span::TyDefId;

use crate::arch::wasm::structure::{Structured, structure};
use crate::mangle::Mangler;

/// 文字列リテラルを作るためにコンパイラが要求するホスト関数。
///
/// 文字列は `externref` (ホストが持つもの) なので、
/// リテラルからそれを作るにはホストの助けが要る。
/// バイト列は線形メモリのデータセグメントに置き、
/// (先頭, 長さ) を渡して作ってもらう。
///
/// これはコンパイラ自身が名前を知っている唯一のホスト関数である。
/// TODO: arch ごとに要求する lang item を変えられるようにして、
/// std が宣言する lang item に移す。
const STRING_CONST_IMPORT: &str = r#"(import "biwa:runtime" "string_const"
    (func $__biwa_string_const (param i32 i32) (result externref)))"#;

/// ランタイムがストーリー起動時に呼ぶ関数の名前。
/// TypeScript バックエンドと同じ規約である。
const ENTRYPOINT_NAME: &str = "__biwa_entrypoint";

#[derive(Debug)]
pub enum WasmError {
    /// エントリポイントが無い。
    NoEntryPoint,
    /// この型を wasm の型に落とせない。
    UnsupportedType { ty: String },
    /// 呼び先の実体が見つからない。
    MissingInstance { name: String },
    /// 制御フローを構造化できなかった。
    Structure(String),
}

impl std::fmt::Display for WasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoEntryPoint => write!(f, "no entry point to generate wasm from"),
            Self::UnsupportedType { ty } => {
                write!(f, "`{ty}` cannot be represented in wasm yet")
            }
            Self::MissingInstance { name } => {
                write!(f, "the instance `{name}` was not monomorphized")
            }
            Self::Structure(m) => write!(f, "{m}"),
        }
    }
}

/// 単相化されたプログラム全体から WAT を作る。
pub fn emit(mono: &MonoMir, mangler: &Mangler) -> Result<String, WasmError> {
    Emitter::new(mono, mangler).run()
}

struct Emitter<'a> {
    mono: &'a MonoMir,
    mangler: &'a Mangler<'a>,
    /// 実体 → wasm の関数名。
    fn_names: HashMap<InstanceKey, String>,
    /// 具体型 → wasm の型名 (struct のみ)。
    ty_names: HashMap<TyInstanceKey, String>,
    /// 具体型 → native 本体に書かれた wasm 型 (native type alias のみ)。
    native_tys: HashMap<TyInstanceKey, String>,
    /// 文字列リテラルの (線形メモリ上の先頭, バイト長)。
    strings: Vec<(u32, u32)>,
}

impl<'a> Emitter<'a> {
    fn new(mono: &'a MonoMir, mangler: &'a Mangler<'a>) -> Self {
        // 名前は実体の索引を添えて一意にする。
        // 同じシンボルでもジェネリック引数が違えば別の関数になるためである。
        let fn_names = mono
            .instances
            .iter()
            .enumerate()
            .map(|(i, inst)| {
                let base = mangler.get_value_mangled(&inst.key.def_id);
                (inst.key.clone(), format!("{base}.i{i}"))
            })
            .collect();

        let mut ty_names = HashMap::new();
        let mut native_tys = HashMap::new();
        for (i, def) in mono.types.iter().enumerate() {
            match &def.kind {
                MonoTyDefKind::Struct { .. } => {
                    let base = mangler.get_type_mangled(&def.key.def_id);
                    ty_names.insert(def.key.clone(), format!("{base}.t{i}"));
                }
                MonoTyDefKind::Native { code } => {
                    native_tys.insert(def.key.clone(), code.trim().to_string());
                }
            }
        }

        // 文字列は 1 バイト境界で詰める。読み取り専用の定数置き場なので整列は要らない。
        let mut strings = Vec::new();
        let mut offset = 0u32;
        for (_, s) in mono.strings.iter() {
            let len = s.len() as u32;
            strings.push((offset, len));
            offset += len;
        }

        Self {
            mono,
            mangler,
            fn_names,
            ty_names,
            native_tys,
            strings,
        }
    }

    fn run(self) -> Result<String, WasmError> {
        let entry = self.mono.entry.ok_or(WasmError::NoEntryPoint)?;

        let mut out = String::new();
        out.push_str("(module\n");

        // --- 前置のネイティブコード (ホスト関数の import) ---
        //
        // import の名前空間は std とエンジンが合意するもので、
        // コンパイラは中身を見ずにそのまま置く。
        for native in &self.mono.module_natives {
            for line in native.lines() {
                let line = line.trim_end();
                if line.trim().is_empty() {
                    continue;
                }
                let _ = writeln!(out, "  {line}");
            }
        }
        let _ = writeln!(out, "  {STRING_CONST_IMPORT}");
        out.push('\n');

        // --- 文字列リテラルの定数プール ---
        if !self.mono.strings.is_empty() {
            let total: u32 = self.strings.iter().map(|(_, len)| len).sum();
            // 1 ページ (64KiB) 単位。定数しか置かないので伸びない。
            let pages = total.div_ceil(65536).max(1);
            let _ = writeln!(out, "  (memory (export \"memory\") {pages})");
            for ((offset, _), (_, s)) in self.strings.iter().zip(self.mono.strings.iter()) {
                let _ = writeln!(out, "  (data (i32.const {offset}) \"{}\")", escape_data(s));
            }
            out.push('\n');
        }

        // --- 型 ---
        //
        // 相互に参照しうるので 1 つの再帰グループにまとめる。
        let structs: Vec<_> = self
            .mono
            .types
            .iter()
            .filter(|d| matches!(d.kind, MonoTyDefKind::Struct { .. }))
            .collect();
        if !structs.is_empty() {
            out.push_str("  (rec\n");
            for def in &structs {
                let MonoTyDefKind::Struct { members } = &def.kind else {
                    unreachable!()
                };
                let name = &self.ty_names[&def.key];
                let mut fields = String::new();
                for (member, ty) in members {
                    let wty = self.wasm_ty(ty)?;
                    let field = self.field_name(*member);
                    let _ = write!(fields, " (field {field} (mut {wty}))");
                }
                let _ = writeln!(out, "    (type ${name} (struct{fields}))");
            }
            out.push_str("  )\n\n");
        }

        // --- 関数 ---
        for inst in &self.mono.instances {
            let body = self.emit_instance(inst)?;
            out.push_str(&body);
            out.push('\n');
        }

        // --- エントリポイント ---
        let entry_name = &self.fn_names[&self.mono.instances[entry].key];
        let _ = writeln!(out, "  (export \"{ENTRYPOINT_NAME}\" (func ${entry_name}))");

        out.push_str(")\n");
        Ok(out)
    }

    // ---- 型 ----

    fn wasm_ty(&self, ty: &Ty) -> Result<String, WasmError> {
        Ok(match &ty.kind {
            TyKind::Int | TyKind::Bool => "i32".to_string(),
            TyKind::Float => "f32".to_string(),
            TyKind::Defined(dt) => {
                let key = TyInstanceKey {
                    def_id: dt.def_id,
                    args: dt.genargs.clone(),
                };
                if let Some(name) = self.ty_names.get(&key) {
                    format!("(ref null ${name})")
                } else if let Some(native) = self.native_tys.get(&key) {
                    native.clone()
                } else if let Some(prim) = prim_of(&dt.def_id) {
                    prim.to_string()
                } else {
                    return Err(WasmError::UnsupportedType {
                        ty: format!("ty#{}", dt.def_id.value()),
                    });
                }
            }
            TyKind::Void => {
                return Err(WasmError::UnsupportedType {
                    ty: "Void".to_string(),
                });
            }
            other => {
                return Err(WasmError::UnsupportedType {
                    ty: format!("{other:?}"),
                });
            }
        })
    }

    /// 値を持たない型か。`Void` の local と戻り値は wasm に現れない。
    fn is_void(ty: &Ty) -> bool {
        matches!(ty.kind, TyKind::Void)
    }

    fn field_name(&self, member: InternedIdent) -> String {
        format!("${}", self.mangler.str_of(&member))
    }

    // ---- 関数 ----

    fn emit_instance(&self, inst: &biwac_mir::MonoInstance) -> Result<String, WasmError> {
        let name = &self.fn_names[&inst.key];
        match &inst.item {
            MirItem::Native(n) => self.emit_native(name, n),
            MirItem::Body(b) => self.emit_body(name, b),
        }
    }

    fn emit_native(&self, name: &str, n: &NativeItem) -> Result<String, WasmError> {
        let mut sig = String::new();
        for ty in n.self_ty.iter().chain(n.args.iter()) {
            let _ = write!(sig, " (param {})", self.wasm_ty(ty)?);
        }
        if !Self::is_void(&n.rty) {
            let _ = write!(sig, " (result {})", self.wasm_ty(&n.rty)?);
        }

        let mut out = format!("  (func ${name}{sig}\n");
        // 本体は std が書いた WAT をそのまま置く。
        for line in n.native_body.lines() {
            let line = line.trim_end();
            if line.trim().is_empty() {
                continue;
            }
            let _ = writeln!(out, "    {}", line.trim_start());
        }
        out.push_str("  )\n");
        Ok(out)
    }

    fn emit_body(&self, name: &str, body: &Body) -> Result<String, WasmError> {
        let mut sig = String::new();
        for local in body.arg_locals() {
            let ty = &body.local_decl(local).ty;
            let _ = write!(sig, " (param {})", self.wasm_ty(ty)?);
        }
        let returns = !Self::is_void(body.return_ty());
        if returns {
            let _ = write!(sig, " (result {})", self.wasm_ty(body.return_ty())?);
        }

        let mut out = format!("  (func ${name}{sig}\n");

        // 宣言する local は「引数でないもの」である。
        // 順序は wasm の索引に合わせる (`_0` が引数の直後)。
        let mut declared = vec![Local::RETURN];
        declared.extend((body.arg_count as u32 + 1..body.locals.len() as u32).map(Local::new));
        for local in &declared {
            let ty = &body.local_decl(*local).ty;
            // Void の local は値を持たないが、索引を詰めると
            // 他の local とずれるので場所だけ空けておく。
            let wty = if Self::is_void(ty) {
                "i32".to_string()
            } else {
                self.wasm_ty(ty)?
            };
            let _ = writeln!(out, "    (local {wty})");
        }

        let tree = structure(body).map_err(|e| WasmError::Structure(e.to_string()))?;
        let mut code = String::new();
        self.emit_structured(&mut code, &tree, body, 2)?;
        out.push_str(&code);

        // 末尾は到達しないことが多いが、型が合っている必要がある。
        if returns {
            let _ = writeln!(
                out,
                "    local.get {}",
                Self::wasm_local(Local::RETURN, body)
            );
        }
        out.push_str("  )\n");
        Ok(out)
    }

    /// MIR の local 番号を wasm の local 番号に写す。
    ///
    /// wasm は引数が先に並ぶが、MIR は `_0` を戻り値スロットにしている。
    fn wasm_local(local: Local, body: &Body) -> u32 {
        let i = local.value();
        let argc = body.arg_count as u32;
        if i == 0 {
            argc
        } else if i <= argc {
            i - 1
        } else {
            i
        }
    }

    // ---- 構造化された制御フロー ----

    fn emit_structured(
        &self,
        out: &mut String,
        tree: &Structured,
        body: &Body,
        depth: usize,
    ) -> Result<(), WasmError> {
        let pad = "  ".repeat(depth);
        match tree {
            Structured::Block(items) => {
                let _ = writeln!(out, "{pad}block");
                for i in items {
                    self.emit_structured(out, i, body, depth + 1)?;
                }
                let _ = writeln!(out, "{pad}end");
            }
            Structured::Loop(inner) => {
                let _ = writeln!(out, "{pad}loop");
                self.emit_structured(out, inner, body, depth + 1)?;
                let _ = writeln!(out, "{pad}end");
            }
            Structured::Simple(bb) => self.emit_block_stmts(out, *bb, body, depth)?,
            Structured::If { bb, then, els } => {
                // 条件は bb の終端子が持っている。文は Simple 側で既に出ている。
                let TerminatorKind::SwitchInt { discr, .. } = &body.block(*bb).term.kind else {
                    unreachable!("compiler bug: If must come from a SwitchInt")
                };
                self.emit_operand(out, discr, body, depth)?;
                let _ = writeln!(out, "{pad}if");
                for i in then {
                    self.emit_structured(out, i, body, depth + 1)?;
                }
                let _ = writeln!(out, "{pad}else");
                for i in els {
                    self.emit_structured(out, i, body, depth + 1)?;
                }
                let _ = writeln!(out, "{pad}end");
            }
            Structured::Br(d) => {
                let _ = writeln!(out, "{pad}br {d}");
            }
            Structured::Return => {
                if !Self::is_void(body.return_ty()) {
                    let _ = writeln!(
                        out,
                        "{pad}local.get {}",
                        Self::wasm_local(Local::RETURN, body)
                    );
                }
                let _ = writeln!(out, "{pad}return");
            }
            Structured::Unreachable => {
                let _ = writeln!(out, "{pad}unreachable");
            }
        }
        Ok(())
    }

    /// 基本ブロックの文と、呼び出しの終端子を出す。
    ///
    /// 分岐そのものは構造化された木のほうが表す。
    fn emit_block_stmts(
        &self,
        out: &mut String,
        bb: BasicBlock,
        body: &Body,
        depth: usize,
    ) -> Result<(), WasmError> {
        let block = body.block(bb);
        for stmt in &block.stmts {
            let StatementKind::Assign(place, rvalue) = &stmt.kind;
            self.emit_assign(out, place, rvalue, body, depth)?;
        }

        // 呼び出しは制御を移す終端子だが、値の計算でもある。
        // 分岐は木のほうが表すので、ここでは呼び出しだけを出す。
        if let TerminatorKind::Call {
            callee, args, dest, ..
        } = &block.term.kind
        {
            self.emit_call(out, callee, args, dest, body, depth)?;
        }
        Ok(())
    }

    fn emit_call(
        &self,
        out: &mut String,
        callee: &Callee,
        args: &[Operand],
        dest: &Place,
        body: &Body,
        depth: usize,
    ) -> Result<(), WasmError> {
        let pad = "  ".repeat(depth);

        let (def_id, genargs) = match callee {
            Callee::Direct { def_id, genargs } => (*def_id, genargs.clone()),
            Callee::Indirect(_) => {
                return Err(WasmError::UnsupportedType {
                    ty: "an indirect call".to_string(),
                });
            }
        };
        let key = InstanceKey::new(def_id, genargs);
        let name = self
            .fn_names
            .get(&key)
            .ok_or_else(|| WasmError::MissingInstance {
                name: self.mangler.get_value_mangled(&def_id),
            })?;

        // メンバへ書く場合は、値より先に対象のオブジェクトを積む。
        self.emit_place_base(out, dest, body, depth)?;
        for a in args {
            self.emit_operand(out, a, body, depth)?;
        }
        let _ = writeln!(out, "{pad}call ${name}");

        // 呼び先が値を返すかは呼び先の実体が決める。
        // 捨てる先が Void なら結果を落とす (novel statement がこの形になる)。
        let returns = self.instance_returns_value(&key);
        if returns {
            if Self::is_void(&body.local_decl(dest.local).ty) && dest.projection.is_empty() {
                let _ = writeln!(out, "{pad}drop");
            } else {
                self.emit_store(out, dest, body, depth)?;
            }
        }
        Ok(())
    }

    fn instance_returns_value(&self, key: &InstanceKey) -> bool {
        self.mono
            .instances
            .iter()
            .find(|i| &i.key == key)
            .map(|i| match &i.item {
                MirItem::Native(n) => !Self::is_void(&n.rty),
                MirItem::Body(b) => !Self::is_void(b.return_ty()),
            })
            .unwrap_or(false)
    }

    fn emit_assign(
        &self,
        out: &mut String,
        place: &Place,
        rvalue: &Rvalue,
        body: &Body,
        depth: usize,
    ) -> Result<(), WasmError> {
        let pad = "  ".repeat(depth);

        // Void の場所への代入は値を持たない。評価だけして捨てる。
        if Self::is_void(&body.local_decl(place.local).ty)
            && place.projection.is_empty()
            && let Rvalue::Use(Operand::Const(Const::Void)) = rvalue
        {
            return Ok(());
        }

        self.emit_place_base(out, place, body, depth)?;
        self.emit_rvalue(out, rvalue, body, depth)?;
        let _ = pad;
        self.emit_store(out, place, body, depth)
    }

    /// メンバへ書くときに要る「対象のオブジェクト」を積む。
    /// local へ直接書くなら何も積まない。
    fn emit_place_base(
        &self,
        out: &mut String,
        place: &Place,
        body: &Body,
        depth: usize,
    ) -> Result<(), WasmError> {
        if place.projection.is_empty() {
            return Ok(());
        }
        let pad = "  ".repeat(depth);
        let _ = writeln!(
            out,
            "{pad}local.get {}",
            Self::wasm_local(place.local, body)
        );

        // 最後の 1 段を除いて読み進める。最後は書き込みになる。
        let mut current = body.local_decl(place.local).ty.clone();
        for elem in &place.projection[..place.projection.len() - 1] {
            let PlaceElem::Field(name, ty) = elem;
            let owner = self.struct_name_of(&current)?;
            let _ = writeln!(out, "{pad}struct.get ${owner} {}", self.field_name(*name));
            current = ty.clone();
        }
        Ok(())
    }

    fn emit_store(
        &self,
        out: &mut String,
        place: &Place,
        body: &Body,
        depth: usize,
    ) -> Result<(), WasmError> {
        let pad = "  ".repeat(depth);
        match place.projection.last() {
            None => {
                let _ = writeln!(
                    out,
                    "{pad}local.set {}",
                    Self::wasm_local(place.local, body)
                );
            }
            Some(PlaceElem::Field(name, _)) => {
                let owner = self.owner_ty_of(place, body)?;
                let _ = writeln!(out, "{pad}struct.set ${owner} {}", self.field_name(*name));
            }
        }
        Ok(())
    }

    /// 射影の最後の 1 段が属する struct の型名。
    fn owner_ty_of(&self, place: &Place, body: &Body) -> Result<String, WasmError> {
        let mut current = body.local_decl(place.local).ty.clone();
        for elem in &place.projection[..place.projection.len() - 1] {
            let PlaceElem::Field(_, ty) = elem;
            current = ty.clone();
        }
        self.struct_name_of(&current)
    }

    fn struct_name_of(&self, ty: &Ty) -> Result<String, WasmError> {
        let TyKind::Defined(dt) = &ty.kind else {
            return Err(WasmError::UnsupportedType {
                ty: format!("{:?}", ty.kind),
            });
        };
        let key = TyInstanceKey {
            def_id: dt.def_id,
            args: dt.genargs.clone(),
        };
        self.ty_names
            .get(&key)
            .cloned()
            .ok_or_else(|| WasmError::UnsupportedType {
                ty: format!("ty#{}", dt.def_id.value()),
            })
    }

    fn emit_rvalue(
        &self,
        out: &mut String,
        rvalue: &Rvalue,
        body: &Body,
        depth: usize,
    ) -> Result<(), WasmError> {
        let pad = "  ".repeat(depth);
        match rvalue {
            Rvalue::Use(op) => self.emit_operand(out, op, body, depth)?,
            Rvalue::UnaryOp(UnOp::Neg, op) => {
                let ty = self.operand_ty(op, body);
                if matches!(ty.map(|t| t.kind), Some(TyKind::Float)) {
                    self.emit_operand(out, op, body, depth)?;
                    let _ = writeln!(out, "{pad}f32.neg");
                } else {
                    let _ = writeln!(out, "{pad}i32.const 0");
                    self.emit_operand(out, op, body, depth)?;
                    let _ = writeln!(out, "{pad}i32.sub");
                }
            }
            Rvalue::BinaryOp(op, l, r) => {
                let is_float = matches!(
                    self.operand_ty(l, body).map(|t| t.kind.clone()),
                    Some(TyKind::Float)
                );
                self.emit_operand(out, l, body, depth)?;
                self.emit_operand(out, r, body, depth)?;
                let _ = writeln!(out, "{pad}{}", bin_op(*op, is_float));
            }
            Rvalue::Aggregate(def_id, members) => {
                // struct.new はフィールドの宣言順に積む。
                // MIR の並びは書いた順だが、要素はすべて local か定数で
                // 副作用が無いので、並べ替えてよい。
                let key = self.aggregate_key(def_id, members, body)?;
                let name = self
                    .ty_names
                    .get(&key)
                    .ok_or_else(|| WasmError::UnsupportedType {
                        ty: format!("ty#{}", def_id.value()),
                    })?;
                let def = self
                    .mono
                    .types
                    .iter()
                    .find(|d| d.key == key)
                    .expect("compiler bug: the type name exists but the definition does not");
                let MonoTyDefKind::Struct { members: fields } = &def.kind else {
                    unreachable!()
                };
                for (field, _) in fields {
                    let op = members
                        .iter()
                        .find(|(n, _)| n == field)
                        .map(|(_, op)| op)
                        .expect("compiler bug: a struct literal is missing a member");
                    self.emit_operand(out, op, body, depth)?;
                }
                let _ = writeln!(out, "{pad}struct.new ${name}");
            }
        }
        Ok(())
    }

    /// 集約が作る具体型。
    ///
    /// 単相化された `MonoTyDef` は 1 つの `TyDefId` に複数あるので、
    /// メンバの型から実体を絞る。
    fn aggregate_key(
        &self,
        def_id: &TyDefId,
        members: &[(InternedIdent, Operand)],
        body: &Body,
    ) -> Result<TyInstanceKey, WasmError> {
        let candidates: Vec<_> = self
            .mono
            .types
            .iter()
            .filter(|d| d.key.def_id == *def_id)
            .collect();
        if let [only] = candidates.as_slice() {
            return Ok(only.key.clone());
        }

        // 複数あるならメンバの型で見分ける。
        for cand in &candidates {
            let MonoTyDefKind::Struct { members: fields } = &cand.kind else {
                continue;
            };
            let matched = fields.iter().all(|(name, ty)| {
                members
                    .iter()
                    .find(|(n, _)| n == name)
                    .and_then(|(_, op)| self.operand_ty(op, body))
                    .map(|actual| actual.kind == ty.kind)
                    .unwrap_or(false)
            });
            if matched {
                return Ok(cand.key.clone());
            }
        }
        Err(WasmError::UnsupportedType {
            ty: format!("ty#{}", def_id.value()),
        })
    }

    fn operand_ty(&self, op: &Operand, body: &Body) -> Option<Ty> {
        match op {
            Operand::Place(p) => Some(match p.projection.last() {
                None => body.local_decl(p.local).ty.clone(),
                Some(PlaceElem::Field(_, ty)) => ty.clone(),
            }),
            Operand::Const(Const::Int(_)) => Some(Ty::new(TyKind::Int, biwac_span::Span::dummy())),
            Operand::Const(Const::Float(_)) => {
                Some(Ty::new(TyKind::Float, biwac_span::Span::dummy()))
            }
            Operand::Const(Const::Bool(_)) => {
                Some(Ty::new(TyKind::Bool, biwac_span::Span::dummy()))
            }
            _ => None,
        }
    }

    fn emit_operand(
        &self,
        out: &mut String,
        op: &Operand,
        body: &Body,
        depth: usize,
    ) -> Result<(), WasmError> {
        let pad = "  ".repeat(depth);
        match op {
            Operand::Place(p) => {
                let _ = writeln!(out, "{pad}local.get {}", Self::wasm_local(p.local, body));
                let mut current = body.local_decl(p.local).ty.clone();
                for elem in &p.projection {
                    let PlaceElem::Field(name, ty) = elem;
                    let owner = self.struct_name_of(&current)?;
                    let _ = writeln!(out, "{pad}struct.get ${owner} {}", self.field_name(*name));
                    current = ty.clone();
                }
            }
            Operand::Const(c) => match c {
                Const::Int(v) => {
                    let _ = writeln!(out, "{pad}i32.const {v}");
                }
                Const::Float(v) => {
                    let _ = writeln!(out, "{pad}f32.const {v}");
                }
                Const::Bool(v) => {
                    let _ = writeln!(out, "{pad}i32.const {}", if *v { 1 } else { 0 });
                }
                Const::Void => {}
                Const::Str(id) => {
                    let (offset, len) = self.strings[id.index()];
                    let _ = writeln!(out, "{pad}i32.const {offset}");
                    let _ = writeln!(out, "{pad}i32.const {len}");
                    let _ = writeln!(out, "{pad}call $__biwa_string_const");
                }
                Const::FnDef(..) => {
                    return Err(WasmError::UnsupportedType {
                        ty: "a function value".to_string(),
                    });
                }
            },
        }
        Ok(())
    }
}

fn bin_op(op: BinOp, is_float: bool) -> &'static str {
    if is_float {
        match op {
            BinOp::Add => "f32.add",
            BinOp::Sub => "f32.sub",
            BinOp::Mul => "f32.mul",
            BinOp::Div => "f32.div",
            // 浮動小数点に剰余は無い。型検査が通っていれば現れない。
            BinOp::Rem => "f32.div",
            BinOp::Eq => "f32.eq",
            BinOp::Ne => "f32.ne",
            BinOp::Lt => "f32.lt",
            BinOp::Le => "f32.le",
            BinOp::Gt => "f32.gt",
            BinOp::Ge => "f32.ge",
        }
    } else {
        match op {
            BinOp::Add => "i32.add",
            BinOp::Sub => "i32.sub",
            BinOp::Mul => "i32.mul",
            BinOp::Div => "i32.div_s",
            BinOp::Rem => "i32.rem_s",
            BinOp::Eq => "i32.eq",
            BinOp::Ne => "i32.ne",
            BinOp::Lt => "i32.lt_s",
            BinOp::Le => "i32.le_s",
            BinOp::Gt => "i32.gt_s",
            BinOp::Ge => "i32.ge_s",
        }
    }
}

/// 予約済みの [`TyDefId`] ならプリミティブの wasm 型に落とす。
fn prim_of(def_id: &TyDefId) -> Option<&'static str> {
    match *def_id {
        TyDefId::INT_TY_DEF_ID | TyDefId::UINT_TY_DEF_ID | TyDefId::BOOL_TY_DEF_ID => Some("i32"),
        TyDefId::FLOAT_TY_DEF_ID => Some("f32"),
        _ => None,
    }
}

/// データセグメントの文字列。WAT の文字列は `\XX` で任意のバイトを書ける。
fn escape_data(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(*b as char),
            _ => {
                let _ = write!(out, "\\{b:02x}");
            }
        }
    }
    out
}
