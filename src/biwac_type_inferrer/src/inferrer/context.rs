use std::collections::HashMap;

use biwac_hir::{ExprId, Hir, Ident, InferTy, Ty, TyDefKind, TyKind, TyVar, ValDefKind};
use biwac_span::{TyDefId, ValDefId, VarId};

use crate::TyError;

#[derive(Debug, Clone)]
pub struct TyCtx {
    pub(super) hir: Hir,
}

impl TyCtx {
    pub(super) fn get_type_definition(&self, def_id: &TyDefId) -> Option<&TyDefKind> {
        self.hir.tys.get(def_id).map(|ty_impl| &ty_impl.ty_content)
    }

    pub(super) fn get_value_definition(&self, def_id: &ValDefId) -> Option<&ValDefKind> {
        self.hir.vals.get(def_id)
    }

    pub(super) fn get_method_def_id(&self, ty: &Ty, method: &Ident) -> Result<ValDefId, TyError> {
        match &ty.kind {
            TyKind::Defined(defined_ty) => {
                let assoc_list = self
                    .hir
                    .tys
                    .get(&defined_ty.def_id)
                    .unwrap()
                    .vals
                    .get(&method.id)
                    .ok_or(TyError::MethodNotFound {
                        ty: Box::new(ty.clone()),
                        method: Box::new(method.clone()),
                    })?;

                let mut matched = Vec::new();
                for (def_id, assoc) in &assoc_list.vals {
                    if assoc.genargs.len() == defined_ty.genargs.len()
                        && assoc
                            .genargs
                            .iter()
                            .zip(defined_ty.genargs.iter())
                            .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind))
                    {
                        matched.push(*def_id);
                    }
                }

                if matched.len() == 1 {
                    Ok(matched[0])
                } else if matched.is_empty() {
                    Err(TyError::MethodNotFound {
                        ty: Box::new(ty.clone()),
                        method: Box::new(method.clone()),
                    })
                } else {
                    panic!("compiler bug: duplicated associated implementation registered")
                }
            }
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FnTyCtx<'tctx> {
    pub(super) tctx: &'tctx TyCtx,
    next_tv: usize,
    // pub(super) schemes: HashMap<TyVar, Scheme>,
    pub(super) substitutions: HashMap<TyVar, Ty>,
    pub(super) vars: HashMap<VarId, Ty>,
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

// ある関数に対して型推論をした結果得られる型情報
pub(super) struct TyInfo {
    pub(super) expr_tys: HashMap<ExprId, Ty>,
    pub(super) var_tys: HashMap<VarId, Ty>,
}

impl TyCtx {
    pub fn new(hir: Hir) -> Self {
        Self { hir }
    }
}

impl<'tctx> FnTyCtx<'tctx> {
    pub(super) fn new(tctx: &'tctx TyCtx, rty: Ty) -> Self {
        Self {
            tctx,
            next_tv: 0,
            // schemes: HashMap::new(),
            substitutions: HashMap::new(),
            vars: HashMap::new(),
            exprs: HashMap::new(),
            rty,
        }
    }

    fn new_ty_var(&mut self) -> TyVar {
        let tv = TyVar::new(self.next_tv);
        self.next_tv += 1;

        tv
    }

    #[inline]
    pub(crate) fn fresh(&mut self) -> TyKind {
        TyKind::Infer(InferTy::Var(self.new_ty_var()))
    }

    pub(crate) fn ty_var_of_infer_ty(&mut self, i: InferTy) -> TyVar {
        match i {
            InferTy::Var(v) => v,
            InferTy::Unknown => self.new_ty_var(),
        }
    }
}
