use std::collections::HashMap;

use crate::ModPath;

#[derive(Debug)]
pub struct SourceHolder {
    pub mods: HashMap<FileId, ModSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(usize);

#[derive(Debug)]
pub struct ModSource {
    pub modu: ModPath,
    pub src: String,
}

impl FileId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}
