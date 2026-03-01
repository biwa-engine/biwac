mod builder;

use std::collections::{HashMap, HashSet, hash_map::Entry};

use biwac_base::ModPath;
use biwac_package_loader::Pkg;
use biwac_parser::{Globals, Ident, ImportDecl, ModAst, QualifiedId, TypeDef};

use crate::{FnId, GenTypId, ResolveError, RsvResult, Typ, TypId, VarId};

// PkgLvlRslvCtx
// package level resolve context
// to check existence of symbols
//
// PkgLvlRslvCtx はパッケージレベルでのシンボルの存在を保持する文脈であり、
// シンボルの存在を検証するのに用いる
#[derive(Debug)]
pub struct PkgLvlRslvCtx {
    fns: HashSet<FnId>,
    pub(crate) typs: HashMap<TypId, PkgLvlRslvCtxTypImpl>,
}

#[derive(Debug, Clone)]
pub struct PkgLvlRslvCtxTypImpl {
    // 関連関数の型名 -> 関連関数のそれぞれの実装
    pub(crate) assocs: HashMap<String, PkgLvlRslvCtxTypAssocImpl>,
    genarg_len: usize,
}

#[derive(Debug, Clone)]
pub struct PkgLvlRslvCtxTypAssocImpl {
    // 関連関数が実装されている際のターゲットの型のジェネリック型具体表明列 がキー
    //  impl[T] Foo[T, Int] {
    //             ^^^^^^^^
    //      fn bar[U]() { ... }
    //  }
    pub(crate) genargs_map: HashMap<Vec<Typ>, (AssocId, Ident)>,
    //                   ^^^^^^^^
    next_assoc_id: usize,
}

// ある型の実装の中で一意な関連関数のid
// つまり TypId + AssocId でcallerはcalleeの実体にアクセスする
// 名前解決以降、ジェネリクス計算をせずにすむようにするため
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssocId(usize);

// ModLocRslvCtx
// means module level resolve context.
// This is to resolve symbol names in module level.
// This contains PkgLvlRslvCtx, the module path, imported names in the module,
// the functions in the module, the types in the module.
#[derive(Debug)]
pub(crate) struct ModLvlRslvCtx<'pctx> {
    pkgctx: &'pctx PkgLvlRslvCtx,
    modpath: ModPath,
    imports: HashMap<String, ImportDecl>,
    fns: HashSet<String>,   // 変数、関数は同じネームスペースで内側から解決
    types: HashSet<String>, // 型は型のみのネームスペースで内側から解決
}

// impl block レベルでジェネリクス型名を解決するためのコンテキスト
#[derive(Debug)]
pub(crate) struct ImplLvlGenTypRslvCtx<'mctx> {
    pub(crate) mctx: &'mctx ModLvlRslvCtx<'mctx>,
    pub(crate) impl_genargs: HashMap<String, GenTypId>,
    // pub(crate) self_typ: Typ,
    //  impl[T] Foo[T, Int] { ... }
    //      ^^^ ^^^^^^^^^^
    //      |   | self_typ
    //      | impl_genargs
    next_gen_id: usize,
}

#[derive(Debug)]
pub(crate) struct FnLvlRslvCtx<'mctx> {
    pub(crate) modctx: &'mctx ModLvlRslvCtx<'mctx>,
    // scope based variable name pool
    // ローカル変数に関数ローカルに一意なid
    // LocVarIdをつける
    // Stmt::VarDecl はLocVarIdも持つようにし、
    // 他の変数の参照をするExprはすべてLocVarIdだけもたせる
    // これにより、以降の型検査などでスコープのネストによる
    // 名前空間を考えなくて良くなる。フラットに考えられる
    scopes: Vec<HashMap<String, (DecledVar, LocVarId)>>,
    next_var_id: usize,
    next_expr_id: usize,
    pub(crate) ictx: Option<&'mctx ImplLvlGenTypRslvCtx<'mctx>>,
    pub(crate) genargs: HashMap<String, GenTypId>,
}

