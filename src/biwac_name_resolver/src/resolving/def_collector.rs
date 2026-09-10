use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::Arc,
};

use biwac_ast::{PathSegmentResolution, TypReprVal};
use biwac_base::{IdentInterner, InternedIdent, ModId, PackageId};
use biwac_dependency_metadata::{
    DepMetadata, DepMetadataModuleView, ExternalPackage, PackageModuleView,
};
use biwac_hir::{Ty, TyKind};
use biwac_package_loader::{LoadedModule, Pkg};
use biwac_span::{
    DefId, DefIdKind, ImplId, PackageLocalDefId, Span, TraitAssocDefId, TraitDefId, TyDefId,
    ValDefId, VariantDefId,
};

use crate::{
    AssocNameTreeItem, ModuleNameTree, ModuleNameTreeItem, NameTree, PackageNameTree, ResolveError,
    ResolveErrorHandler, TyNameTree,
    name_tree::{AssocNameTree, AssocNameTreeItemKind},
    resolving::context::{ResolveCtx, impl_level::ImplResolveCtx, module_level::ModuleResolveCtx},
};

pub(crate) enum TyOrVal<T, V> {
    Ty(T),
    Val(V),
    Trait(TraitDefId),
}

/// DefCollector collects definitions in the self package.
pub struct DefCollector {
    next_pkg_local_def_id: u32,
    /// Maps alias TyDefId → canonical (non-alias) TyDefId; populated during collect().
    pub(super) alias_canonical: HashMap<TyDefId, TyDefId>,
    /// 自パッケージの trait が宣言した項目。宣言順。
    ///
    /// `T::guee()` の解決で「制限にある trait がこの名前を持つか」を引く。
    pub(crate) trait_items: HashMap<TraitDefId, Vec<(InternedIdent, TraitAssocDefId)>>,
    pub(crate) impl_collector: ImplCollector,
}

impl Default for DefCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl DefCollector {
    pub fn new() -> Self {
        Self {
            next_pkg_local_def_id: 0,
            alias_canonical: HashMap::new(),
            trait_items: HashMap::new(),
            impl_collector: ImplCollector::new(),
        }
    }

    pub(crate) fn alloc_def_id(&mut self) -> DefId {
        let pkg_local_def_id = PackageLocalDefId::new(self.next_pkg_local_def_id);
        self.next_pkg_local_def_id += 1;
        DefId::new_in_self_pkg(pkg_local_def_id)
    }

    pub fn collect(
        &mut self,
        pkg_name: InternedIdent,
        pkg: &Pkg,
        external_packages: Vec<ExternalPackage>,
        interner: &IdentInterner,
    ) -> Result<NameTree, Vec<ResolveError>> {
        // trait も他のシンボルと同じ走査で採番する。
        // 制限 (`struct Foo[T: A]` の `A`) が解決されるのは Step 2 以降で、
        // そのときには全 DefId が振り終わっている。
        // 構造体のメンバの型が後方宣言でよいのと同じ理屈である。

        // Step 1: assign IDs to all non-impl symbols, build module-level NameTree.
        let root_module_tree = self.collect_in_module(&pkg.root_module)?;
        let package_tree = PackageNameTree {
            pkg_id: PackageId::SELF_PACKAGE,
            root_module_tree,
        };

        // PackageId は driver が決定済み。そのまま lookup maps に格納する。
        //
        // 名前で引ける (= import の根になれる) のは直接依存だけである。
        // 推移的な依存はパスに書けないが、
        // 直接依存のシグニチャがその型を参照しうるのでデータは保持する。
        // (Rust の extern prelude と同じ区別)
        let mut ext_pkg_views: HashMap<InternedIdent, Arc<dyn PackageModuleView>> = HashMap::new();
        let mut ext_pkg_data: HashMap<PackageId, Arc<DepMetadata>> = HashMap::new();
        for p in external_packages {
            if p.direct {
                let view = DepMetadataModuleView::new_root(Arc::clone(&p.meta), p.pkg_id);
                ext_pkg_views.insert(p.ident, Arc::new(view) as Arc<dyn PackageModuleView>);
            }
            ext_pkg_data.insert(p.pkg_id, p.meta);
        }

        let mut packages = HashMap::new();
        packages.insert(pkg_name, package_tree);
        let name_tree = NameTree {
            self_pkg_name: pkg_name,
            packages,
            ext_pkg_views,
            ext_pkg_data,
        };

        // Build TyDefId → &TyNameTree and ModId → &ModuleNameTree indexes.
        let root = &name_tree.packages[&pkg_name].root_module_tree;
        let mut ty_index: HashMap<TyDefId, &TyNameTree> = HashMap::new();
        collect_ty_trees(root, &mut ty_index);
        let mut mod_index: HashMap<ModId, &ModuleNameTree> = HashMap::new();
        collect_mod_trees(root, &mut mod_index);

        // Step 2: resolve type alias RHS paths, detect cycles, populate alias_target.
        self.resolve_alias_targets(pkg_name, pkg, &name_tree, &ty_index, &mod_index, interner)?;

        // Step 3: collect impl-block symbols under their canonical (non-alias) types.
        self.collect_impls(pkg_name, &name_tree, pkg, &ty_index, &mod_index, interner)?;

        // Step 4: collect trait impl blocks.
        //
        // Step 3 と分けているのは、名前の衝突検査が
        // 「その型が既に持っている関連名の一覧」を要るからである。
        self.collect_trait_impls(pkg_name, &name_tree, pkg, &ty_index, &mod_index, interner)?;

        Ok(name_tree)
    }

