use std::collections::HashMap;

use biwac_base::{IdentInterner, InternedIdent, PackageId, PackageName};
use biwac_hash::Hash64;
use biwac_hir::{DefinedTy, FnTy, Ty, TyKind};
use biwac_span::{DefId, GenDefId, LocalGenDefId, PackageLocalDefId, Span, TyDefId, ValDefId};

use crate::codec::{BIWAC_MIR_FORMAT_VERSION, DecodedMir, MirDecodeError};
use crate::{
    BasicBlock, BasicBlockData, BinOp, Body, Callee, Const, GenArgs, Local, LocalDecl, Mir,
    MirItem, NativeItem, Operand, Place, PlaceElem, Rvalue, StatementKind, StringPool,
    SwitchTargets, TerminatorKind, UnOp,
};

/// 組み立て途中の項目。
///
/// `native` / `fn` の行で始まり、次の項目かファイル末尾で閉じる。
/// 終わりを表すタグを置かないのは、タグが一意なので要らないためである。
enum Pending {
    Native {
        def_id: ValDefId,
        genargs: Vec<LocalGenDefId>,
        self_ty: Option<Ty>,
        args: Vec<Ty>,
        rty: Option<Ty>,
        body: Option<String>,
    },
    Body {
        def_id: ValDefId,
        genargs: Vec<LocalGenDefId>,
        arg_count: usize,
        locals: Vec<Ty>,
        blocks: Vec<BasicBlockData>,
        /// いま文を積んでいるブロック。`bb` 行で切り替わる。
        current: Option<usize>,
        stmts: Vec<crate::Statement>,
        term: Option<TerminatorKind>,
    },
}

struct Decoder<'a> {
    interner: &'a mut IdentInterner,
    line: usize,
    /// いま読んでいる行 (コメントと前後の空白を落としたもの)。
    /// 文字列リテラルのように、空白で分割してはいけないものを取り出すのに使う。
    text: String,
    /// 別名 → PackageId の生値
    pkgs: HashMap<u32, u32>,
    pkg_name: Option<PackageName>,
    self_pkg: Option<PackageId>,
    meta_svh: Option<Hash64>,
    strings: StringPool,
    module_natives: Vec<String>,
    tys: Vec<Ty>,
    gas: Vec<GenArgs>,
    items: Vec<(ValDefId, MirItem)>,
}

pub(super) fn decode(
    text: &str,
    interner: &mut IdentInterner,
) -> Result<DecodedMir, MirDecodeError> {
    let mut d = Decoder {
        interner,
        line: 0,
        text: String::new(),
        pkgs: HashMap::new(),
        pkg_name: None,
        self_pkg: None,
        meta_svh: None,
        strings: StringPool::default(),
        module_natives: Vec::new(),
        tys: Vec::new(),
        gas: Vec::new(),
        items: Vec::new(),
    };

    let mut pending: Option<Pending> = None;

    for (i, raw) in text.lines().enumerate() {
        d.line = i + 1;
        let Some(line) = strip(raw) else { continue };
        d.text = line.to_string();
        let toks: Vec<&str> = line.split_whitespace().collect();
        let tag = toks[0];

        // 項目の切れ目。
        if tag == "native" || tag == "fn" {
            if let Some(p) = pending.take() {
                d.finish(p)?;
            }
            pending = Some(d.start_item(tag, &toks)?);
            continue;
        }

        match &mut pending {
            Some(p) => d.item_line(p, tag, &toks)?,
            None => d.header_line(tag, &toks)?,
        }
    }

    if let Some(p) = pending.take() {
        d.finish(p)?;
    }

    d.build()
}

/// コメントと空白を落とす。中身が残らなければ `None`。
///
/// コメントは **トークンの先頭にある `#`** から始まる。
/// ジェネリック引数の `p0:19#0` のようにトークンの内側に現れる `#` は
/// コメントの開始ではない。
fn strip(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let mut cut = raw.len();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            cut = i;
            break;
        }
    }
    let line = raw[..cut].trim();
    if line.is_empty() { None } else { Some(line) }
}

