use std::{
    cell::RefCell,
    collections::{HashMap, hash_map::Entry},
    sync::Arc,
};

use biwac_ast::{AbsolutePathHeader, Globals, ModAst, Path, PathSegmentResolution};
use biwac_base::{IdentInterner, InternedIdent, ModId, PackageId};
use biwac_dependency_metadata::{
    DepMetadata, DepMetadataModuleView, ExternalChildKind, ExternalChildRef, PackageModuleView,
};
use biwac_hir::TyTraitImpl;
use biwac_span::{DefIdKind, TraitDefId, TyDefId};
use biwac_trait_solver::{Solved, TraitEnv, TraitSolveError};

use crate::{
    ModuleNameTree, ModuleNameTreeItem, NameTree, ResolveError, TyNameTree,
    lowering::def_id_kind_from_path, name_tree::AssocNameTreeItemKind,
    resolving::context::ResolveCtx,
};

#[derive(Debug)]
pub struct ModuleResolveCtx<'t> {
    global_tree: &'t NameTree,
    self_pkg_name: InternedIdent,
    module: &'t ModuleNameTree,
    imports: HashMap<InternedIdent, &'t Path>,
    ty_index: &'t HashMap<TyDefId, &'t TyNameTree>,
    mod_index: &'t HashMap<ModId, &'t ModuleNameTree>,
    interner: &'t IdentInterner,
    /// 型に対する trait impl の表。
    ///
    /// 直接の関連アイテムが空振りしたときのフォールバックに使う。
    /// def collection の途中 (Step 2/3/4) ではまだ出来ていないので `None`。
    trait_impls: Option<&'t HashMap<TyDefId, Vec<TyTraitImpl>>>,
    /// このモジュールで使える trait。
    /// [`Self::prepare_trait_scope`] が埋める。
    traits_in_scope: RefCell<Vec<TraitDefId>>,
}