    // NOTE: impl blocks are not processed here because they require name resolution
    // to determine the self type.
    fn collect_in_module(
        &mut self,
        module: &LoadedModule,
    ) -> Result<ModuleNameTree, Vec<ResolveError>> {
        let mut children = HashMap::<InternedIdent, (ModuleNameTreeItem, Span)>::new();
        let mut errors = Vec::new();
        // バリアントは enum の名前ツリーの children に載せる。
        // その木は下の insert で初めて出来るので、名前だけ控えて後で回す。
        let mut pending_variants = Vec::<InternedIdent>::new();

        for g in &module.ast.globals {
            let opt_ident_and_def_id = match g {
                biwac_ast::Globals::FnDef(fn_def) => {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    fn_def.def_id.set(def_id).unwrap();
                    Some((fn_def.id.clone(), TyOrVal::Val(def_id)))
                }
                biwac_ast::Globals::NativeFnDef(fn_def) => {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    fn_def.def_id.set(def_id).unwrap();
                    Some((fn_def.id.clone(), TyOrVal::Val(def_id)))
                }
                biwac_ast::Globals::VarDecl(_var_decl) => {
                    // TODO:
                    None
                }
                biwac_ast::Globals::Import(_) => None,
                biwac_ast::Globals::TypeDef(type_def) => match type_def {
                    biwac_ast::TypeDef::Struct(struct_def) => {
                        let def_id = TyDefId::new(self.alloc_def_id());
                        struct_def.def_id.set(def_id).unwrap();
                        Some((struct_def.id.clone(), TyOrVal::Ty(def_id)))
                    }
                    biwac_ast::TypeDef::Enum(enum_def) => {
                        let def_id = TyDefId::new(self.alloc_def_id());
                        enum_def.def_id.set(def_id).unwrap();

                        // バリアントにも DefId を振る。
                        // 単体で import できるようにするためで、rustc と同じ扱いである。
                        // 採番の順序が `.biwameta` に出るので、宣言順のまま回す。
                        for variant in &enum_def.variants {
                            let variant_def_id = VariantDefId::new(self.alloc_def_id());
                            variant.def_id.set(variant_def_id).unwrap();
                        }

                        pending_variants.push(enum_def.id.id);
                        Some((enum_def.id.clone(), TyOrVal::Ty(def_id)))
                    }
                    biwac_ast::TypeDef::TypeAlias(alias_def) => {
                        let def_id = TyDefId::new(self.alloc_def_id());
                        alias_def.def_id.set(def_id).unwrap();
                        Some((alias_def.ident.clone(), TyOrVal::Ty(def_id)))
                    }
                    biwac_ast::TypeDef::NativeTypeAlias(alias_def) => {
                        let def_id = TyDefId::new(self.alloc_def_id());
                        alias_def.def_id.set(def_id).unwrap();
                        Some((alias_def.ident.clone(), TyOrVal::Ty(def_id)))
                    }
                },
                biwac_ast::Globals::TraitDef(trait_def) => {
                    let def_id = TraitDefId::new(self.alloc_def_id());
                    trait_def.def_id.set(def_id).unwrap();

                    // 項目にも id を振る。
                    // 採番の順序が `.biwameta` に出るので、宣言順のまま回す
                    // (enum のバリアントと同じ)。
                    let mut items = Vec::with_capacity(trait_def.items.len());
                    for item in &trait_def.items {
                        let item_def_id = TraitAssocDefId::new(self.alloc_def_id());
                        item.def_id.set(item_def_id).unwrap();
                        items.push((item.id.id, item_def_id));
                    }
                    self.trait_items.insert(def_id, items);

                    Some((trait_def.id.clone(), TyOrVal::Trait(def_id)))
                }
                biwac_ast::Globals::NativeCode(_) => None,
                biwac_ast::Globals::NovelScene(scene_def) => {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    scene_def.def_id.set(def_id).unwrap();
                    Some((scene_def.id.clone(), TyOrVal::Val(def_id)))
                }
                biwac_ast::Globals::ImplBlock(_) => None,
            };

            if let Some((ident, def_id)) = opt_ident_and_def_id {
                match children.entry(ident.id) {
                    Entry::Vacant(e) => match def_id {
                        TyOrVal::Ty(def_id) => {
                            e.insert((
                                ModuleNameTreeItem::Ty(TyNameTree {
                                    def_id,
                                    children: RefCell::new(HashMap::new()),
                                    alias_target: RefCell::new(None),
                                }),
                                ident.span.clone(),
                            ));
                        }
                        TyOrVal::Val(def_id) => {
                            e.insert((ModuleNameTreeItem::Val(def_id), ident.span.clone()));
                        }
                        TyOrVal::Trait(def_id) => {
                            e.insert((ModuleNameTreeItem::Trait(def_id), ident.span.clone()));
                        }
                    },
                    Entry::Occupied(e) => {
                        errors.push(ResolveError::DuplicatedSymbolName {
                            name: ident.id,
                            // span1 は先に来た方
                            span1: e.get().1.clone(),
                            span2: ident.span.clone(),
                        });
                    }
                }
            }
        }

        // enum のバリアントを、その enum の名前ツリーに登録する。
        //
        // 関連関数と同じ children に載るので、
        // `enum Foo { Bar }` と `impl Foo { fn Bar() }` は衝突する
        // (`register_assoc` が検出する)。
        for g in &module.ast.globals {
            let biwac_ast::Globals::TypeDef(biwac_ast::TypeDef::Enum(enum_def)) = g else {
                continue;
            };
            if !pending_variants.contains(&enum_def.id.id) {
                // 名前が衝突して木に入らなかった enum。
                continue;
            }
            let Some((ModuleNameTreeItem::Ty(ty_tree), _)) = children.get(&enum_def.id.id) else {
                continue;
            };

            let mut ty_children = ty_tree.children.borrow_mut();
            for variant in &enum_def.variants {
                let def_id = *variant
                    .def_id
                    .get()
                    .expect("compiler bug: variant def_id not allocated");

                let assoc = ty_children
                    .entry(variant.id.id)
                    .or_insert(AssocNameTree { assocs: Vec::new() });

                if let Err(e) = assoc.register_assoc(
                    variant.id.id,
                    Vec::new(),
                    AssocNameTreeItemKind::Variant(def_id),
                ) {
                    errors.push(e);
                }
            }
        }

        // 子モジュールも決定論的な順序で処理する。
        // ここで DefId を採番するので、順序がぶれると .biwameta がビルドごとに変わる。
        for (interned_mod_name, module) in module.children_ordered() {
            match children.entry(*interned_mod_name) {
                Entry::Vacant(e) => match self.collect_in_module(module) {
                    Ok(module_tree) => {
                        e.insert((
                            ModuleNameTreeItem::Mod(module_tree),
                            Span::new(module.mod_id, 0, 0),
                        ));
                    }
                    Err(errs) => {
                        errors.extend(errs);
                    }
                },
                Entry::Occupied(e) => {
                    errors.push(ResolveError::DuplicatedSymbolAndModuleName {
                        name: *interned_mod_name,
                        mod_id: module.mod_id,
                        symbol_span: e.get().1.clone(),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(ModuleNameTree {
                mod_id: module.mod_id,
                children: children
                    .into_iter()
                    .map(|(interned, (item, _))| (interned, item))
                    .collect(),
            })
        } else {
            Err(errors)
        }
    }

    /// Resolves the RHS path of every TypeAlias in the package, builds the alias→canonical map,
    /// detects cycles, and records alias_target in TyNameTree.
    fn resolve_alias_targets(
        &mut self,
        pkg_name: InternedIdent,
        pkg: &Pkg,
        name_tree: &NameTree,
        ty_index: &HashMap<TyDefId, &TyNameTree>,
        mod_index: &HashMap<ModId, &ModuleNameTree>,
        interner: &IdentInterner,
    ) -> Result<(), Vec<ResolveError>> {
        // direct_map[alias_id] = immediate_target_id
        let mut direct_map: HashMap<TyDefId, TyDefId> = HashMap::new();
        // span_map[alias_id] = span of the alias name (for cycle error reporting)
        let mut span_map: HashMap<TyDefId, Span> = HashMap::new();

        self.collect_alias_direct_targets(
            pkg_name,
            pkg,
            name_tree,
            ty_index,
            mod_index,
            interner,
            &mut direct_map,
            &mut span_map,
        )?;

        // Cycle detection
        let mut errors = Vec::new();
        detect_alias_cycles(&direct_map, &span_map, &mut errors);
        if !errors.is_empty() {
            return Err(errors);
        }

        // Follow chains to compute canonical (non-alias) targets.
        let canonical_map: HashMap<TyDefId, TyDefId> = direct_map
            .keys()
            .map(|&alias_id| (alias_id, follow_alias_chain(alias_id, &direct_map)))
            .collect();

        // Populate TyNameTree.alias_target and store in self.
        for (&alias_id, &canonical_id) in &canonical_map {
            if let Some(ty_tree) = ty_index.get(&alias_id) {
                *ty_tree.alias_target.borrow_mut() = Some(canonical_id);
            }
        }
        self.alias_canonical = canonical_map;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_alias_direct_targets(
        &self,
        pkg_name: InternedIdent,
        pkg: &Pkg,
        name_tree: &NameTree,
        ty_index: &HashMap<TyDefId, &TyNameTree>,
        mod_index: &HashMap<ModId, &ModuleNameTree>,
        interner: &IdentInterner,
        direct_map: &mut HashMap<TyDefId, TyDefId>,
        span_map: &mut HashMap<TyDefId, Span>,
    ) -> Result<(), Vec<ResolveError>> {
        let root_module_tree = &name_tree.packages[&pkg_name].root_module_tree;
        self.collect_alias_direct_targets_in_module(
            pkg_name,
            name_tree,
            ty_index,
            mod_index,
            interner,
            root_module_tree,
            &pkg.root_module,
            direct_map,
            span_map,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_alias_direct_targets_in_module(
        &self,
        pkg_name: InternedIdent,
        name_tree: &NameTree,
        ty_index: &HashMap<TyDefId, &TyNameTree>,
        mod_index: &HashMap<ModId, &ModuleNameTree>,
        interner: &IdentInterner,
        module_tree: &ModuleNameTree,
        module: &LoadedModule,
        direct_map: &mut HashMap<TyDefId, TyDefId>,
        span_map: &mut HashMap<TyDefId, Span>,
    ) -> Result<(), Vec<ResolveError>> {
        let mctx = ModuleResolveCtx::new(
            name_tree,
            pkg_name,
            module_tree,
            &module.ast,
            ty_index,
            mod_index,
            interner,
        )?;
        let mut errors = Vec::new();

        for g in &module.ast.globals {
            if let biwac_ast::Globals::TypeDef(biwac_ast::TypeDef::TypeAlias(alias)) = g {
                let alias_id = match alias.def_id.get() {
                    Some(id) => *id,
                    None => continue,
                };
                span_map.insert(alias_id, alias.ident.span.clone());

                // Resolve only the main path of the RHS (genargs resolved in full pass).
                let target_id = match &alias.right.val {
                    TypReprVal::Defined(def_typ) => match mctx.resolve_path(&def_typ.path) {
                        Ok(()) => {
                            let last_seg = def_typ.path.segments.last().unwrap();
                            match last_seg.resolved_id.get() {
                                Some(PathSegmentResolution::Ok(DefIdKind::Ty(id))) => Some(*id),
                                _ => None,
                            }
                        }
                        Err(e) => {
                            errors.push(e);
                            None
                        }
                    },
                    _ => None, // Primitive or Self aliases have no TyDefId target.
                };

                if let Some(target_id) = target_id {
                    direct_map.insert(alias_id, target_id);
                }
            }
        }

        for (child_name, child_module) in module.children_ordered() {
            let child_tree = match module_tree.children.get(child_name) {
                Some(ModuleNameTreeItem::Mod(m)) => m,
                _ => continue,
            };
            if let Err(errs) = self.collect_alias_direct_targets_in_module(
                pkg_name,
                name_tree,
                ty_index,
                mod_index,
                interner,
                child_tree,
                child_module,
                direct_map,
                span_map,
            ) {
                errors.extend(errs);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn collect_impls(
        &mut self,
        pkg_name: InternedIdent,
        name_tree: &NameTree,
        pkg: &Pkg,
        ty_index: &HashMap<TyDefId, &TyNameTree>,
        mod_index: &HashMap<ModId, &ModuleNameTree>,
        interner: &IdentInterner,
    ) -> Result<(), Vec<ResolveError>> {
        let root_module_tree = &name_tree.packages[&pkg_name].root_module_tree;
        self.collect_impls_in_module(
            name_tree,
            pkg_name,
            root_module_tree,
            &pkg.root_module,
            ty_index,
            mod_index,
            interner,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_impls_in_module(
        &mut self,
        name_tree: &NameTree,
        pkg_name: InternedIdent,
        module_tree: &ModuleNameTree,
        module: &LoadedModule,
        ty_index: &HashMap<TyDefId, &TyNameTree>,
        mod_index: &HashMap<ModId, &ModuleNameTree>,
        interner: &IdentInterner,
    ) -> Result<(), Vec<ResolveError>> {
        let mctx = ModuleResolveCtx::new(
            name_tree,
            pkg_name,
            module_tree,
            &module.ast,
            ty_index,
            mod_index,
            interner,
        )?;
        let mut errors = Vec::new();

        for g in &module.ast.globals {
            if let biwac_ast::Globals::ImplBlock(impl_block) = g {
                // trait impl は Step 4 が扱う。
                // ここで通すと項目が名前ツリーに載ってしまい、
                // import 無しで見えるようになってしまう。
                if impl_block.trait_typ.is_some() {
                    continue;
                }

                let ictx = match ImplResolveCtx::new(&mctx, impl_block, self) {
                    Ok(ictx) => ictx,
                    Err(errs) => {
                        errors.extend(errs);
                        continue;
                    }
                };

                let self_ty = ictx.opt_self_ty().unwrap();
                let impl_id = self.impl_collector.register_self_ty(self_ty.clone());
                impl_block.impl_id.set(impl_id).unwrap();

                // Determine the canonical TyDefId (following alias chain).
                let canonical_id = match canonical_ty_def_id(&self_ty, &self.alias_canonical) {
                    Some(id) => id,
                    None => {
                        // Primitive or Fn types: associated items not tracked in TyNameTree.
                        // They are handled via hir.special_ty_impls in lowering.
                        for f in &impl_block.assoc_fns {
                            let def_id = ValDefId::new(self.alloc_def_id());
                            let _ = f.def_id.set(def_id);
                        }
                        for m in &impl_block.methods {
                            let def_id = ValDefId::new(self.alloc_def_id());
                            let _ = m.def_id.set(def_id);
                        }
                        for f in &impl_block.native_assoc_fns {
                            let def_id = ValDefId::new(self.alloc_def_id());
                            let _ = f.def_id.set(def_id);
                        }
                        for m in &impl_block.native_methods {
                            let def_id = ValDefId::new(self.alloc_def_id());
                            let _ = m.def_id.set(def_id);
                        }
                        continue;
                    }
                };

                let ty_tree = match ty_index.get(&canonical_id) {
                    Some(t) => t,
                    None => {
                        // Type defined in external package or not found; skip.
                        continue;
                    }
                };

                // TODO:
                // if !canonical_id.pkg().is_self() &&
                //   !(canonical_id.pkg() == PackageId::BUILTIN_RESERVED_PACKAGE && no_std) {
                //   // foreign impl エラー
                //   // std のみ プリミティブ型への impl が許可されていることに注意
                // }

                for f in &impl_block.assoc_fns {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    if let Some(assocs) = ty_tree.children.borrow_mut().get_mut(&f.id.id) {
                        assocs
                            .register_assoc(
                                f.id.id,
                                match &self_ty {
                                    TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                                    _ => Vec::new(),
                                },
                                AssocNameTreeItemKind::Val(def_id),
                            )
                            .handle(&mut errors);

                        f.def_id.set(def_id).unwrap();
                    } else {
                        ty_tree.children.borrow_mut().insert(
                            f.id.id,
                            AssocNameTree {
                                assocs: vec![AssocNameTreeItem {
                                    genargs: match &self_ty {
                                        TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                                        _ => Vec::new(),
                                    },
                                    kind: AssocNameTreeItemKind::Val(def_id),
                                }],
                            },
                        );
                        f.def_id.set(def_id).unwrap();
                    }
                }

                for m in &impl_block.methods {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    if let Some(assocs) = ty_tree.children.borrow_mut().get_mut(&m.id.id) {
                        assocs
                            .register_assoc(
                                m.id.id,
                                match &self_ty {
                                    TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                                    _ => Vec::new(),
                                },
                                AssocNameTreeItemKind::Val(def_id),
                            )
                            .handle(&mut errors);

                        m.def_id.set(def_id).unwrap();
                    } else {
                        ty_tree.children.borrow_mut().insert(
                            m.id.id,
                            AssocNameTree {
                                assocs: vec![AssocNameTreeItem {
                                    genargs: match &self_ty {
                                        TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                                        _ => Vec::new(),
                                    },
                                    kind: AssocNameTreeItemKind::Val(def_id),
                                }],
                            },
                        );
                        m.def_id.set(def_id).unwrap();
                    }
                }

                for f in &impl_block.native_assoc_fns {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    if let Some(assocs) = ty_tree.children.borrow_mut().get_mut(&f.id.id) {
                        assocs
                            .register_assoc(
                                f.id.id,
                                match &self_ty {
                                    TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                                    _ => Vec::new(),
                                },
                                AssocNameTreeItemKind::Val(def_id),
                            )
                            .handle(&mut errors);

                        f.def_id.set(def_id).unwrap();
                    } else {
                        ty_tree.children.borrow_mut().insert(
                            f.id.id,
                            AssocNameTree {
                                assocs: vec![AssocNameTreeItem {
                                    genargs: match &self_ty {
                                        TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                                        _ => Vec::new(),
                                    },
                                    kind: AssocNameTreeItemKind::Val(def_id),
                                }],
                            },
                        );
                        f.def_id.set(def_id).unwrap();
                    }
                }

                for m in &impl_block.native_methods {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    if let Some(assocs) = ty_tree.children.borrow_mut().get_mut(&m.id.id) {
                        assocs
                            .register_assoc(
                                m.id.id,
                                match &self_ty {
                                    TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                                    _ => Vec::new(),
                                },
                                AssocNameTreeItemKind::Val(def_id),
                            )
                            .handle(&mut errors);

                        m.def_id.set(def_id).unwrap();
                    } else {
                        ty_tree.children.borrow_mut().insert(
                            m.id.id,
                            AssocNameTree {
                                assocs: vec![AssocNameTreeItem {
                                    genargs: match &self_ty {
                                        TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                                        _ => Vec::new(),
                                    },
                                    kind: AssocNameTreeItemKind::Val(def_id),
                                }],
                            },
                        );
                        m.def_id.set(def_id).unwrap();
                    }
                }
            }
        }

        for (child_name, child_module) in module.children_ordered() {
            let child_tree = match module_tree.children.get(child_name) {
                Some(ModuleNameTreeItem::Mod(m)) => m,
                _ => continue,
            };
            if let Err(errs) = self.collect_impls_in_module(
                name_tree,
                pkg_name,
                child_tree,
                child_module,
                ty_index,
                mod_index,
                interner,
            ) {
                errors.extend(errs);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Step 4: trait impl ブロックを集める。
    ///
    /// 直接の impl (Step 3) と違い、項目は名前ツリーに載せない。
    /// スコープにある trait を経由してしか引けないようにするためで、
    /// これが「使う箇所で trait を import していること」という規則の実体である。
    fn collect_trait_impls(
        &mut self,
        pkg_name: InternedIdent,
        name_tree: &NameTree,
        pkg: &Pkg,
        ty_index: &HashMap<TyDefId, &TyNameTree>,
        mod_index: &HashMap<ModId, &ModuleNameTree>,
        interner: &IdentInterner,
    ) -> Result<(), Vec<ResolveError>> {
        let root_module_tree = &name_tree.packages[&pkg_name].root_module_tree;
        // (対象の型, trait) -> 既に登録した impl。重複検査に使う。
        let mut seen: HashMap<(TyDefId, TraitDefId), Vec<(Vec<Ty>, Span)>> = HashMap::new();
        // 対象の型 -> その型に trait impl が生やした名前。名前の衝突検査に使う。
        let mut trait_impl_names: HashMap<TyDefId, HashMap<InternedIdent, Span>> = HashMap::new();

        self.collect_trait_impls_in_module(
            name_tree,
            pkg_name,
            root_module_tree,
            &pkg.root_module,
            ty_index,
            mod_index,
            interner,
            &mut seen,
            &mut trait_impl_names,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_trait_impls_in_module(
        &mut self,
        name_tree: &NameTree,
        pkg_name: InternedIdent,
        module_tree: &ModuleNameTree,
        module: &LoadedModule,
        ty_index: &HashMap<TyDefId, &TyNameTree>,
        mod_index: &HashMap<ModId, &ModuleNameTree>,
        interner: &IdentInterner,
        seen: &mut HashMap<(TyDefId, TraitDefId), Vec<(Vec<Ty>, Span)>>,
        trait_impl_names: &mut HashMap<TyDefId, HashMap<InternedIdent, Span>>,
    ) -> Result<(), Vec<ResolveError>> {
        let mctx = ModuleResolveCtx::new(
            name_tree,
            pkg_name,
            module_tree,
            &module.ast,
            ty_index,
            mod_index,
            interner,
        )?;
        let mut errors = Vec::new();

        for g in &module.ast.globals {
            let biwac_ast::Globals::ImplBlock(impl_block) = g else {
                continue;
            };
            let Some(trait_typ) = &impl_block.trait_typ else {
                continue;
            };

            let ictx = match ImplResolveCtx::new(&mctx, impl_block, self) {
                Ok(ictx) => ictx,
                Err(errs) => {
                    errors.extend(errs);
                    continue;
                }
            };

            // `impl[T] Foo[T]: Bar[T, Int]` の右辺。
            // impl ブロックのジェネリック引数が見える文脈で解決する。
            if let Err(errs) = ictx.resolve_trait_typ(trait_typ) {
                errors.extend(errs);
                continue;
            }

            let (trait_def_id, trait_genargs) = match trait_ref_of(trait_typ) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };

            let self_ty = ictx.opt_self_ty().unwrap();
            let impl_id = self.impl_collector.register_self_ty(self_ty.clone());
            impl_block.impl_id.set(impl_id).unwrap();

            let Some(canonical_id) = canonical_impl_target(&self_ty, &self.alias_canonical) else {
                // 関数型など、impl の対象にならない型。
                errors.push(ResolveError::ForeignTraitImpl {
                    span: impl_block.self_typ.span.clone(),
                });
                continue;
            };

            // --- 孤児則 ---
            //
            // trait 自身か対象の型のいずれかが自パッケージであること。
            // プリミティブ型は BUILTIN_RESERVED_PACKAGE なので、
            // trait が自パッケージのときだけ通る (std がそれに当たる)。
            if !canonical_id.pkg().is_self() && !trait_def_id.pkg().is_self() {
                errors.push(ResolveError::ForeignTraitImpl {
                    span: impl_block.span.clone(),
                });
                continue;
            }

            let ty_genargs: Vec<Ty> = match &self_ty {
                TyKind::Defined(defined_ty) => defined_ty.genargs.clone(),
                _ => Vec::new(),
            };

            // --- 重複 ---
            let entry = seen.entry((canonical_id, trait_def_id)).or_default();
            if let Some((_, span1)) = entry.iter().find(|(genargs, _)| {
                genargs.len() == ty_genargs.len()
                    && genargs
                        .iter()
                        .zip(&ty_genargs)
                        .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind))
            }) {
                errors.push(ResolveError::DuplicatedTraitImpl {
                    span1: span1.clone(),
                    span2: impl_block.span.clone(),
                });
                continue;
            }
            entry.push((ty_genargs.clone(), impl_block.span.clone()));

            // --- 項目に ValDefId を振り、名前の衝突を見る ---
            let mut vals: HashMap<InternedIdent, ValDefId> = HashMap::new();
            let mut item_names: Vec<(InternedIdent, Span)> = Vec::new();

            for f in &impl_block.assoc_fns {
                item_names.push((f.id.id, f.id.span.clone()));
            }
            for m in &impl_block.methods {
                item_names.push((m.id.id, m.id.span.clone()));
            }
            for f in &impl_block.native_assoc_fns {
                item_names.push((f.id.id, f.id.span.clone()));
            }
            for m in &impl_block.native_methods {
                item_names.push((m.id.id, m.id.span.clone()));
            }

            let taken = trait_impl_names.entry(canonical_id).or_default();
            for (name, span) in &item_names {
                let conflicts =
                    ty_has_assoc_name(canonical_id, *name, ty_index, name_tree, interner)
                        || taken.contains_key(name);
                if conflicts {
                    errors.push(ResolveError::TraitImplNameConflict {
                        name: *name,
                        span: span.clone(),
                    });
                } else {
                    taken.insert(*name, span.clone());
                }
            }

            let mut alloc = |cell: &std::cell::OnceCell<ValDefId>, name: InternedIdent| {
                let def_id = ValDefId::new(self.alloc_def_id());
                let _ = cell.set(def_id);
                vals.insert(name, def_id);
            };
            for f in &impl_block.assoc_fns {
                alloc(&f.def_id, f.id.id);
            }
            for m in &impl_block.methods {
                alloc(&m.def_id, m.id.id);
            }
            for f in &impl_block.native_assoc_fns {
                alloc(&f.def_id, f.id.id);
            }
            for m in &impl_block.native_methods {
                alloc(&m.def_id, m.id.id);
            }

            self.impl_collector
                .impl_traits
                .insert(impl_id, (trait_def_id, trait_genargs.clone()));

            // 名前解決のフォールバック用の索引。
            //
            // `ty_genargs` はここでは正確でないことがある。
            // 実装対象が型エイリアスで書かれていると、その右辺の型引数は
            // まだ解決されていないためである
            // (`type C = Character[P]` に対して `[]` になる)。
            // パスの解決 (`solve_assoc`) は特殊化で絞らないので影響しない。
            // 特殊化まで見る `solve_method` が読むのは
            // lowering が組み直した `DefinedTyImpl::trait_impls` の方である。
            self.impl_collector
                .trait_impls
                .entry(canonical_id)
                .or_default()
                .push(biwac_hir::TyTraitImpl {
                    trait_def_id,
                    impl_block_genargs: crate::lowering::globals::collect_impl_block_genargs_map(
                        impl_block,
                    ),
                    ty_genargs,
                    trait_genargs,
                    vals,
                    span: impl_block.span.clone(),
                });
        }

        for (child_name, child_module) in module.children_ordered() {
            let child_tree = match module_tree.children.get(child_name) {
                Some(ModuleNameTreeItem::Mod(m)) => m,
                _ => continue,
            };
            if let Err(errs) = self.collect_trait_impls_in_module(
                name_tree,
                pkg_name,
                child_tree,
                child_module,
                ty_index,
                mod_index,
                interner,
                seen,
                trait_impl_names,
            ) {
                errors.extend(errs);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// `impl Foo: Bar[Int]` の `Bar[Int]` を (trait, ジェネリック引数) に分解する。
fn trait_ref_of(trait_typ: &biwac_ast::TypRepr) -> Result<(TraitDefId, Vec<Ty>), ResolveError> {
    let TypReprVal::Defined(def_typ) = &trait_typ.val else {
        return Err(ResolveError::TraitExpected {
            path: Box::new(biwac_ast::Path::new(None, vec![])),
        });
    };

    match crate::lowering::def_id_kind_from_path(&def_typ.path)? {
        DefIdKind::Trait(def_id) => {
            let genargs = def_typ
                .genargs
                .iter()
                .flat_map(|gs| {
                    gs.iter()
                        .map(|t| crate::lowering::ty_from_typ_repr(t, None))
                })
                .collect();
            Ok((def_id, genargs))
        }
        _ => Err(ResolveError::TraitExpected {
            path: Box::new(def_typ.path.clone()),
        }),
    }
}

/// impl の対象になれる型の、エイリアスを辿った先の `TyDefId`。
///
/// [`canonical_ty_def_id`] と違い、プリミティブ型も返す。
/// 孤児則の判定はプリミティブ型にも及ぶためである
/// (`impl Int: Foo` は `Foo` が自パッケージのときだけ通る)。
fn canonical_impl_target(
    ty_kind: &TyKind,
    alias_canonical: &HashMap<TyDefId, TyDefId>,
) -> Option<TyDefId> {
    match ty_kind {
        TyKind::Defined(biwac_hir::DefinedTy { def_id, .. }) => {
            Some(*alias_canonical.get(def_id).unwrap_or(def_id))
        }
        _ => ty_kind.def_id(),
    }
}

/// その型が既にこの名前の関連アイテムを持っているか。
///
/// 自パッケージの型は名前ツリーから、
/// 外部パッケージの型は `.biwameta` から引く。
fn ty_has_assoc_name(
    ty_def_id: TyDefId,
    name: InternedIdent,
    ty_index: &HashMap<TyDefId, &TyNameTree>,
    name_tree: &NameTree,
    interner: &IdentInterner,
) -> bool {
    if let Some(ty_tree) = ty_index.get(&ty_def_id) {
        return ty_tree.children.borrow().contains_key(&name);
    }

    let pkg_id = ty_def_id.pkg();
    let Some(dep_arc) = name_tree.ext_pkg_data.get(&pkg_id) else {
        return false;
    };
    let view =
        DepMetadataModuleView::new_for_sym_idx(Arc::clone(dep_arc), ty_def_id.local_idx(), pkg_id);
    view.lookup_assoc(ty_def_id.local_idx(), name, interner)
        .is_some()
}

#[derive(Debug)]
pub(crate) struct ImplCollector {
    next_impl_id: u32,
    pub(crate) impl_self_tys: HashMap<ImplId, TyKind>,
    /// trait impl なら、その trait とジェネリック引数。直接の impl は載らない。
    pub(crate) impl_traits: HashMap<ImplId, (TraitDefId, Vec<Ty>)>,
    /// エイリアスを辿った先の型 -> その型に対する trait impl。
    ///
    /// 名前解決のフォールバック (`biwac_trait_solver`) と、
    /// lowering が `DefinedTyImpl::trait_impls` を組むのに使う。
    pub(crate) trait_impls: HashMap<TyDefId, Vec<biwac_hir::TyTraitImpl>>,
}

impl ImplCollector {
    fn new() -> Self {
        Self {
            next_impl_id: 0,
            impl_self_tys: HashMap::new(),
            impl_traits: HashMap::new(),
            trait_impls: HashMap::new(),
        }
    }

    fn register_self_ty(&mut self, ty_kind: TyKind) -> ImplId {
        let id = ImplId::new(self.next_impl_id);
        self.next_impl_id += 1;
        self.impl_self_tys.insert(id, ty_kind);
        id
    }
}

/// Builds a flat TyDefId → &TyNameTree index by walking the module tree.
pub(super) fn collect_ty_trees<'a>(
    module: &'a ModuleNameTree,
    map: &mut HashMap<TyDefId, &'a TyNameTree>,
) {
    for item in module.children.values() {
        match item {
            ModuleNameTreeItem::Ty(ty_tree) => {
                map.insert(ty_tree.def_id, ty_tree);
            }
            ModuleNameTreeItem::Mod(mod_tree) => {
                collect_ty_trees(mod_tree, map);
            }
            ModuleNameTreeItem::Val(_) | ModuleNameTreeItem::Trait(_) => {}
        }
    }
}

/// Builds a flat ModId → &ModuleNameTree index by walking the module tree.
/// Used to navigate from a resolved module DefIdKind into its children during path resolution.
pub(super) fn collect_mod_trees<'a>(
    module: &'a ModuleNameTree,
    map: &mut HashMap<ModId, &'a ModuleNameTree>,
) {
    map.insert(module.mod_id, module);
    for item in module.children.values() {
        if let ModuleNameTreeItem::Mod(mod_tree) = item {
            collect_mod_trees(mod_tree, map);
        }
    }
}

/// Returns the canonical (non-alias) TyDefId for a TyKind, or None for non-defined types.
fn canonical_ty_def_id(
    ty_kind: &TyKind,
    alias_canonical: &HashMap<TyDefId, TyDefId>,
) -> Option<TyDefId> {
    match ty_kind {
        TyKind::Defined(biwac_hir::DefinedTy { def_id, .. }) => {
            Some(*alias_canonical.get(def_id).unwrap_or(def_id))
        }
        _ => None,
    }
}

fn detect_alias_cycles(
    direct_map: &HashMap<TyDefId, TyDefId>,
    span_map: &HashMap<TyDefId, Span>,
    errors: &mut Vec<ResolveError>,
) {
    let mut visited: HashSet<TyDefId> = HashSet::new();
    let mut in_progress: HashSet<TyDefId> = HashSet::new();

    for &alias_id in direct_map.keys() {
        if !visited.contains(&alias_id) {
            dfs_detect(
                alias_id,
                direct_map,
                span_map,
                &mut visited,
                &mut in_progress,
                errors,
            );
        }
    }
}

fn dfs_detect(
    current: TyDefId,
    direct_map: &HashMap<TyDefId, TyDefId>,
    span_map: &HashMap<TyDefId, Span>,
    visited: &mut HashSet<TyDefId>,
    in_progress: &mut HashSet<TyDefId>,
    errors: &mut Vec<ResolveError>,
) {
    in_progress.insert(current);

    if let Some(&next) = direct_map.get(&current) {
        if in_progress.contains(&next) {
            if let Some(span) = span_map.get(&next) {
                errors.push(ResolveError::CyclingTypeAlias {
                    def_id: Box::new(next),
                    detected_position: Box::new(span.clone()),
                });
            }
        } else if !visited.contains(&next) {
            dfs_detect(next, direct_map, span_map, visited, in_progress, errors);
        }
    }

    in_progress.remove(&current);
    visited.insert(current);
}

fn follow_alias_chain(start: TyDefId, direct_map: &HashMap<TyDefId, TyDefId>) -> TyDefId {
    let mut current = start;
    let mut seen: HashSet<TyDefId> = HashSet::new();
    seen.insert(current);
    while let Some(&next) = direct_map.get(&current) {
        if seen.contains(&next) {
            break; // cycle (already reported); stop here
        }
        seen.insert(next);
        current = next;
    }
    current
}
