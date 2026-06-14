use std::collections::{HashMap, hash_map::Entry};

use biwac_ast::{AbsolutePathHeader, Globals, ModAst, Path, PathSegmentResolution};
use biwac_base::InternedIdent;
use biwac_span::DefIdKind;

use crate::{
    ModuleNameTree, ModuleNameTreeItem, NameTree, ResolveError, resolving::context::ResolveCtx,
};

#[derive(Debug)]
pub struct ModuleResolveCtx<'t> {
    global_tree: &'t NameTree,
    self_pkg_name: InternedIdent,
    module: &'t ModuleNameTree,
    imports: HashMap<InternedIdent, &'t Path>,
}

impl<'t> ModuleResolveCtx<'t> {
    pub(crate) fn new(
        global_tree: &'t NameTree,
        self_pkg_name: InternedIdent,
        module_tree: &'t ModuleNameTree,
        module_ast: &'t ModAst,
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
            })
        } else {
            Err(errors)
        }
    }
}

impl ResolveCtx for ModuleResolveCtx<'_> {
    fn resolve_path(&self, path: &Path) -> Result<(), ResolveError> {
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

                resolve_path_in_module(path, 0, &self_package.root_module_tree)
            }

            // keyword `Self` can be used only in type definition or type implementation.
            Some(AbsolutePathHeader::SelfTyp(span)) => {
                Err(ResolveError::UnexpectedSelfType { span: span.clone() })
            }

            None => {
                let first_segment_ident = &path.segments[0].ident;
                match self.module.children.get(&first_segment_ident.id) {
                    Some(ModuleNameTreeItem::Mod(_))
                    | Some(ModuleNameTreeItem::Ty(_))
                    | Some(ModuleNameTreeItem::Val(_)) => Ok(()),
                    None => match self.imports.get(&first_segment_ident.id) {
                        Some(path) => self.resolve_path(path),
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
                                // returns Some(PackageId)
                                todo!()
                            } else {
                                resolve_path_in_module(path, 1, &package.root_module_tree)
                            }
                        }
                    },
                }
            }
        }
    }
}

/// `depth` means Path.segments[depth] should be resolved now.
/// Path.abs_header must have been resolved.
fn resolve_path_in_module(
    path: &Path,
    depth: usize,
    module: &ModuleNameTree,
) -> Result<(), ResolveError> {
    let segment = &path.segments[depth];
    match resolve_ident_in_module(&segment.ident.id, module) {
        Some(def_id_kind) => {
            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                resolve_path_in_module(path, depth + 1, module)
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

fn resolve_ident_in_module(name: &InternedIdent, module: &ModuleNameTree) -> Option<DefIdKind> {
    module.children.get(name).map(|item| match item {
        ModuleNameTreeItem::Mod(module) => DefIdKind::Mod(module.mod_id),
        ModuleNameTreeItem::Ty(ty) => DefIdKind::Ty(ty.def_id),
        ModuleNameTreeItem::Val(val_def_id) => DefIdKind::Val(*val_def_id),
    })
}
