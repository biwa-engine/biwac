use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use biwac_base::{IdentInterner, InternedIdent, PackageId};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{
    AssocValDefKind, DefinedTyImpl, ExprId, Hir, Ident, InferTy, Ty, TyDefKind, TyKind, TyVar,
    ValDefKind,
};
use biwac_span::{TyDefId, ValDefId, VarId};

use crate::TyError;

pub struct TyCtx<'a> {
    pub(super) hir: Hir,
    pub(super) ext_pkgs: Vec<(PackageId, Arc<DepMetadata>)>,
    /// &'a mut IdentInterner を RefCell でラップして &self メソッドから変更可能にする。
    /// interner は borrow_mut() で短時間だけ借用し、返却後に解放する。
    pub(super) interner: RefCell<&'a mut IdentInterner>,
    /// fn シンボルの遅延キャッシュ (self package の assoc fn + 外部パッケージの fn)。
    /// Box<ValDefKind> によりヒープ番地が安定し、&ValDefKind を &self 寿命で返せる。
    pub(super) val_cache: RefCell<HashMap<ValDefId, Box<ValDefKind>>>,
    /// 外部パッケージの struct シンボル遅延キャッシュ。同上。
    pub(super) ext_ty_cache: RefCell<HashMap<TyDefId, Box<DefinedTyImpl>>>,
    /// assoc fn ValDefId → (struct TyDefId, method 名 InternedIdent) マップ。
    /// self package 分は TyCtx::new() で hir.tys を走査して一括登録。
    /// 外部パッケージ分は get_ty_impl で struct を遅延ロードした際に追加。
    /// get_value_definition から assoc fn を引く際に使用する。
    pub(super) assoc_val_map: RefCell<HashMap<ValDefId, (TyDefId, InternedIdent)>>,
}

impl<'a> TyCtx<'a> {
    pub fn new(
        hir: Hir,
        ext_pkgs: Vec<(PackageId, Arc<DepMetadata>)>,
        interner: &'a mut IdentInterner,
    ) -> Self {
        // self package の assoc fn を assoc_val_map に一括登録する。
        // hir.vals には top-level fn のみ存在するため、assoc fn は別途登録が必要。
        let mut assoc_val_map: HashMap<ValDefId, (TyDefId, InternedIdent)> = HashMap::new();
        for (ty_def_id, ty_impl) in &hir.tys {
            if !ty_def_id.pkg().is_self() {
                continue;
            }
            for (method_id, impl_list) in &ty_impl.vals {
                for val_def_id in impl_list.vals.keys() {
                    assoc_val_map.insert(*val_def_id, (*ty_def_id, *method_id));
                }
            }
        }

        Self {
            hir,
            ext_pkgs,
            interner: RefCell::new(interner),
            val_cache: RefCell::new(HashMap::new()),
            ext_ty_cache: RefCell::new(HashMap::new()),
            assoc_val_map: RefCell::new(assoc_val_map),
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
    /// 外部 struct をロードした際、assoc fn の ValDefId も assoc_val_map に登録する。
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

        // 外部 struct の assoc fn を assoc_val_map に登録する。
        // ty_impl を Box に移す前に登録する。
        {
            let mut assoc_map = self.assoc_val_map.borrow_mut();
            for (method_id, impl_list) in &ty_impl.vals {
                for val_def_id in impl_list.vals.keys() {
                    assoc_map
                        .entry(*val_def_id)
                        .or_insert((*def_id, *method_id));
                }
            }
        }

        let mut cache = self.ext_ty_cache.borrow_mut();
        let b = cache.entry(*def_id).or_insert_with(|| Box::new(ty_impl));
        // SAFETY: 同上
        Some(unsafe { &*(b.as_ref() as *const DefinedTyImpl) })
    }

    pub(super) fn get_type_definition(&self, def_id: &TyDefId) -> Option<&TyDefKind> {
        self.get_ty_impl(def_id).map(|di| &di.ty_content)
    }

    pub(super) fn get_value_definition(&self, def_id: &ValDefId) -> Option<&ValDefKind> {
        // self package の top-level fn は hir.vals から直接返す。
        if def_id.pkg().is_self()
            && let Some(v) = self.hir.vals.get(def_id)
        {
            return Some(v);
            // hir.vals に存在しない場合は assoc fn の可能性があるため以下の共通パスへ進む。
        }

        // val_cache 確認 (assoc fn のキャッシュを含む)
        {
            let cache = self.val_cache.borrow();
            if let Some(b) = cache.get(def_id) {
                // SAFETY: get_ty_impl のコメントと同じ理由で安全。
                return Some(unsafe { &*(b.as_ref() as *const ValDefKind) });
            }
        }

        // assoc fn マップ確認 (self package + 外部パッケージ共通)。
        // val_content から FnSignature を取り出して ValDefKind::ExternalFn に変換してキャッシュする。
        // self / external 共に署名が取れれば型推論上は等価であるため ExternalFn で統一する。
        {
            let assoc_map = self.assoc_val_map.borrow();
            if let Some(&(ty_def_id, method_id)) = assoc_map.get(def_id) {
                drop(assoc_map); // get_ty_impl が ext_ty_cache を borrow するため先に解放

                let ty_impl = self.get_ty_impl(&ty_def_id)?;
                let sig =
                    ty_impl
                        .vals
                        .get(&method_id)?
                        .vals
                        .get(def_id)
                        .map(|pair| match &pair.val_content {
                            AssocValDefKind::NativeFn(f) => Some(f.signature.clone()),
                            AssocValDefKind::Fn(f) => Some(f.signature.clone()),
                        })??;

                let mut cache = self.val_cache.borrow_mut();
                let b = cache
                    .entry(*def_id)
                    .or_insert_with(|| Box::new(ValDefKind::ExternalFn(Box::new(sig))));
                // SAFETY: 同上
                return Some(unsafe { &*(b.as_ref() as *const ValDefKind) });
            }
        }

        // self package でここまで来た場合は該当なし (assoc fn でも top-level fn でもない)
        if def_id.pkg().is_self() {
            return None;
        }

        // 外部パッケージの top-level fn パス
        let pkg_id = def_id.pkg();
        let dep = self.find_ext_dep(pkg_id)?;
        let val_kind = {
            let mut ig = self.interner.borrow_mut();
            dep.get_ext_val_kind(def_id.local_idx(), pkg_id, &mut ig)?
        };

        let mut cache = self.val_cache.borrow_mut();
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
            _ => {
                println!("{ty:?}");
                todo!()
            }
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
