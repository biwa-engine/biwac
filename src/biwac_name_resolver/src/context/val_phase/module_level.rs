use std::collections::{HashMap, HashSet, hash_map::Entry};

use biwac_base::ModPath;
use biwac_hir::{AssocCallee, DefinedTy, Hir, InferTy, Ty, TyId, ValDefContentKind, ValId};
use biwac_parser::{DefTyp, ImportDecl, PrimTyp, QualifiedId, TypRepr, TypReprVal};

use crate::{
    ResolveError, RsvResult,
    context::{ty_phase::module_level::ModuleLevelTyResolveCtx, val_phase::ResolvedValue},
};

#[derive(Debug)]
pub(crate) enum ImportedSym {
    Mod(ModPath),
    Ty(TyId),
    Val(ValId),
}

#[derive(Debug)]
pub(crate) struct ModuleLevelResolveCtx {
    modpath: ModPath,
    imports: HashMap<String, (ImportDecl, ImportedSym)>,
    types: HashSet<String>, // 型は型のみのネームスペースで内側から解決
    vals: HashSet<String>,  // 値は値のみのネームスペースで内側から解決
}

impl ModuleLevelResolveCtx {
    pub(crate) fn new(mctx: ModuleLevelTyResolveCtx, hir: &Hir) -> RsvResult<Self> {
        let modpath = mctx.modpath;
        let types = mctx.types;
        let vals = mctx.vals;
        let mut imports: HashMap<String, (ImportDecl, ImportedSym)> = mctx
            .imports
            .into_iter()
            .map(|(id, (import_decl, sym))| {
                (
                    id,
                    (
                        import_decl,
                        match sym {
                            crate::context::ty_phase::module_level::ImportedSym::Ty(tid) => {
                                ImportedSym::Ty(tid)
                            }
                            crate::context::ty_phase::module_level::ImportedSym::Mod(module) => {
                                ImportedSym::Mod(module)
                            }
                        },
                    ),
                )
            })
            .collect();

        for import_decl in mctx.other_imports.into_values() {
            match imports.entry(import_decl.qualid.id.clone()) {
                Entry::Vacant(e) => {
                    // module -> type -> value の順で解決する
                    // すでにmodule, typeの解決は試行済みなので、
                    // valueに解決されなければならない
                    //
                    // import package::foo::bar; の場合
                    let vid = if import_decl.qualid.is_from_root {
                        ValId::new(
                            import_decl.qualid.quals.clone(),
                            import_decl.qualid.id.clone(),
                        )
                    } else {
                        // import foo::bar; の場合
                        // モジュール自身が package::baz::qux.biwa ならば
                        // package::baz::qux::foo::bar に解決
                        ValId::new(
                            [modpath.clone().into(), import_decl.qualid.quals.clone()].concat(),
                            import_decl.qualid.id.clone(),
                        )
                    };

                    if hir.vals.contains_key(&vid) {
                        e.insert((import_decl.clone(), ImportedSym::Val(vid)));
                    } else {
                        // error
                        todo!()
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

        Ok(Self {
            modpath,
            imports,
            types,
            vals,
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

    pub(crate) fn try_resolve_defined_ty(&self, deftyp: &DefTyp, hir: &Hir) -> RsvResult<Ty> {
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
                    ImportedSym::Val(_) => {
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
                ImportedSym::Val(_) => {
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
            if deftyp.genargs.len() == ty_existence.genarg_len {
                Ok(Ty::Defined(DefinedTy {
                    tid,
                    genargs: deftyp
                        .genargs
                        .iter()
                        .map(|typ| self.try_resolve_ty(typ, hir))
                        .collect::<RsvResult<_>>()?,
                }))
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

    // 値を解決する
    // 通常のグローバルな値(fn, const) -> 関連関数
    // の順で解決を試みる
    pub(crate) fn try_resolve_value(
        &self,
        qualid: &QualifiedId,
        hir: &Hir,
    ) -> RsvResult<ResolvedValue> {
        let vid = if qualid.is_from_root {
            // `package::hoge::fuga` の場合、直ちにOk
            RsvResult::Ok(ValId::new(qualid.quals.clone(), qualid.id.clone()))
        } else if qualid.quals.is_empty() {
            // `hoge` の場合
            if self.vals.contains(&qualid.id) {
                Ok(ValId::from_modpath(&self.modpath, qualid.id.clone()))
            } else if let Some((i, sym)) = self.imports.get(&qualid.id) {
                match sym {
                    ImportedSym::Ty(_) => {
                        // error
                        todo!()
                    }
                    ImportedSym::Mod(_) => {
                        // error
                        todo!()
                    }
                    ImportedSym::Val(vid) => Ok(vid.clone()),
                }
            } else {
                // 現在のモジュールからの相対パス
                // TODO: 外部packageとの区別
                Ok(ValId::new(
                    [self.modpath.clone().into(), qualid.quals.clone()].concat(),
                    qualid.id.clone(),
                ))
            }
        } else if let Some((i, sym)) = self.imports.get(qualid.quals.first().unwrap()) {
            // `import hoge::fuga; fuga::piyo::foo` の場合
            match sym {
                ImportedSym::Mod(module) => {
                    let mut module: Vec<String> = module.clone().into();
                    module.pop(); // hoge::fuga -> hoge
                    let quals: Vec<String> = [module, qualid.quals.clone()].concat();

                    Ok(ValId::new(quals, qualid.id.clone()))
                }
                ImportedSym::Ty(_) => {
                    // `import hoge::fuga; fuga::piyo`
                    // hoge::fuga は型 の場合
                    if qualid.quals.len() == 1 {
                        // TODO: hoge::fuga の関連値piyoを解決
                    }
                    // error
                    todo!()
                }
                ImportedSym::Val(_) => {
                    // error
                    todo!()
                }
            }
        } else {
            // 現在のモジュールからの相対パス
            // TODO: 外部packageとの区別
            Ok(ValId::new(
                [self.modpath.clone().into(), qualid.quals.clone()].concat(),
                qualid.id.clone(),
            ))
        }?;

        // パッケージ内の存在確認
        let err = if let Some(val) = hir.vals.get(&vid) {
            return match val {
                ValDefContentKind::Fn(_) => Ok(ResolvedValue::Global(vid)),
                ValDefContentKind::Native(_) => Ok(ResolvedValue::Global(vid)),
            };
        } else {
            Err(ResolveError::ValueNotFound {
                qualid: Box::new(qualid.clone()),
                vid: Box::new(vid),
            })
        };

        // qualid.qualsをTyIdとして関連関数を検索
        if !qualid.quals.is_empty() {
            // TODO: 正しくTyIdを構成
            let tid = TyId::new(
                qualid.quals[..qualid.quals.len() - 1].to_vec(),
                qualid.quals.last().unwrap().clone(),
            );

            if let Some(ty_existence) = hir.get_type_existence(&tid) {
                // TODO:
                // 明示的にジェネリック引数を記述している場合はそれを利用
                // foo::bar[Foo, Bar]::baz()
                // ない場合は、すべて推論が必要扱いで生成
                let genargs = vec![Ty::Infer(InferTy::Unknown); ty_existence.genarg_len];
                let ty = Ty::Defined(DefinedTy { tid, genargs });

                // 一意に取得できた場合のみ返す
                if let Some(impl_vid) = hir.get_impl_value_id_of_type(&ty, &qualid.id)? {
                    return Ok(ResolvedValue::Assoc(AssocCallee {
                        ty,
                        assoc: qualid.id.clone(),
                        impl_vid,
                    }));
                }
            }
        }

        // 通常の関数として解決を試みたときのエラーを返す
        err
    }
}
