use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use biwac_base::{IdentInterner, InternedIdent, ModId, PackageId};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{
    AssocValDefKind, DefinedTyImpl, EnumDef, ExprId, FnSignature, Hir, Ident, InferTy, Ty,
    TyDefKind, TyKind, TyTraitImpl, TyVar, ValDefKind, VariantDef, VariantOwner,
};
use biwac_lang_item::{LangItem, LangItemKind, LangItemTable};
use biwac_span::{LocalGenDefId, TraitDefId, TyDefId, ValDefId, VarId, VariantDefId};
use biwac_trait_solver::{Solved, TraitEnv, TraitSolveError};

use crate::{TyError, TyErrorReport, TyNames, TyResult};

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
    /// 外部パッケージの trait 宣言の遅延キャッシュ。同上。
    pub(super) ext_trait_cache: RefCell<HashMap<TraitDefId, Box<biwac_hir::TraitDef>>>,
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
            ext_trait_cache: RefCell::new(HashMap::new()),
        }
    }

    /// エラー表示に要る名前を引いて表にする。
    ///
    /// 型推論の外では [`TyDefId`] から名前を辿る手段が無いので、
    /// ここで解決して [`TyErrorReport`] に持たせる。
    /// エラー時にしか通らないので、素朴に全部歩いてよい。
    pub(crate) fn report(&self, error: TyError) -> TyErrorReport {
        let mut tys = HashMap::new();
        for ty in crate::error_tys(&error) {
            self.collect_ty_names(&ty.kind, &mut tys);
        }
        for def_id in crate::error_ty_def_ids(&error) {
            self.insert_ty_name(def_id, &mut tys);
        }

        TyErrorReport::new(
            error,
            TyNames {
                tys,
                gens: self.collect_gen_names(),
            },
        )
    }

    fn insert_ty_name(&self, def_id: TyDefId, out: &mut HashMap<TyDefId, String>) {
        if out.contains_key(&def_id) {
            return;
        }

        // 自パッケージも外部パッケージも `get_ty_impl` で引ける
        // (外部は `.biwameta` から遅延ロードされる)。
        let Some(ty_impl) = self.get_ty_impl(&def_id) else {
            return;
        };
        let Some(content) = &ty_impl.ty_content else {
            return;
        };

        let ident = match content {
            TyDefKind::Struct(struct_def) => &struct_def.name,
            TyDefKind::Enum(enum_def) => &enum_def.name,
            TyDefKind::NativeTypeAlias(alias_def) => &alias_def.name,
        };

        if let Some(name) = self.interner.borrow().get_str(&ident.id) {
            out.insert(def_id, name.to_string());
        }
    }

    fn collect_ty_names(&self, kind: &TyKind, out: &mut HashMap<TyDefId, String>) {
        match kind {
            TyKind::Defined(defined_ty) => {
                self.insert_ty_name(defined_ty.def_id, out);
                for g in &defined_ty.genargs {
                    self.collect_ty_names(&g.kind, out);
                }
            }
            TyKind::Fn(fty) => {
                for a in &fty.args {
                    self.collect_ty_names(&a.kind, out);
                }
                self.collect_ty_names(&fty.rty.kind, out);
            }
            _ => {}
        }
    }

    /// 自パッケージで宣言されたジェネリック引数の名前を集める。
    ///
    /// 外部パッケージのものは `.biwameta` が名前を持たないので入らない。
    /// その場合は `_` として表示される。
    fn collect_gen_names(&self) -> HashMap<u64, String> {
        let mut out = HashMap::new();
        let interner = self.interner.borrow();

        let push_signature = |sig: &FnSignature, out: &mut HashMap<u64, String>| {
            for g in &sig.genargs {
                if let Some(name) = interner.get_str(&g.name.id) {
                    out.insert(g.def_id.value(), name.to_string());
                }
            }
        };

        for val in self.hir.vals.values() {
            match val {
                ValDefKind::Fn(f) => push_signature(&f.signature, &mut out),
                ValDefKind::Native(f) => push_signature(&f.signature, &mut out),
                ValDefKind::NovelScene(s) => push_signature(&s.signature, &mut out),
            }
        }

        for ty_impl in self.hir.tys.values() {
            for impl_list in ty_impl.vals.values() {
                for pair in impl_list.vals.values() {
                    for (name, (def_id, _)) in &pair.impl_block_genargs {
                        if let Some(name) = interner.get_str(name) {
                            out.insert(def_id.value(), name.to_string());
                        }
                    }
                    match &pair.val_content {
                        AssocValDefKind::Fn(f) => push_signature(&f.signature, &mut out),
                        AssocValDefKind::NativeFn(f) => push_signature(&f.signature, &mut out),
                    }
                }
            }
        }

        out
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

    /// バリアントから親の enum と宣言を引く。
    ///
    /// 自パッケージ分は `Hir::variant_owners` にある。
    /// 外部パッケージ分は `.biwameta` から遅延で引く。
    pub(super) fn get_variant(&self, def_id: &VariantDefId) -> Option<(VariantOwner, VariantDef)> {
        let owner = match self.hir.variant_owners.get(def_id) {
            Some(owner) => *owner,
            None => {
                // 外部パッケージ。バリアントもシンボルとして載っているので、
                // その本体から親 enum のシンボル番号と宣言順の添字を引く。
                let pkg_id = def_id.pkg();
                let dep = self.find_ext_dep(pkg_id)?;
                let (enum_sym_idx, index) = dep.variant_owner(def_id.local_idx())?;

                VariantOwner {
                    enum_def_id: TyDefId::new(biwac_span::DefId::new(
                        pkg_id,
                        biwac_span::PackageLocalDefId::new(enum_sym_idx),
                    )),
                    index,
                }
            }
        };

        let TyDefKind::Enum(enum_def) = self.get_type_definition(&owner.enum_def_id)? else {
            return None;
        };

        Some((owner, enum_def.variants.get(owner.index as usize)?.clone()))
    }

    /// trait の宣言を引く。外部パッケージは遅延ロードしてキャッシュする。
    pub(super) fn get_trait_def(&self, def_id: &TraitDefId) -> Option<&biwac_hir::TraitDef> {
        if def_id.pkg().is_self() {
            return self.hir.traits.get(def_id);
        }

        {
            let cache = self.ext_trait_cache.borrow();
            if let Some(b) = cache.get(def_id) {
                // SAFETY: `ext_ty_cache` と同じ理由。
                // Box はヒープ番地が安定しており、append-only で削除が無い。
                return Some(unsafe { &*(b.as_ref() as *const biwac_hir::TraitDef) });
            }
        }

        let pkg_id = def_id.pkg();
        let dep = self.find_ext_dep(pkg_id)?;
        let trait_def = {
            let mut ig = self.interner.borrow_mut();
            dep.get_ext_trait_def(def_id.local_idx(), pkg_id, &mut ig)?
        };

        let mut cache = self.ext_trait_cache.borrow_mut();
        let b = cache.entry(*def_id).or_insert_with(|| Box::new(trait_def));
        // SAFETY: 同上
        Some(unsafe { &*(b.as_ref() as *const biwac_hir::TraitDef) })
    }

    /// trait の項目の宣言を引く。
    pub(super) fn get_trait_item(
        &self,
        assoc: &biwac_span::TraitAssocDefId,
    ) -> Option<(&biwac_hir::TraitDef, &biwac_hir::TraitItemDef)> {
        let owner = match self.hir.trait_assoc_owners.get(assoc) {
            Some(owner) => *owner,
            None => {
                // 外部パッケージ。項目のシンボルから親 trait を引く。
                let dep = self.find_ext_dep(assoc.pkg())?;
                let (trait_sym, index) = dep.trait_assoc_owner(assoc.local_idx())?;
                biwac_hir::TraitAssocOwner {
                    trait_def_id: TraitDefId::new(biwac_span::DefId::new(
                        assoc.pkg(),
                        biwac_span::PackageLocalDefId::new(trait_sym),
                    )),
                    index,
                }
            }
        };

        let trait_def = self.get_trait_def(&owner.trait_def_id)?;
        let item = trait_def.items.get(owner.index as usize)?;
        Some((trait_def, item))
    }

    /// enum の定義を引く。
    pub(super) fn get_enum_definition(&self, def_id: &TyDefId) -> Option<&EnumDef> {
        match self.get_type_definition(def_id)? {
            TyDefKind::Enum(enum_def) => Some(enum_def),
            _ => None,
        }
    }

    pub(super) fn get_value_ty(&self, def_id: &ValDefId) -> Option<Ty> {
        self.get_value_signature(def_id).map(|s| s.as_ty())
    }

    /// シンボルのシグニチャ。
    ///
    /// [`Self::get_value_ty`] が返す [`biwac_hir::FnTy`] は `self` を落としてしまうので、
    /// レシーバまで含めて見たい場合はこちらを使う。
    pub(super) fn get_value_signature(&self, def_id: &ValDefId) -> Option<FnSignature> {
        if def_id.pkg().is_self() {
            // self package の top-level fn は hir.vals から直接返す。
            if let Some(v) = self.hir.vals.get(def_id) {
                let sign = match v {
                    ValDefKind::Fn(fn_def) => fn_def.signature.clone(),
                    ValDefKind::Native(fn_def) => fn_def.signature.clone(),
                    ValDefKind::NovelScene(scene_def) => scene_def.signature.clone(),
                };
                Some(sign)
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
                    AssocValDefKind::Fn(fn_def) => Some(fn_def.signature.clone()),
                    AssocValDefKind::NativeFn(fn_def) => Some(fn_def.signature.clone()),
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
                Some(sign)
            } else {
                // 外部パッケージ assoc fn マップ確認
                // val_content から FnSignature を取り出して ValDefKind::ExternalFn に変換してキャッシュする。
                let assoc_map = self.ext_assoc_val_map.borrow();
                let &(ty_def_id, method_id) = assoc_map.get(def_id)?;

                drop(assoc_map); // get_ty_impl が ext_ty_cache を borrow するため先に解放

                let ty_impl = self.get_ty_impl(&ty_def_id)?;
                ty_impl.vals.get(&method_id)?.vals.get(def_id).map(|pair| {
                    match &pair.val_content {
                        AssocValDefKind::NativeFn(f) => Some(f.signature.clone()),
                        AssocValDefKind::Fn(f) => Some(f.signature.clone()),
                    }
                })?
            }
        }
    }

    /// このモジュールで使える trait。
    ///
    /// 自パッケージのモジュールだけが対象である
    /// (外部パッケージのコードは既に解決済みなので推論に来ない)。
    fn traits_in_scope(&self, module: ModId) -> Vec<TraitDefId> {
        self.hir
            .trait_scopes
            .get(&module)
            .cloned()
            .unwrap_or_default()
    }

    /// 1 つのモジュールに閉じた [`TraitEnv`] を作る。
    pub(super) fn trait_env(
        &self,
        module: ModId,
        bounds: HashMap<LocalGenDefId, Vec<biwac_hir::TraitCond>>,
    ) -> ModuleTraitEnv<'_, 'a> {
        ModuleTraitEnv {
            tctx: self,
            in_scope: self.traits_in_scope(module),
            bounds,
        }
    }

    /// メソッドの解決。
    ///
    /// まず直接の impl を探し、見つからなかったときに初めて trait を探す。
    /// `module` は呼び出し元の関数が置かれているモジュールで、
    /// どの trait がスコープにあるかを決める。
    #[allow(clippy::type_complexity)]
    pub(super) fn get_method_target(
        &self,
        ty: &Ty,
        method: &Ident,
        module: ModId,
        bounds: &HashMap<LocalGenDefId, Vec<biwac_hir::TraitCond>>,
    ) -> Result<biwac_hir::MethodTarget, TyError> {
        let not_found = || TyError::MethodNotFound {
            ty: Box::new(ty.clone()),
            method: Box::new(method.clone()),
        };

        // ジェネリック引数には直接の impl が無い。制限から探す。
        if matches!(ty.kind, TyKind::LocGen(_) | TyKind::Gen(_)) {
            return self.solve_method_by_trait(ty, method, module, bounds);
        }

        let ty_def_id = ty.kind.def_id().ok_or_else(not_found)?;

        let ty_genargs: &[Ty] = match &ty.kind {
            TyKind::Defined(dt) => &dt.genargs,
            _ => &[],
        };

        let mut matched = Vec::new();
        if let Some(assoc_list) = self
            .get_ty_impl(&ty_def_id)
            .and_then(|di| di.vals.get(&method.id))
        {
            for (def_id, assoc) in &assoc_list.vals {
                // trait impl の項目は直接は見えない。
                // スコープにある trait を経由してしか引けない。
                if assoc.trait_of.is_some() {
                    continue;
                }

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
        }

        match matched.as_slice() {
            [id] => Ok(biwac_hir::MethodTarget::Direct(*id)),
            [] => self.solve_method_by_trait(ty, method, module, bounds),
            _ => panic!("compiler bug: duplicated associated implementation registered"),
        }
    }

    fn solve_method_by_trait(
        &self,
        ty: &Ty,
        method: &Ident,
        module: ModId,
        bounds: &HashMap<LocalGenDefId, Vec<biwac_hir::TraitCond>>,
    ) -> Result<biwac_hir::MethodTarget, TyError> {
        let env = self.trait_env(module, bounds.clone());
        match biwac_trait_solver::solve_method(&ty.kind, method.id, &env) {
            Ok(Solved::Impl(def_id)) => Ok(biwac_hir::MethodTarget::Direct(def_id)),
            Ok(Solved::Deferred { assoc, .. }) => Ok(biwac_hir::MethodTarget::Trait(assoc)),
            Err(TraitSolveError::NotInScope { .. }) => Err(TyError::MethodNotInScope {
                ty: Box::new(ty.clone()),
                method: Box::new(method.clone()),
            }),
            Err(TraitSolveError::Ambiguous { .. }) => Err(TyError::AmbiguousMethod {
                ty: Box::new(ty.clone()),
                method: Box::new(method.clone()),
            }),
            Err(TraitSolveError::Unresolved) => Err(TyError::InsufficientContext),
            Err(_) => Err(TyError::MethodNotFound {
                ty: Box::new(ty.clone()),
                method: Box::new(method.clone()),
            }),
        }
    }
}

/// 1 つのモジュールに閉じた [`TraitEnv`]。
///
/// `trait_impls_of` は外部パッケージの型も引ける
/// (`get_ty_impl` が `.biwameta` から遅延ロードする)。
/// 外部パッケージが宣言した trait impl も、依存すべてを走査して拾う。
pub(super) struct ModuleTraitEnv<'tctx, 'a> {
    tctx: &'tctx TyCtx<'a>,
    in_scope: Vec<TraitDefId>,
    /// いま推論している関数から見えるジェネリック引数の制限。
    bounds: HashMap<LocalGenDefId, Vec<biwac_hir::TraitCond>>,
}

impl ModuleTraitEnv<'_, '_> {
    /// 型の種類から trait impl を引く。プリミティブ型も扱う。
    pub(super) fn trait_impls_of_kind(&self, kind: &TyKind) -> Vec<TyTraitImpl> {
        let def_id = match kind {
            TyKind::Defined(dt) => dt.def_id,
            TyKind::Int => TyDefId::INT_TY_DEF_ID,
            TyKind::Float => TyDefId::FLOAT_TY_DEF_ID,
            TyKind::Bool => TyDefId::BOOL_TY_DEF_ID,
            TyKind::Void => TyDefId::VOID_TY_DEF_ID,
            _ => return Vec::new(),
        };
        self.trait_impls_of(def_id)
    }
}

impl TraitEnv for ModuleTraitEnv<'_, '_> {
    fn trait_impls_of(&self, ty: TyDefId) -> Vec<TyTraitImpl> {
        // 自パッケージが宣言した impl。
        //
        // 対象が外部パッケージの型でも、索引は自パッケージの `hir.tys` に張ってある
        // (`get_ty_impl` は外部の型を `.biwameta` から読むので、こちらには来ない)。
        let mut out: Vec<TyTraitImpl> = self
            .tctx
            .hir
            .tys
            .get(&ty)
            .map(|di| di.trait_impls.clone())
            .unwrap_or_default();

        // trait impl は対象の型のパッケージに載るとは限らない
        // (自分の trait を他パッケージの型に実装できる) ので、
        // 依存すべてを見る必要がある。
        let interner = self.tctx.interner.borrow();
        for (pkg_id, dep) in &self.tctx.ext_pkgs {
            out.extend(dep.trait_impls_for(ty, *pkg_id, &interner));
        }

        out
    }

    fn traits_in_scope(&self) -> &[TraitDefId] {
        &self.in_scope
    }

    fn trait_item(
        &self,
        trait_def_id: TraitDefId,
        name: InternedIdent,
    ) -> Option<biwac_span::TraitAssocDefId> {
        let trait_def = self.tctx.get_trait_def(&trait_def_id)?;
        trait_def.item(&name).map(|(_, item)| item.def_id)
    }

    fn bounds_of_local_gen(&self, def_id: LocalGenDefId) -> Vec<biwac_hir::TraitCond> {
        self.bounds.get(&def_id).cloned().unwrap_or_default()
    }
}

pub struct FnTyCtx<'tctx, 'a> {
    pub(super) tctx: &'tctx TyCtx<'a>,
    /// いま推論している関数が置かれているモジュール。
    /// trait 越しのメソッド解決で、どの trait がスコープにあるかを決める。
    pub(super) module: ModId,
    /// いま推論している関数から見えるジェネリック引数の制限。
    /// impl ブロックのぶんと関数自身のぶんの和である。
    pub(super) genarg_bounds: HashMap<LocalGenDefId, Vec<biwac_hir::TraitCond>>,
    /// 呼び出し位置で積まれた「この型がこの trait を満たすこと」という宿題。
    ///
    /// 呼び出しの時点では割り当てがまだ型変数のことがあるので、
    /// 本体を推論し終えてからまとめて解く。
    pub(super) obligations: Vec<Obligation>,
    next_tv: usize,
    pub(super) substitutions: HashMap<TyVar, Ty>,
    pub(super) vars: HashMap<VarId, Ty>,
    pub(super) exprs: HashMap<ExprId, Ty>,

    /// 呼び出し式ごとの、呼び先のジェネリック型への割り当て。
    /// [`biwac_hir::FnDef::call_genargs`] にそのまま渡る。
    pub(super) call_genargs: HashMap<ExprId, Vec<(LocalGenDefId, Ty)>>,
    pub(super) rty: Ty,
}

/// 呼び出し位置で積まれた制限の宿題。
pub(super) struct Obligation {
    /// 呼び先のジェネリック引数に割り当てられた型。
    /// 本体を推論し終えてから `resolve_ty` を通す。
    pub(super) ty: Ty,
    /// 満たさなければならない制限。
    pub(super) cond: biwac_hir::TraitCond,
    /// 呼び出し位置。診断の下線に使う。
    pub(super) span: biwac_span::Span,
}

// ある関数に対して型推論をした結果得られる型情報
pub(super) struct TyInfo {
    pub(super) expr_tys: HashMap<ExprId, Ty>,
    pub(super) var_tys: HashMap<VarId, Ty>,
    pub(super) call_genargs: HashMap<ExprId, Vec<(LocalGenDefId, Ty)>>,
}

impl<'tctx, 'a> FnTyCtx<'tctx, 'a> {
    pub(super) fn new(
        tctx: &'tctx TyCtx<'a>,
        rty: Ty,
        module: ModId,
        genarg_bounds: HashMap<LocalGenDefId, Vec<biwac_hir::TraitCond>>,
    ) -> Self {
        Self {
            tctx,
            module,
            genarg_bounds,
            obligations: Vec::new(),
            next_tv: 0,
            substitutions: HashMap::new(),
            vars: HashMap::new(),
            exprs: HashMap::new(),
            call_genargs: HashMap::new(),
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
