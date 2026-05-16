use std::{
    cell::RefCell,
    collections::{HashMap, hash_map::Entry},
};

use biwac_base::{InternedIdent, ModId, PackageId};
use biwac_package_loader::{LoadedModule, Pkg};
use biwac_span::{DefId, PackageLocalDefId, Span, TyDefId, ValDefId};

use crate::{ModuleNameTree, ModuleNameTreeItem, NameTree, PackageNameTree, TyNameTree};

pub(crate) enum TyOrVal<T, V> {
    Ty(T),
    Val(V),
}
pub(crate) enum DefCollectError {
    // module 側から既存の symbol との重複を検知した場合
    DuplicatedSymbolAndModuleName {
        name: InternedIdent,
        mod_id: ModId,
        symbol_span: Span,
    },

    // module 内で symbol 同士の重複を検知した場合
    DuplicatedSymbolName {
        name: InternedIdent,
        span1: Span,
        span2: Span,
    },
}

/// DefCollector
/// collects definitions in self package.
pub struct DefCollector {
    next_pkg_local_def_id: u32,
}

impl DefCollector {
    pub fn new() -> Self {
        Self {
            next_pkg_local_def_id: 0,
        }
    }

    fn alloc_def_id(&mut self) -> DefId {
        let pkg_local_def_id = PackageLocalDefId::new(self.next_pkg_local_def_id);
        self.next_pkg_local_def_id += 1;

        DefId::new_in_self_pkg(pkg_local_def_id)
    }

    pub fn collect(
        &mut self,
        pkg_name: InternedIdent,
        pkg_id: PackageId,
        pkg: &Pkg,
        external_package_trees: HashMap<InternedIdent, PackageNameTree>,
    ) -> Result<NameTree, Vec<DefCollectError>> {
        let root_module_tree = self.collect_in_module(&pkg.root_module)?;
        let package_tree = PackageNameTree {
            pkg_id,
            root_mod_id: root_module_tree.mod_id,
            children: root_module_tree.children,
        };

        let mut packages = external_package_trees;
        packages.insert(pkg_name, package_tree);
        let name_tree = NameTree { packages };

        // TODO: collect impls

        self.collect_impls(&pkg_name, &name_tree)?;

        Ok(name_tree)
    }

    // NOTE: impl-block はimpl対象の型を名前解決する必要があるため、
    // ここでは処理できない
    fn collect_in_module(
        &mut self,
        module: &LoadedModule,
    ) -> Result<ModuleNameTree, Vec<DefCollectError>> {
        let mut children = HashMap::<InternedIdent, (ModuleNameTreeItem, Span)>::new();
        let mut errors = Vec::new();

        for g in &module.ast.globals {
            let opt_ident_and_def_id = match g {
                biwac_ast::Globals::FnDef(fn_def) => {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    fn_def.def_id.set(def_id).unwrap();

                    Some((fn_def.id.clone(), TyOrVal::Val(def_id)))
                }
                biwac_ast::Globals::NativeFnDef(fn_def) => {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    fn_def.def_id.set(def_id).unwrap();

                    Some((fn_def.id.clone(), TyOrVal::Val(def_id)))
                }
                biwac_ast::Globals::MethodDef(method_def) => {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    method_def.def_id.set(def_id).unwrap();

                    Some((method_def.id.clone(), TyOrVal::Val(def_id)))
                }
                biwac_ast::Globals::NativeMethodDef(method_def) => {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    method_def.def_id.set(def_id).unwrap();

                    Some((method_def.id.clone(), TyOrVal::Val(def_id)))
                }
                biwac_ast::Globals::VarDecl(_var_decl) => {
                    // TODO:
                    None
                }
                biwac_ast::Globals::Import(_) => None,
                biwac_ast::Globals::TypeDef(type_def) => match type_def {
                    biwac_ast::TypeDef::Struct(struct_def) => {
                        let def_id = TyDefId::new(self.alloc_def_id());
                        struct_def.def_id.set(def_id).unwrap();

                        Some((struct_def.id.clone(), TyOrVal::Ty(def_id)))
                    }
                    biwac_ast::TypeDef::TypeAlias(alias_def) => {
                        let def_id = TyDefId::new(self.alloc_def_id());
                        alias_def.def_id.set(def_id).unwrap();

                        Some((alias_def.ident.clone(), TyOrVal::Ty(def_id)))
                    }
                    biwac_ast::TypeDef::NativeTypeAlias(alias_def) => {
                        let def_id = TyDefId::new(self.alloc_def_id());
                        alias_def.def_id.set(def_id).unwrap();

                        Some((alias_def.ident.clone(), TyOrVal::Ty(def_id)))
                    }
                },
                biwac_ast::Globals::NativeCode(_) => None,
                biwac_ast::Globals::NovelScene(scene_def) => {
                    let def_id = ValDefId::new(self.alloc_def_id());
                    scene_def.def_id.set(def_id).unwrap();

                    Some((scene_def.id.clone(), TyOrVal::Val(def_id)))
                }
                biwac_ast::Globals::ImplBlock(_) => {
                    // TODO:

                    None
                }
            };

            if let Some((ident, def_id)) = opt_ident_and_def_id {
                match children.entry(ident.id) {
                    Entry::Vacant(e) => match def_id {
                        TyOrVal::Ty(def_id) => {
                            e.insert((
                                ModuleNameTreeItem::Ty(TyNameTree {
                                    def_id,

                                    // TODO: collect ty associated items
                                    children: RefCell::new(HashMap::new()),
                                }),
                                ident.span.clone(),
                            ));
                        }
                        TyOrVal::Val(def_id) => {
                            e.insert((ModuleNameTreeItem::Val(def_id), ident.span.clone()));
                        }
                    },
                    Entry::Occupied(e) => {
                        errors.push(DefCollectError::DuplicatedSymbolName {
                            name: ident.id,
                            span1: ident.span.clone(),
                            span2: e.get().1.clone(),
                        });
                    }
                }
            }
        }

        for (interned_mod_name, module) in &module.children {
            match children.entry(*interned_mod_name) {
                Entry::Vacant(e) => match self.collect_in_module(&module) {
                    Ok(module_tree) => {
                        e.insert((
                            ModuleNameTreeItem::Mod(module_tree),
                            Span::new(module.mod_id, 0, 0),
                        ));
                    }
                    Err(errs) => {
                        errors.extend(errs);
                    }
                },
                Entry::Occupied(e) => {
                    errors.push(DefCollectError::DuplicatedSymbolAndModuleName {
                        name: *interned_mod_name,
                        mod_id: module.mod_id,
                        symbol_span: e.get().1.clone(),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(ModuleNameTree {
                mod_id: module.mod_id,
                children: children
                    .into_iter()
                    .map(|(interned, (item, _))| (interned, item))
                    .collect(),
            })
        } else {
            Err(errors)
        }
    }

    fn collect_impls(
        &mut self,
        pkg_name: &InternedIdent,
        name_tree: &NameTree,
    ) -> Result<(), Vec<DefCollectError>> {
        // TODO:
        Ok(())
    }
}
