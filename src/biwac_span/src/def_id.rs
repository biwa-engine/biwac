use std::hash::Hash;

use biwac_base::{ModId, PackageId};

/// 64 bit definition id constructed from [`PackageId`] and [`PackageLocalDefId`].
/// This is unique in global scope (inter-package) and inter-session,
/// so used in incremental compilation cache as global unique symbol id.
/// Uniqueness must be ensured by definition collector implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct DefId {
    pkg: PackageId,
    local: PackageLocalDefId,
}

impl DefId {
    #[inline]
    pub fn new(pkg: PackageId, local: PackageLocalDefId) -> Self {
        Self { pkg, local }
    }

    #[inline]
    pub fn new_in_self_pkg(local: PackageLocalDefId) -> Self {
        Self {
            pkg: PackageId::SELF_PACKAGE,
            local,
        }
    }

    #[inline]
    pub fn pkg(&self) -> PackageId {
        self.pkg
    }
}

impl Hash for DefId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // `local` has higher entropy than `pkg`.
        self.local.hash(state);
        self.pkg.hash(state);
    }
}

/// 32 bit local definition id (simple increment).
/// This is unique inter session.
/// Uniqueness must be ensured by definition collector implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageLocalDefId(u32);

impl PackageLocalDefId {
    #[inline]
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyDefId(DefId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValDefId(DefId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefIdKind {
    Mod(ModId),
    Ty(TyDefId),
    Val(ValDefId),
}

impl TyDefId {
    #[inline]
    pub fn new(def_id: DefId) -> Self {
        Self(def_id)
    }

    #[inline]
    pub fn pkg(&self) -> PackageId {
        self.0.pkg
    }
}

impl ValDefId {
    #[inline]
    pub fn new(def_id: DefId) -> Self {
        Self(def_id)
    }

    #[inline]
    pub fn pkg(&self) -> PackageId {
        self.0.pkg
    }
}
