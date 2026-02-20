use std::collections::{HashMap, hash_map::Entry};

use biwac_name_resolver::{
    AbsId, ExprId, FnDefContent, FnTyp, LocVarId, MethodDefContent, ModSym, NativeFnDefContent,
    PkgSymMap, StructDefContent, Typ, TypeDefContent,
};
use biwac_parser::Ident;

use crate::{FnTy, Scheme, Ty, TyError, TyResult, TyVar, inferrer::types::StructTy};

#[derive(Debug, Clone)]
pub(crate) struct PkgTyCtx {
    pub(super) syms: HashMap<AbsId, SymTy>,
    pub(super) method_impls: HashMap<Ty, HashMap<String, FnTy>>,
}

#[derive(Debug, Clone)]
pub struct TyCtx<'pctx> {
    pub(super) pctx: &'pctx PkgTyCtx,
    next_tv: usize,
    pub(super) schemes: HashMap<TyVar, Scheme>,
    pub(super) substitutions: HashMap<TyVar, Ty>,
    pub(super) vars: HashMap<LocVarId, Ty>,
    pub(super) exprs: HashMap<ExprId, Ty>,
    // 変数から型のマップ、
    // 式から型のマップがほしい
    // その式や変数を推論した時点で一意な型が決定不能なら
    // Ty::Varが付き、
    // それ以降の推論でsubstitutionsが付くだろう
    // もし、推論終了時にvarsやexprsのTy::Varをsubstitutionsに発見できなければ
    // 文脈不足である
    //
    // schemesは一意に型が確定するよりも前の段階で記録されている場所
    // として扱うと上手く行きそう
    pub(super) rty: Ty,
}

#[derive(Debug, Clone)]
pub(crate) enum SymTy {
    Struct(StructTy),
    Fn(FnTy),
}

impl PkgTyCtx {
    pub(crate) fn new(pkg: &PkgSymMap) -> TyResult<Self> {
        TyCtxBuilder::new(pkg).build()
    }
}

impl<'pctx> TyCtx<'pctx> {
    pub fn new(pctx: &'pctx PkgTyCtx, rty: Ty) -> Self {
        Self {
            pctx,
            next_tv: 0,
            schemes: HashMap::new(),
            substitutions: HashMap::new(),
            vars: HashMap::new(),
            exprs: HashMap::new(),
            rty,
        }
    }

    fn new_ty_var(&mut self) -> TyVar {
        let tv = TyVar(self.next_tv);
        self.next_tv += 1;

        tv
    }

    pub(crate) fn fresh(&mut self) -> Ty {
        Ty::Var(self.new_ty_var())
    }
}

struct TyCtxBuilder<'ast> {
    pkg: &'ast PkgSymMap,
    syms: HashMap<AbsId, SymTy>,
    method_impls: HashMap<Ty, HashMap<String, (FnTy, Ident)>>, // Identを保持して重複実装発生時にエラー出力する
}

impl<'ast> TyCtxBuilder<'ast> {
    fn new(pkg: &'ast PkgSymMap) -> Self {
        Self {
            pkg,
            syms: HashMap::new(),
            method_impls: HashMap::new(),
        }
    }

    fn build(mut self) -> TyResult<PkgTyCtx> {
        for (id, sym) in &self.pkg.syms {
            match sym {
                ModSym::TypeDef(typ) => match typ {
                    TypeDefContent::Struct(styp) => {
                        self.build_and_store_struct_ty(id, styp)?;
                    }
                },
                ModSym::FnDef(f) => {
                    self.build_and_store_fn(id, f)?;
                }
                ModSym::NativeFnDef(f) => {
                    self.build_and_store_native_fn(id, f)?;
                }
                ModSym::VarDecl(_) => {
                    // 型を記録
                    // グローバル変数は型アノテーションを必須とするくらいの制約を設けたほうが良い
                    todo!()
                }
                ModSym::MethodDef(m) => {
                    // method_implsに記録
                    let ty = Ty::from(
                        m.vars
                            .get(&m.self_id)
                            .unwrap()
                            .typ
                            .as_ref()
                            .unwrap()
                            .clone(),
                    );

                    let fty = self.build_method(m)?;

                    if let Some(methods) = self.method_impls.get_mut(&ty) {
                        match methods.entry(m.ident.id.clone()) {
                            Entry::Vacant(e) => {
                                e.insert((fty, m.ident.clone()));
                            }
                            Entry::Occupied(e) => {
                                return Err(TyError::MethodConfliced {
                                    ty,
                                    method1: Box::new(e.get().1.clone()),
                                    method2: Box::new(m.ident.clone()),
                                });
                            }
                        }
                    } else {
                        self.method_impls
                            .insert(ty, [(m.ident.id.clone(), (fty, m.ident.clone()))].into());
                    }
                }
            }
        }

        Ok(PkgTyCtx {
            syms: self.syms,
            method_impls: self
                .method_impls
                .into_iter()
                .map(|(ty, methods)| {
                    (
                        ty,
                        methods.into_iter().map(|(m, (fty, _))| (m, fty)).collect(),
                    )
                })
                .collect(),
        })
    }