#[derive(Debug, Clone)]
pub struct DecledVar {
    pub id: Ident,
    pub typ: Option<Typ>, // None means not annotationed
}

impl PkgLvlRslvCtx {
    pub fn new(pkg: &Pkg) -> RsvResult<Self> {
        let mut fns = HashSet::new();
        let mut typs = HashMap::new();
        let pre_pctx = builder::PrePkgLvlRslvCtx::new(pkg);

        // pre_pctxで収集済みのパッケージ内のすべての型をtypsに予め記録
        for (typid, &genarg_len) in &pre_pctx.typs {
            typs.insert(
                typid.clone(),
                PkgLvlRslvCtxTypImpl {
                    assocs: HashMap::new(),
                    genarg_len,
                },
            );
        }
        // TODO: Intなどプリミティブ型も記録
        // stdに限る?

        for (modpath, modu) in &pkg.modules {
            let pre_mctx = builder::PreModLvlRslvCtx::new(&pre_pctx, modpath.clone(), modu)?;

            for g in &modu.globals {
                match g {
                    Globals::Import(_) => {}
                    Globals::FnDef(f) => {
                        // 関連関数の場合
                        if let Some(impl_ctx) = &f.impl_ctx {
                            let self_typ = pre_mctx.try_resolve_typ(&impl_ctx.self_typ)?;
                            let self_typ_genargs = self_typ.get_genargs();
                            let typid = TypId::try_from(self_typ.clone())?;

                            // SAFETY: pre_mctxですでに存在は確認済み
                            let typ_impl =
                                typs.get_mut(&typid).expect("compiler bug: type not found");

                            // self_typ_genargs.len() の一致確認
                            if self_typ_genargs.len() != typ_impl.genarg_len {
                                return Err(ResolveError::GenericArgLengthMismatched {
                                    typ: Box::new(self_typ),
                                    typ_impl_repr: Box::new(impl_ctx.self_typ.clone()),
                                    actual_len: typ_impl.genarg_len,
                                });
                            }

                            // すでに関連関数の実装が1つでもあれば
                            // 今回追加する名前の関連関数がすでに無いことを確認し、
                            // AssocIdを付与して記録
                            if let Some(assoc_impl) = typ_impl.assocs.get_mut(&f.id.id) {
                                match assoc_impl.genargs_map.entry(self_typ_genargs) {
                                    Entry::Vacant(e) => {
                                        let associd = AssocId(assoc_impl.next_assoc_id);
                                        assoc_impl.next_assoc_id += 1;
                                        e.insert((associd, f.id.clone()));
                                    }
                                    Entry::Occupied(e) => {
                                        return Err(
                                            ResolveError::DuplicatedImplementationForType {
                                                typ: self_typ,
                                                fid1: Box::new(e.get().1.clone()),
                                                fid2: Box::new(f.id.clone()),
                                            },
                                        );
                                    }
                                }
                            } else {
                                // まだ関連関数の実装がなければ
                                // 初期化とともに記録
                                typ_impl.assocs.insert(
                                    f.id.id.clone(),
                                    PkgLvlRslvCtxTypAssocImpl {
                                        genargs_map: [(
                                            self_typ_genargs,
                                            (AssocId(0), f.id.clone()),
                                        )]
                                        .into(),
                                        next_assoc_id: 1,
                                    },
                                );
                            }
                        } else {
                            // 通常の関数の場合
                            fns.insert(FnId::from_modpath(modpath, f.id.id.clone()));
                        }
                    }
                    Globals::NativeFnDef(f) => {
                        fns.insert(FnId::from_modpath(modpath, f.id.id.clone()));
                    }
                    Globals::MethodDef(_) => {
                        // メソッドはFnIdでアクセスされるわけではないため、記録しない
                    }
                    Globals::TypeDef(_) => {
                        // すでに記録済み
                    }
                    Globals::VarDecl(_) => {
                        todo!()
                    }
                }
            }
        }

        Ok(Self { fns, typs })
    }

    #[inline]
    fn exists_fn(&self, fid: &FnId) -> bool {
        self.fns.contains(fid)
    }

