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

use biwac_base::{IdentInterner, InternedIdent, PackageId, PackageName};
use biwac_dependency_metadata::ExternalPackage;

use biwac_hir::Hir;
use biwac_package_loader::Pkg;

use crate::resolving::{lang_item_collector::collect_lang_items, resolve_in_self_package};

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

/// 名前解決パスの成果物。
///
/// lang item テーブルは Hir とは別に持つ。
/// Hir は「lowering されたプログラム本体」であり、
/// lang item はパッケージ横断のメタ情報だからである。
/// (rustc が lang_items を HIR の外の TyCtxt クエリとして置くのと同じ)
pub struct ResolveOutput {
    pub hir: Hir,
    pub lang_items: biwac_lang_item::LangItemTable,
}

pub struct NameResolver {
    pkg: Pkg,
    pkg_name: InternedIdent,
    pkg_package_name: PackageName,
    no_std: bool,
    /// 依存グラフの推移閉包すべて。
    /// import の根になれるのは `direct` なものだけだが、
    /// シンボルの解決にはすべてが要る。
    external_packages: Vec<ExternalPackage>,
}

impl NameResolver {
    pub fn new(
        metadata: &biwac_base::MetadataHolder,
        external_packages: Vec<ExternalPackage>,
        pkg_name: InternedIdent,
        pkg: Pkg,
    ) -> Result<Self, ResolveError> {
        let pkg_package_name = metadata.metadata.name.clone();
        Ok(Self {
            pkg,
            pkg_name,
            pkg_package_name,
            no_std: metadata.metadata.no_std,
            external_packages,
        })
    }

    pub fn try_resolve(
        self,
        interner: &mut IdentInterner,
    ) -> Result<ResolveOutput, Vec<ResolveError>> {
        // codegen がマングリングでパッケージ名を引くので、
        // 直接依存かどうかにかかわらず推移閉包すべてを入れる。
        let mut pkg_names = self
            .external_packages
            .iter()
            .map(|p| (p.pkg_id, p.ident))
            .collect::<HashMap<_, _>>();
        pkg_names.insert(PackageId::SELF_PACKAGE, self.pkg_name);

        // definition collection (package internal + external package ID assignment)
        let mut def_collector = resolving::def_collector::DefCollector::new();
        let name_tree = def_collector.collect(
            self.pkg_name,
            &self.pkg,
            self.external_packages.clone(),
            interner,
        )?;
        let external_packages = self.external_packages;

        // lang item collection
        //
        // def collection の直後に行う。
        // DefId は AST の OnceCell に入っているのでここで読める。
        // 名前解決より前でよい: lang item は名前解決に関与せず、
        // 逆に名前解決が lang item を必要とすることもない。
        let lang_items = collect_lang_items(&self.pkg, &external_packages, self.no_std, interner)?;

        // TODO: cache on disk
        // def_collector
        // name_tree
        // Even if name resolution failed,
        // def_collector and name_tree must be cached
        // because symbol definition is not affected by name resolution result.

        // symbol resolution (package internal)
        let trait_scopes =
            resolve_in_self_package(&self.pkg, &name_tree, &mut def_collector, interner)?;

        // TODO: cache on disk
        // symbol signature

        // lowering to HIR
        let hir = lowering::lower(
            self.pkg_package_name,
            self.pkg,
            pkg_names,
            &def_collector.impl_collector,
            trait_scopes,
            &external_packages,
            interner,
        )?;

        Ok(ResolveOutput { hir, lang_items })
    }
}
