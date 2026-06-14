use biwac_base::InternedIdent;
use biwac_package_loader::{LoadedModule, Pkg};

use crate::{
    ModuleNameTree, NameTree, ResolveError,
    resolving::{
        context::{LocalResolveCtx, ResolveCtx, module_level::ModuleResolveCtx},
        def_collector::DefCollector,
    },
};

mod context;
pub(crate) mod def_collector;
mod symbols;

trait NameResolve<C: ResolveCtx> {
    fn resolve(&self, ctx: &C, def_collector: &mut DefCollector) -> Result<(), Vec<ResolveError>>;
}

trait LocalNameResolve<C: LocalResolveCtx> {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<ResolveError>>;
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
        def_collector,
    )
}

fn resolve_in_module(
    name_tree: &NameTree,
    pkg_name: InternedIdent,
    module_tree: &ModuleNameTree,
    module: &LoadedModule,
    def_collector: &mut DefCollector,
) -> Result<(), Vec<ResolveError>> {
    let ctx = ModuleResolveCtx::new(name_tree, pkg_name, module_tree, &module.ast)?;

    let mut errors = Vec::new();

    for g in &module.ast.globals {
        if let Some(Err(errs)) = match g {
            biwac_ast::Globals::FnDef(fn_def) => Some(fn_def.resolve(&ctx, def_collector)),
            biwac_ast::Globals::NativeFnDef(fn_def) => Some(fn_def.resolve(&ctx, def_collector)),
            biwac_ast::Globals::VarDecl(_var_decl) => {
                // TODO:
                todo!()
            }
            biwac_ast::Globals::Import(_) => None,
            biwac_ast::Globals::TypeDef(type_def) => match type_def {
                biwac_ast::TypeDef::Struct(struct_def) => {
                    Some(struct_def.resolve(&ctx, def_collector))
                }
                biwac_ast::TypeDef::TypeAlias(alias_def) => {
                    Some(alias_def.resolve(&ctx, def_collector))
                }
                biwac_ast::TypeDef::NativeTypeAlias(alias_def) => {
                    Some(alias_def.resolve(&ctx, def_collector))
                }
            },
            biwac_ast::Globals::NativeCode(_) => None,
            biwac_ast::Globals::NovelScene(scene_def) => {
                Some(scene_def.resolve(&ctx, def_collector))
            }
            biwac_ast::Globals::ImplBlock(impl_block) => {
                Some(impl_block.resolve(&ctx, def_collector))
            }
        } {
            errors.extend(errs);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