    #[inline]
    fn exists_typ(&self, tid: &TypId) -> bool {
        self.typs.contains_key(tid)
    }

    // 関連関数をジェネリック型引数列からの選択をしたうえで取得する
    // 関連関数はname_resolverのフェーズで特定してしまう。
    fn exists_associated_fn(
        &self,
        tid: &TypId,
        id: &String,
        genargs: Option<&Vec<Typ>>,
    ) -> RsvResult<AssocId> {
        // TODO:
        //  if FnId.qualsをTypIdとしたときにその型がtypsに見つかる
        //      if ジェネリック型引数具体表明がない:
        //          // パース時には関数呼び出しと区別がつかない。
        //          // 通常の関数呼び出しの解決へのフォールバックはこの外部で行う
        //          if id の関連関数名を実装するジェネリック型引数列が1つのみならば:
        //              解決
        //          else:
        //              エラー
        //      else:
        //          if id の関連関数名を実装するジェネリック型引数列のうちgenargsが満たすものが1つのみに定まるならば:
        //              解決
        //          else:
        //              エラー
        //  else:
        //      エラー
        //
        //
        todo!()
        // self.typs.get(tid).map(|timpl| {
        //     &timpl.impls.values().find(|impls| {
        //         if let Some(f) = impls.get(id)
        //             && let ImpledFn::Assoc(_) = f
        //         {
        //             true
        //         } else {
        //             false
        //         }
        //     })
        // })
    }
}

