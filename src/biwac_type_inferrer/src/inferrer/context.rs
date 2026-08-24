use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use biwac_base::{IdentInterner, InternedIdent, PackageId};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{
    AssocValDefKind, DefinedTyImpl, ExprId, Hir, Ident, InferTy, Ty, TyDefKind, TyKind, TyVar,
    ValDefKind,
};
use biwac_lang_item::{LangItem, LangItemKind, LangItemTable};
use biwac_span::{TyDefId, ValDefId, VarId};

use crate::{TyError, TyResult};

pub struct TyCtx<'a> {
    pub(super) hir: Hir,
    pub(super) ext_pkgs: Vec<(PackageId, Arc<DepMetadata>)>,
    /// lang item テーブル。
    /// 名前解決パスが構築したものをそのまま持つ。
    /// Hir の外に置くのは、これがプログラム本体ではなく
    /// パッケージ横断のメタ情報だからである。
    pub(super) lang_items: LangItemTable,
    /// &'a mut IdentInterner を RefCell でラップして &self メソッドから変更可能にする。
    /// interner は borrow_mut() で短時間だけ借用し、返却後に解放する。
    pub(super) interner: RefCell<&'a mut IdentInterner>,
    /// 外部パッケージの struct シンボル遅延キャッシュ。同上。
    pub(super) ext_ty_cache: RefCell<HashMap<TyDefId, Box<DefinedTyImpl>>>,
    /// 外部パッケージの assoc fn ValDefId -> (TyDefId, method 名 InternedIdent) マップ。
    /// self package 分は Hir 内にすでにある
    /// 外部パッケージ分は get_ty_impl で struct を遅延ロードした際に追加。
    /// get_value_definition から assoc fn を引く際に使用する。
    pub(super) ext_assoc_val_map: RefCell<HashMap<ValDefId, (TyDefId, InternedIdent)>>,
}

impl<'a> TyCtx<'a> {
    pub fn new(
        hir: Hir,
        lang_items: LangItemTable,
        ext_pkgs: Vec<(PackageId, Arc<DepMetadata>)>,
        interner: &'a mut IdentInterner,
    ) -> Self {
        Self {
            hir,
            ext_pkgs,
            lang_items,
            interner: RefCell::new(interner),
            ext_ty_cache: RefCell::new(HashMap::new()),
            ext_assoc_val_map: RefCell::new(HashMap::new()),
        }
    }

    /// 型の lang item の DefId を引く。
    ///
    /// rustc の `tcx.require_lang_item` に相当する。
    /// 種別 (型 / 関数) は回収時に検証済みなので、
    /// ここで取り違えることはない。
    pub(super) fn require_ty(&self, item: LangItem) -> Result<TyDefId, TyError> {
        debug_assert_eq!(item.kind(), LangItemKind::Ty);
        self.lang_items
            .get(&item)
            .map(TyDefId::new)
            .ok_or(TyError::MissingLangItem { item })
    }

    /// 値 (関数) の lang item の DefId を引く。
    pub(super) fn require_val(&self, item: LangItem) -> Result<ValDefId, TyError> {
        debug_assert_eq!(item.kind(), LangItemKind::Fn);
        self.lang_items
            .get(&item)
            .map(ValDefId::new)
            .ok_or(TyError::MissingLangItem { item })
    }

    /// lang item で指定された型を `Ty` として組み立てる。
    ///
    /// ジェネリクスを取らない lang item 型 (`string` など) 専用。
    pub(super) fn lang_item_ty(&self, item: LangItem, span: biwac_span::Span) -> TyResult<Ty> {
        Ok(Ty::new(
            TyKind::Defined(biwac_hir::DefinedTy {
                def_id: self.require_ty(item)?,
                genargs: Vec::new(),
            }),
            span,
        ))
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
        if def_id.pkg().is_self() || def_id.pkg() == PackageId::BUILTIN_RESERVED_PACKAGE {
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
            let mut assoc_map = self.ext_assoc_val_map.borrow_mut();
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
        self.get_ty_impl(def_id)
            .and_then(|di| di.ty_content.as_ref())
    }

    pub(super) fn get_value_ty(&self, def_id: &ValDefId) -> Option<Ty> {
        if def_id.pkg().is_self() {
            // self package の top-level fn は hir.vals から直接返す。
            if let Some(v) = self.hir.vals.get(def_id) {
                let fty = match v {
                    ValDefKind::Fn(fn_def) => fn_def.signature.as_ty(),
                    ValDefKind::Native(fn_def) => fn_def.signature.as_ty(),
                    ValDefKind::NovelScene(scene_def) => scene_def.signature.as_ty(),
                };
                Some(fty)
            } else {
                let (ty_def_id, assoc_name) = self.hir.assoc_val_map.get(def_id)?;

                match &self
                    .hir
                    .tys
                    .get(ty_def_id)
                    .unwrap()
                    .vals
                    .get(assoc_name)?
                    .vals
                    .get(def_id)
                    .unwrap()
                    .val_content
                {
                    AssocValDefKind::Fn(fn_def) => Some(fn_def.signature.as_ty()),
                    AssocValDefKind::NativeFn(fn_def) => Some(fn_def.signature.as_ty()),
                }
            }
        } else {
            // 外部パッケージの top-level fn パス
            let pkg_id = def_id.pkg();
            let dep = self.find_ext_dep(pkg_id)?;
            if let Some(sign) = {
                let mut ig = self.interner.borrow_mut();
                dep.get_ext_val_kind(def_id.local_idx(), pkg_id, &mut ig)
            } {
                Some(sign.as_ty())
            } else {
                // 外部パッケージ assoc fn マップ確認
                // val_content から FnSignature を取り出して ValDefKind::ExternalFn に変換してキャッシュする。
                let assoc_map = self.ext_assoc_val_map.borrow();
                let &(ty_def_id, method_id) = assoc_map.get(def_id)?;

                drop(assoc_map); // get_ty_impl が ext_ty_cache を borrow するため先に解放

                let ty_impl = self.get_ty_impl(&ty_def_id)?;
                ty_impl
                    .vals
                    .get(&method_id)?
                    .vals
                    .get(def_id)
                    .map(|pair| match &pair.val_content {
                        AssocValDefKind::NativeFn(f) => Some(f.signature.clone()),
                        AssocValDefKind::Fn(f) => Some(f.signature.clone()),
                    })?
                    .map(|sign| sign.as_ty())
            }
        }
    }

    pub(super) fn get_method_def_id(&self, ty: &Ty, method: &Ident) -> Result<ValDefId, TyError> {
        let not_found = || TyError::MethodNotFound {
            ty: Box::new(ty.clone()),
            method: Box::new(method.clone()),
        };

        let ty_def_id = ty.kind.def_id().ok_or_else(not_found)?;

        let ty_genargs: &[Ty] = match &ty.kind {
            TyKind::Defined(dt) => &dt.genargs,
            _ => &[],
        };

        let assoc_list = self
            .get_ty_impl(&ty_def_id)
            .and_then(|di| di.vals.get(&method.id))
            .ok_or_else(not_found)?;

        let mut matched = Vec::new();
        for (def_id, assoc) in &assoc_list.vals {
            if assoc.genargs.len() == ty_genargs.len()
                && assoc
                    .genargs
                    .iter()
                    .zip(ty_genargs.iter())
                    .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind))
            {
                matched.push(*def_id);
            }
        }

        match matched.as_slice() {
            [id] => Ok(*id),
            [] => Err(not_found()),
            _ => panic!("compiler bug: duplicated associated implementation registered"),
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
