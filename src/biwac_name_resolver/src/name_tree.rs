use std::{cell::RefCell, collections::HashMap};

use biwac_base::{InternedIdent, ModId, PackageId};
use biwac_span::{TyDefId, ValDefId};

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
    pub(crate) children: RefCell<HashMap<InternedIdent, AssocNameTreeItem>>,
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
pub enum AssocNameTreeItem {
    Ty { def_id: TyDefId },
    Val { def_id: ValDefId },
}