impl<'pctx> ModLvlRslvCtx<'pctx> {
    pub fn new(pkgctx: &'pctx PkgLvlRslvCtx, modpath: ModPath, modu: &ModAst) -> RsvResult<Self> {
        let mut imports = HashMap::new();
        let mut fns = HashMap::<String, Ident>::new();
        let mut types = HashMap::<String, Ident>::new();

        for g in &modu.globals {
            match g {
                Globals::Import(i) => match imports.entry(i.qualid.id.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(i.clone());
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedImportedName {
                            name: i.qualid.id.clone(),
                            imp1: Box::new(e.remove()),
                            imp2: Box::new(i.clone()),
                        });
                    }
                },
                Globals::FnDef(f) => match fns.entry(f.id.id.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(f.id.clone());
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedFnName {
                            fid1: Box::new(f.id.clone()),
                            fid2: Box::new(e.remove()),
                        });
                    }
                },
                Globals::NativeFnDef(f) => match fns.entry(f.id.id.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(f.id.clone());
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedFnName {
                            fid1: Box::new(f.id.clone()),
                            fid2: Box::new(e.remove()),
                        });
                    }
                },
                Globals::TypeDef(t) => match t {
                    TypeDef::Struct(s) => match types.entry(s.id.id.clone()) {
                        Entry::Vacant(e) => {
                            e.insert(s.id.clone());
                        }
                        Entry::Occupied(e) => {
                            return Err(ResolveError::DuplicatedTypeName {
                                tid1: Box::new(s.id.clone()),
                                tid2: Box::new(e.remove()),
                            });
                        }
                    },
                },
                Globals::VarDecl(_) => todo!(),
                Globals::MethodDef(_) => {
                    // nothing to do
                }
            }
        }

        Ok(Self {
            pkgctx,
            modpath,
            imports,
            fns: fns.into_keys().collect(),
            types: types.into_keys().collect(),
        })
    }

    // try_resolve_fn
    // は関数名を解決する
    // 関連関数である場合にも対応。
    // まずqualid.qualsを型名と仮定して型を引き、
    // 存在するならば関連関数を引く
    fn try_resolve_fn(&self, qualid: &QualifiedId) -> RsvResult<ResolvedFn> {
        // qualid.qualsをTypIdとして関連関数を検索
        if !qualid.quals.is_empty() {
            let typid = TypId::new(
                qualid.quals[..qualid.quals.len() - 1].to_vec(),
                qualid.quals.last().unwrap().clone(),
            );

            // typidで引いて型が存在するなら、
            // 関連関数でなければならないので、
            if self.pkgctx.exists_typ(&typid) {
                let associd = self.pkgctx.exists_associated_fn(&typid, &qualid.id, None)?;

                return Ok(ResolvedFn::Assoc(typid, associd));
            }
        }

        let fid = if qualid.is_from_root {
            // `package::hoge::fuga` の場合、直ちにOk
            Ok(FnId::new(qualid.quals.clone(), qualid.id.clone()))
        } else if qualid.quals.is_empty() {
            // `hoge` の場合
            if self.fns.contains(&qualid.id) {
                Ok(FnId::from_modpath(&self.modpath, qualid.id.clone()))
            } else if let Some(i) = self.imports.get(&qualid.id) {
                // `import package::piyo::foo::hoge` の場合
                if i.qualid.is_from_root {
                    Ok(FnId::new(i.qualid.quals.clone(), qualid.id.clone()))
                } else {
                    // `import piyo::foo::hoge` の場合
                    // TODO: 外部packageとの区別

                    Ok(FnId::new(
                        self.modpath.clone().extend(i.qualid.quals.clone()).into(),
                        qualid.id.clone(),
                    ))
                }
            } else {
                // 現在のモジュールからの相対パス
                // TODO: 外部packageとの区別
                Ok(FnId::new(
                    [self.modpath.clone().into(), qualid.quals.clone()].concat(),
                    qualid.id.clone(),
                ))
            }
        } else if let Some(i) = self.imports.get(qualid.quals.first().unwrap()) {
            // `import hoge::fuga; fuga::piyo::foo` の場合

            if i.qualid.is_from_root {
                // `import package::hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [i.qualid.quals.clone(), qualid.quals.clone()].concat();

                Ok(FnId::new(quals, qualid.id.clone()))
            } else {
                // `import hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [
                    self.modpath.clone().into(),
                    i.qualid.quals.clone(),
                    qualid.quals.clone(),
                ]
                .concat();

                Ok(FnId::new(quals, qualid.id.clone()))
            }
        } else {
            // 現在のモジュールからの相対パス
            // TODO: 外部packageとの区別
            Ok(FnId::new(
                [self.modpath.clone().into(), qualid.quals.clone()].concat(),
                qualid.id.clone(),
            ))
        }?;

        // パッケージ内の存在確認
        if self.pkgctx.exists_fn(&fid) {
            Ok(ResolvedFn::Fn(fid))
        } else {
            Err(ResolveError::FunctionNotFound {
                qualid: Box::new(qualid.clone()),
                fid: Box::new(fid),
            })
        }
    }

    // try_resolve_deftyp
    // は型名を解決する
    pub fn try_resolve_deftyp(&self, qualid: &QualifiedId) -> RsvResult<TypId> {
        let typid = if qualid.is_from_root {
            // `package::hoge::fuga` の場合、直ちにOk
            Ok(TypId::new(qualid.quals.clone(), qualid.id.clone()))
        } else if qualid.quals.is_empty() {
            // `hoge` の場合
            if self.types.contains(&qualid.id) {
                Ok(TypId::from_modpath(&self.modpath, qualid.id.clone()))
            } else if let Some(i) = self.imports.get(&qualid.id) {
                // `import package::piyo::foo::hoge` の場合
                if i.qualid.is_from_root {
                    Ok(TypId::new(i.qualid.quals.clone(), qualid.id.clone()))
                } else {
                    // `import piyo::foo::hoge` の場合
                    // TODO: 外部packageとの区別

                    Ok(TypId::new(
                        self.modpath.clone().extend(i.qualid.quals.clone()).into(),
                        qualid.id.clone(),
                    ))
                }
            } else {
                // 現在のモジュールからの相対パス
                // TODO: 外部packageとの区別
                Ok(TypId::new(
                    [self.modpath.clone().into(), qualid.quals.clone()].concat(),
                    qualid.id.clone(),
                ))
            }
        } else if let Some(i) = self.imports.get(qualid.quals.first().unwrap()) {
            // `import hoge::fuga; fuga::piyo::foo` の場合

            if i.qualid.is_from_root {
                // `import package::hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [i.qualid.quals.clone(), qualid.quals.clone()].concat();

                Ok(TypId::new(quals, qualid.id.clone()))
            } else {
                // `import hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [
                    self.modpath.clone().into(),
                    i.qualid.quals.clone(),
                    qualid.quals.clone(),
                ]
                .concat();

                Ok(TypId::new(quals, qualid.id.clone()))
            }
        } else {
            // 現在のモジュールからの相対パス
            // TODO: 外部packageとの区別
            Ok(TypId::new(
                [self.modpath.clone().into(), qualid.quals.clone()].concat(),
                qualid.id.clone(),
            ))
        }?;

        if self.pkgctx.exists_typ(&typid) {
            Ok(typid)
        } else {
            Err(ResolveError::TypeNotFound {
                qualid: Box::new(qualid.clone()),
                typid: Box::new(typid),
            })
        }
    }
}

