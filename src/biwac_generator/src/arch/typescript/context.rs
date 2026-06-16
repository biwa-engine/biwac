use std::collections::HashMap;

use biwac_base::{IdentInterner, InternedIdent, ModPath, SourceHolder};
use biwac_hir::{ExprId, Hir, Ident, Ty, TyDefKind, ValDefKind};
use biwac_span::{TyDefId, ValDefId, VarId};

pub(super) struct AstBuildCtx<'a> {
    hir: &'a Hir,
    interner: &'a IdentInterner,
    srcs: &'a SourceHolder,
    pub(super) allocator: &'a oxc_allocator::Allocator,
}

pub(super) struct FnAstBuildCtx<'a> {
    pub(super) expr_tys: &'a HashMap<ExprId, Ty>,
    pub(super) var_tys: &'a HashMap<VarId, Ty>,
    pub(super) stmts: Vec<oxc_ast::ast::Statement<'a>>,
}

impl<'a> AstBuildCtx<'a> {
    pub(super) fn new(
        hir: &'a Hir,
        interner: &'a IdentInterner,
        srcs: &'a SourceHolder,
        allocator: &'a oxc_allocator::Allocator,
    ) -> Self {
        Self {
            hir,
            interner,
            srcs,
            allocator,
        }
    }

    pub(super) fn str_of(&self, interned: &InternedIdent) -> &str {
        self.interner.get_str(interned).unwrap()
    }
    pub(super) fn get_package_name_of_type(&self, def_id: &TyDefId) -> &str {
        let module = self
            .srcs
            .mods
            .get(&self.get_type_ident(def_id).span.module())
            .unwrap();
        let pkg_name_interned = self.hir.packages.get(&module.pkg_id).unwrap();
        self.interner.get_str(pkg_name_interned).unwrap()
    }

    pub(super) fn get_package_name_of_value(&self, def_id: &ValDefId) -> &str {
        let module = self
            .srcs
            .mods
            .get(&self.get_value_ident(def_id).span.module())
            .unwrap();
        let pkg_name_interned = self.hir.packages.get(&module.pkg_id).unwrap();
        self.interner.get_str(pkg_name_interned).unwrap()
    }

    fn get_value_ident(&self, def_id: &ValDefId) -> &Ident {
        match self.hir.vals.get(def_id).unwrap() {
            ValDefKind::Fn(fn_def) => &fn_def.name,
            ValDefKind::Native(fn_def) => &fn_def.name,
            ValDefKind::NovelScene(scene_def) => &scene_def.name,
            ValDefKind::ExternalFn(_) => todo!(),
        }
    }

    fn get_type_ident(&self, def_id: &TyDefId) -> &Ident {
        match &self.hir.tys.get(def_id).unwrap().ty_content {
            TyDefKind::Struct(struct_def) => &struct_def.name,
            TyDefKind::NativeTypeAlias(alias_def) => &alias_def.name,
        }
    }
    pub(super) fn get_value_mangled(&self, def_id: &ValDefId) -> String {
        self.get_symbol_mangled(self.get_value_ident(def_id))
    }

    pub(super) fn get_type_mangled(&self, def_id: &TyDefId) -> String {
        self.get_symbol_mangled(self.get_type_ident(def_id))
    }
    fn get_symbol_mangled(&self, ident: &Ident) -> String {
        let module = self.srcs.mods.get(&ident.span.module()).unwrap();
        let pkg_name_interned = self.hir.packages.get(&module.pkg_id).unwrap();
        let pkg_name = self.interner.get_str(pkg_name_interned).unwrap();
        let module_path = &module.modu;
        let sym_name = self.interner.get_str(&ident.id).unwrap();

        let mut result = String::from("_Z");

        // ネストがある場合は N ... E で囲む
        result.push('N');

        result.push_str(&format!("{}{}", pkg_name.len(), pkg_name));

        match module_path {
            ModPath::Main | ModPath::Lib => {}
            ModPath::Mod(path) => {
                for p in path {
                    result.push_str(&format!("{}{}", p.len(), p));
                }
            }
        }

        result.push_str(&format!("{}{}", sym_name.len(), sym_name));
        result.push('E');

        result
    }
}
