use std::{cell::RefCell, collections::HashMap};

use biwac_ast::PathSegment;
use biwac_base::{InternedIdent, ModId, PackageId};
use biwac_hir::Ty;
use biwac_span::{TyDefId, ValDefId};

use crate::ResolveError;

#[derive(Debug)]
pub struct NameTree {
    pub(crate) self_pkg_name: InternedIdent,
    pub(crate) packages: HashMap<InternedIdent, PackageNameTree>,
}

#[derive(Debug)]
pub struct PackageNameTree {
    pub(crate) pkg_id: PackageId,
    pub(crate) root_module_tree: ModuleNameTree,
}

#[derive(Debug)]
pub struct ModuleNameTree {
    pub(crate) mod_id: ModId,
    pub(crate) children: HashMap<InternedIdent, ModuleNameTreeItem>,
}

#[derive(Debug)]
pub struct TyNameTree {
    pub(crate) def_id: TyDefId,
    /// Associated items (fns, types) registered via impl blocks, keyed by item name.
    pub(crate) children: RefCell<HashMap<InternedIdent, AssocNameTree>>,
    /// If this entry is a type alias, stores the canonical (chain-followed) non-alias TyDefId.
    pub(crate) alias_target: RefCell<Option<TyDefId>>,
}

#[derive(Debug)]
pub enum ModuleNameTreeItem {
    Mod(ModuleNameTree),
    Ty(TyNameTree),
    Val(ValDefId),
}

#[derive(Debug)]
pub struct AssocNameTree {
    pub(crate) assocs: Vec<AssocNameTreeItem>,
}

#[derive(Debug, Clone)]
pub struct AssocNameTreeItem {
    pub genargs: Vec<Ty>,
    pub kind: AssocNameTreeItemKind,
}

#[derive(Debug, Clone)]
pub enum AssocNameTreeItemKind {
    // TODO:
    // Ty(AssocNameTreeTyItem),
    Val(ValDefId),
}

impl AssocNameTree {
    pub(crate) fn find_matched(
        &self,
        genargs: Option<&[Ty]>,
        segment: &PathSegment,
    ) -> Result<&AssocNameTreeItemKind, ResolveError> {
        match genargs {
            Some(genargs) => {
                for item in &self.assocs {
                    if item.genargs.len() == genargs.len()
                        && item
                            .genargs
                            .iter()
                            .zip(genargs)
                            .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind))
                    {
                        return Ok(&item.kind);
                    }
                }

                Err(ResolveError::AssocItemNotFoundForGenArgs {
                    segment: segment.clone(),
                })
            }
            None => {
                if self.assocs.len() == 1 {
                    Ok(&self.assocs[0].kind)
                } else {
                    Err(ResolveError::AmbiguousAssocItem {
                        segment: segment.clone(),
                    })
                }
            }
        }
    }

    pub(crate) fn register_assoc(
        &mut self,
        genargs: Vec<Ty>,
        assoc: AssocNameTreeItemKind,
    ) -> Result<(), ResolveError> {
        for item in &self.assocs {
            if item.genargs.len() == genargs.len()
                && item
                    .genargs
                    .iter()
                    .zip(&genargs)
                    .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind))
            {
                return Err(ResolveError::DuplicatedAssociatedItemForGenArgs {
                    assoc1: item.clone(),
                    assoc2: AssocNameTreeItem {
                        genargs,
                        kind: assoc,
                    },
                });
            }
        }

        self.assocs.push(AssocNameTreeItem {
            genargs,
            kind: assoc,
        });

        Ok(())
    }
}