// ExprId
// function local expression id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExprId(usize);

// LocVarId
// function local variable id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocVarId(usize);

impl LocVarId {
    pub fn value(&self) -> &usize {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedIdent {
    Var(LocVarId),
    Fn(FnId),
    Assoc(TypId, AssocId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedVar {
    Local(LocVarId),
    Global(VarId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedFn {
    Var(LocVarId),
    Fn(FnId),
    Assoc(TypId, AssocId),
}

impl<'ictx> FnLvlRslvCtx<'ictx> {
    pub(crate) fn try_from_ictx(
        genargs: Option<&Vec<Ident>>,
        ictx: &'ictx ImplLvlGenTypRslvCtx<'ictx>,
    ) -> RsvResult<Self> {
        let mut next_gen_id = ictx.next_gen_id;
        let mut genarg_map = HashMap::<String, (GenTypId, Ident)>::new();

        if let Some(genargs) = genargs {
            for ident in genargs {
                match genarg_map.entry(ident.id.clone()) {
                    Entry::Vacant(e) => {
                        let id = GenTypId::new(next_gen_id);
                        next_gen_id += 1;
                        e.insert((id, ident.clone()));
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedGenericTypeDeclaration {
                            tid1: Box::new(e.get().1.clone()),
                            tid2: Box::new(ident.clone()),
                        });
                    }
                }
            }
        }

        Ok(Self {
            modctx: ictx.mctx,
            scopes: vec![HashMap::new()],
            next_var_id: 0,
            next_expr_id: 0,
            ictx: Some(ictx),
            genargs: genarg_map
                .into_iter()
                .map(|(name, (id, _))| (name, id))
                .collect::<HashMap<_, _>>(),
        })
    }
}

impl<'mctx> FnLvlRslvCtx<'mctx> {
    pub(crate) fn try_from_mctx(
        genargs: Option<&Vec<Ident>>,
        mctx: &'mctx ModLvlRslvCtx,
    ) -> RsvResult<Self> {
        let mut next_gen_id = 0;
        let mut genarg_map = HashMap::<String, (GenTypId, Ident)>::new();

        if let Some(genargs) = genargs {
            for ident in genargs {
                match genarg_map.entry(ident.id.clone()) {
                    Entry::Vacant(e) => {
                        let id = GenTypId::new(next_gen_id);
                        next_gen_id += 1;
                        e.insert((id, ident.clone()));
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedGenericTypeDeclaration {
                            tid1: Box::new(e.get().1.clone()),
                            tid2: Box::new(ident.clone()),
                        });
                    }
                }
            }
        }

        Ok(Self {
            modctx: mctx,
            scopes: vec![HashMap::new()],
            next_var_id: 0,
            next_expr_id: 0,
            ictx: None,
            genargs: genarg_map
                .into_iter()
                .map(|(name, (id, _))| (name, id))
                .collect::<HashMap<_, _>>(),
        })
    }

    // try_resolve_fn
    // は関数名を解決する
    pub fn try_resolve_fn(&mut self, qualid: &QualifiedId) -> RsvResult<ResolvedFn> {
        // 先に関数ローカルで、内側のスコープから、解決を試みる
        if !qualid.is_from_root && qualid.quals.is_empty() {
            for scope in self.scopes.iter().rev() {
                if let Some((_, var_id)) = scope.get(&qualid.id) {
                    return Ok(ResolvedFn::Var(*var_id));
                }
            }
        }

        Ok(self.modctx.try_resolve_fn(qualid)?)
    }

    // try_resolve_variable
    // は変数名を解決する
    pub fn try_resolve_variable(&self, ident: &Ident) -> RsvResult<ResolvedVar> {
        // 先に関数ローカルで、内側のスコープから、解決を試みる
        for scope in self.scopes.iter().rev() {
            if let Some((_, var_id)) = scope.get(&ident.id) {
                return Ok(ResolvedVar::Local(*var_id));
            }
        }

        // TODO: グローバル変数の解決
        todo!()
        // Ok(ResolvedIdent::Fn(self.modctx.try_resolve_fn(
        //     &QualifiedId {
        //         is_from_root: false,
        //         quals: vec![],
        //         id: ident.id.clone(),
        //         span: ident.span.clone(),
        //     },
        // )?))
    }

    // try_resolve_deftyp
    // は型名を解決する
    pub(crate) fn try_resolve_deftyp(&mut self, qualid: &QualifiedId) -> RsvResult<TypId> {
        self.modctx.try_resolve_deftyp(qualid)
    }

    pub(crate) fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(crate) fn exit_scope(&mut self) {
        // popping when empty is compiler bug
        self.scopes.pop().unwrap();
    }

    pub(crate) fn declare_variable(
        &mut self,
        var: &Ident,
        typ: Option<Typ>,
    ) -> RsvResult<LocVarId> {
        match self.scopes.last_mut().unwrap().entry(var.id.clone()) {
            Entry::Vacant(e) => {
                let var_id = LocVarId(self.next_var_id);
                e.insert((
                    DecledVar {
                        id: var.clone(),
                        typ,
                    },
                    var_id,
                ));
                self.next_var_id += 1;

                Ok(var_id)
            }
            Entry::Occupied(e) => Err(ResolveError::DuplicatedVarName {
                vid1: Box::new(var.clone()),
                vid2: Box::new(e.get().0.id.clone()),
            }),
        }
    }

    pub(crate) fn new_expr_id(&mut self) -> ExprId {
        let id = ExprId(self.next_expr_id);
        self.next_expr_id += 1;

        id
    }

    pub(crate) fn into_vars(self) -> HashMap<LocVarId, DecledVar> {
        self.scopes
            .into_iter()
            .flat_map(|scope| scope.into_iter())
            .map(|(_, (ident, var_id))| (var_id, ident))
            .collect()
    }
}

impl<'mctx> ImplLvlGenTypRslvCtx<'mctx> {
    pub(crate) fn new(
        impl_genargs: Option<&Vec<Ident>>,
        mctx: &'mctx ModLvlRslvCtx,
    ) -> RsvResult<Self> {
        let mut next_gen_id = 0;
        let mut impl_genarg_map = HashMap::<String, (GenTypId, Ident)>::new();

        if let Some(impl_genargs) = &impl_genargs {
            for ident in impl_genargs.iter() {
                match impl_genarg_map.entry(ident.id.clone()) {
                    Entry::Vacant(e) => {
                        let id = GenTypId::new(next_gen_id);
                        next_gen_id += 1;
                        e.insert((id, ident.clone()));
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedGenericTypeDeclaration {
                            tid1: Box::new(e.get().1.clone()),
                            tid2: Box::new(ident.clone()),
                        });
                    }
                }
            }
        }

        Ok(Self {
            mctx,
            impl_genargs: impl_genarg_map
                .into_iter()
                .map(|(name, (id, _))| (name, id))
                .collect::<HashMap<_, _>>(),
            next_gen_id,
        })
    }

    pub(crate) fn new_empty(mctx: &'mctx ModLvlRslvCtx) -> Self {
        Self {
            impl_genargs: HashMap::new(),
            mctx,
            next_gen_id: 0,
        }
    }
}
