use std::collections::{HashMap, HashSet};

use biwac_ast::{TypReprVal, TypeAlias};
use biwac_base::{ModPath, Span};
use biwac_hir::{DefinedTy, FnTy, GenTyId, Hir, Ty, TyId, TyKind, TypeAliasDefContent};

use crate::{
    ResolveError, RsvResult,
    context::{
        ty_from_primitive,
        ty_phase::{module_level::ModuleLevelTyResolveCtx, ty_def_level::TyDefLevelTyResolveCtx},
    },
};

#[derive(Debug)]
pub(crate) struct TyAliasResolveCtx<'ast> {
    mctxes: &'ast HashMap<ModPath, ModuleLevelTyResolveCtx>,
    alias_defs: HashMap<TyId, &'ast TypeAlias>,

    // 名前解決が済んだのみで循環参照などがあり得る type alias
    name_resolved_aliases: HashMap<TyId, (Ty, Vec<GenTyId>, Span)>,

    // 正規化まで完了した type alias
    normalized_aliases: HashMap<TyId, (Ty, Vec<GenTyId>)>,
}

impl<'ast> TyAliasResolveCtx<'ast> {
    pub fn new(
        mctxes: &'ast HashMap<ModPath, ModuleLevelTyResolveCtx>,
        alias_defs: HashMap<TyId, &'ast TypeAlias>,
    ) -> Self {
        Self {
            mctxes,
            alias_defs,
            name_resolved_aliases: HashMap::new(),
            normalized_aliases: HashMap::new(),
        }
    }

    pub fn try_resolve(mut self, hir: &Hir) -> RsvResult<HashMap<TyId, TypeAliasDefContent>> {
        // 名前解決
        self.try_resolve_rhs_typ_name(hir)?;

        // 循環参照検査
        self.check_cycles()?;

        // 正規化
        self.normalize();

        // 戻り値を構成
        let mut aliases = HashMap::new();
        for (tid, (ty, genargs)) in self.normalized_aliases {
            let alias_name_span = self.alias_defs.get(&tid).unwrap().ident.span.clone();

            aliases.insert(
                tid,
                TypeAliasDefContent {
                    genargs,
                    right: ty,
                    alias_name_span,
                },
            );
        }

        Ok(aliases)
    }

    // alias 定義の右辺の型の名前解決を行う
    fn try_resolve_rhs_typ_name(&mut self, hir: &Hir) -> RsvResult<()> {
        for (tid, alias) in &self.alias_defs {
            let (ty, genargs) = match &alias.right.val {
                TypReprVal::Primitive(p) => {
                    (ty_from_primitive(p, alias.right.span.clone()), Vec::new())
                }
                TypReprVal::Defined(_) => {
                    let mctx = self
                        .mctxes
                        .get(alias.ident.span.module())
                        .expect("compiler bug: module not found");

                    //  type alias の左辺で宣言されたジェネリクス型を解決するために
                    //  TyDefLevelTyResolveCtx を構成
                    //  ```
                    //  type Foo[T] = Bar[T, Int];
                    //           ^        ^
                    //  ```
                    let tdctx = TyDefLevelTyResolveCtx::new(mctx, &alias.genargs)?;

                    (
                        tdctx.try_resolve_ty(&alias.right, hir)?,
                        tdctx.ty_def_genarg_vec,
                    )
                }
            };

            self.name_resolved_aliases
                .insert(tid.clone(), (ty, genargs, alias.ident.span.clone()));
        }

        Ok(())
    }

    // 名前解決のみ完了した alias のリストについて
    // alias only な循環参照がないことを検査する
    fn check_cycles(&self) -> RsvResult<()> {
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        for (tid, (_, _, span)) in &self.name_resolved_aliases {
            self.dfs_cycle(tid, span.clone(), &mut visiting, &mut visited)?;
        }

        Ok(())
    }

