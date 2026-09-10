//! trait の解決。
//!
//! 関連関数もメソッドも、まず **直接の impl** を探し、
//! 見つからなかったときに初めてここへ来る。
//! 呼ぶ側は 2 つある。
//!
//! - `biwac_name_resolver` — `<type>::foo()` のパス解決
//! - `biwac_type_inferrer` — `<expr>.foo()` のメソッド解決
//!
//! biwa のパス解決は型推論とブートストラップしないので、
//! どちらも同じ手順で解ける。
//!
//! # 環境の引き方
//!
//! 2 つの呼び手は、自パッケージと外部パッケージの引き方が違う。
//! name resolver は `NameTree::ext_pkg_data` から、
//! type inferrer は `TyCtx` の遅延キャッシュから `.biwameta` を読む。
//! そこで環境を [`TraitEnv`] として抽象し、実装は呼ぶ側に任せている。
//! このクレートが `biwac_dependency_metadata` に依存せずに済むのはそのためである。

use biwac_base::InternedIdent;
use biwac_hir::{TraitCond, Ty, TyKind, TyTraitImpl};
use biwac_span::{LocalGenDefId, TraitAssocDefId, TraitDefId, TyDefId, ValDefId};

/// solver が環境に問い合わせること。
///
/// 自パッケージと外部パッケージの区別は実装側が吸収する。
pub trait TraitEnv {
    /// この型に対する trait impl。パッケージを問わない。
    ///
    /// フォールバックの経路でしか呼ばれず、
    /// 1 つの型に付く impl は多くないので所有権ごと返す。
    fn trait_impls_of(&self, ty: TyDefId) -> Vec<TyTraitImpl>;

    /// 使える trait。
    /// 宣言されたものと `import` されたものの和である。
    ///
    /// 環境はモジュール 1 つ分に閉じて作る。
    /// 名前解決はいま辿っているモジュール、
    /// 型推論はいま推論している関数のモジュールに対して作る。
    fn traits_in_scope(&self) -> &[TraitDefId];

    /// この trait がこの名前の項目を宣言していれば、その id。
    fn trait_item(&self, trait_def_id: TraitDefId, name: InternedIdent) -> Option<TraitAssocDefId>;

    /// ジェネリック引数に付いた制限。
    ///
    /// いま推論している関数から見えるものだけを返す。
    fn bounds_of_local_gen(&self, def_id: LocalGenDefId) -> Vec<TraitCond>;
}

/// 解決の結果。
#[derive(Debug, Clone)]
pub enum Solved {
    /// 実装が確定した。
    Impl(ValDefId),
    /// 型がジェネリック引数なので、実装は単相化まで決まらない。
    ///
    /// `cond` はその名前を提供した制限で、
    /// 呼び先のシグニチャを具体化するのに使う。
    Deferred {
        assoc: TraitAssocDefId,
        cond: TraitCond,
    },
}

#[derive(Debug, Clone)]
pub enum TraitSolveError {
    /// スコープにある trait のどれにも見つからなかった。
    NotFound,

    /// 複数の trait が同じ名前を提供していて絞れない。
    Ambiguous { candidates: Vec<TraitDefId> },

    /// スコープに無い trait には実装があった。
    ///
    /// `import` を促せるので、`NotFound` と分けている。
    NotInScope { candidates: Vec<TraitDefId> },

    /// この型には trait を実装できない (関数型など)。
    NotImplementable,

    /// 型が未確定のまま来た。呼ぶ側が先に弾くべきである。
    Unresolved,
}

/// `<type>::foo()` を解く。name resolver から呼ばれる。
///
/// パスにはジェネリック引数を書く構文がまだ無いので、
/// 直接の impl の探索 (`AssocNameTree::find_matched(None, ..)`) と同じく
/// 特殊化では絞らず、候補がちょうど 1 つであることを求める。
pub fn solve_assoc<E: TraitEnv + ?Sized>(
    self_ty: TyDefId,
    name: InternedIdent,
    env: &E,
) -> Result<Solved, TraitSolveError> {
    solve(self_ty, None, name, env)
}

