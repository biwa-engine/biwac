use std::collections::HashMap;

use biwac_hir::{ExprId, Hir, InferTy, LocVarId, Ty, TyVar};

#[derive(Debug, Clone)]
pub struct TyCtx {
    pub(super) hir: Hir,
}

#[derive(Debug, Clone)]
pub struct FnTyCtx<'tctx> {
    pub(super) tctx: &'tctx TyCtx,
    next_tv: usize,
    // pub(super) schemes: HashMap<TyVar, Scheme>,
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

// ある関数に対して型推論をした結果得られる型情報
pub(super) struct TyInfo {
    pub(super) expr_tys: HashMap<ExprId, Ty>,
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
    pub(crate) fn fresh(&mut self) -> Ty {
        Ty::Infer(InferTy::Var(self.new_ty_var()))
    }

    pub(crate) fn ty_var_of_infer_ty(&mut self, i: InferTy) -> TyVar {
        match i {
            InferTy::Var(v) => v,
            InferTy::Unknown => self.new_ty_var(),
        }
    }
}
