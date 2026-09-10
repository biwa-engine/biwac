use std::collections::HashMap;

use biwac_base::{IdentInterner, InternedIdent, ModId};
use biwac_hir::TyTraitImpl;
use biwac_package_loader::{LoadedModule, Pkg};
use biwac_span::{TraitAssocDefId, TraitDefId, TyDefId};

use crate::{
    ModuleNameTree, NameTree, ResolveError, ResolveErrorHandler, TyNameTree,
    resolving::{
        context::{LocalResolveCtx, ResolveCtx, module_level::ModuleResolveCtx},
        def_collector::{DefCollector, collect_mod_trees, collect_ty_trees},
    },
};

mod context;
pub(crate) mod def_collector;
pub(crate) mod lang_item_collector;
mod symbols;

trait NameResolve<C: ResolveCtx> {
    fn resolve(&self, ctx: &C, def_collector: &mut DefCollector) -> Result<(), Vec<ResolveError>>;
}

trait LocalNameResolve<C: LocalResolveCtx> {
    fn resolve(&self, ctx: &mut C) -> Result<(), Vec<ResolveError>>;
}

/// 自パッケージの名前解決を行い、モジュールごとの trait スコープを返す。
///
/// スコープは HIR に持ち越す。
/// 型推論のメソッド解決が import 規則を判断するのに要るためである。
pub(crate) fn resolve_in_self_package(
    pkg: &Pkg,
    name_tree: &NameTree,
    def_collector: &mut DefCollector,
    interner: &IdentInterner,
) -> Result<HashMap<ModId, Vec<TraitDefId>>, Vec<ResolveError>> {
    let root_module_tree = &name_tree
        .packages
        .get(&name_tree.self_pkg_name)
        .unwrap()
        .root_module_tree;

    // Build TyDefId -> &TyNameTree and ModId -> &ModuleNameTree indexes.
    let mut ty_index: HashMap<TyDefId, &TyNameTree> = HashMap::new();
    collect_ty_trees(root_module_tree, &mut ty_index);
    let mut mod_index: HashMap<ModId, &ModuleNameTree> = HashMap::new();
    collect_mod_trees(root_module_tree, &mut mod_index);

    // 表はここで完成している。
    // `def_collector` は下で可変借用するので、複製を取って持ち回る。
    let trait_impls = def_collector.impl_collector.trait_impls.clone();
    let trait_items = def_collector.trait_items.clone();
    let mut trait_scopes: HashMap<ModId, Vec<TraitDefId>> = HashMap::new();

    resolve_in_module(
        name_tree,
        name_tree.self_pkg_name,
        root_module_tree,
        &pkg.root_module,
        def_collector,
        &ty_index,
        &mod_index,
        interner,
        &trait_impls,
        &trait_items,
        &mut trait_scopes,
    )?;

    Ok(trait_scopes)
}

#[allow(clippy::too_many_arguments)]
fn resolve_in_module(
    name_tree: &NameTree,
    pkg_name: InternedIdent,
    module_tree: &ModuleNameTree,
    module: &LoadedModule,
    def_collector: &mut DefCollector,
    ty_index: &HashMap<TyDefId, &TyNameTree>,
    mod_index: &HashMap<ModId, &ModuleNameTree>,
    interner: &IdentInterner,
    trait_impls: &HashMap<TyDefId, Vec<TyTraitImpl>>,
    trait_items: &HashMap<TraitDefId, Vec<(InternedIdent, TraitAssocDefId)>>,
    trait_scopes: &mut HashMap<ModId, Vec<TraitDefId>>,
) -> Result<(), Vec<ResolveError>> {
    let ctx = ModuleResolveCtx::new(
        name_tree,
        pkg_name,
        module_tree,
        &module.ast,
        ty_index,
        mod_index,
        interner,
    )?
    .with_trait_impls(trait_impls, trait_items);

    let mut errors = Vec::new();

    // trait のフォールバックより先に、このモジュールのスコープを決める。
    trait_scopes.insert(module.mod_id, ctx.prepare_trait_scope(&mut errors));

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
                biwac_ast::TypeDef::Enum(enum_def) => Some(enum_def.resolve(&ctx, def_collector)),
                biwac_ast::TypeDef::TypeAlias(alias_def) => {
                    Some(alias_def.resolve(&ctx, def_collector))
                }
                biwac_ast::TypeDef::NativeTypeAlias(alias_def) => {
                    Some(alias_def.resolve(&ctx, def_collector))
                }
            },
            biwac_ast::Globals::TraitDef(trait_def) => Some(trait_def.resolve(&ctx, def_collector)),
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

    for (module_name, module) in module.children_ordered() {
        match module_tree.children.get(module_name).unwrap() {
            crate::ModuleNameTreeItem::Mod(module_tree) => {
                resolve_in_module(
                    name_tree,
                    pkg_name,
                    module_tree,
                    module,
                    def_collector,
                    ty_index,
                    mod_index,
                    interner,
                    trait_impls,
                    trait_items,
                    trait_scopes,
                )
                .handle(&mut errors);
            }
            _ => panic!("compiler bug: module name tree item expected module but another found"),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
