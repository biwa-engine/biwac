use std::collections::HashMap;

use crate::ModPath;

pub struct SourceHolder {
    mods: HashMap<FileId, ModSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(usize);

pub struct ModSource {
    modu: ModPath,
    src: String,
}

impl FileId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}
