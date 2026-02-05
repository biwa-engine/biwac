pub(crate) mod loader;
pub(crate) mod resolver;

use crate::loader::{PkgLoadError, PkgSymMap};
pub use crate::resolver::{
    AbsId,
    symbols::{
        expressions::{Exprs, Primary},
        globals::{FnDefContent, GlobalVarDec, TypeDefContent},
        statements::{IfStmt, Stmt, WhileStmt},
    },
    types::Typ,
};

#[derive(Debug, Clone)]
struct ModulePath(Vec<String>);

impl ModulePath {
    pub fn extend(self, child: String) -> Self {
        let mut path = self.0;
        path.push(child);

        Self(path)
    }
}

pub fn load(rootpath: &str) -> Result<PkgSymMap, PkgLoadError> {
    PkgSymMap::load(rootpath)
}
