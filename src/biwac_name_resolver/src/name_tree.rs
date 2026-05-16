use std::{cell::RefCell, collections::HashMap};

use biwac_base::{InternedIdent, ModId, PackageId};
use biwac_span::{DefIdKind, TyDefId, ValDefId};

#[derive(Debug)]
pub struct ModuleResolveCtx<'t> {
    pub(crate) global_tree: &'t PackageNameTree,
    pub(crate) mocule: &'t ModuleNameTree,
    pub(crate) imports: HashMap<InternedIdent, DefIdKind>,
}

#[derive(Debug)]
pub struct NameTree {
    pub(crate) packages: HashMap<InternedIdent, PackageNameTree>,
}

#[derive(Debug)]
pub struct PackageNameTree {
    pub(crate) pkg_id: PackageId,
    pub(crate) root_mod_id: ModId,
    pub(crate) children: HashMap<InternedIdent, ModuleNameTreeItem>,
}

#[derive(Debug)]
pub struct ModuleNameTree {
    pub(crate) mod_id: ModId,
    pub(crate) children: HashMap<InternedIdent, ModuleNameTreeItem>,
}

#[derive(Debug)]
pub struct TyNameTree {
    pub(crate) def_id: TyDefId,
    pub(crate) children: RefCell<HashMap<InternedIdent, AssocNameTreeItem>>,
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