    fn dfs_cycle(
        &self,
        tid: &TyId,
        span: Span,
        visiting: &mut HashSet<TyId>,
        visited: &mut HashSet<TyId>,
    ) -> RsvResult<()> {
        // 探索済みノードにあるなら
        // すなわちその type alias の依存について探索済みであるので
        // 現在の対象ノードである type alias がその依存に含まれない
        // 直ちに Ok
        if visited.contains(tid) {
            return Ok(());
        }

        // 現在の対象ノードである type alias が
        // すでに依存を探索中のノードにあるなら
        // 依存は循環している
        //  TODO: visiting が各 type alias の Span を保持すれば、
        //  エラーリターン時に含まれるすべてを返すことで依存関係すべてを示せる
        //  なおその場合、
        //  以下のような依存グラフ
        //  A -> B,
        //  B -> C,
        //  B -> D,
        //  D -> A,
        //  ```
        //  A ---> B ---> C
        //  ^    /
        //  \   /
        //  \  v
        //   D
        //  ```
        //  では、
        //  C は循環とは関係ないが表示されるという問題もある
        if !visiting.insert(tid.clone()) {
            return Err(ResolveError::CyclingTypeAlias {
                tid: Box::new(tid.clone()),
                detected_position: Box::new(span),
            });
        }

        // 依存を再帰的に探索
        if let Some((aliased_ty, _, _)) = self.name_resolved_aliases.get(tid) {
            let mut deps = Vec::new();
            collect_alias_deps(aliased_ty, &mut deps);

            for dep in deps {
                if let Some((_, _, span)) = self.name_resolved_aliases.get(&dep) {
                    self.dfs_cycle(&dep, span.clone(), visiting, visited)?;
                }
            }
        }

        visiting.remove(tid);
        visited.insert(tid.clone());

        Ok(())
    }

    // 名前解決済みでかつ循環参照がないことを検証済みの alias に対して、
    // 最も具体な型に正規化する
    fn normalize(&mut self) {
        for (tid, (aliased_ty, genargs, _)) in &self.name_resolved_aliases {
            self.normalized_aliases.insert(
                tid.clone(),
                (self.normalize_ty(aliased_ty.clone()), genargs.clone()),
            );
        }
    }

    fn normalize_ty(&self, ty: Ty) -> Ty {
        match ty.kind {
            TyKind::Defined(defined_ty) => {
                // すでに正規化済みならそれを使用
                if let Some((normalized_ty, genargs)) = self.normalized_aliases.get(&defined_ty.tid)
                {
                    // generic arity check
                    if genargs.len() != defined_ty.genargs.len() {
                        panic!("generic argument mismatch");
                    }

                    // GenTyId -> TyKind の割り当て
                    let assigns = genargs
                        .iter()
                        .cloned()
                        .zip(defined_ty.genargs.iter().map(|ty| ty.kind.clone()))
                        .collect::<HashMap<GenTyId, TyKind>>();

                    // ジェネリクス型を代入して具体化する
                    normalized_ty.clone().embody_by_gen_ty_id(&assigns)
                } else if let Some((aliased_ty, genargs, _)) =
                    self.name_resolved_aliases.get(&defined_ty.tid)
                {
                    // 正規化されていない場合

                    // generic arity check
                    if genargs.len() != defined_ty.genargs.len() {
                        panic!("generic argument mismatch");
                    }

                    // GenTyId -> Ty の割り当て
                    let assigns = genargs
                        .iter()
                        .cloned()
                        .zip(defined_ty.genargs.iter().map(|ty| ty.kind.clone()))
                        .collect::<HashMap<GenTyId, TyKind>>();

                    // ジェネリクス型を代入して具体化する
                    let embodied_ty = aliased_ty.clone().embody_by_gen_ty_id(&assigns);

                    // alias されている右辺の型について再帰的に正規化
                    self.normalize_ty(embodied_ty)
                } else {
                    // name_resolved_aliases にないならば
                    // type alias 以外のユーザ定義型(struct, enum)
                    // であることは名前解決時に保証済み
                    Ty::new(
                        TyKind::Defined(DefinedTy {
                            tid: defined_ty.tid.clone(),
                            genargs: defined_ty
                                .genargs
                                .into_iter()
                                .map(|t| self.normalize_ty(t))
                                .collect(),
                        }),
                        ty.span,
                    )
                }
            }
            TyKind::Fn(fty) => Ty::new(
                TyKind::Fn(FnTy {
                    args: fty
                        .args
                        .into_iter()
                        .map(|ty| self.normalize_ty(ty))
                        .collect(),
                    rty: Box::new(self.normalize_ty(*fty.rty)),
                    genargs: fty.genargs,
                }),
                ty.span,
            ),
            _ => ty,
        }
    }
}

// 型の依存関係にあるユーザ定義型(のTyId)を収集する
fn collect_alias_deps(ty: &Ty, tids: &mut Vec<TyId>) {
    match &ty.kind {
        TyKind::Defined(defined_ty) => {
            tids.push(defined_ty.tid.clone());

            for arg in &defined_ty.genargs {
                collect_alias_deps(arg, tids);
            }
        }
        TyKind::Fn(fty) => {
            for arg in &fty.args {
                collect_alias_deps(arg, tids);
            }

            collect_alias_deps(&fty.rty, tids);
        }
        TyKind::Int
        | TyKind::Float
        | TyKind::Bool
        | TyKind::Void
        | TyKind::Infer(_)
        | TyKind::Gen(_)
        | TyKind::LocGen(_) => {}
    }
}
