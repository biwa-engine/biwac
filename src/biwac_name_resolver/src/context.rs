use std::collections::{HashMap, HashSet, hash_map::Entry};

use biwac_base::ModPath;
use biwac_package_loader::Pkg;
use biwac_parser::{Globals, Ident, ImportDecl, ModAst, PrimTyp, QualifiedId, TypRepr, TypeDef};

use crate::{AbsId, ResolveError, RsvResult, TryResolve, Typ};

// PkgLvlRslvCtx
// package level resolve context
// to check existence of symbols
#[derive(Debug)]
pub(crate) struct PkgLvlRslvCtx {
    syms: HashSet<AbsId>,
}

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

#[derive(Debug)]
pub(crate) struct FnLvlRslvCtx<'mctx> {
    modctx: &'mctx ModLvlRslvCtx<'mctx>,
    // TODO:
    // scope based variable name pool
    // ローカル変数に関数ローカルに一意なid
    // VarIdをつけたい
    // VarDecStmt はVarIdも持つようにし、
    // 他の変数の参照をするExprはすべてVarIdだけもたせる
    // これにより、以降の型検査などでスコープのネストによる
    // 名前空間を考えなくて良くなる。フラットに考えられる
    next_var_id: usize,
    // mut である必要がある
}

impl PkgLvlRslvCtx {
    pub fn new(pkg: &Pkg) -> Self {
        let mut syms = HashSet::new();

        for (modpath, modu) in &pkg.modules {
            for g in &modu.globals {
                match g {
                    Globals::Import(_) => {}
                    Globals::FnDef(f) => {
                        syms.insert(AbsId::from_modpath(modpath, f.id.id.clone()));
                    }
                    Globals::TypeDef(t) => match t {
                        TypeDef::Struct(s) => {
                            syms.insert(AbsId::from_modpath(modpath, s.id.id.clone()));
                        }
                    },
                    Globals::VarDecl(v) => {
                        syms.insert(AbsId::from_modpath(modpath, v.id.id.clone()));
                    }
                }
            }
        }

        Self { syms }
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

    // try_resolve_qualid
    // は変数名、関数名を解決する
    pub fn try_resolve_qualid(&self, qualid: &QualifiedId) -> RsvResult<AbsId> {
        let absid = if qualid.is_from_root {
            // `package::hoge::fuga` の場合、直ちにOk
            Ok(AbsId::new(qualid.quals.clone(), qualid.id.clone()))
        } else if qualid.quals.is_empty() {
            // `hoge` の場合
            if self.fns.contains(&qualid.id) {
                Ok(AbsId::from_modpath(&self.modpath, qualid.id.clone()))
            } else if let Some(i) = self.imports.get(&qualid.id) {
                // `import package::piyo::foo::hoge` の場合
                if i.qualid.is_from_root {
                    Ok(AbsId::new(qualid.quals.clone(), qualid.id.clone()))
                } else {
                    // `import piyo::foo::hoge` の場合
                    // TODO: 外部packageとの区別

                    Ok(AbsId::new(
                        self.modpath.clone().extend(i.qualid.quals.clone()).into(),
                        qualid.id.clone(),
                    ))
                }
            } else {
                Err(ResolveError::IdentifierNotFound(qualid.clone()))
            }
        } else if let Some(i) = self.imports.get(qualid.quals.first().unwrap()) {
            // `import hoge::fuga; fuga::piyo::foo` の場合

            if i.qualid.is_from_root {
                // `import package::hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [i.qualid.quals.clone(), qualid.quals.clone()].concat();

                Ok(AbsId::new(quals, qualid.id.clone()))
            } else {
                // `import hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [
                    self.modpath.clone().into(),
                    i.qualid.quals.clone(),
                    qualid.quals.clone(),
                ]
                .concat();

                Ok(AbsId::new(quals, qualid.id.clone()))
            }
        } else {
            Err(ResolveError::IdentifierNotFound(qualid.clone()))
        }?;

        if self.pkgctx.syms.contains(&absid) {
            Ok(absid)
        } else {
            Err(ResolveError::PackageSymbolNotFound(absid))
        }
    }

