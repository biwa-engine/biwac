pub(crate) mod context;
mod symbols;
mod types;

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, hash_map::Entry},
    fmt::Display,
};

use biwac_base::ModPath;
use biwac_package_loader::Pkg;
use biwac_parser::{Ident, ImportDecl, QualifiedId, TypeDef};

use crate::context::{FnLvlRslvCtx, ModLvlRslvCtx, PkgLvlRslvCtx};
pub use crate::{
    context::{DecledVar, ExprId, LocVarId, ResolvedIdent},
    symbols::{
        ModSym,
        expressions::{
            BlockExpr, Callee, Expr, ExprVal, FnCall, Literal, MemberAccess, Primary,
            StructLiteral, Variable,
        },
        globals::{
            DecledArg, FnDefContent, GlobalVarDecl, MethodDefContent, NativeFnArgDecl,
            NativeFnDefContent, StructDefContent, TypeDefContent,
        },
        statements::{
            AssignStmt, BlockStmt, ExprStmt, IfStmt, ReturnStmt, Stmt, VarDecl, WhileStmt,
        },
    },
    types::{FnTyp, Typ},
};

// このcrate biwac_name_resolver は、
// package内のあらゆる名前の解決をすることを目指す。
// - 関数や型といったグローバルなシンボルはもちろん、
// - 関数ローカルな変数の名前、
// - ~~構造体のメンバの名前に至るまで、~~
//
// - 同一の名前空間で名前の重複がないこと、
// - 名前空間の優先度に従って、その名前のシンボルの存在を確認し、絶対的な参照名に変換すること
// - モジュールレベルの公開設定に違反せずに、名前解決が出来ること
//   - ただし、構造体のメンバは型推論して始めて構造体の種類が特定できるなどの理由から
//     このパスでは行わないこととする
//     型の内容のチェックは次のパスに任せる
// のすべてを満たしつつ解決を目指す。

#[derive(Debug)]
pub enum ResolveError {
    PackageSymbolNotFound {
        qualid: Box<QualifiedId>,
        absid: Box<AbsId>,
    },
    IdentifierNotFound {
        qualid: QualifiedId,
    },
    DuplicatedImportedName {
        name: String,
        imp1: Box<ImportDecl>,
        imp2: Box<ImportDecl>,
    },
    DuplicatedFnName {
        fid1: Box<Ident>,
        fid2: Box<Ident>,
    },
    DuplicatedTypeName {
        tid1: Box<Ident>,
        tid2: Box<Ident>,
    },
    DuplicatedVarName {
        vid1: Box<Ident>,
        vid2: Box<Ident>,
    },
    DuplicatedImplementationForType {
        typ: Typ,
        fid1: Box<Ident>,
        fid2: Box<Ident>,
    },
}

pub type RsvResult<T> = Result<T, ResolveError>;

#[derive(Debug)]
pub struct PkgSymMap {
    pub syms: HashMap<AbsId, ModSym>,
}

