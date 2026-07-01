use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use biwac_base::{IdentInterner, PackageId};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{
    DefinedTyImpl, ExprId, Hir, Ident, InferTy, Ty, TyDefKind, TyKind, TyVar, ValDefKind,
};
use biwac_span::{TyDefId, ValDefId, VarId};

use crate::TyError;

pub struct TyCtx<'a> {
    pub(super) hir: Hir,
    pub(super) ext_pkgs: Vec<(PackageId, Arc<DepMetadata>)>,
    /// &'a mut IdentInterner を RefCell でラップして &self メソッドから変更可能にする。
    /// interner は borrow_mut() で短時間だけ借用し、返却後に解放する。
    pub(super) interner: RefCell<&'a mut IdentInterner>,
    /// 外部パッケージの fn シンボル遅延キャッシュ。
    /// Box<ValDefKind> によりヒープ番地が安定し、&ValDefKind を &self 寿命で返せる。
    pub(super) ext_val_cache: RefCell<HashMap<ValDefId, Box<ValDefKind>>>,
    /// 外部パッケージの struct シンボル遅延キャッシュ。同上。
    pub(super) ext_ty_cache: RefCell<HashMap<TyDefId, Box<DefinedTyImpl>>>,
}

impl<'a> TyCtx<'a> {
    pub fn new(
        hir: Hir,
        ext_pkgs: Vec<(PackageId, Arc<DepMetadata>)>,
        interner: &'a mut IdentInterner,
    ) -> Self {
        Self {
            hir,
            ext_pkgs,
            interner: RefCell::new(interner),
            ext_val_cache: RefCell::new(HashMap::new()),
            ext_ty_cache: RefCell::new(HashMap::new()),
        }
    }

    /// 外部パッケージに対応する DepMetadata を PackageId で引く。
    fn find_ext_dep(&self, pkg_id: PackageId) -> Option<&Arc<DepMetadata>> {
        self.ext_pkgs
            .iter()
            .find(|(pid, _)| *pid == pkg_id)
            .map(|(_, d)| d)
    }

    /// DefinedTyImpl を返す内部ヘルパー。外部パッケージは遅延ロードしてキャッシュする。
    /// 返す参照のライフタイムは &self と同じ。
    fn get_ty_impl(&self, def_id: &TyDefId) -> Option<&DefinedTyImpl> {
        if def_id.pkg().is_self() {
            return self.hir.tys.get(def_id);
        }

        // キャッシュ確認
        {
            let cache = self.ext_ty_cache.borrow();
            if let Some(b) = cache.get(def_id) {
                // SAFETY: Box<DefinedTyImpl> はヒープ番地が安定している。
                // HashMap のリハッシュでも Box の中身は動かない。
                // &self が生きている限り Box も生きており、append-only なので削除はない。
                return Some(unsafe { &*(b.as_ref() as *const DefinedTyImpl) });
            }
        }

        // キャッシュミス: DepMetadata から遅延ロード
        let pkg_id = def_id.pkg();
        let dep = self.find_ext_dep(pkg_id)?;
        let ty_impl = {
            let mut ig = self.interner.borrow_mut();
            dep.get_ext_ty_impl(def_id.local_idx(), pkg_id, &mut ig)?
        };

        let mut cache = self.ext_ty_cache.borrow_mut();
        let b = cache.entry(*def_id).or_insert_with(|| Box::new(ty_impl));
        // SAFETY: 同上
        Some(unsafe { &*(b.as_ref() as *const DefinedTyImpl) })
    }

    pub(super) fn get_type_definition(&self, def_id: &TyDefId) -> Option<&TyDefKind> {
        self.get_ty_impl(def_id).map(|di| &di.ty_content)
    }

    pub(super) fn get_value_definition(&self, def_id: &ValDefId) -> Option<&ValDefKind> {
        if def_id.pkg().is_self() {
            return self.hir.vals.get(def_id);
        }

        // キャッシュ確認
        {
            let cache = self.ext_val_cache.borrow();
            if let Some(b) = cache.get(def_id) {
                // SAFETY: get_ty_impl のコメントと同じ理由で安全。
                return Some(unsafe { &*(b.as_ref() as *const ValDefKind) });
            }
        }

        // キャッシュミス: DepMetadata から遅延ロード
        let pkg_id = def_id.pkg();
        let dep = self.find_ext_dep(pkg_id)?;
        let val_kind = {
            let mut ig = self.interner.borrow_mut();
            dep.get_ext_val_kind(def_id.local_idx(), pkg_id, &mut ig)?
        };

        let mut cache = self.ext_val_cache.borrow_mut();
        let b = cache.entry(*def_id).or_insert_with(|| Box::new(val_kind));
        // SAFETY: 同上
        Some(unsafe { &*(b.as_ref() as *const ValDefKind) })
    }

    pub(super) fn get_method_def_id(&self, ty: &Ty, method: &Ident) -> Result<ValDefId, TyError> {
        match &ty.kind {
            TyKind::Defined(defined_ty) => {
                let assoc_list = self
                    .get_ty_impl(&defined_ty.def_id)
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

pub struct FnTyCtx<'tctx, 'a> {
    pub(super) tctx: &'tctx TyCtx<'a>,
    next_tv: usize,
    pub(super) substitutions: HashMap<TyVar, Ty>,
    pub(super) vars: HashMap<VarId, Ty>,
    pub(super) exprs: HashMap<ExprId, Ty>,
    pub(super) rty: Ty,
}

// ある関数に対して型推論をした結果得られる型情報
pub(super) struct TyInfo {
    pub(super) expr_tys: HashMap<ExprId, Ty>,
    pub(super) var_tys: HashMap<VarId, Ty>,
}

impl<'tctx, 'a> FnTyCtx<'tctx, 'a> {
    pub(super) fn new(tctx: &'tctx TyCtx<'a>, rty: Ty) -> Self {
        Self {
            tctx,
            next_tv: 0,
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
