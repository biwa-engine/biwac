use biwac_base::InternedIdent;
use biwac_package_loader::{LoadedModule, Pkg};

use crate::{DefCollector, ModuleNameTree, NameTree, ResolveError, context::ResolveCtx};

mod expressions;
mod globals;
mod novel;
mod statements;

trait NameResolve<C: ResolveCtx> {
    fn resolve(&self, ctx: &C, def_collector: &mut DefCollector) -> Result<(), Vec<ResolveError>>;
}

pub(crate) fn resolve_in_self_package(
    pkg: &Pkg,
    name_tree: &NameTree,
    def_collector: &mut DefCollector,
) -> Result<(), Vec<ResolveError>> {
    let root_module_tree = &name_tree
        .packages
        .get(&name_tree.self_pkg_name)
        .unwrap()
        .root_module_tree;

    resolve_in_module(
        name_tree,
        name_tree.self_pkg_name,
        root_module_tree,
        &pkg.root_module,
    )
}

fn resolve_in_module(
    name_tree: &NameTree,
    pkg_name: InternedIdent,
    module_tree: &ModuleNameTree,
    module: &LoadedModule,
) -> Result<(), Vec<ResolveError>> {
    todo!()
}