    // try_resolve_deftyp
    // は型名を解決する
    pub fn try_resolve_deftyp(&self, qualid: &QualifiedId) -> RsvResult<AbsId> {
        let absid = if qualid.is_from_root {
            // `package::hoge::fuga` の場合、直ちにOk
            Ok(AbsId::new(qualid.quals.clone(), qualid.id.clone()))
        } else if qualid.quals.is_empty() {
            // `hoge` の場合
            if self.types.contains(&qualid.id) {
                Ok(AbsId::from_modpath(&self.modpath, qualid.id.clone()))
            } else if let Some(i) = self.imports.get(&qualid.id) {
                // `import package::piyo::foo::hoge` の場合
                if i.qualid.is_from_root {
                    Ok(AbsId::new(qualid.quals.clone(), qualid.id.clone()))
                } else {
                    // `import piyo::foo::hoge` の場合
                    // TODO: 外部packageとの区別

                    Ok(AbsId::new(
                        self.modpath.clone().extend(i.qualid.quals.clone()).into(),
                        qualid.id.clone(),
                    ))
                }
            } else {
                Err(ResolveError::IdentifierNotFound(qualid.clone()))
            }
        } else if let Some(i) = self.imports.get(qualid.quals.first().unwrap()) {
            // `import hoge::fuga; fuga::piyo::foo` の場合

            if i.qualid.is_from_root {
                // `import package::hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [i.qualid.quals.clone(), qualid.quals.clone()].concat();

                Ok(AbsId::new(quals, qualid.id.clone()))
            } else {
                // `import hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [
                    self.modpath.clone().into(),
                    i.qualid.quals.clone(),
                    qualid.quals.clone(),
                ]
                .concat();

                Ok(AbsId::new(quals, qualid.id.clone()))
            }
        } else {
            Err(ResolveError::IdentifierNotFound(qualid.clone()))
        }?;

        if self.pkgctx.syms.contains(&absid) {
            Ok(absid)
        } else {
            Err(ResolveError::PackageSymbolNotFound(absid))
        }
    }
}

// LocVarId
// function local variable id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocVarId(usize);

pub(crate) enum ResolvedIdent {
    Var(LocVarId),
    Abs(AbsId),
}

impl<'mctx> FnLvlRslvCtx<'mctx> {
    pub fn new(modctx: &'mctx ModLvlRslvCtx) -> Self {
        Self {
            modctx,
            next_var_id: 0,
        }
    }

    // try_resolve_qualid
    // は変数名、関数名を解決する
    pub fn try_resolve_qualid(&self, qualid: &QualifiedId) -> RsvResult<ResolvedIdent> {
        // TODO: <identifier> なら先に関数ローカルで解決を試みる
        // LocVarId を返す

        Ok(ResolvedIdent::Abs(self.modctx.try_resolve_qualid(qualid)?))
    }

    // try_resolve_deftyp
    // は型名を解決する
    #[inline]
    pub fn try_resolve_deftyp(&self, qualid: &QualifiedId) -> RsvResult<AbsId> {
        // NOTE: 関数ローカルに型は宣言できないのでmodctxをそのまま呼び出すだけ
        // modctxを直接触らせないため必要
        self.modctx.try_resolve_deftyp(qualid)
    }
}

impl TryResolve<TypRepr> for Typ {
    fn try_resolve<'mctx>(
        value: TypRepr,
        fctx: &mut crate::context::FnLvlRslvCtx<'mctx>,
    ) -> crate::RsvResult<Self> {
        match value {
            TypRepr::Primitive(p) => match p {
                PrimTyp::Int => Ok(Typ::Int),
                PrimTyp::Uint => Ok(Typ::Int),
                PrimTyp::Bool => Ok(Typ::Int),
            },
            TypRepr::Defined(deftyp) => Ok(Typ::Defined(
                fctx.modctx.try_resolve_deftyp(&deftyp.qualid)?,
            )),
        }
    }
}