impl<'t> ModuleResolveCtx<'t> {
    pub(crate) fn new(
        global_tree: &'t NameTree,
        self_pkg_name: InternedIdent,
        module_tree: &'t ModuleNameTree,
        module_ast: &'t ModAst,
        ty_index: &'t HashMap<TyDefId, &'t TyNameTree>,
        mod_index: &'t HashMap<ModId, &'t ModuleNameTree>,
        interner: &'t IdentInterner,
    ) -> Result<Self, Vec<ResolveError>> {
        let mut imports = HashMap::new();
        let mut errors = Vec::new();
        for g in &module_ast.globals {
            if let Globals::Import(import_decl) = g {
                let imported_ident = &import_decl.path.segments.last().unwrap().ident;
                match imports.entry(imported_ident.id) {
                    Entry::Vacant(e) => {
                        e.insert(&import_decl.path);
                    }
                    Entry::Occupied(e) => {
                        errors.push(ResolveError::DuplicatedSymbolName {
                            name: imported_ident.id,
                            // span1 は先に来た方
                            span1: e.get().segments.last().unwrap().span(),
                            span2: import_decl.path.segments.last().unwrap().span(),
                        });
                    }
                }

                if let Some(item) = module_tree.children.get(&imported_ident.id) {
                    errors.push(ResolveError::DuplicatedSymbolAndDefIdName {
                        name: imported_ident.id,
                        span: imported_ident.span.clone(),
                        def_id_kind: match item {
                            ModuleNameTreeItem::Mod(module) => DefIdKind::Mod(module.mod_id),
                            ModuleNameTreeItem::Ty(ty) => DefIdKind::Ty(ty.def_id),
                            ModuleNameTreeItem::Val(val_def_id) => DefIdKind::Val(*val_def_id),
                            ModuleNameTreeItem::Trait(def_id) => DefIdKind::Trait(*def_id),
                        },
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(Self {
                global_tree,
                self_pkg_name,
                module: module_tree,
                imports,
                ty_index,
                mod_index,
                interner,
                trait_impls: None,
                traits_in_scope: RefCell::new(Vec::new()),
            })
        } else {
            Err(errors)
        }
    }

    /// フォールバックに使う trait impl の表を渡す。
    ///
    /// def collection の途中では表がまだ出来ていないので、
    /// 本番の名前解決パスでだけ呼ぶ。
    pub(crate) fn with_trait_impls(
        mut self,
        trait_impls: &'t HashMap<TyDefId, Vec<TyTraitImpl>>,
    ) -> Self {
        self.trait_impls = Some(trait_impls);
        self
    }

    /// import をすべて解決し、このモジュールで使える trait を集める。
    ///
    /// import は名前で引かれたときに初めて解決されるので、
    /// ここで明示的に回す必要がある。
    /// trait の import は名前で参照されないまま
    /// 「その trait をスコープに入れる」ためだけに書かれるからである。
    pub(crate) fn prepare_trait_scope(&self, errors: &mut Vec<ResolveError>) -> Vec<TraitDefId> {
        let mut scope = Vec::new();

        // このモジュールで宣言された trait。
        for item in self.module.children.values() {
            if let ModuleNameTreeItem::Trait(def_id) = item {
                scope.push(*def_id);
            }
        }

        // import された trait。
        for path in self.imports.values() {
            match self.resolve_path(path) {
                Ok(()) => {
                    if let Ok(DefIdKind::Trait(def_id)) = def_id_kind_from_path(path) {
                        scope.push(def_id);
                    }
                }
                Err(e) => errors.push(e),
            }
        }

        scope.sort_by_key(|t| t.value());
        scope.dedup();
        *self.traits_in_scope.borrow_mut() = scope.clone();
        scope
    }
}

impl ModuleResolveCtx<'_> {
    fn local_tree_ctx(&self) -> LocalTreeCtx<'_> {
        LocalTreeCtx {
            ty_index: self.ty_index,
            ext_pkg_data: &self.global_tree.ext_pkg_data,
            interner: self.interner,
            trait_env: ModuleTraitEnv {
                trait_impls: self.trait_impls,
                ext_pkg_data: &self.global_tree.ext_pkg_data,
                interner: self.interner,
                in_scope: self.traits_in_scope.borrow().clone(),
            },
        }
    }
}

impl ResolveCtx for ModuleResolveCtx<'_> {
    fn resolve_path(&self, path: &Path) -> Result<(), ResolveError> {
        // If already resolved, fast path.
        for (i, segment) in path.segments.iter().enumerate() {
            match segment.resolved_id.get() {
                Some(PathSegmentResolution::Ok(_)) => {
                    if i + 1 == path.segments.len() {
                        return Ok(());
                    } else {
                        continue;
                    }
                }
                Some(PathSegmentResolution::Err) => {
                    return Err(ResolveError::PathResolutionFailed {
                        path: Box::new(path.clone()),
                    });
                }
                None => {
                    break;
                }
            }
        }

        match &path.abs_header {
            Some(AbsolutePathHeader::Package(_)) => {
                let self_package = &self.global_tree.packages.get(&self.self_pkg_name).unwrap();
                resolve_path_in_module(
                    path,
                    0,
                    &self_package.root_module_tree,
                    &self.local_tree_ctx(),
                )
            }

            Some(AbsolutePathHeader::SelfTyp(self_typ)) => Err(ResolveError::UnexpectedSelfType {
                span: self_typ.span.clone(),
            }),

            None => {
                let first_segment_ident = &path.segments[0].ident;
                if self.module.children.contains_key(&first_segment_ident.id) {
                    // Relative path found in current module - resolve properly.
                    resolve_path_in_module(path, 0, self.module, &self.local_tree_ctx())
                } else {
                    match self.imports.get(&first_segment_ident.id) {
                        Some(import_path) => match self.resolve_path(import_path) {
                            Ok(()) => {
                                let imported_kind = def_id_kind_from_path(import_path).unwrap();
                                path.segments[0]
                                    .resolved_id
                                    .set(PathSegmentResolution::Ok(imported_kind.clone()))
                                    .unwrap();

                                if path.segments.len() == 1 {
                                    return Ok(());
                                }

                                // Dispatch segment[1..] based on what the import resolved to.
                                match imported_kind {
                                    DefIdKind::Ty(ty_id) => {
                                        if ty_id.pkg().is_self() {
                                            match self.ty_index.get(&ty_id) {
                                                Some(ty_tree) => resolve_path_in_ty(
                                                    path,
                                                    1,
                                                    ty_tree,
                                                    &self.local_tree_ctx(),
                                                ),
                                                None => {
                                                    path.segments[1]
                                                        .resolved_id
                                                        .set(PathSegmentResolution::Err)
                                                        .unwrap();
                                                    Err(ResolveError::PathResolutionFailed {
                                                        path: Box::new(path.clone()),
                                                    })
                                                }
                                            }
                                        } else {
                                            // 外部パッケージの型: assoc アイテム解決
                                            let pkg_id = ty_id.pkg();
                                            match self.global_tree.ext_pkg_data.get(&pkg_id) {
                                                Some(dep_arc) => {
                                                    let view =
                                                        DepMetadataModuleView::new_for_sym_idx(
                                                            Arc::clone(dep_arc),
                                                            ty_id.local_idx(),
                                                            pkg_id,
                                                        );
                                                    resolve_path_in_ext_ty(
                                                        path,
                                                        1,
                                                        ty_id.local_idx(),
                                                        &view,
                                                        pkg_id,
                                                        self.interner,
                                                        &self.local_tree_ctx(),
                                                    )
                                                }
                                                None => {
                                                    path.segments[1]
                                                        .resolved_id
                                                        .set(PathSegmentResolution::Err)
                                                        .unwrap();
                                                    Err(ResolveError::PathResolutionFailed {
                                                        path: Box::new(path.clone()),
                                                    })
                                                }
                                            }
                                        }
                                    }
                                    DefIdKind::Mod(mod_id) => {
                                        if mod_id.is_self_pkg() {
                                            match self.mod_index.get(&mod_id) {
                                                Some(mod_tree) => resolve_path_in_module(
                                                    path,
                                                    1,
                                                    mod_tree,
                                                    &self.local_tree_ctx(),
                                                ),
                                                None => {
                                                    path.segments[1]
                                                        .resolved_id
                                                        .set(PathSegmentResolution::Err)
                                                        .unwrap();
                                                    Err(ResolveError::PathResolutionFailed {
                                                        path: Box::new(path.clone()),
                                                    })
                                                }
                                            }
                                        } else {
                                            // 外部パッケージのモジュール: PackageModuleView 経由で解決
                                            let pkg_id = PackageId::new(mod_id.pkg_id_bits());
                                            let sym_idx = mod_id.sym_idx();
                                            match self.global_tree.ext_pkg_data.get(&pkg_id) {
                                                Some(dep_arc) => {
                                                    let sub_view =
                                                        DepMetadataModuleView::new_for_sym_idx(
                                                            Arc::clone(dep_arc),
                                                            sym_idx,
                                                            pkg_id,
                                                        );
                                                    resolve_path_in_ext_pkg(
                                                        path,
                                                        1,
                                                        &sub_view,
                                                        pkg_id,
                                                        self.interner,
                                                        &self.local_tree_ctx(),
                                                    )
                                                }
                                                None => {
                                                    path.segments[1]
                                                        .resolved_id
                                                        .set(PathSegmentResolution::Err)
                                                        .unwrap();
                                                    Err(ResolveError::PathResolutionFailed {
                                                        path: Box::new(path.clone()),
                                                    })
                                                }
                                            }
                                        }
                                    }
                                    _ => {
                                        path.segments[1]
                                            .resolved_id
                                            .set(PathSegmentResolution::Err)
                                            .unwrap();
                                        Err(ResolveError::PathResolutionFailed {
                                            path: Box::new(path.clone()),
                                        })
                                    }
                                }
                            }
                            Err(e) => {
                                path.segments[0]
                                    .resolved_id
                                    .set(PathSegmentResolution::Err)
                                    .unwrap();

                                Err(e)
                            }
                        },
                        None => {
                            if let Some(package) =
                                self.global_tree.packages.get(&first_segment_ident.id)
                            {
                                if path.segments.len() == 1 {
                                    path.segments[0]
                                        .resolved_id
                                        .set(PathSegmentResolution::Ok(DefIdKind::Package(
                                            package.pkg_id,
                                        )))
                                        .unwrap();

                                    Ok(())
                                } else {
                                    resolve_path_in_module(
                                        path,
                                        1,
                                        &package.root_module_tree,
                                        &self.local_tree_ctx(),
                                    )
                                }
                            } else if let Some(view) =
                                self.global_tree.ext_pkg_views.get(&first_segment_ident.id)
                            {
                                let pkg_id = view.pkg_id();
                                path.segments[0]
                                    .resolved_id
                                    .set(PathSegmentResolution::Ok(DefIdKind::Package(pkg_id)))
                                    .unwrap();
                                if path.segments.len() == 1 {
                                    Ok(())
                                } else {
                                    resolve_path_in_ext_pkg(
                                        path,
                                        1,
                                        view.as_ref(),
                                        pkg_id,
                                        self.interner,
                                        &self.local_tree_ctx(),
                                    )
                                }
                            } else {
                                path.segments[0]
                                    .resolved_id
                                    .set(PathSegmentResolution::Err)
                                    .unwrap();
                                Err(ResolveError::IdentNotFound {
                                    ident: first_segment_ident.clone(),
                                })
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 自パッケージの名前ツリーを辿るあいだ持ち回る参照。
///
/// `ty_index` だけでは足りないのは、型エイリアスの右辺が
/// 外部パッケージの型を指しうるためである
/// (`type CharacterBiwa = std::game::Character[P]` のような形)。
/// その場合 canonical な型はローカルのツリーに居ないので、
/// 依存メタデータ側に降りて関連アイテムを引く必要がある。
struct LocalTreeCtx<'t> {
    ty_index: &'t HashMap<TyDefId, &'t TyNameTree>,
    ext_pkg_data: &'t HashMap<PackageId, Arc<DepMetadata>>,
    interner: &'t IdentInterner,
    trait_env: ModuleTraitEnv<'t>,
}

/// 1 つのモジュールに閉じた [`TraitEnv`]。
struct ModuleTraitEnv<'t> {
    trait_impls: Option<&'t HashMap<TyDefId, Vec<TyTraitImpl>>>,
    ext_pkg_data: &'t HashMap<PackageId, Arc<DepMetadata>>,
    interner: &'t IdentInterner,
    in_scope: Vec<TraitDefId>,
}

impl TraitEnv for ModuleTraitEnv<'_> {
    fn trait_impls_of(&self, ty: TyDefId) -> Vec<TyTraitImpl> {
        let mut out: Vec<TyTraitImpl> = self
            .trait_impls
            .and_then(|m| m.get(&ty))
            .cloned()
            .unwrap_or_default();

        // trait impl は対象の型のパッケージに載るとは限らない
        // (自分の trait を他パッケージの型に実装できる) ので、
        // 依存すべてを見る必要がある。
        for (pkg_id, dep) in self.ext_pkg_data {
            out.extend(dep.trait_impls_for(ty, *pkg_id, self.interner));
        }

        out
    }

    fn traits_in_scope(&self) -> &[TraitDefId] {
        &self.in_scope
    }
}

/// 直接の関連アイテムが見つからなかったときの、trait 越しの解決。
fn solve_assoc_fallback(
    ty_def_id: TyDefId,
    segment: &biwac_ast::PathSegment,
    ctx: &LocalTreeCtx<'_>,
) -> Result<DefIdKind, ResolveError> {
    match biwac_trait_solver::solve_assoc(ty_def_id, segment.ident.id, &ctx.trait_env) {
        Ok(Solved::Impl(val_def_id)) => Ok(DefIdKind::Val(val_def_id)),
        Err(TraitSolveError::NotInScope { candidates }) => Err(ResolveError::TraitNotInScope {
            segment: segment.clone(),
            candidates,
        }),
        Err(TraitSolveError::Ambiguous { candidates }) => Err(ResolveError::AmbiguousTraitAssoc {
            segment: segment.clone(),
            candidates,
        }),
        Err(_) => Err(ResolveError::TraitAssocNotFound {
            segment: segment.clone(),
        }),
    }
}

/// Resolves path starting at `depth` within a module tree.
/// Sets `resolved_id` on each path segment and navigates into child modules or type children.
fn resolve_path_in_module(
    path: &Path,
    depth: usize,
    module: &ModuleNameTree,
    ctx: &LocalTreeCtx<'_>,
) -> Result<(), ResolveError> {
    let segment = &path.segments[depth];
    match module.children.get(&segment.ident.id) {
        Some(item) => {
            let def_id_kind = module_item_to_def_id_kind(item);
            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                match item {
                    ModuleNameTreeItem::Mod(child_module) => {
                        resolve_path_in_module(path, depth + 1, child_module, ctx)
                    }
                    ModuleNameTreeItem::Ty(ty_tree) => {
                        resolve_path_in_ty(path, depth + 1, ty_tree, ctx)
                    }
                    // 値と trait はどちらもここで終端である。
                    // trait の項目をパスから直接引く構文は無い。
                    ModuleNameTreeItem::Val(_) | ModuleNameTreeItem::Trait(_) => {
                        path.segments[depth + 1]
                            .resolved_id
                            .set(PathSegmentResolution::Err)
                            .unwrap();
                        Err(ResolveError::PathResolutionFailed {
                            path: Box::new(path.clone()),
                        })
                    }
                }
            }
        }
        None => {
            segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
            Err(ResolveError::PathResolutionFailed {
                path: Box::new(path.clone()),
            })
        }
    }
}

/// Resolves path starting at `depth` within a type's associated items.
/// Follows alias_target if the type is an alias.
fn resolve_path_in_ty(
    path: &Path,
    depth: usize,
    ty_tree: &TyNameTree,
    ctx: &LocalTreeCtx<'_>,
) -> Result<(), ResolveError> {
    // Follow alias chain to find the canonical type's children.
    let alias_target = *ty_tree.alias_target.borrow();

    // 右辺が外部パッケージの型を指すエイリアスは、
    // canonical な型がローカルのツリーに居ない。
    // `type C = std::game::Character[P]; C::new(..)` を引けるように、
    // 依存メタデータの view に降りて関連アイテムを解決する。
    if let Some(canonical_id) = alias_target
        && !canonical_id.pkg().is_self()
    {
        let pkg_id = canonical_id.pkg();
        let Some(dep_arc) = ctx.ext_pkg_data.get(&pkg_id) else {
            path.segments[depth]
                .resolved_id
                .set(PathSegmentResolution::Err)
                .unwrap();
            return Err(ResolveError::PathResolutionFailed {
                path: Box::new(path.clone()),
            });
        };

        let view = DepMetadataModuleView::new_for_sym_idx(
            Arc::clone(dep_arc),
            canonical_id.local_idx(),
            pkg_id,
        );

        return resolve_path_in_ext_ty(
            path,
            depth,
            canonical_id.local_idx(),
            &view,
            pkg_id,
            ctx.interner,
            ctx,
        );
    }

    let canonical_tree = match alias_target {
        Some(canonical_id) => ctx.ty_index.get(&canonical_id).copied().unwrap_or(ty_tree),
        None => ty_tree,
    };

    let segment = &path.segments[depth];
    let children = canonical_tree.children.borrow();
    match children.get(&segment.ident.id) {
        Some(assoc_tree) => {
            // TODO: segment に genargs: Option<Vec<TypRepr>> を持たせて解決
            let def_id_kind = match assoc_tree.find_matched(None, segment) {
                Ok(AssocNameTreeItemKind::Val(def_id)) => DefIdKind::Val(*def_id),
                Ok(AssocNameTreeItemKind::Variant(def_id)) => DefIdKind::Variant(*def_id),
                Err(e) => {
                    segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
                    return Err(e);
                }
            };

            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                path.segments[depth + 1]
                    .resolved_id
                    .set(PathSegmentResolution::Err)
                    .unwrap();
                Err(ResolveError::PathResolutionFailed {
                    path: Box::new(path.clone()),
                })
            }
        }
        None => {
            // 直接の impl に無い。ここで初めて trait を探す。
            drop(children);
            match solve_assoc_fallback(canonical_tree.def_id, segment, ctx) {
                Ok(def_id_kind) => {
                    segment
                        .resolved_id
                        .set(PathSegmentResolution::Ok(def_id_kind))
                        .unwrap();

                    if path.segments.len() == depth + 1 {
                        Ok(())
                    } else {
                        path.segments[depth + 1]
                            .resolved_id
                            .set(PathSegmentResolution::Err)
                            .unwrap();
                        Err(ResolveError::PathResolutionFailed {
                            path: Box::new(path.clone()),
                        })
                    }
                }
                Err(e) => {
                    segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
                    Err(e)
                }
            }
        }
    }
}

fn module_item_to_def_id_kind(item: &ModuleNameTreeItem) -> DefIdKind {
    match item {
        ModuleNameTreeItem::Mod(module) => DefIdKind::Mod(module.mod_id),
        // 型の位置では alias を canonical な型に潰さない。
        //
        // 潰すと `type MyGame = Game[A, B]` の [A, B] が失われてしまう
        // (ここは TyDefId しか運べないため)。
        // alias 自身の TyDefId のまま HIR まで運び、
        // lowering の最後で alias_expansion が右辺ごと置き換える。
        //
        // 一方、関連アイテムの解決 (resolve_path_in_ty) は
        // `PairIntT::new` を `Pair::new` に解決する必要があるので
        // 引き続き alias_target を辿る。
        ModuleNameTreeItem::Ty(ty) => DefIdKind::Ty(ty.def_id),
        ModuleNameTreeItem::Val(val_def_id) => DefIdKind::Val(*val_def_id),
        ModuleNameTreeItem::Trait(def_id) => DefIdKind::Trait(*def_id),
    }
}

// ============================================================
// 外部パッケージ解決ヘルパー
// ============================================================

/// 外部パッケージ内のモジュールをパスで辿る。
/// `depth` = path.segments 内の開始インデックス (パッケージ名セグメントの次)。
fn resolve_path_in_ext_pkg(
    path: &Path,
    depth: usize,
    view: &dyn PackageModuleView,
    pkg_id: PackageId,
    interner: &IdentInterner,
    trait_ctx: &LocalTreeCtx<'_>,
) -> Result<(), ResolveError> {
    let segment = &path.segments[depth];
    match view.lookup_child(segment.ident.id, interner) {
        None => {
            segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
            Err(ResolveError::PathResolutionFailed {
                path: Box::new(path.clone()),
            })
        }
        Some(child_ref) => {
            let def_id_kind = ext_child_ref_to_def_id_kind(&child_ref, pkg_id);
            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                match child_ref.kind {
                    ExternalChildKind::Mod => {
                        let sub_view = view.get_module_view(child_ref.sym_idx);
                        resolve_path_in_ext_pkg(
                            path,
                            depth + 1,
                            sub_view.as_ref(),
                            pkg_id,
                            interner,
                            trait_ctx,
                        )
                    }
                    ExternalChildKind::Ty => resolve_path_in_ext_ty(
                        path,
                        depth + 1,
                        child_ref.sym_idx,
                        view,
                        pkg_id,
                        interner,
                        trait_ctx,
                    ),
                    // 値・バリアント・trait はどれもここで終端である。
                    // その先にセグメントがあれば解決できない。
                    ExternalChildKind::Val
                    | ExternalChildKind::Variant
                    | ExternalChildKind::Trait => {
                        path.segments[depth + 1]
                            .resolved_id
                            .set(PathSegmentResolution::Err)
                            .unwrap();
                        Err(ResolveError::PathResolutionFailed {
                            path: Box::new(path.clone()),
                        })
                    }
                }
            }
        }
    }
}

/// 外部パッケージの型の assoc アイテムをパスで解決する。
/// `local_ty_idx` = その型のシンボルインデックス。
fn resolve_path_in_ext_ty(
    path: &Path,
    depth: usize,
    local_ty_idx: u32,
    view: &dyn PackageModuleView,
    pkg_id: PackageId,
    interner: &IdentInterner,
    trait_ctx: &LocalTreeCtx<'_>,
) -> Result<(), ResolveError> {
    let segment = &path.segments[depth];
    match view.lookup_assoc(local_ty_idx, segment.ident.id, interner) {
        None => {
            // 外部パッケージの型でも、自パッケージが trait を実装していれば引ける。
            let ty_def_id = TyDefId::new(biwac_span::DefId::new(
                pkg_id,
                biwac_span::PackageLocalDefId::new(local_ty_idx),
            ));
            match solve_assoc_fallback(ty_def_id, segment, trait_ctx) {
                Ok(def_id_kind) => {
                    segment
                        .resolved_id
                        .set(PathSegmentResolution::Ok(def_id_kind))
                        .unwrap();
                    if path.segments.len() == depth + 1 {
                        Ok(())
                    } else {
                        path.segments[depth + 1]
                            .resolved_id
                            .set(PathSegmentResolution::Err)
                            .unwrap();
                        Err(ResolveError::PathResolutionFailed {
                            path: Box::new(path.clone()),
                        })
                    }
                }
                Err(e) => {
                    segment.resolved_id.set(PathSegmentResolution::Err).unwrap();
                    Err(e)
                }
            }
        }
        Some(child_ref) => {
            let def_id_kind = ext_child_ref_to_def_id_kind(&child_ref, pkg_id);
            segment
                .resolved_id
                .set(PathSegmentResolution::Ok(def_id_kind))
                .unwrap();

            if path.segments.len() == depth + 1 {
                Ok(())
            } else {
                // assoc アイテムの先をさらに辿ることは現時点でサポートしない
                path.segments[depth + 1]
                    .resolved_id
                    .set(PathSegmentResolution::Err)
                    .unwrap();
                Err(ResolveError::PathResolutionFailed {
                    path: Box::new(path.clone()),
                })
            }
        }
    }
}

/// `ExternalChildRef` を `DefIdKind` に変換する。
/// 外部モジュール (`Mod`) は `ModId::new_ext` でエンコードした `DefIdKind::Mod` として記録する。
fn ext_child_ref_to_def_id_kind(child_ref: &ExternalChildRef, pkg_id: PackageId) -> DefIdKind {
    match child_ref.kind {
        ExternalChildKind::Ty => DefIdKind::Ty(child_ref.as_ty_def_id(pkg_id)),
        ExternalChildKind::Val => DefIdKind::Val(child_ref.as_val_def_id(pkg_id)),
        ExternalChildKind::Variant => DefIdKind::Variant(child_ref.as_variant_def_id(pkg_id)),
        ExternalChildKind::Trait => DefIdKind::Trait(child_ref.as_trait_def_id(pkg_id)),
        ExternalChildKind::Mod => DefIdKind::Mod(ModId::new_ext(pkg_id.value(), child_ref.sym_idx)),
    }
}
