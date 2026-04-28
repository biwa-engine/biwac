use biwac_hash::{AsHash64, Hash64};

use crate::PackageHashId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefPath {
    segments: Vec<DefPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefPathSegment {
    segment: String,
}

/// 128 bit definition path stable hash generated from [`PackageHashId`] and [`DefPath`]..
/// This is unique in global scope (inter-package) and inter-session,
/// so used in incremental compilation cache as global unique symbol id.
/// Uniqueness must be ensured by definition collector implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct DefPathHash {
    pkg_hash: PackageHashId,
    local_hash: Hash64,
}

impl DefPathHash {
    pub fn new(pkg_hash: PackageHashId, def_path: &DefPath) -> Self {
        Self {
            pkg_hash,
            local_hash: def_path.as_hash64(),
        }
    }
}

impl AsHash64 for DefPath {
    fn as_hash64(&self) -> Hash64 {
        todo!()
    }
}

impl DefPath {
    pub fn new(segments: Vec<String>) -> Self {
        Self {
            segments: segments.into_iter().map(DefPathSegment::new).collect(),
        }
    }
}

impl DefPathSegment {
    fn new(segment: String) -> Self {
        Self { segment }
    }
}
