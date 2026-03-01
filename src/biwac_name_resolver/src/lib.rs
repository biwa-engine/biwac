pub(crate) mod context;
mod symbols;
mod types;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, hash_map::Entry};

use biwac_base::ModPath;
use biwac_package_loader::Pkg;
use biwac_parser::{Ident, ImportDecl, QualifiedId, TypRepr, TypeDef};

use crate::context::{FnLvlRslvCtx, ImplLvlGenTypRslvCtx, ModLvlRslvCtx, PkgLvlRslvCtx};
pub use crate::{
    context::{AssocId, DecledVar, ExprId, LocVarId, ResolvedVar},
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
    types::{FnTyp, GenTypId, Typ},
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
    TypeNotFound {
        qualid: Box<QualifiedId>,
        typid: Box<TypId>,
    },
    FunctionNotFound {
        qualid: Box<QualifiedId>,
        fid: Box<FnId>,
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
    DuplicatedGenericTypeDeclaration {
        tid1: Box<Ident>,
        tid2: Box<Ident>,
    },
    GenericArgLengthMismatched {
        // TODO: エラーメッセージを正確に出しやすく
        typ: Box<Typ>,
        typ_impl_repr: Box<TypRepr>,
        actual_len: usize,
    },
    CanNotBeImplementedForType {
        typ: Typ,
    },
}

pub type RsvResult<T> = Result<T, ResolveError>;

// <----                           function id                                      ---->
// <----                               type id                                      ---->
// <package-name> :: <module-name>::<module-name>                         :: <identifier> :: [  ]
//
//
// <self-type>                                                            :: <identifier> :: [  ]
//
// <package-name> :: <module-name>::<module-name> :: <identifier> :: [  ]
// <----                      type id                       ---->
//
//

#[derive(Debug)]
pub struct ResolvedPkg {
    fns: HashMap<FnId, FnContent>,
    typs: HashMap<TypId, TypImpl>,
}

// グローバル変数のid
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VarId {
    // pub pkg: enum Package { Internal, External(String)}
    quals: Vec<String>,
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnId {
    // pub pkg: enum Package { Internal, External(String)}
    quals: Vec<String>,
    id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypId {
    // pub pkg: enum Package { Internal, External(String)}
    quals: Vec<String>,
    id: String,
}

#[derive(Debug, Clone)]
pub struct TypImpl {
    typ: TypeDefContent,
    methods: HashMap<String, TypMethodImpl>,
    assocs: HashMap<AssocId, TypAssocImpl>,
}

#[derive(Debug, Clone)]
pub struct TypAssocImpl {
    target_genargs: Vec<Typ>,
    f: FnDefContent,
    //  impl[T] Foo[T, Int] {
    //             ^^^^^^^^
    //             | target_genargs
    //      fn bar[U]() { ... }
    //  }
}

#[derive(Debug, Clone)]
pub struct TypMethodImpl {
    genargs_map: HashMap<Vec<Typ>, MethodDefContent>,
}

#[derive(Debug, Clone)]
pub enum FnContent {
    Fn(FnDefContent),
    Native(NativeFnDefContent),
}

//
// - 関数:
//  fnsからFnIdを引く。ジェネリクスの具体化表明は名前解決するものの、それ以上のチェックはせず、FnCall 側に記録
//
// - 関連関数:
//  パース時には(Selfについてのジェネリクス具体表明がない限り)関数呼び出しと区別がつかない。
//  if FnId.qualsをTypIdとしたときにその型がtypsに見つかる
//      && いずれか1つ以上のジェネリクス引数列に対して、FnId.idの実装があれば:
//          解決
//  else:
//      通常の関数呼び出しとして解決に移る
//
//  - メソッド:
//   名前解決の時点では特にされることはない。
//   pkgには記録され、次の型推論フェーズで解決される
//   型推論フェーズでは、
//   <expression> . <method-name> ( (<expression>,)* )
//   1. 左辺値の推論をする
//   2. if 左辺値が(ジェネリック型引数含め)具体の型である:
//          if typsを引くと存在する:
//              if ジェネリック型引数列で該当するものが一意に定まる:
//                  解決
//              else:
//                  エラー
//          else:
//              エラー
//      else:
//          エラー

// 重複エラーを検知するための一時的な型
#[derive(Debug, Clone)]
pub struct TmpTypImpl {
    typ: TypeDefContent,
    methods: HashMap<String, TmpTypMethodImpl>,
    assocs: HashMap<AssocId, TypAssocImpl>,
}

// 重複エラーを検知するための一時的な型
#[derive(Debug, Clone)]
struct TmpTypMethodImpl {
    genargs_map: HashMap<Vec<Typ>, (MethodDefContent, Ident)>,
}

impl ResolvedPkg {
    pub fn try_resolve(pkg: Pkg) -> RsvResult<Self> {
        let pctx = PkgLvlRslvCtx::new(&pkg)?;
        let mut fns = HashMap::new();
        let mut typs = HashMap::<TypId, TmpTypImpl>::new();

        // TODO:
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
        // let mut typ_impls = HashMap::<Typ, HashMap<String, Ident>>::new();

        // 予め型を記録する
        for (modpath, modu) in &pkg.modules {
            let mctx = ModLvlRslvCtx::new(&pctx, modpath.clone(), modu)?;

            for g in &modu.globals {
                if let biwac_parser::Globals::TypeDef(t) = g {
                    match t {
                        TypeDef::Struct(s) => {
                            let tid = TypId::from_modpath(modpath, s.id.id.clone());
                            typs.insert(
                                tid,
                                TmpTypImpl {
                                    typ: TypeDefContent::Struct(
                                        StructDefContent::try_resolve_in_module(s.clone(), &mctx)?,
                                    ),
                                    methods: HashMap::new(),
                                    assocs: HashMap::new(),
                                },
                            );
                        }
                    }
                }
            }
        }

        for (modpath, modu) in pkg.modules {
            let mctx = ModLvlRslvCtx::new(&pctx, modpath.clone(), &modu)?;

            for g in modu.globals {
                match g {
                    biwac_parser::Globals::Import(_) => {}
                    biwac_parser::Globals::FnDef(f) => {
                        // 関連関数の場合
                        if let Some(impl_ctx) = &f.impl_ctx {
                            let ictx = ImplLvlGenTypRslvCtx::new(Some(&impl_ctx.genargs), &mctx)?;

                            let self_typ = Typ::try_resolve_in_impl(&impl_ctx.self_typ, &ictx)?;
                            let self_typ_genargs = self_typ.get_genargs();
                            let typid = TypId::try_from(self_typ)?;

                            // SAFETY: try_resolve_in_impl ですでに存在は確認済み
                            let typ_impl =
                                typs.get_mut(&typid).expect("compiler bug: type not found");
                            // SAFETY: ctx 構成時に確認済み
                            let associd = pctx
                                .typs
                                .get(&typid)
                                .expect("compiler bug: type not found")
                                .assocs
                                .get(&f.id.id)
                                .expect("compiler bug: associated function not found")
                                .genargs_map
                                .get(&self_typ_genargs)
                                .expect("compiler bug: associated function for a generic argument not found")
                                .0;

                            typ_impl.assocs.insert(
                                associd,
                                TypAssocImpl {
                                    target_genargs: self_typ_genargs,
                                    f: FnDefContent::try_resolve_in_impl(f, &ictx)?,
                                },
                            );
                        } else {
                            // 一般の関数の場合
                            let fid = FnId::from_modpath(&modpath, f.id.id.clone());
                            let ictx = ImplLvlGenTypRslvCtx::new_empty(&mctx); // 空のimpl文脈を生成
                            fns.insert(
                                fid,
                                FnContent::Fn(FnDefContent::try_resolve_in_impl(f, &ictx)?),
                            );
                        };
                    }
                    biwac_parser::Globals::NativeFnDef(f) => {
                        let fid = FnId::from_modpath(&modpath, f.id.id.clone());
                        fns.insert(
                            fid,
                            FnContent::Native(NativeFnDefContent::try_resolve_in_module(f, &mctx)?),
                        );
                    }
                    biwac_parser::Globals::MethodDef(m) => {
                        let ictx = ImplLvlGenTypRslvCtx::new(Some(&m.impl_genargs), &mctx)?;

                        let self_typ = Typ::try_resolve_in_impl(&m.self_typ, &ictx)?;
                        let self_typ_genargs = self_typ.get_genargs();
                        let typid = TypId::try_from(self_typ.clone())?;

                        // SAFETY: try_resolve_in_impl ですでに存在は確認済み
                        let typ_impl = typs.get_mut(&typid).expect("compiler bug: type not found");

                        if let Some(typ_method_impl) = typ_impl.methods.get_mut(&m.id.id) {
                            match typ_method_impl.genargs_map.entry(self_typ_genargs) {
                                Entry::Vacant(e) => {
                                    let mid = m.id.clone();
                                    e.insert((
                                        MethodDefContent::try_resolve_in_impl(m, &ictx)?,
                                        mid,
                                    ));
                                }
                                Entry::Occupied(e) => {
                                    return Err(ResolveError::DuplicatedImplementationForType {
                                        typ: self_typ,
                                        fid1: Box::new(e.get().1.clone()),
                                        fid2: Box::new(m.id.clone()),
                                    });
                                }
                            }
                        } else {
                            let mid = m.id.clone();
                            typ_impl.methods.insert(
                                m.id.id.clone(),
                                TmpTypMethodImpl {
                                    genargs_map: [(
                                        self_typ_genargs,
                                        (MethodDefContent::try_resolve_in_impl(m, &ictx)?, mid),
                                    )]
                                    .into(),
                                },
                            );
                        }
                    }
                    biwac_parser::Globals::VarDecl(_) => {
                        todo!()
                        // let id = AbsId::from_modpath(&modpath, v.id.id.clone(), Some(&vec![]));
                        // syms.insert(
                        //     id,
                        //     ModSym::VarDecl(GlobalVarDecl::try_resolve_in_module(v, &mctx)?),
                        // );
                    }
                    biwac_parser::Globals::TypeDef(_) => {
                        // すでに記録済み
                    }
                }
            }
        }

        Ok(Self {
            fns,
            typs: typs
                .into_iter()
                .map(|(typid, tmp_typ_impl)| {
                    (
                        typid,
                        TypImpl {
                            typ: tmp_typ_impl.typ,
                            assocs: tmp_typ_impl.assocs,
                            methods: tmp_typ_impl
                                .methods
                                .into_iter()
                                .map(|(mid, tmp_typ_method_impl)| {
                                    (
                                        mid,
                                        TypMethodImpl {
                                            genargs_map: tmp_typ_method_impl
                                                .genargs_map
                                                .into_iter()
                                                .map(|(genargs, (method, _))| (genargs, method))
                                                .collect(),
                                        },
                                    )
                                })
                                .collect(),
                        },
                    )
                })
                .collect(),
        })
    }

    #[inline]
    pub fn get_fn(&self, fid: &FnId) -> Option<&FnContent> {
        self.fns.get(fid)
    }

    #[inline]
    pub fn get_typ(&self, tid: &TypId) -> Option<&TypeDefContent> {
        self.typs.get(tid).map(|timpl| &timpl.typ)
    }

    // メソッドをジェネリック型引数列からの選択をしたうえで取得する
    pub fn get_method(
        &self,
        tid: &TypId,
        id: &String,
        genargs: Option<&Vec<Typ>>,
    ) -> Option<&FnTyp> {
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

impl FnId {
    pub(crate) fn new(quals: Vec<String>, id: String) -> Self {
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
}

impl TypId {
    pub(crate) fn new(quals: Vec<String>, id: String) -> Self {
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
}

impl TryFrom<Typ> for TypId {
    type Error = ResolveError;

    fn try_from(value: Typ) -> Result<Self, Self::Error> {
        match value {
            Typ::Int => Ok(Self {
                quals: vec![],
                id: "Int".to_string(),
            }),
            Typ::Float => Ok(Self {
                quals: vec![],
                id: "Float".to_string(),
            }),
            Typ::Bool => Ok(Self {
                quals: vec![],
                id: "Bool".to_string(),
            }),
            Typ::Defined(deftyp) => Ok(deftyp.id.clone()),
            // NOTE: とりあえずジェネリック型はstruct GenTypId(usize)のusizeをそのまま文字列とした
            Typ::Gen(gid) => Ok(Self {
                quals: vec![],
                id: gid.value().to_string(),
            }),
            Typ::Fn(_) => Err(ResolveError::CanNotBeImplementedForType { typ: value }),
        }
    }
}

trait TryResolve<T>: Sized {
    fn try_resolve<'mctx>(value: T, fctx: &mut FnLvlRslvCtx<'mctx>) -> RsvResult<Self>;
}

trait ImplLevelTryResolve<T>: Sized {
    fn try_resolve_in_impl<'mctx>(value: T, ictx: &ImplLvlGenTypRslvCtx<'mctx>) -> RsvResult<Self>;
}

trait ModuleLevelTryResolve<T>: Sized {
    fn try_resolve_in_module<'pctx>(value: T, mctx: &ModLvlRslvCtx<'pctx>) -> RsvResult<Self>;
}
