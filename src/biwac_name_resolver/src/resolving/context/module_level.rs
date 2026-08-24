use std::{
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
};

use biwac_ast::{AbsolutePathHeader, Globals, ModAst, Path, PathSegmentResolution};
use biwac_base::{IdentInterner, InternedIdent, ModId, PackageId};
use biwac_dependency_metadata::{
    DepMetadataModuleView, ExternalChildKind, ExternalChildRef, PackageModuleView,
};
use biwac_span::{DefIdKind, TyDefId};

use crate::{
    ModuleNameTree, ModuleNameTreeItem, NameTree, ResolveError, TyNameTree,
    lowering::def_id_kind_from_path, name_tree::AssocNameTreeItemKind,
    resolving::context::ResolveCtx,
};

#[derive(Debug)]
pub struct ModuleResolveCtx<'t> {
    global_tree: &'t NameTree,
    self_pkg_name: InternedIdent,
    module: &'t ModuleNameTree,
    imports: HashMap<InternedIdent, &'t Path>,
    ty_index: &'t HashMap<TyDefId, &'t TyNameTree>,
    mod_index: &'t HashMap<ModId, &'t ModuleNameTree>,
    interner: &'t IdentInterner,
}

impl<'t> ModuleResolveCtx<'t> {
    pub(crate) fn new(
        global_tree: &'t NameTree,
        self_pkg_name: InternedIdent,
        module_tree: &'t ModuleNameTree,
        module_ast: &'t ModAst,
        ty_index: &'t HashMap<TyDefId, &'t TyNameTree>,
        mod_index: &'t HashMap<ModId, &'t ModuleNameTree>,
        interner: &'t IdentInterner,
    ) -> Result<Self, Vec<ResolveError>> {
        let mut imports = HashMap::new();
        let mut errors = Vec::new();
        for g in &module_ast.globals {
            if let Globals::Import(import_decl) = g {
                let imported_ident = &import_decl.path.segments.last().unwrap().ident;
                match imports.entry(imported_ident.id) {
                    Entry::Vacant(e) => {
                        e.insert(&import_decl.path);
                    }
                    Entry::Occupied(e) => {
                        errors.push(ResolveError::DuplicatedSymbolName {
                            name: imported_ident.id,
                            span1: import_decl.path.segments.last().unwrap().span(),
                            span2: e.get().segments.last().unwrap().span(),
                        });
                    }
                }

                if let Some(item) = module_tree.children.get(&imported_ident.id) {
                    errors.push(ResolveError::DuplicatedSymbolAndDefIdName {
                        name: imported_ident.id,
                        span: imported_ident.span.clone(),
                        def_id_kind: match item {
                            ModuleNameTreeItem::Mod(module) => DefIdKind::Mod(module.mod_id),
                            ModuleNameTreeItem::Ty(ty) => DefIdKind::Ty(ty.def_id),
                            ModuleNameTreeItem::Val(val_def_id) => DefIdKind::Val(*val_def_id),
                        },
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(Self {
                global_tree,
                self_pkg_name,
                module: module_tree,
                imports,
                ty_index,
                mod_index,
                interner,
            })
        } else {
            Err(errors)
        }
    }
}

impl ResolveCtx for ModuleResolveCtx<'_> {
    fn resolve_path(&self, path: &Path) -> Result<(), ResolveError> {
        // If already resolved, fast path.
        for (i, segment) in path.segments.iter().enumerate() {
            match segment.resolved_id.get() {
                Some(PathSegmentResolution::Ok(_)) => {
                    if i + 1 == path.segments.len() {
                        return Ok(());
                    } else {
                        continue;
                    }
                }
                Some(PathSegmentResolution::Err) => {
                    return Err(ResolveError::PathResolutionFailed {
                        path: Box::new(path.clone()),
                    });
                }
                None => {
                    break;
                }
            }
        }

        match &path.abs_header {
            Some(AbsolutePathHeader::Package(_)) => {
                let self_package = &self.global_tree.packages.get(&self.self_pkg_name).unwrap();
                resolve_path_in_module(path, 0, &self_package.root_module_tree, self.ty_index)
            }

            Some(AbsolutePathHeader::SelfTyp(self_typ)) => Err(ResolveError::UnexpectedSelfType {
                span: self_typ.span.clone(),
            }),

            None => {
                let first_segment_ident = &path.segments[0].ident;
                if self.module.children.contains_key(&first_segment_ident.id) {
                    // Relative path found in current module - resolve properly.
                    resolve_path_in_module(path, 0, self.module, self.ty_index)
                } else {
                    match self.imports.get(&first_segment_ident.id) {
                        Some(import_path) => match self.resolve_path(import_path) {
                            Ok(()) => {
                                let imported_kind = def_id_kind_from_path(import_path).unwrap();
                                path.segments[0]
                                    .resolved_id
                                    .set(PathSegmentResolution::Ok(imported_kind.clone()))
                                    .unwrap();

                                if path.segments.len() == 1 {
                                    return Ok(());
                                }

                                // Dispatch segment[1..] based on what the import resolved to.
                                match imported_kind {
                                    DefIdKind::Ty(ty_id) => {
                                        if ty_id.pkg().is_self() {
                                            match self.ty_index.get(&ty_id) {
                                                Some(ty_tree) => resolve_path_in_ty(
                                                    path,
                                                    1,
                                                    ty_tree,
                                                    self.ty_index,
                                                ),
                                                None => {
                                                    path.segments[1]
                                                        .resolved_id
                                                        .set(PathSegmentResolution::Err)
                                                        .unwrap();
                                                    Err(ResolveError::PathResolutionFailed {
                                                        path: Box::new(path.clone()),
                                                    })
                                                }
                                            }
                                        } else {
                                            // 外部パッケージの型: assoc アイテム解決
                                            let pkg_id = ty_id.pkg();
                                            match self.global_tree.ext_pkg_data.get(&pkg_id) {
                                                Some(dep_arc) => {
                                                    let view =
                                                        DepMetadataModuleView::new_for_sym_idx(
                                                            Arc::clone(dep_arc),
                                                            ty_id.local_idx(),
                                                            pkg_id,
                                                        );
                                                    resolve_path_in_ext_ty(
                                                        path,
                                                        1,
                                                        ty_id.local_idx(),
                                                        &view,
                                                        pkg_id,
                                                        self.interner,
                                                    )
                                                }
                                                None => {
                                                    path.segments[1]
                                                        .resolved_id
                                                        .set(PathSegmentResolution::Err)
                                                        .unwrap();
                                                    Err(ResolveError::PathResolutionFailed {
                                                        path: Box::new(path.clone()),
                                                    })
                                                }
                                            }
                                        }
                                    }
                                    DefIdKind::Mod(mod_id) => {
                                        if mod_id.is_self_pkg() {
                                            match self.mod_index.get(&mod_id) {
                                                Some(mod_tree) => resolve_path_in_module(
                                                    path,
                                                    1,
                                                    mod_tree,
                                                    self.ty_index,
                                                ),
                                                None => {
                                                    path.segments[1]
                                                        .resolved_id
                                                        .set(PathSegmentResolution::Err)
                                                        .unwrap();
                                                    Err(ResolveError::PathResolutionFailed {
                                                        path: Box::new(path.clone()),
                                                    })
                                                }
                                            }
                                        } else {
                                            // 外部パッケージのモジュール: PackageModuleView 経由で解決
                                            let pkg_id = PackageId::new(mod_id.pkg_id_bits());
                                            let sym_idx = mod_id.sym_idx();
                                            match self.global_tree.ext_pkg_data.get(&pkg_id) {
                                                Some(dep_arc) => {
                                                    let sub_view =
                                                        DepMetadataModuleView::new_for_sym_idx(
                                                            Arc::clone(dep_arc),
                                                            sym_idx,
                                                            pkg_id,
                                                        );
                                                    resolve_path_in_ext_pkg(
                                                        path,
                                                        1,
                                                        &sub_view,
                                                        pkg_id,
                                                        self.interner,
                                                    )
                                                }
                                                None => {
                                                    path.segments[1]
                                                        .resolved_id
                                                        .set(PathSegmentResolution::Err)
                                                        .unwrap();
                                                    Err(ResolveError::PathResolutionFailed {
                                                        path: Box::new(path.clone()),
                                                    })
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        path.segments[1]
                                            .resolved_id
                                            .set(PathSegmentResolution::Err)
                                            .unwrap();
                                        Err(ResolveError::PathResolutionFailed {
                                            path: Box::new(path.clone()),
                                        })
                                    }
                                }
                            }
                            Err(e) => {
                                path.segments[0]
                                    .resolved_id
                                    .set(PathSegmentResolution::Err)
                                    .unwrap();

                                Err(e)
                            }
                        },
                        None => {
                            if let Some(package) =
                                self.global_tree.packages.get(&first_segment_ident.id)
                            {
                                if path.segments.len() == 1 {
                                    path.segments[0]
                                        .resolved_id
                                        .set(PathSegmentResolution::Ok(DefIdKind::Package(
                                            package.pkg_id,
                                        )))
                                        .unwrap();

                                    Ok(())
                                } else {
                                    resolve_path_in_module(
                                        path,
                                        1,
                                        &package.root_module_tree,
                                        self.ty_index,
                                    )
                                }
                            } else if let Some(view) =
                                self.global_tree.ext_pkg_views.get(&first_segment_ident.id)
                            {
                                let pkg_id = view.pkg_id();
                                path.segments[0]
                                    .resolved_id
                                    .set(PathSegmentResolution::Ok(DefIdKind::Package(pkg_id)))
                                    .unwrap();
                                if path.segments.len() == 1 {
                                    Ok(())
                                } else {
                                    resolve_path_in_ext_pkg(
                                        path,
                                        1,
                                        view.as_ref(),
                                        pkg_id,
                                        self.interner,
                                    )
                                }
                            } else {
                                path.segments[0]
                                    .resolved_id
                                    .set(PathSegmentResolution::Err)
                                    .unwrap();
                                Err(ResolveError::IdentNotFound {
                                    ident: first_segment_ident.clone(),
                                })
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Resolves path starting at `depth` within a module tree.
/// Sets `resolved_id` on each path segment and navigates into child modules or type children.
fn resolve_path_in_module(
    path: &Path,
    depth: usize,
    module: &ModuleNameTree,
    ty_index: &HashMap<TyDefId, &TyNameTree>,
) -> Result<(), ResolveError> {
    let segment = &path.segments[depth];
    match module.children.get(&segment.ident.id) {
        Some(item) => {
            let def_id_kind = module_item_to_def_id_kind(item);
            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                match item {
                    ModuleNameTreeItem::Mod(child_module) => {
                        resolve_path_in_module(path, depth + 1, child_module, ty_index)
                    }
                    ModuleNameTreeItem::Ty(ty_tree) => {
                        resolve_path_in_ty(path, depth + 1, ty_tree, ty_index)
                    }
                    ModuleNameTreeItem::Val(_) => {
                        path.segments[depth + 1]
                            .resolved_id
                            .set(PathSegmentResolution::Err)
                            .unwrap();
                        Err(ResolveError::PathResolutionFailed {
                            path: Box::new(path.clone()),
                        })
                    }
                }
            }
        }
        None => {
            segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
            Err(ResolveError::PathResolutionFailed {
                path: Box::new(path.clone()),
            })
        }
    }
}

/// Resolves path starting at `depth` within a type's associated items.
/// Follows alias_target if the type is an alias.
fn resolve_path_in_ty(
    path: &Path,
    depth: usize,
    ty_tree: &TyNameTree,
    ty_index: &HashMap<TyDefId, &TyNameTree>,
) -> Result<(), ResolveError> {
    // Follow alias chain to find the canonical type's children.
    let canonical_tree = if let Some(canonical_id) = *ty_tree.alias_target.borrow() {
        ty_index.get(&canonical_id).copied().unwrap_or(ty_tree)
    } else {
        ty_tree
    };

    let segment = &path.segments[depth];
    let children = canonical_tree.children.borrow();
    match children.get(&segment.ident.id) {
        Some(assoc_tree) => {
            // TODO: segment に genargs: Option<Vec<TypRepr>> を持たせて解決
            let def_id_kind = match assoc_tree.find_matched(None, segment) {
                Ok(AssocNameTreeItemKind::Val(def_id)) => DefIdKind::Val(*def_id),
                Err(e) => {
                    segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
                    return Err(e);
                }
            };

            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                path.segments[depth + 1]
                    .resolved_id
                    .set(PathSegmentResolution::Err)
                    .unwrap();
                Err(ResolveError::PathResolutionFailed {
                    path: Box::new(path.clone()),
                })
            }
        }
        None => {
            segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
            Err(ResolveError::PathResolutionFailed {
                path: Box::new(path.clone()),
            })
        }
    }
}

fn module_item_to_def_id_kind(item: &ModuleNameTreeItem) -> DefIdKind {
    match item {
        ModuleNameTreeItem::Mod(module) => DefIdKind::Mod(module.mod_id),
        // 型の位置では alias を canonical な型に潰さない。
        //
        // 潰すと `type MyGame = Game[A, B]` の [A, B] が失われてしまう
        // (ここは TyDefId しか運べないため)。
        // alias 自身の TyDefId のまま HIR まで運び、
        // lowering の最後で alias_expansion が右辺ごと置き換える。
        //
        // 一方、関連アイテムの解決 (resolve_path_in_ty) は
        // `PairIntT::new` を `Pair::new` に解決する必要があるので
        // 引き続き alias_target を辿る。
        ModuleNameTreeItem::Ty(ty) => DefIdKind::Ty(ty.def_id),
        ModuleNameTreeItem::Val(val_def_id) => DefIdKind::Val(*val_def_id),
    }
}

// ============================================================
// 外部パッケージ解決ヘルパー
// ============================================================

/// 外部パッケージ内のモジュールをパスで辿る。
/// `depth` = path.segments 内の開始インデックス (パッケージ名セグメントの次)。
fn resolve_path_in_ext_pkg(
    path: &Path,
    depth: usize,
    view: &dyn PackageModuleView,
    pkg_id: PackageId,
    interner: &IdentInterner,
) -> Result<(), ResolveError> {
    let segment = &path.segments[depth];
    match view.lookup_child(segment.ident.id, interner) {
        None => {
            segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
            Err(ResolveError::PathResolutionFailed {
                path: Box::new(path.clone()),
            })
        }
        Some(child_ref) => {
            let def_id_kind = ext_child_ref_to_def_id_kind(&child_ref, pkg_id);
            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                match child_ref.kind {
                    ExternalChildKind::Mod => {
                        let sub_view = view.get_module_view(child_ref.sym_idx);
                        resolve_path_in_ext_pkg(
                            path,
                            depth + 1,
                            sub_view.as_ref(),
                            pkg_id,
                            interner,
                        )
                    }
                    ExternalChildKind::Ty => resolve_path_in_ext_ty(
                        path,
                        depth + 1,
                        child_ref.sym_idx,
                        view,
                        pkg_id,
                        interner,
                    ),
                    ExternalChildKind::Val => {
                        path.segments[depth + 1]
                            .resolved_id
                            .set(PathSegmentResolution::Err)
                            .unwrap();
                        Err(ResolveError::PathResolutionFailed {
                            path: Box::new(path.clone()),
                        })
                    }
                }
            }
        }
    }
}

/// 外部パッケージの型の assoc アイテムをパスで解決する。
/// `local_ty_idx` = その型のシンボルインデックス。
fn resolve_path_in_ext_ty(
    path: &Path,
    depth: usize,
    local_ty_idx: u32,
    view: &dyn PackageModuleView,
    pkg_id: PackageId,
    interner: &IdentInterner,
) -> Result<(), ResolveError> {
    let segment = &path.segments[depth];
    match view.lookup_assoc(local_ty_idx, segment.ident.id, interner) {
        None => {
            segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
            Err(ResolveError::PathResolutionFailed {
                path: Box::new(path.clone()),
            })
        }
        Some(child_ref) => {
            let def_id_kind = ext_child_ref_to_def_id_kind(&child_ref, pkg_id);
            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                // assoc アイテムの先をさらに辿ることは現時点でサポートしない
                path.segments[depth + 1]
                    .resolved_id
                    .set(PathSegmentResolution::Err)
                    .unwrap();
                Err(ResolveError::PathResolutionFailed {
                    path: Box::new(path.clone()),
                })
            }
        }
    }
}

/// `ExternalChildRef` を `DefIdKind` に変換する。
/// 外部モジュール (`Mod`) は `ModId::new_ext` でエンコードした `DefIdKind::Mod` として記録する。
fn ext_child_ref_to_def_id_kind(child_ref: &ExternalChildRef, pkg_id: PackageId) -> DefIdKind {
    match child_ref.kind {
        ExternalChildKind::Ty => DefIdKind::Ty(child_ref.as_ty_def_id(pkg_id)),
        ExternalChildKind::Val => DefIdKind::Val(child_ref.as_val_def_id(pkg_id)),
        ExternalChildKind::Mod => DefIdKind::Mod(ModId::new_ext(pkg_id.value(), child_ref.sym_idx)),
    }
}
