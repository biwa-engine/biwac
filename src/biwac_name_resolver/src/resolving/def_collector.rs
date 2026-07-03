use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, hash_map::Entry},
    sync::Arc,
};

use biwac_ast::{PathSegmentResolution, TypReprVal};
use biwac_base::{IdentInterner, InternedIdent, ModId, PackageId};
use biwac_dependency_metadata::{DepMetadata, DepMetadataModuleView, PackageModuleView};
use biwac_hir::TyKind;
use biwac_package_loader::{LoadedModule, Pkg};
use biwac_span::{DefId, DefIdKind, ImplId, PackageLocalDefId, Span, TyDefId, ValDefId};

use crate::{
    AssocNameTreeItem, ModuleNameTree, ModuleNameTreeItem, NameTree, PackageNameTree, ResolveError,
    ResolveErrorHandler, TyNameTree,
    name_tree::{AssocNameTree, AssocNameTreeItemKind},
    resolving::context::{ResolveCtx, impl_level::ImplResolveCtx, module_level::ModuleResolveCtx},
};

pub(crate) enum TyOrVal<T, V> {
    Ty(T),
    Val(V),
}

/// DefCollector collects definitions in the self package.
pub struct DefCollector {
    next_pkg_local_def_id: u32,
    /// Maps alias TyDefId → canonical (non-alias) TyDefId; populated during collect().
    pub(super) alias_canonical: HashMap<TyDefId, TyDefId>,
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
        external_packages: Vec<(InternedIdent, PackageId, Arc<DepMetadata>)>,
        interner: &IdentInterner,
    ) -> Result<NameTree, Vec<ResolveError>> {
        // Step 1: assign IDs to all non-impl symbols, build module-level NameTree.
        let root_module_tree = self.collect_in_module(&pkg.root_module)?;
        let package_tree = PackageNameTree {
            pkg_id: PackageId::SELF_PACKAGE,
            root_module_tree,
        };

        // PackageId は driver が決定済み。そのまま lookup maps に格納する。
        let mut ext_pkg_views: HashMap<InternedIdent, Arc<dyn PackageModuleView>> = HashMap::new();
        let mut ext_pkg_data: HashMap<PackageId, Arc<DepMetadata>> = HashMap::new();
        for (pkg_ident, pkg_id, dep_arc) in external_packages {
            let view = DepMetadataModuleView::new_root(Arc::clone(&dep_arc), pkg_id);
            ext_pkg_views.insert(pkg_ident, Arc::new(view) as Arc<dyn PackageModuleView>);
            ext_pkg_data.insert(pkg_id, dep_arc);
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
                    },
                    Entry::Occupied(e) => {
                        errors.push(ResolveError::DuplicatedSymbolName {
                            name: ident.id,
                            span1: ident.span.clone(),
                            span2: e.get().1.clone(),
                        });
                    }
                }
            }
        }

        for (interned_mod_name, module) in &module.children {
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

        for (child_name, child_module) in &module.children {
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

                for f in &impl_block.assoc_fns {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    if let Some(assocs) = ty_tree.children.borrow_mut().get_mut(&f.id.id) {
                        assocs
                            .register_assoc(
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

        for (child_name, child_module) in &module.children {
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
}

#[derive(Debug)]
pub(crate) struct ImplCollector {
    next_impl_id: u32,
    pub(crate) impl_self_tys: HashMap<ImplId, TyKind>,
}

impl ImplCollector {
    fn new() -> Self {
        Self {
            next_impl_id: 0,
            impl_self_tys: HashMap::new(),
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
            ModuleNameTreeItem::Val(_) => {}
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
