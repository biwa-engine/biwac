use std::collections::HashMap;

use crate::ModPath;

#[derive(Debug, Default)]
pub struct SourceHolder {
    pub mods: HashMap<ModId, ModSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModId(usize);

#[derive(Debug)]
pub struct ModSource {
    pub modu: ModPath,
    pub src: String,
}

impl ModId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }
}
