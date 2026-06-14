use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::{AbsolutePathHeader, Globals, ModAst, Path, PathSegmentResolution};
use biwac_base::InternedIdent;
use biwac_span::{DefIdKind, TyDefId};

use crate::{
    ModuleNameTree, ModuleNameTreeItem, NameTree, ResolveError, TyNameTree,
    name_tree::AssocNameTreeItemKind, resolving::context::ResolveCtx,
};

#[derive(Debug)]
pub struct ModuleResolveCtx<'t> {
    global_tree: &'t NameTree,
    self_pkg_name: InternedIdent,
    module: &'t ModuleNameTree,
    imports: HashMap<InternedIdent, &'t Path>,
    ty_index: &'t HashMap<TyDefId, &'t TyNameTree>,
}

impl<'t> ModuleResolveCtx<'t> {
    pub(crate) fn new(
        global_tree: &'t NameTree,
        self_pkg_name: InternedIdent,
        module_tree: &'t ModuleNameTree,
        module_ast: &'t ModAst,
        ty_index: &'t HashMap<TyDefId, &'t TyNameTree>,
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

            Some(AbsolutePathHeader::SelfTyp(span)) => {
                Err(ResolveError::UnexpectedSelfType { span: span.clone() })
            }

            None => {
                let first_segment_ident = &path.segments[0].ident;
                if self.module.children.contains_key(&first_segment_ident.id) {
                    // Relative path found in current module - resolve properly.
                    resolve_path_in_module(path, 0, self.module, self.ty_index)
                } else {
                    match self.imports.get(&first_segment_ident.id) {
                        Some(import_path) => self.resolve_path(import_path),
                        None => {
                            let package =
                                match self.global_tree.packages.get(&first_segment_ident.id) {
                                    Some(package) => package,
                                    None => {
                                        path.segments[0]
                                            .resolved_id
                                            .set(PathSegmentResolution::Err)
                                            .unwrap();
                                        return Err(ResolveError::IdentNotFound {
                                            ident: first_segment_ident.clone(),
                                        });
                                    }
                                };

                            if path.segments.len() == 1 {
                                // Returns Some(PackageId) — not yet supported.
                                todo!()
                            } else {
                                resolve_path_in_module(
                                    path,
                                    1,
                                    &package.root_module_tree,
                                    self.ty_index,
                                )
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
            let def_id_kind = module_item_to_def_id_kind(item, ty_index);
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
            let def_id_kind = match assoc_tree.find_matched(None, segment)? {
                AssocNameTreeItemKind::Val(def_id) => DefIdKind::Val(*def_id),
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

fn module_item_to_def_id_kind(
    item: &ModuleNameTreeItem,
    ty_index: &HashMap<TyDefId, &TyNameTree>,
) -> DefIdKind {
    match item {
        ModuleNameTreeItem::Mod(module) => DefIdKind::Mod(module.mod_id),
        ModuleNameTreeItem::Ty(ty) => {
            if let Some(def_id) = ty_index
                .get(&ty.def_id)
                .and_then(|ty_def| *ty_def.alias_target.borrow())
            {
                DefIdKind::Ty(def_id)
            } else {
                DefIdKind::Ty(ty.def_id)
            }
        }
        ModuleNameTreeItem::Val(val_def_id) => DefIdKind::Val(*val_def_id),
    }
}
