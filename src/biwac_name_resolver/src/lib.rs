mod error;
mod lowering;
mod name_tree;
mod resolving;

pub use name_tree::{
    AssocNameTreeItem, ModuleNameTree, ModuleNameTreeItem, NameTree, PackageNameTree, TyNameTree,
};

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use biwac_base::{InternedIdent, PackageName};

use biwac_hir::Hir;
use biwac_package_loader::Pkg;

use crate::resolving::resolve_in_self_package;

// このcrate biwac_name_resolver は、
// package内のあらゆる名前の解決をすることを目指す。
// - 関数や型といったグローバルなシンボルはもちろん、
// - 関数ローカルな変数の名前、
// - ~~構造体のメンバの名前に至るまで、~~
//
// - 同一の名前空間で名前の重複がないこと、
// - 名前空間の優先度に従って、その名前のシンボルの存在を確認し、絶対的な参照名に変換すること
// - モジュールレベルの公開設定に違反せずに、名前解決が出来ること
//   - ただし、構造体のメンバは型推論して始めて構造体の種類が特定できるなどの理由から
//     このパスでは行わないこととする
//     型の内容のチェックは次のパスに任せる
// のすべてを満たしつつ解決を目指す。

pub use error::ResolveError;

trait ResolveErrorHandler {
    fn handle(self, errors: &mut Vec<ResolveError>);
}

impl<T> ResolveErrorHandler for Result<T, Vec<ResolveError>> {
    fn handle(self, errors: &mut Vec<ResolveError>) {
        if let Err(errs) = self {
            errors.extend(errs);
        }
    }
}

impl<T> ResolveErrorHandler for Result<T, ResolveError> {
    fn handle(self, errors: &mut Vec<ResolveError>) {
        if let Err(e) = self {
            errors.push(e);
        }
    }
}

pub struct NameResolver {
    pkg: Pkg,
    pkg_name: InternedIdent,
    pkg_package_name: PackageName,
}

impl NameResolver {
    pub fn new(
        metadata: &biwac_base::MetadataHolder,
        deps: &biwac_dependency_loader::Deps,
        pkg_name: InternedIdent,
        pkg: Pkg,
    ) -> Result<Self, ResolveError> {
        let pkg_package_name = metadata.metadata.name.clone();
        Ok(Self {
            pkg,
            pkg_name,
            pkg_package_name,
        })
    }

    pub fn try_resolve(self) -> Result<Hir, Vec<ResolveError>> {
        let external_package_trees = HashMap::new();

        // definition collection (package internal)
        let mut def_collector = resolving::def_collector::DefCollector::new();
        let name_tree = def_collector.collect(self.pkg_name, &self.pkg, external_package_trees)?;

        // TODO: cache on disk
        // def_collector
        // name_tree
        // Even if name resolution failed,
        // def_collector and name_tree must be cached
        // because symbol definition is not affected by name resolution result.

        // symbol resolution (package internal)
        resolve_in_self_package(&self.pkg, &name_tree, &mut def_collector)?;

        // TODO: cache on disk
        // symbol signature

        // lowering to HIR
        lowering::lower(self.pkg_package_name, self.pkg)
    }
}
