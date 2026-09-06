use std::{cell::RefCell, collections::HashMap, sync::Arc};

use biwac_ast::PathSegment;
use biwac_base::{InternedIdent, ModId, PackageId};
use biwac_dependency_metadata::{DepMetadata, PackageModuleView};
use biwac_hir::Ty;
use biwac_span::{TyDefId, ValDefId, VariantDefId};

use crate::ResolveError;

pub struct NameTree {
    pub(crate) self_pkg_name: InternedIdent,
    /// 自パッケージのみ保持 (型付きアクセス・ミューテーション用)。
    /// 外部パッケージは ext_pkg_views に格納する。
    pub(crate) packages: HashMap<InternedIdent, PackageNameTree>,
    /// 外部パッケージの lazy モジュール view (PackageModuleView トレイト経由)。
    pub(crate) ext_pkg_views: HashMap<InternedIdent, Arc<dyn PackageModuleView>>,
    /// PackageId → DepMetadata (型情報の lazy アクセス用)。
    pub(crate) ext_pkg_data: HashMap<PackageId, Arc<DepMetadata>>,
}

impl std::fmt::Debug for NameTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NameTree")
            .field("self_pkg_name", &self.self_pkg_name)
            .field("packages", &self.packages)
            .field(
                "ext_pkg_views",
                &self.ext_pkg_views.keys().collect::<Vec<_>>(),
            )
            .field(
                "ext_pkg_data",
                &self.ext_pkg_data.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Debug)]
pub struct PackageNameTree {
    pub(crate) pkg_id: PackageId,
    pub(crate) root_module_tree: ModuleNameTree,
}

#[derive(Debug)]
pub struct ModuleNameTree {
    pub(crate) mod_id: ModId,
    pub(crate) children: HashMap<InternedIdent, ModuleNameTreeItem>,
}

#[derive(Debug)]
pub struct TyNameTree {
    pub(crate) def_id: TyDefId,
    /// Associated items (fns, types) registered via impl blocks, keyed by item name.
    pub(crate) children: RefCell<HashMap<InternedIdent, AssocNameTree>>,
    /// If this entry is a type alias, stores the canonical (chain-followed) non-alias TyDefId.
    pub(crate) alias_target: RefCell<Option<TyDefId>>,
}

#[derive(Debug)]
pub enum ModuleNameTreeItem {
    Mod(ModuleNameTree),
    Ty(TyNameTree),
    Val(ValDefId),
}

#[derive(Debug)]
pub struct AssocNameTree {
    pub(crate) assocs: Vec<AssocNameTreeItem>,
}

#[derive(Debug, Clone)]
pub struct AssocNameTreeItem {
    pub genargs: Vec<Ty>,
    pub kind: AssocNameTreeItemKind,
}

#[derive(Debug, Clone)]
pub enum AssocNameTreeItemKind {
    // TODO:
    // Ty(AssocNameTreeTyItem),
    Val(ValDefId),
    /// enum のバリアント。
    ///
    /// 関連関数と同じ children に載る。名前空間が型と値で分かれていないので、
    /// `Color::Red` も `Color::from_hex` も同じ表から一意に引ける。
    Variant(VariantDefId),
}

impl AssocNameTree {
    pub(crate) fn find_matched(
        &self,
        genargs: Option<&[Ty]>,
        segment: &PathSegment,
    ) -> Result<&AssocNameTreeItemKind, ResolveError> {
        match genargs {
            Some(genargs) => {
                for item in &self.assocs {
                    if item.genargs.len() == genargs.len()
                        && item
                            .genargs
                            .iter()
                            .zip(genargs)
                            .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind))
                    {
                        return Ok(&item.kind);
                    }
                }

                Err(ResolveError::AssocItemNotFoundForGenArgs {
                    segment: segment.clone(),
                })
            }
            None => {
                if self.assocs.len() == 1 {
                    Ok(&self.assocs[0].kind)
                } else {
                    Err(ResolveError::AmbiguousAssocItem {
                        segment: segment.clone(),
                    })
                }
            }
        }
    }

    pub(crate) fn register_assoc(
        &mut self,
        name: InternedIdent,
        genargs: Vec<Ty>,
        assoc: AssocNameTreeItemKind,
    ) -> Result<(), ResolveError> {
        for item in &self.assocs {
            // バリアントは impl のジェネリック引数で分かれない。
            // 同じ名前に何かが既にあれば、それだけで衝突である
            // (`enum Foo { Bar }` と `impl Foo { fn Bar() }` など)。
            let variant_involved = matches!(item.kind, AssocNameTreeItemKind::Variant(_))
                || matches!(assoc, AssocNameTreeItemKind::Variant(_));

            if variant_involved
                || (item.genargs.len() == genargs.len()
                    && item
                        .genargs
                        .iter()
                        .zip(&genargs)
                        .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind)))
            {
                return Err(ResolveError::DuplicatedAssociatedItemForGenArgs {
                    name,
                    assoc1: item.clone(),
                    assoc2: AssocNameTreeItem {
                        genargs,
                        kind: assoc,
                    },
                });
            }
        }

        self.assocs.push(AssocNameTreeItem {
            genargs,
            kind: assoc,
        });

        Ok(())
    }
}
