pub(crate) mod loader;
pub(crate) mod resolver;

use crate::packager::loader::PackageSymbolMap;

#[derive(Debug, Clone)]
struct ModulePath(Vec<String>);

impl ModulePath {
    pub fn extend(self, child: String) -> Self {
        let mut path = self.0;
        path.push(child);

        Self(path)
    }
}

pub fn load(rootpath: &str) -> PackageSymbolMap {
    PackageSymbolMap::load(rootpath)
}