    fn build_ty(&mut self, typ: &Typ) -> TyResult<Ty> {
        match typ {
            Typ::Int => Ok(Ty::Int),
            Typ::Float => Ok(Ty::Float),
            Typ::Bool => Ok(Ty::Bool),
            Typ::Fn(f) => Ok(Ty::Fn(self.build_fn_ty(f)?)),
            Typ::Defined(id) => {
                let sym = self.pkg.syms.get(id).unwrap(); // 名前解決はしてあるので、存在はする

                match sym {
                    ModSym::FnDef(_) => Err(TyError::SymbolNotAType { id: id.clone() }),
                    ModSym::NativeFnDef(_) => Err(TyError::SymbolNotAType { id: id.clone() }),
                    ModSym::VarDecl(_) => Err(TyError::SymbolNotAType { id: id.clone() }),
                    ModSym::TypeDef(t) => match t {
                        TypeDefContent::Struct(_) => Ok(Ty::Struct(id.clone())),
                    },
                    ModSym::MethodDef(_) => Err(TyError::SymbolNotAType { id: id.clone() }),
                }
            }
        }
    }

    fn build_fn_ty(&mut self, f: &FnTyp) -> TyResult<FnTy> {
        Ok(FnTy {
            args: f
                .args
                .iter()
                .map(|a| self.build_ty(a))
                .collect::<TyResult<_>>()?,
            ret: Box::new(self.build_ty(&f.ret)?),
        })
    }

    pub(crate) fn build_and_store_fn(&mut self, id: &AbsId, f: &FnDefContent) -> TyResult<()> {
        let fty = FnTy {
            args: f
                .args
                .iter()
                .map(|a| self.build_ty(f.vars.get(&a.id).as_ref().unwrap().typ.as_ref().unwrap()))
                .collect::<TyResult<_>>()?,
            ret: match &f.rtype {
                Some(typ) => Box::new(self.build_ty(typ)?),
                None => Box::new(Ty::Void),
            },
        };

        self.syms.insert(id.clone(), SymTy::Fn(fty));

        Ok(())
    }

    pub(crate) fn build_and_store_native_fn(
        &mut self,
        id: &AbsId,
        f: &NativeFnDefContent,
    ) -> TyResult<()> {
        let fty = FnTy {
            args: f.args.iter().map(|a| Ty::from(a.typ.clone())).collect(),
            ret: match &f.rtype {
                Some(typ) => Box::new(self.build_ty(typ)?),
                None => Box::new(Ty::Void),
            },
        };

        self.syms.insert(id.clone(), SymTy::Fn(fty));

        Ok(())
    }

    // メソッドの第一引数selfは記録されない。
    // selfの型をキーとしてFnTyを取得するため、必要ない
    pub(crate) fn build_method(&mut self, m: &MethodDefContent) -> TyResult<FnTy> {
        let fty = FnTy {
            args: m
                .args
                .iter()
                .map(|a| self.build_ty(m.vars.get(&a.id).as_ref().unwrap().typ.as_ref().unwrap()))
                .collect::<TyResult<_>>()?,
            ret: match &m.rtype {
                Some(typ) => Box::new(self.build_ty(typ)?),
                None => Box::new(Ty::Void),
            },
        };

        Ok(fty)
    }

    pub(crate) fn build_and_store_struct_ty(
        &mut self,
        id: &AbsId,
        styp: &StructDefContent,
    ) -> TyResult<()> {
        let mut members = HashMap::<&str, (&Ident, Ty)>::new();
        for (m, ty) in &styp.members {
            match members.entry(&m.id) {
                Entry::Vacant(e) => {
                    e.insert((m, self.build_ty(ty)?));
                }
                Entry::Occupied(e) => {
                    return Err(TyError::StructMemberConfliced {
                        id: id.clone(),
                        member2: Box::new(e.get().0.clone()),
                        member1: Box::new(m.clone()),
                    });
                }
            }
        }

        self.syms.insert(
            id.clone(),
            SymTy::Struct(StructTy {
                members: members
                    .into_iter()
                    .map(|(id, (_, ty))| (id.to_string(), ty))
                    .collect(),
                vars: vec![],
            }),
        );

        Ok(())
    }

    // pub(crate) fn build_and_store_method(
    //     &mut self,
    //     typ: Typ,
    //     method: MethodName,
    //     ftyp: FnType,
    // ) -> TyResult<()> {
    //     // TODO:
    //     let ty = self.build_ty(typ)?;
    //     let fty = self.build_fn_ty(ftyp)?;
    //     if let Some(methods) = self.method_impls.get_mut(&ty) {
    //         match methods.entry(method.clone()) {
    //             Entry::Occupied(_) => Err(TyError::MethodConfliced(ty, method)),
    //             Entry::Vacant(e) => {
    //                 e.insert(fty);
    //                 Ok(())
    //             }
    //         }
    //     } else {
    //         self.method_impls.insert(ty, [(method, fty)].into());
    //         Ok(())
    //     }
    // }
}
