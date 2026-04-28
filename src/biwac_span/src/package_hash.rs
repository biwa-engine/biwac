use biwac_base::{PackageName, PackageVersion};
use biwac_hash::Hash64;

/// Package stable hash generated from [`PackageName`] and [`PackageVersion`].
/// This is unique in global scope (inter-package) and inter-session.
/// Uniqueness must be ensured by depended package collector implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageHashId(Hash64);

impl PackageHashId {
    pub fn new(pkg_name: &PackageName, pkg_version: &PackageVersion) -> Self {
        todo!()
    }
}