impl Decoder<'_> {
    fn err<T>(&self, message: impl Into<String>) -> Result<T, MirDecodeError> {
        Err(MirDecodeError {
            line: self.line,
            message: message.into(),
        })
    }

    // ---- ヘッダと各表 ----

    fn header_line(&mut self, tag: &str, toks: &[&str]) -> Result<(), MirDecodeError> {
        match tag {
            "biwamir" => {
                let v: u32 = self.num(toks.get(1))?;
                if v != BIWAC_MIR_FORMAT_VERSION {
                    return self.err(format!(
                        "unsupported format version {v} (this compiler writes {BIWAC_MIR_FORMAT_VERSION})"
                    ));
                }
            }
            "pkg" => {
                // pkg <別名> <名前> <PackageId>
                let alias: u32 = self.num(toks.get(1))?;
                let name = *toks.get(2).unwrap_or(&"-");
                let pkg: u32 = self.num(toks.get(3))?;
                self.pkgs.insert(alias, pkg);
                if alias == 0 {
                    self.self_pkg = Some(PackageId::new(pkg));
                    self.pkg_name = name.parse().ok();
                }
            }
            "meta-svh" => {
                let s = toks.get(1).copied().unwrap_or("");
                let v = u64::from_str_radix(s, 16)
                    .map_err(|_| self.error(format!("`{s}` is not a hex hash")))?;
                self.meta_svh = Some(Hash64::from_u64(v));
            }
            "modnative" => {
                // modnative "..."
                let rest = self.after_tokens(toks, 1)?;
                let code = self.unescape(&rest)?;
                self.module_natives.push(code);
            }
            "str" => {
                // str <索引> "..."
                let idx: usize = self.num(toks.get(1))?;
                let rest = self.after_tokens(toks, 2)?;
                let s = self.unescape(&rest)?;
                let got = self.strings.intern(&s);
                if got.index() != idx {
                    return self.err(format!(
                        "string {idx} is out of order (interned as {})",
                        got.index()
                    ));
                }
            }
            "ty" => {
                let idx: usize = self.num(toks.get(1))?;
                if idx != self.tys.len() {
                    return self.err(format!("type {idx} is out of order"));
                }
                let ty = self.parse_ty_entry(&toks[2..])?;
                self.tys.push(ty);
            }
            "ga" => {
                let idx: usize = self.num(toks.get(1))?;
                if idx != self.gas.len() {
                    return self.err(format!("generic argument list {idx} is out of order"));
                }
                let mut entry: GenArgs = Vec::new();
                let rest = &toks[2..];
                if !rest.len().is_multiple_of(2) {
                    return self.err("generic argument list must be pairs");
                }
                for pair in rest.chunks(2) {
                    let g = self.parse_fn_genarg(pair[0])?;
                    let ty = self.ty_ref(pair[1])?;
                    entry.push((g, ty));
                }
                self.gas.push(entry);
            }
            other => return self.err(format!("unexpected tag `{other}`")),
        }
        Ok(())
    }

    fn parse_ty_entry(&mut self, toks: &[&str]) -> Result<Ty, MirDecodeError> {
        let Some(kind) = toks.first() else {
            return self.err("empty type entry");
        };
        let kind = match *kind {
            "int" => TyKind::Int,
            "float" => TyKind::Float,
            "bool" => TyKind::Bool,
            "void" => TyKind::Void,
            "def" => {
                let def_id = TyDefId::new(self.parse_sym(toks.get(1))?);
                let mut genargs = Vec::new();
                for t in &toks[2..] {
                    genargs.push(self.ty_ref(t)?);
                }
                TyKind::Defined(DefinedTy { def_id, genargs })
            }
            "gen" => TyKind::Gen(self.parse_ty_genarg(toks.get(1).copied().unwrap_or(""))?),
            "locgen" => TyKind::LocGen(self.parse_fn_genarg(toks.get(1).copied().unwrap_or(""))?),
            "fn" => {
                let Some(arrow) = toks.iter().position(|t| *t == "->") else {
                    return self.err("function type needs `->`");
                };
                let mut args = Vec::new();
                for t in &toks[1..arrow] {
                    args.push(self.ty_ref(t)?);
                }
                let rty = self.ty_ref(toks.get(arrow + 1).copied().unwrap_or(""))?;
                TyKind::Fn(FnTy {
                    args,
                    rty: Box::new(rty),
                    // ジェネリック引数の宣言は書き出していない。
                    // 呼び出しの実体化に使うのは Callee の genargs のほうで、
                    // 型としての FnTy にこれが要る場面が今は無い。
                    genargs: Vec::new(),
                })
            }
            other => return self.err(format!("unknown type kind `{other}`")),
        };
        Ok(Ty::new(kind, Span::dummy()))
    }

    // ---- 項目 ----

    fn start_item(&mut self, tag: &str, toks: &[&str]) -> Result<Pending, MirDecodeError> {
        let def_id = ValDefId::new(self.parse_sym(toks.get(1))?);
        Ok(match tag {
            "native" => Pending::Native {
                def_id,
                genargs: Vec::new(),
                self_ty: None,
                args: Vec::new(),
                rty: None,
                body: None,
            },
            _ => Pending::Body {
                def_id,
                genargs: Vec::new(),
                arg_count: 0,
                locals: Vec::new(),
                blocks: Vec::new(),
                current: None,
                stmts: Vec::new(),
                term: None,
            },
        })
    }

    fn item_line(
        &mut self,
        pending: &mut Pending,
        tag: &str,
        toks: &[&str],
    ) -> Result<(), MirDecodeError> {
        match pending {
            Pending::Native {
                genargs,
                self_ty,
                args,
                rty,
                body,
                ..
            } => match tag {
                "genargs" => {
                    for t in &toks[1..] {
                        genargs.push(self.parse_fn_genarg(t)?);
                    }
                }
                "self" => *self_ty = Some(self.ty_ref(toks.get(1).copied().unwrap_or(""))?),
                "args" => {
                    for t in &toks[1..] {
                        args.push(self.ty_ref(t)?);
                    }
                }
                "rty" => *rty = Some(self.ty_ref(toks.get(1).copied().unwrap_or(""))?),
                "body" => {
                    let rest = self.after_tokens(toks, 1)?;
                    *body = Some(self.unescape(&rest)?);
                }
                other => return self.err(format!("unexpected tag `{other}` in a native item")),
            },
            Pending::Body { .. } => return self.body_line(pending, tag, toks),
        }
        Ok(())
    }

    fn body_line(
        &mut self,
        pending: &mut Pending,
        tag: &str,
        toks: &[&str],
    ) -> Result<(), MirDecodeError> {
        match tag {
            "genargs" => {
                let mut parsed = Vec::new();
                for t in &toks[1..] {
                    parsed.push(self.parse_fn_genarg(t)?);
                }
                let Pending::Body { genargs, .. } = pending else {
                    unreachable!()
                };
                *genargs = parsed;
            }
            "argc" => {
                let n: usize = self.num(toks.get(1))?;
                let Pending::Body { arg_count, .. } = pending else {
                    unreachable!()
                };
                *arg_count = n;
            }
            "local" => {
                let idx: usize = self.num(toks.get(1))?;
                let ty = self.ty_ref(toks.get(2).copied().unwrap_or(""))?;
                let Pending::Body { locals, .. } = pending else {
                    unreachable!()
                };
                if idx != locals.len() {
                    return self.err(format!("local {idx} is out of order"));
                }
                locals.push(ty);
            }
            "bb" => {
                let idx: usize = self.num(toks.get(1))?;
                self.close_block(pending)?;
                let Pending::Body {
                    blocks, current, ..
                } = pending
                else {
                    unreachable!()
                };
                if idx != blocks.len() {
                    return self.err(format!("basic block {idx} is out of order"));
                }
                *current = Some(idx);
            }

            // ここから先はブロックの中身。
            "goto" | "switch" | "call" | "ret" | "unreachable" => {
                self.require_block(pending, tag)?;
                let kind = self.parse_terminator(tag, toks)?;
                let Pending::Body { term, .. } = pending else {
                    unreachable!()
                };
                if term.is_some() {
                    return self.err("basic block has more than one terminator");
                }
                *term = Some(kind);
            }
            _ if tag.starts_with('_') => {
                self.require_block(pending, tag)?;
                let stmt = self.parse_assign(toks)?;
                let Pending::Body { stmts, .. } = pending else {
                    unreachable!()
                };
                stmts.push(stmt);
            }
            other => return self.err(format!("unexpected tag `{other}` in a function item")),
        }
        Ok(())
    }

    /// 文と終端子は `bb` 行の後にしか置けない。
    fn require_block(&self, pending: &Pending, tag: &str) -> Result<(), MirDecodeError> {
        let Pending::Body { current, .. } = pending else {
            unreachable!()
        };
        if current.is_none() {
            return Err(self.error(format!("`{tag}` appeared outside a basic block")));
        }
        Ok(())
    }

    fn close_block(&mut self, pending: &mut Pending) -> Result<(), MirDecodeError> {
        let Pending::Body {
            blocks,
            current,
            stmts,
            term,
            ..
        } = pending
        else {
            unreachable!()
        };
        if current.take().is_none() {
            return Ok(());
        }
        let Some(kind) = term.take() else {
            return self.err("basic block has no terminator");
        };
        blocks.push(BasicBlockData {
            stmts: std::mem::take(stmts),
            term: kind.with_span(Span::dummy()),
        });
        Ok(())
    }

    fn finish(&mut self, mut pending: Pending) -> Result<(), MirDecodeError> {
        match &mut pending {
            Pending::Native { .. } => {}
            Pending::Body { .. } => self.close_block(&mut pending)?,
        }

        match pending {
            Pending::Native {
                def_id,
                genargs,
                self_ty,
                args,
                rty,
                body,
            } => {
                let Some(rty) = rty else {
                    return self.err("native item has no return type");
                };
                self.items.push((
                    def_id,
                    MirItem::Native(NativeItem {
                        def_id,
                        self_ty,
                        args,
                        rty,
                        genargs,
                        native_body: body.unwrap_or_default(),
                        native_span: Span::dummy(),
                        span: Span::dummy(),
                    }),
                ));
            }
            Pending::Body {
                def_id,
                genargs,
                arg_count,
                locals,
                blocks,
                ..
            } => {
                self.items.push((
                    def_id,
                    MirItem::Body(Body {
                        def_id,
                        arg_count,
                        locals: locals
                            .into_iter()
                            .map(|ty| LocalDecl {
                                ty,
                                span: Span::dummy(),
                            })
                            .collect(),
                        blocks,
                        genargs,
                        span: Span::dummy(),
                    }),
                ));
            }
        }
        Ok(())
    }

    // ---- 文と終端子 ----

    fn parse_assign(&mut self, toks: &[&str]) -> Result<crate::Statement, MirDecodeError> {
        if toks.get(1) != Some(&"=") {
            return self.err("assignment must be `<place> = <rvalue>`");
        }
        let place = self.parse_place(toks[0])?;
        let rvalue = self.parse_rvalue(&toks[2..])?;
        Ok(StatementKind::Assign(place, rvalue).with_span(Span::dummy()))
    }

    fn parse_rvalue(&mut self, toks: &[&str]) -> Result<Rvalue, MirDecodeError> {
        let Some(head) = toks.first().copied() else {
            return self.err("empty rvalue");
        };

        if let Some(op) = bin_op(head) {
            let l = self.parse_operand(toks.get(1).copied().unwrap_or(""))?;
            let r = self.parse_operand(toks.get(2).copied().unwrap_or(""))?;
            return Ok(Rvalue::BinaryOp(op, l, r));
        }
        if let Some(op) = un_op(head) {
            let o = self.parse_operand(toks.get(1).copied().unwrap_or(""))?;
            return Ok(Rvalue::UnaryOp(op, o));
        }
        if head == "agg" {
            let def_id = TyDefId::new(self.parse_sym(toks.get(1))?);
            let mut members = Vec::new();
            for t in &toks[2..] {
                let Some((name, op)) = t.split_once('=') else {
                    return self.err(format!("`{t}` is not a `name=operand` pair"));
                };
                let name = self.intern(name);
                members.push((name, self.parse_operand(op)?));
            }
            return Ok(Rvalue::Aggregate(def_id, members));
        }

        // 演算子でも集約でもなければ、被演算子そのもの (Use)。
        Ok(Rvalue::Use(self.parse_operand(head)?))
    }

    fn parse_terminator(
        &mut self,
        tag: &str,
        toks: &[&str],
    ) -> Result<TerminatorKind, MirDecodeError> {
        match tag {
            "goto" => Ok(TerminatorKind::Goto {
                target: BasicBlock::new(self.num(toks.get(1))?),
            }),
            "ret" => Ok(TerminatorKind::Return),
            "unreachable" => Ok(TerminatorKind::Unreachable),
            "switch" => {
                let discr = self.parse_operand(toks.get(1).copied().unwrap_or(""))?;
                let mut values = Vec::new();
                let mut targets = Vec::new();
                let mut otherwise = None;
                for t in &toks[2..] {
                    let Some((k, v)) = t.split_once(':') else {
                        return self.err(format!("`{t}` is not a `value:block` pair"));
                    };
                    let bb = BasicBlock::new(
                        v.parse()
                            .map_err(|_| self.error(format!("`{v}` is not a block number")))?,
                    );
                    if k == "else" {
                        otherwise = Some(bb);
                    } else {
                        values.push(
                            k.parse::<u128>()
                                .map_err(|_| self.error(format!("`{k}` is not a number")))?,
                        );
                        targets.push(bb);
                    }
                }
                let Some(otherwise) = otherwise else {
                    return self.err("switch has no `else:` arm");
                };
                Ok(TerminatorKind::SwitchInt {
                    discr,
                    targets: SwitchTargets::new(values, targets, otherwise),
                })
            }
            "call" => {
                // call <場所> = d <シンボル> ga<n> <被演算子>... -> <bb>
                // call <場所> = i <被演算子>      <被演算子>... -> <bb>
                if toks.get(2) != Some(&"=") {
                    return self.err("call must be `call <place> = ...`");
                }
                let dest = self.parse_place(toks[1])?;
                let Some(arrow) = toks.iter().position(|t| *t == "->") else {
                    return self.err("call needs `->`");
                };
                let target = BasicBlock::new(self.num(toks.get(arrow + 1))?);

                let (callee, args_from) = match toks.get(3).copied() {
                    Some("d") => {
                        let def_id = ValDefId::new(self.parse_sym(toks.get(4))?);
                        let genargs = self.genargs_ref(toks.get(5).copied().unwrap_or(""))?;
                        (Callee::Direct { def_id, genargs }, 6)
                    }
                    Some("i") => {
                        let op = self.parse_operand(toks.get(4).copied().unwrap_or(""))?;
                        (Callee::Indirect(op), 5)
                    }
                    _ => return self.err("call must be `d` (direct) or `i` (indirect)"),
                };

                let mut args = Vec::new();
                for t in &toks[args_from..arrow] {
                    args.push(self.parse_operand(t)?);
                }

                Ok(TerminatorKind::Call {
                    callee,
                    args,
                    dest,
                    target,
                })
            }
            other => self.err(format!("unknown terminator `{other}`")),
        }
    }

    // ---- 細かい構文 ----

    fn parse_place(&mut self, tok: &str) -> Result<Place, MirDecodeError> {
        let mut parts = tok.split('.');
        let Some(head) = parts.next() else {
            return self.err("empty place");
        };
        let Some(local) = head.strip_prefix('_') else {
            return self.err(format!("`{tok}` is not a place"));
        };
        let local = Local::new(
            local
                .parse()
                .map_err(|_| self.error(format!("`{head}` is not a local")))?,
        );

        let mut projection = Vec::new();
        for p in parts {
            let Some((name, ty)) = p.split_once('@') else {
                return self.err(format!("`{p}` is not a `field@type` pair"));
            };
            let ty = self.ty_ref(ty)?;
            projection.push(PlaceElem::Field(self.intern(name), ty));
        }

        Ok(Place { local, projection })
    }

    fn parse_operand(&mut self, tok: &str) -> Result<Operand, MirDecodeError> {
        if tok.starts_with('_') {
            return Ok(Operand::Place(self.parse_place(tok)?));
        }
        if tok == "void" {
            return Ok(Operand::Const(Const::Void));
        }
        let Some((tag, rest)) = tok.split_once(':') else {
            return self.err(format!("`{tok}` is not an operand"));
        };
        let c = match tag {
            "int" => Const::Int(
                rest.parse()
                    .map_err(|_| self.error(format!("`{rest}` is not an integer")))?,
            ),
            "float" => Const::Float(
                rest.parse()
                    .map_err(|_| self.error(format!("`{rest}` is not a float")))?,
            ),
            "bool" => Const::Bool(rest == "true"),
            "str" => {
                let idx: u32 = rest
                    .parse()
                    .map_err(|_| self.error(format!("`{rest}` is not a string index")))?;
                match self.strings.id(idx) {
                    Some(id) => Const::Str(id),
                    None => return self.err(format!("string {idx} is not defined")),
                }
            }
            "fn" => {
                // fn:p0:19/ga1
                let Some((sym, ga)) = rest.rsplit_once('/') else {
                    return self.err(format!("`{tok}` is not a function reference"));
                };
                let def_id = ValDefId::new(self.parse_sym(Some(&sym))?);
                Const::FnDef(def_id, self.genargs_ref(ga)?)
            }
            other => return self.err(format!("unknown constant kind `{other}`")),
        };
        Ok(Operand::Const(c))
    }

    /// `p<別名>:<シンボル索引>`
    fn parse_sym(&mut self, tok: Option<&&str>) -> Result<DefId, MirDecodeError> {
        let Some(tok) = tok else {
            return self.err("missing symbol reference");
        };
        let Some((pkg, sym)) = tok.split_once(':') else {
            return self.err(format!("`{tok}` is not a symbol reference"));
        };
        let pkg = self.pkg_of(pkg)?;
        let sym: u32 = sym
            .parse()
            .map_err(|_| self.error(format!("`{sym}` is not a symbol index")))?;
        Ok(DefId::new(pkg, PackageLocalDefId::new(sym)))
    }

    /// `p<別名>:<所属シンボル>#<序数>`
    fn parse_genarg_raw(&mut self, tok: &str) -> Result<DefId, MirDecodeError> {
        let Some((pkg, rest)) = tok.split_once(':') else {
            return self.err(format!("`{tok}` is not a generic argument reference"));
        };
        let Some((owner, ord)) = rest.split_once('#') else {
            return self.err(format!("`{tok}` is missing its ordinal"));
        };
        let pkg = self.pkg_of(pkg)?;
        let owner: u32 = owner
            .parse()
            .map_err(|_| self.error(format!("`{owner}` is not a symbol index")))?;
        let ord: u32 = ord
            .parse()
            .map_err(|_| self.error(format!("`{ord}` is not an ordinal")))?;
        Ok(DefId::new(
            pkg,
            PackageLocalDefId::new(biwac_dependency_metadata::compose_genarg_local_idx(
                owner, ord,
            )),
        ))
    }

    fn parse_ty_genarg(&mut self, tok: &str) -> Result<GenDefId, MirDecodeError> {
        Ok(GenDefId::new(self.parse_genarg_raw(tok)?))
    }

    fn parse_fn_genarg(&mut self, tok: &str) -> Result<LocalGenDefId, MirDecodeError> {
        Ok(LocalGenDefId::new(self.parse_genarg_raw(tok)?))
    }

    fn pkg_of(&self, tok: &str) -> Result<PackageId, MirDecodeError> {
        let Some(alias) = tok.strip_prefix('p') else {
            return Err(self.error(format!("`{tok}` is not a package alias")));
        };
        let alias: u32 = alias
            .parse()
            .map_err(|_| self.error(format!("`{tok}` is not a package alias")))?;
        match self.pkgs.get(&alias) {
            Some(pkg) => Ok(PackageId::new(*pkg)),
            None => Err(self.error(format!("package alias {alias} is not declared"))),
        }
    }

    fn ty_ref(&self, tok: &str) -> Result<Ty, MirDecodeError> {
        let idx: usize = tok
            .parse()
            .map_err(|_| self.error(format!("`{tok}` is not a type index")))?;
        match self.tys.get(idx) {
            Some(t) => Ok(t.clone()),
            None => Err(self.error(format!("type {idx} is not defined"))),
        }
    }

    fn genargs_ref(&self, tok: &str) -> Result<GenArgs, MirDecodeError> {
        let Some(idx) = tok.strip_prefix("ga") else {
            return Err(self.error(format!("`{tok}` is not a generic argument list")));
        };
        let idx: usize = idx
            .parse()
            .map_err(|_| self.error(format!("`{tok}` is not a generic argument list")))?;
        match self.gas.get(idx) {
            Some(g) => Ok(g.clone()),
            None => Err(self.error(format!("generic argument list {idx} is not defined"))),
        }
    }

    fn intern(&mut self, name: &str) -> InternedIdent {
        if name == "self" {
            return InternedIdent::SELF;
        }
        self.interner.get_or_insert(name)
    }

    fn num<T: std::str::FromStr>(&self, tok: Option<&&str>) -> Result<T, MirDecodeError> {
        let Some(tok) = tok else {
            return Err(self.error("missing number"));
        };
        tok.parse()
            .map_err(|_| self.error(format!("`{tok}` is not a number")))
    }

    /// 先頭から `skip` 個のトークンを飛ばした残りを、**元の行から**取り出す。
    ///
    /// 空白で分割して結合し直すと、文字列リテラルの中の
    /// インデントや連続する空白が潰れてしまう。
    fn after_tokens(&self, _toks: &[&str], skip: usize) -> Result<String, MirDecodeError> {
        let mut rest = self.text.as_str();
        for _ in 0..skip {
            rest = rest.trim_start();
            match rest.find(char::is_whitespace) {
                Some(i) => rest = &rest[i..],
                None => return Err(self.error("missing string literal")),
            }
        }
        let rest = rest.trim_start();
        if rest.is_empty() {
            return Err(self.error("missing string literal"));
        }
        Ok(rest.to_string())
    }

    fn error(&self, message: impl Into<String>) -> MirDecodeError {
        MirDecodeError {
            line: self.line,
            message: message.into(),
        }
    }

    fn unescape(&self, quoted: &str) -> Result<String, MirDecodeError> {
        let s = quoted.trim();
        let inner = s
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or_else(|| self.error("string literal must be quoted"))?;

        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('0') => out.push('\0'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                Some('u') => {
                    // \u{XXXX}
                    let mut hex = String::new();
                    if chars.next() != Some('{') {
                        return Err(self.error("`\\u` must be followed by `{`"));
                    }
                    for c in chars.by_ref() {
                        if c == '}' {
                            break;
                        }
                        hex.push(c);
                    }
                    let v = u32::from_str_radix(&hex, 16)
                        .map_err(|_| self.error(format!("`{hex}` is not a code point")))?;
                    out.push(
                        char::from_u32(v)
                            .ok_or_else(|| self.error(format!("{v} is not a character")))?,
                    );
                }
                Some('x') => {
                    let hi = chars.next().unwrap_or('0');
                    let lo = chars.next().unwrap_or('0');
                    let v = u32::from_str_radix(&format!("{hi}{lo}"), 16)
                        .map_err(|_| self.error("`\\x` needs two hex digits"))?;
                    out.push(
                        char::from_u32(v)
                            .ok_or_else(|| self.error(format!("{v} is not a character")))?,
                    );
                }
                other => {
                    return Err(self.error(format!("unknown escape `\\{}`", other.unwrap_or(' '))));
                }
            }
        }
        Ok(out)
    }

    fn build(self) -> Result<DecodedMir, MirDecodeError> {
        let Some(pkg_id) = self.self_pkg else {
            return Err(MirDecodeError {
                line: 0,
                message: "missing `pkg 0` line".to_string(),
            });
        };
        let Some(meta_svh) = self.meta_svh else {
            return Err(MirDecodeError {
                line: 0,
                message: "missing `meta-svh` line".to_string(),
            });
        };
        let pkg_name = self.pkg_name.clone().unwrap_or_else(|| {
            "unknown"
                .parse()
                .expect("`unknown` is a valid package name")
        });

        let mut mir = Mir::new(pkg_name, pkg_id);
        mir.strings = self.strings;
        mir.module_natives = self.module_natives;
        for (def_id, item) in self.items {
            mir.items.insert(def_id, item);
        }
        Ok(DecodedMir { mir, meta_svh })
    }
}

fn bin_op(tok: &str) -> Option<BinOp> {
    Some(match tok {
        "add" => BinOp::Add,
        "sub" => BinOp::Sub,
        "mul" => BinOp::Mul,
        "div" => BinOp::Div,
        "rem" => BinOp::Rem,
        "eq" => BinOp::Eq,
        "ne" => BinOp::Ne,
        "lt" => BinOp::Lt,
        "le" => BinOp::Le,
        "gt" => BinOp::Gt,
        "ge" => BinOp::Ge,
        _ => return None,
    })
}

fn un_op(tok: &str) -> Option<UnOp> {
    match tok {
        "neg" => Some(UnOp::Neg),
        _ => None,
    }
}