impl PkgSymMap {
    pub fn try_resolve_symbol(pkg: Pkg) -> RsvResult<Self> {
        let pctx = PkgLvlRslvCtx::new(&pkg)?;
        let mut syms = HashMap::new();

        // 同じ型に対する同じ名前の関連関数, メソッドの重複を検出する
        // ```hoge.biwa
        // impl Fuga {
        //   fn piyo() { ... }
        //
        //   fn piyo(self) { ... }
        // }
        // ```
        // のいずれも
        // hoge::Fuga::piyoという関数に解決されるため、重複検知が必要
        let mut typ_impls = HashMap::<Typ, HashMap<String, Ident>>::new();

        for (modpath, modu) in pkg.modules {
            let mctx = ModLvlRslvCtx::new(&pctx, modpath.clone(), &modu)?;

            for g in modu.globals {
                match g {
                    biwac_parser::Globals::Import(_) => {}
                    biwac_parser::Globals::FnDef(f) => {
                        let id = if let Some(self_typ) = &f.self_typ {
                            let typ = Typ::try_resolve_in_module(self_typ, &mctx)?;

                            if let Some(impls) = typ_impls.get_mut(&typ) {
                                match impls.entry(f.id.id.clone()) {
                                    Entry::Vacant(e) => {
                                        e.insert(f.id.clone());
                                    }
                                    Entry::Occupied(e) => {
                                        return Err(
                                            ResolveError::DuplicatedImplementationForType {
                                                typ,
                                                fid1: Box::new(e.get().clone()),
                                                fid2: Box::new(f.id.clone()),
                                            },
                                        );
                                    }
                                }
                            } else {
                                typ_impls
                                    .insert(typ.clone(), [(f.id.id.clone(), f.id.clone())].into());
                            }

                            AbsId::new_type_impl(&typ, f.id.id.clone())
                        } else {
                            AbsId::from_modpath(&modpath, f.id.id.clone())
                        };
                        syms.insert(
                            id,
                            ModSym::FnDef(FnDefContent::try_resolve_in_module(f, &mctx)?),
                        );
                    }
                    biwac_parser::Globals::TypeDef(t) => match t {
                        TypeDef::Struct(s) => {
                            let id = AbsId::from_modpath(&modpath, s.id.id.clone());
                            syms.insert(
                                id,
                                ModSym::TypeDef(TypeDefContent::Struct(
                                    StructDefContent::try_resolve_in_module(s, &mctx)?,
                                )),
                            );
                        }
                    },
                    biwac_parser::Globals::VarDecl(v) => {
                        let id = AbsId::from_modpath(&modpath, v.id.id.clone());
                        syms.insert(
                            id,
                            ModSym::VarDecl(GlobalVarDecl::try_resolve_in_module(v, &mctx)?),
                        );
                    }
                    biwac_parser::Globals::NativeFnDef(f) => {
                        let id = AbsId::from_modpath(&modpath, f.id.id.clone());
                        syms.insert(
                            id,
                            ModSym::NativeFnDef(NativeFnDefContent::try_resolve_in_module(
                                f, &mctx,
                            )?),
                        );
                    }
                    biwac_parser::Globals::MethodDef(m) => {
                        let typ = Typ::try_resolve_in_module(&m.self_typ, &mctx)?;

                        if let Some(impls) = typ_impls.get_mut(&typ) {
                            match impls.entry(m.id.id.clone()) {
                                Entry::Vacant(e) => {
                                    e.insert(m.id.clone());
                                }
                                Entry::Occupied(e) => {
                                    return Err(ResolveError::DuplicatedImplementationForType {
                                        typ,
                                        fid1: Box::new(e.get().clone()),
                                        fid2: Box::new(m.id.clone()),
                                    });
                                }
                            }
                        } else {
                            typ_impls.insert(typ.clone(), [(m.id.id.clone(), m.id.clone())].into());
                        }

                        let id = AbsId::new_type_impl(&typ, m.id.id.clone());

                        syms.insert(
                            id,
                            ModSym::MethodDef(MethodDefContent::try_resolve_in_module(m, &mctx)?),
                        );
                    }
                }
            }
        }

        Ok(Self { syms })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbsId {
    // pub package: enum Package { Internal, External(String)}
    pub quals: Vec<String>,
    pub id: String,
}

impl AbsId {
    pub fn new(quals: Vec<String>, id: String) -> Self {
        Self { quals, id }
    }

    pub(crate) fn from_modpath(modpath: &ModPath, id: String) -> Self {
        Self {
            quals: match modpath {
                ModPath::Main => vec![],
                ModPath::Lib => vec![],
                ModPath::Mod(m) => m.clone(),
            },
            id,
        }
    }

    pub(crate) fn new_type_impl(typ: &Typ, id: String) -> Self {
        let quals = match typ {
            Typ::Int => vec!["Int".to_string()],
            Typ::Float => vec!["Float".to_string()],
            Typ::Bool => vec!["Bool".to_string()],
            Typ::Defined(absid) => {
                let mut quals = absid.quals.clone();
                quals.push(absid.id.clone());

                quals
            }
            Typ::Fn(_) => {
                panic!("compiler bug: function type cannot be implemented related functions")
            }
        };

        Self { quals, id }
    }
}

trait TryResolve<T>: Sized {
    fn try_resolve<'mctx>(value: T, fctx: &mut FnLvlRslvCtx<'mctx>) -> RsvResult<Self>;
}

trait ModuleLevelTryResolve<T>: Sized {
    fn try_resolve_in_module<'pctx>(value: T, mctx: &ModLvlRslvCtx<'pctx>) -> RsvResult<Self>;
}

impl Display for AbsId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.quals.is_empty() {
            write!(f, "{}", &self.id)
        } else {
            write!(f, "{}::{}", self.quals.join("::"), &self.id)
        }
    }
}
