use std::collections::HashMap;
use std::sync::Arc;

use biwac_base::{IdentInterner, InternedIdent, ModPath, PackageId, SourceHolder};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{AssocValDefKind, ExprId, Hir, Ident, Ty, TyDefKind, ValDefKind};
use biwac_lang_item::{LangItem, LangItemTable};
use biwac_span::{DefId, TyDefId, ValDefId, VarId};

pub(super) struct AstBuildCtx<'a> {
    hir: &'a Hir,
    interner: &'a IdentInterner,
    srcs: &'a SourceHolder,
    /// 依存パッケージのメタデータ。
    /// 外部シンボルは HIR に無く、span もダミーなので、
    /// マングリングに必要な名前とモジュールパスはここから引く。
    ext_pkgs: &'a [(PackageId, Arc<DepMetadata>)],
    /// lang item テーブル。
    /// novel statement を std の関数呼び出しに展開する際に使う。
    lang_items: &'a LangItemTable,
    pub(super) allocator: &'a oxc_allocator::Allocator,
}

impl<'a> AstBuildCtx<'a> {
    pub(super) fn new(
        hir: &'a Hir,
        interner: &'a IdentInterner,
        srcs: &'a SourceHolder,
        ext_pkgs: &'a [(PackageId, Arc<DepMetadata>)],
        lang_items: &'a LangItemTable,
        allocator: &'a oxc_allocator::Allocator,
    ) -> Self {
        Self {
            hir,
            interner,
            srcs,
            ext_pkgs,
            lang_items,
            allocator,
        }
    }

    /// lang item の関数のマングル済み名を返す。
    ///
    /// 型検査を通っていれば必ず解決できる
    /// (型推論が同じ lang item を require 済み)。
    pub(super) fn lang_item_fn_mangled(&self, item: LangItem) -> String {
        let def_id = self.lang_items.get(&item).unwrap_or_else(|| {
            panic!(
                "compiler bug: lang item `{}` is missing at codegen",
                item.key()
            )
        });

        self.get_value_mangled(&ValDefId::new(def_id))
    }

    fn find_ext_dep(&self, pkg_id: PackageId) -> Option<&Arc<DepMetadata>> {
        self.ext_pkgs
            .iter()
            .find(|(pid, _)| *pid == pkg_id)
            .map(|(_, d)| d)
    }

    /// 外部パッケージのシンボルの (名前, モジュールパス) を引く。
    fn ext_symbol_info(&self, def_id: &DefId) -> Option<(&str, ModPath)> {
        self.find_ext_dep(def_id.pkg())?
            .symbol_mangling_info(def_id.local_idx())
    }

    fn pkg_name_of(&self, pkg_id: PackageId) -> &str {
        let interned = self
            .hir
            .packages
            .get(&pkg_id)
            .expect("compiler bug: unknown package id");
        self.interner.get_str(interned).unwrap()
    }

    pub(super) fn str_of(&self, interned: &InternedIdent) -> &str {
        self.interner.get_str(interned).unwrap()
    }
    pub(super) fn get_package_name_of_type(&self, def_id: &TyDefId) -> &str {
        self.pkg_name_of(def_id.pkg())
    }

    pub(super) fn get_package_name_of_value(&self, def_id: &ValDefId) -> &str {
        self.pkg_name_of(def_id.pkg())
    }

    fn get_value_ident(&self, def_id: &ValDefId) -> &Ident {
        if let Some(val) = self.hir.vals.get(def_id) {
            match val {
                ValDefKind::Fn(fn_def) => &fn_def.name,
                ValDefKind::Native(fn_def) => &fn_def.name,
                ValDefKind::NovelScene(scene_def) => &scene_def.name,
            }
        } else {
            let (ty_def_id, assoc_name) = self.hir.assoc_val_map.get(def_id).unwrap();
            match &self
                .hir
                .tys
                .get(ty_def_id)
                .unwrap()
                .vals
                .get(assoc_name)
                .unwrap()
                .vals
                .get(def_id)
                .unwrap()
                .val_content
            {
                AssocValDefKind::Fn(fn_def) => &fn_def.name,
                AssocValDefKind::NativeFn(fn_def) => &fn_def.name,
            }
        }
    }

    fn get_type_ident(&self, def_id: &TyDefId) -> &Ident {
        match self
            .hir
            .tys
            .get(def_id)
            .unwrap()
            .ty_content
            .as_ref()
            .unwrap()
        {
            TyDefKind::Struct(struct_def) => &struct_def.name,
            TyDefKind::NativeTypeAlias(alias_def) => &alias_def.name,
        }
    }
    pub(super) fn get_value_mangled(&self, def_id: &ValDefId) -> String {
        if let Some((name, modu)) = self.ext_symbol_info(&def_id.def_id()) {
            return Self::mangle(self.pkg_name_of(def_id.pkg()), &modu, name);
        }

        self.get_symbol_mangled(self.get_value_ident(def_id))
    }

    pub(super) fn get_type_mangled(&self, def_id: &TyDefId) -> String {
        if let Some((name, modu)) = self.ext_symbol_info(&def_id.def_id()) {
            return Self::mangle(self.pkg_name_of(def_id.pkg()), &modu, name);
        }

        self.get_symbol_mangled(self.get_type_ident(def_id))
    }

    fn get_symbol_mangled(&self, ident: &Ident) -> String {
        let module = self.srcs.mods.get(&ident.span.module()).unwrap();
        let pkg_name_interned = self.hir.packages.get(&module.pkg_id).unwrap();
        let pkg_name = self.interner.get_str(pkg_name_interned).unwrap();
        let sym_name = self.interner.get_str(&ident.id).unwrap();

        Self::mangle(pkg_name, &module.modu, sym_name)
    }

    fn mangle(pkg_name: &str, module_path: &ModPath, sym_name: &str) -> String {
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

pub(super) struct FnAstBuildCtx<'a> {
    pub(super) expr_tys: &'a HashMap<ExprId, Ty>,
    pub(super) var_tys: &'a HashMap<VarId, Ty>,
    pub(super) stmts: Vec<oxc_ast::ast::Statement<'a>>,
}

impl<'a> FnAstBuildCtx<'a> {
    pub(super) fn new(expr_tys: &'a HashMap<ExprId, Ty>, var_tys: &'a HashMap<VarId, Ty>) -> Self {
        Self {
            expr_tys,
            var_tys,
            stmts: Vec::new(),
        }
    }
}