/// `<expr>.foo()` を解く。type inferrer から呼ばれる。
///
/// レシーバの型は推論済みなので、特殊化まで見て絞れる。
pub fn solve_method<E: TraitEnv + ?Sized>(
    receiver_ty: &TyKind,
    name: InternedIdent,
    env: &E,
) -> Result<Solved, TraitSolveError> {
    let ty_def_id = match receiver_ty {
        TyKind::Defined(defined_ty) => defined_ty.def_id,
        TyKind::Int => TyDefId::INT_TY_DEF_ID,
        TyKind::Float => TyDefId::FLOAT_TY_DEF_ID,
        TyKind::Bool => TyDefId::BOOL_TY_DEF_ID,
        TyKind::Void => TyDefId::VOID_TY_DEF_ID,

        // レシーバがジェネリック引数なら、その制限から名前を引く。
        // 実装は単相化まで決まらない。
        TyKind::LocGen(lgid) => {
            return solve_in_bounds(&env.bounds_of_local_gen(*lgid), name, env);
        }

        // 型定義のジェネリック引数への制限は未対応 (第 3 段)。
        TyKind::Gen(_) => return Err(TraitSolveError::NotFound),

        // 関数型への impl は入れない。
        TyKind::Fn(_) => return Err(TraitSolveError::NotImplementable),

        // 呼ぶ側が InsufficientContext で先に弾いている。
        TyKind::Infer(_) => return Err(TraitSolveError::Unresolved),
    };

    let ty_genargs: &[Ty] = match receiver_ty {
        TyKind::Defined(dt) => &dt.genargs,
        _ => &[],
    };

    solve(ty_def_id, Some(ty_genargs), name, env)
}

fn solve<E: TraitEnv + ?Sized>(
    ty_def_id: TyDefId,
    ty_genargs: Option<&[Ty]>,
    name: InternedIdent,
    env: &E,
) -> Result<Solved, TraitSolveError> {
    let in_scope = env.traits_in_scope();

    let mut matched: Vec<(TraitDefId, ValDefId)> = Vec::new();
    let mut out_of_scope: Vec<TraitDefId> = Vec::new();

    for imp in env.trait_impls_of(ty_def_id) {
        let Some(val_def_id) = imp.vals.get(&name).copied() else {
            continue;
        };

        // impl の対象ジェネリック引数が、いま見ている型に適合するか。
        //
        // 直接の impl の探索 (`TyCtx::get_method_def_id`) と同じ判定を使う。
        // 重複する impl は収集の段で禁じてあるので、
        // 重なるものは高々 1 つしか残らない。
        if let Some(ty_genargs) = ty_genargs
            && (imp.ty_genargs.len() != ty_genargs.len()
                || !imp
                    .ty_genargs
                    .iter()
                    .zip(ty_genargs.iter())
                    .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind)))
        {
            continue;
        }

        if in_scope.contains(&imp.trait_def_id) {
            matched.push((imp.trait_def_id, val_def_id));
        } else {
            out_of_scope.push(imp.trait_def_id);
        }
    }

    match matched.as_slice() {
        [(_, val_def_id)] => Ok(Solved::Impl(*val_def_id)),
        [] => {
            if out_of_scope.is_empty() {
                Err(TraitSolveError::NotFound)
            } else {
                out_of_scope.sort_by_key(|t| t.value());
                out_of_scope.dedup();
                Err(TraitSolveError::NotInScope {
                    candidates: out_of_scope,
                })
            }
        }
        _ => Err(TraitSolveError::Ambiguous {
            candidates: matched.into_iter().map(|(t, _)| t).collect(),
        }),
    }
}

/// ジェネリック引数の制限から名前を引く。
///
/// 実装は単相化まで決まらないので [`Solved::Deferred`] を返す。
/// 制限にある trait は `import` の有無を問わない。
/// 制限を書いた時点でその trait は名前解決を通っているからである。
pub fn solve_in_bounds<E: TraitEnv + ?Sized>(
    bounds: &[TraitCond],
    name: InternedIdent,
    env: &E,
) -> Result<Solved, TraitSolveError> {
    let mut matched = Vec::new();

    for cond in bounds {
        if let Some(assoc) = env.trait_item(cond.def_id, name) {
            matched.push((cond.clone(), assoc));
        }
    }

    match matched.len() {
        1 => {
            let (cond, assoc) = matched.into_iter().next().unwrap();
            Ok(Solved::Deferred { assoc, cond })
        }
        0 => Err(TraitSolveError::NotFound),
        _ => Err(TraitSolveError::Ambiguous {
            candidates: matched.into_iter().map(|(c, _)| c.def_id).collect(),
        }),
    }
}

/// 2 つの制限が同じものかを見る。
///
/// blanket impl を禁じてあるので、部分的に重なる制限どうしを
/// 解く必要は無い。trait と、そのジェネリック引数の構造的な一致だけを見る。
pub fn cond_matches(a: &TraitCond, b: &TraitCond) -> bool {
    a.def_id == b.def_id
        && a.genargs.len() == b.genargs.len()
        && a.genargs
            .iter()
            .zip(&b.genargs)
            .all(|(x, y)| x.kind == y.kind)
}
