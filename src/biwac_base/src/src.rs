use std::collections::HashMap;

use crate::{ModPath, PackageId};

#[derive(Debug, Default)]
pub struct SourceHolder {
    pub mods: HashMap<ModId, ModSource>,
}

/// [`ModId`] is global scope (inter-package) module id.
/// Incremental compilation cache keeps map of
/// ModId to enum {
///     SelfPkg,
///     External {
///         pkg: PackageName,
///         module: ModPath,
///     }
/// }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModId(usize);

#[derive(Debug)]
pub struct ModSource {
    pub pkg_id: PackageId,
    pub modu: ModPath,
    pub src: String,
}

impl ModId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}
