use std::collections::{HashMap, HashSet, hash_map::Entry};

use biwac_base::ModPath;
use biwac_package_loader::Pkg;
use biwac_parser::{
    Globals, Ident, ImportDecl, ModAst, PrimTyp, QualifiedId, TypRepr, TypReprVal, TypeDef,
};

use crate::{ResolveError, RsvResult, Typ, TypId, types::DefinedTyp};

// PkgLvlRslvCtxを構築する際に
// 関連関数はSelf型名をTypIdに解決してから
// TypId :: <identifier> でアクセスされる
// PrePkgLvlRslvCtx, PreModLvlRslvCtxはそのためのcontext
#[derive(Debug)]
pub(super) struct PrePkgLvlRslvCtx {
    pub(super) typs: HashMap<TypId, usize>, // genarg len
}

#[derive(Debug)]
pub(super) struct PreModLvlRslvCtx<'pctx> {
    pkgctx: &'pctx PrePkgLvlRslvCtx,
    modpath: ModPath,
    imports: HashMap<String, ImportDecl>,
    types: HashSet<String>, // 型は型のみのネームスペースで内側から解決
}

impl PrePkgLvlRslvCtx {
    pub fn new(pkg: &Pkg) -> Self {
        let mut typs = HashMap::new();

        // 型名のみ収集
        for (modpath, modu) in &pkg.modules {
            for g in &modu.globals {
                if let Globals::TypeDef(t) = g {
                    match t {
                        TypeDef::Struct(s) => {
                            typs.insert(
                                TypId::from_modpath(modpath, s.id.id.clone()),
                                s.genargs.len(),
                            );
                        }
                    }
                }
            }
        }

        Self { typs }
    }
}

impl<'pctx> PreModLvlRslvCtx<'pctx> {
    pub fn new(
        pkgctx: &'pctx PrePkgLvlRslvCtx,
        modpath: ModPath,
        modu: &ModAst,
    ) -> RsvResult<Self> {
        let mut imports = HashMap::new();
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
                _ => {
                    // nothing to do
                }
            }
        }

        Ok(Self {
            pkgctx,
            modpath,
            imports,
            types: types.into_keys().collect(),
        })
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

        // パッケージ内での存在確認
        if self.pkgctx.typs.contains_key(&typid) {
            Ok(typid)
        } else {
            Err(ResolveError::TypeNotFound {
                qualid: Box::new(qualid.clone()),
                typid: Box::new(typid),
            })
        }
    }

    pub fn try_resolve_typ(&self, typ: &TypRepr) -> RsvResult<Typ> {
        let typ = match &typ.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => Typ::Int,
                PrimTyp::Uint => Typ::Int,
                PrimTyp::Float => Typ::Float,
                PrimTyp::Bool => Typ::Bool,
            },
            TypReprVal::Defined(deftyp) => Typ::Defined(DefinedTyp {
                id: self.try_resolve_deftyp(&deftyp.qualid)?,
                genargs: deftyp
                    .genargs
                    .iter()
                    .map(|typ| self.try_resolve_typ(typ))
                    .collect::<RsvResult<_>>()?,
            }),
        };

        Ok(typ)
    }
}
