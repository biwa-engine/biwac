use std::collections::{HashMap, HashSet, hash_map::Entry};

use biwac_base::ModPath;
use biwac_hir::{DefinedTy, Hir, InferTy, Ty, TyExistence, TyId};
use biwac_parser::{
    DefTyp, Globals, Ident, ImportDecl, ModAst, PrimTyp, TypRepr, TypReprVal, TypeDef,
};

use crate::{ResolveError, RsvResult};

#[derive(Debug)]
pub(crate) enum ImportedSym {
    Mod(ModPath),
    Ty(TyId),
}

#[derive(Debug)]
pub(crate) struct ModuleLevelTyResolveCtx {
    pub(crate) modpath: ModPath,
    pub(crate) imports: HashMap<String, (ImportDecl, ImportedSym)>,
    pub(crate) other_imports: HashMap<String, ImportDecl>, // 型名として解決できなかったimport
    pub(crate) types: HashSet<String>, // 型は型のみのネームスペースで内側から解決
    pub(crate) vals: HashSet<String>,  // 値は値のみのネームスペースで内側から解決
}

impl ModuleLevelTyResolveCtx {
    pub(crate) fn new(modpath: ModPath, modu: &ModAst, hir: &Hir) -> RsvResult<Self> {
        let mut imports = HashMap::new();
        let mut other_imports = HashMap::new();
        let mut types = HashMap::<String, Ident>::new();
        let mut vals = HashMap::<String, Ident>::new();

        for g in &modu.globals {
            match g {
                Globals::Import(import_decl) => {
                    match imports.entry(import_decl.qualid.id.clone()) {
                        Entry::Vacant(e) => {
                            // module -> type -> value の順で解決する
                            // この時点でvalueは登録されていないため、typeまで
                            // 解決されなければvalueであることを期待してother_importsに保持する
                            //
                            // import package::foo::bar; の場合
                            if import_decl.qualid.is_from_root {
                                let module = ModPath::Mod(
                                    [
                                        import_decl.qualid.quals.clone(),
                                        vec![import_decl.qualid.id.clone()],
                                    ]
                                    .concat(),
                                );

                                if hir.modules.contains(&module) {
                                    e.insert((import_decl.clone(), ImportedSym::Mod(module)));
                                } else {
                                    let tid = TyId::new(
                                        import_decl.qualid.quals.clone(),
                                        import_decl.qualid.id.clone(),
                                    );

                                    if hir.tys.contains_key(&tid) {
                                        e.insert((import_decl.clone(), ImportedSym::Ty(tid)));
                                    } else {
                                        other_imports.insert(
                                            import_decl.qualid.id.clone(),
                                            import_decl.clone(),
                                        );
                                    }
                                }
                            } else {
                                // import foo::bar; の場合
                                // モジュール自身が package::baz::qux.biwa ならば
                                // package::baz::qux::foo::bar に解決
                                let module = ModPath::Mod(
                                    [
                                        modpath.clone().into(),
                                        import_decl.qualid.quals.clone(),
                                        vec![import_decl.qualid.id.clone()],
                                    ]
                                    .concat(),
                                );

                                if hir.modules.contains(&module) {
                                    e.insert((import_decl.clone(), ImportedSym::Mod(module)));
                                } else {
                                    let tid = TyId::new(
                                        [modpath.clone().into(), import_decl.qualid.quals.clone()]
                                            .concat(),
                                        import_decl.qualid.id.clone(),
                                    );

                                    if hir.tys.contains_key(&tid) {
                                        e.insert((import_decl.clone(), ImportedSym::Ty(tid)));
                                    } else {
                                        other_imports.insert(
                                            import_decl.qualid.id.clone(),
                                            import_decl.clone(),
                                        );
                                    }
                                }
                            }
                        }
                        Entry::Occupied(e) => {
                            return Err(ResolveError::DuplicatedImportedName {
                                name: import_decl.qualid.id.clone(),
                                imp1: Box::new(e.remove().0),
                                imp2: Box::new(import_decl.clone()),
                            });
                        }
                    }
                }
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
                    TypeDef::TypeAlias(alias) => match types.entry(alias.ident.id.clone()) {
                        Entry::Vacant(e) => {
                            e.insert(alias.ident.clone());
                        }
                        Entry::Occupied(e) => {
                            return Err(ResolveError::DuplicatedTypeName {
                                tid1: Box::new(alias.ident.clone()),
                                tid2: Box::new(e.remove()),
                            });
                        }
                    },
                    TypeDef::NativeTypeAlias(native) => {
                        match types.entry(native.ident.id.clone()) {
                            Entry::Vacant(e) => {
                                e.insert(native.ident.clone());
                            }
                            Entry::Occupied(e) => {
                                return Err(ResolveError::DuplicatedTypeName {
                                    tid1: Box::new(native.ident.clone()),
                                    tid2: Box::new(e.remove()),
                                });
                            }
                        }
                    }
                },
                Globals::FnDef(fn_def) => match vals.entry(fn_def.id.id.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(fn_def.id.clone());
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedTypeName {
                            tid1: Box::new(fn_def.id.clone()),
                            tid2: Box::new(e.remove()),
                        });
                    }
                },
                Globals::NativeFnDef(fn_def) => match vals.entry(fn_def.id.id.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(fn_def.id.clone());
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedTypeName {
                            tid1: Box::new(fn_def.id.clone()),
                            tid2: Box::new(e.remove()),
                        });
                    }
                },
                Globals::VarDecl(var_decl) => match vals.entry(var_decl.id.id.clone()) {
                    Entry::Vacant(e) => {
                        e.insert(var_decl.id.clone());
                    }
                    Entry::Occupied(e) => {
                        return Err(ResolveError::DuplicatedTypeName {
                            tid1: Box::new(var_decl.id.clone()),
                            tid2: Box::new(e.remove()),
                        });
                    }
                },
                Globals::MethodDef(_) => {
                    // nothing to do
                }
            }
        }

        Ok(Self {
            modpath,
            imports,
            other_imports,
            types: types.into_keys().collect(),
            vals: vals.into_keys().collect(),
        })
    }

    fn try_resolve_ty(&self, typ: &TypRepr, hir: &Hir) -> RsvResult<Ty> {
        match &typ.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => Ok(Ty::Int),
                PrimTyp::Uint => Ok(Ty::Int), // TODO
                PrimTyp::Float => Ok(Ty::Float),
                PrimTyp::Bool => Ok(Ty::Bool),
            },
            TypReprVal::Defined(deftyp) => self.try_resolve_defined_ty(deftyp, hir),
        }
    }

    // ジェネリック引数の数が合うかも検査する
    pub(crate) fn try_resolve_defined_tid(
        &self,
        deftyp: &DefTyp,
        hir: &Hir,
    ) -> RsvResult<(TyId, TyExistence)> {
        let tid = if deftyp.qualid.is_from_root {
            // `package::hoge::fuga` の場合、直ちにOk
            TyId::new(deftyp.qualid.quals.clone(), deftyp.qualid.id.clone())
        } else if deftyp.qualid.quals.is_empty() {
            // `hoge` の場合
            if self.types.contains(&deftyp.qualid.id) {
                TyId::from_modpath(&self.modpath, deftyp.qualid.id.clone())
            } else if let Some((_, sym)) = self.imports.get(&deftyp.qualid.id) {
                match sym {
                    ImportedSym::Ty(tid) => tid.clone(),
                    ImportedSym::Mod(_) => {
                        // error
                        todo!()
                    }
                }
            } else {
                // 現在のモジュールからの相対パス
                // TODO: 外部packageとの区別
                TyId::new(
                    [self.modpath.clone().into(), deftyp.qualid.quals.clone()].concat(),
                    deftyp.qualid.id.clone(),
                )
            }
        } else if let Some((_, sym)) = self.imports.get(deftyp.qualid.quals.first().unwrap()) {
            // `import hoge::fuga; fuga::piyo::foo` の場合
            match sym {
                ImportedSym::Mod(module) => {
                    // `import package::hoge::fuga; fuga::piyo::foo` の場合
                    let mut module: Vec<String> = module.clone().into();
                    module.pop(); // hoge::fuga -> hoge
                    let quals: Vec<String> = [module, deftyp.qualid.quals.clone()].concat();

                    TyId::new(quals, deftyp.qualid.id.clone())
                }
                ImportedSym::Ty(_) => {
                    // error
                    todo!()
                }
            }
        } else {
            // 現在のモジュールからの相対パス
            // TODO: 外部packageとの区別
            TyId::new(
                [self.modpath.clone().into(), deftyp.qualid.quals.clone()].concat(),
                deftyp.qualid.id.clone(),
            )
        };

        if let Some(ty_existence) = hir.get_type_existence(&tid) {
            // ジェネリック引数の数が合うか検査
            if let Some(genargs) = &deftyp.genargs
                && genargs.len() == ty_existence.genarg_len
            {
                Ok((tid, ty_existence))
            } else if deftyp.genargs.is_none() {
                Ok((tid, ty_existence))
            } else {
                Err(ResolveError::GenericArgLengthMismatched {
                    deftyp: Box::new(deftyp.clone()),
                    tid: Box::new(tid),
                    ty_existence: Box::new(ty_existence),
                })
            }
        } else {
            Err(ResolveError::TypeNotFound {
                qualid: Box::new(deftyp.qualid.clone()),
                tid: Box::new(tid),
            })
        }
    }

    pub(crate) fn try_resolve_defined_ty(&self, deftyp: &DefTyp, hir: &Hir) -> RsvResult<Ty> {
        // ジェネリック引数の数が合うか検査済み
        let (tid, ty_existence) = self.try_resolve_defined_tid(deftyp, hir)?;

        Ok(Ty::Defined(DefinedTy {
            tid,
            genargs: if let Some(genargs) = &deftyp.genargs {
                genargs
                    .iter()
                    .map(|typ| self.try_resolve_ty(typ, hir))
                    .collect::<RsvResult<_>>()?
            } else {
                vec![Ty::Infer(InferTy::Unknown); ty_existence.genarg_len]
            },
        }))
    }
}
