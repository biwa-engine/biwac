use std::collections::HashMap;
use std::sync::Arc;

use biwac_base::{IdentInterner, InternedIdent, PackageId, SourceHolder};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{ExprId, Hir, Ty, ValDefKind};
use biwac_span::{TyDefId, ValDefId, VarId};

use crate::mangle::Mangler;

pub(super) struct AstBuildCtx<'a> {
    hir: &'a Hir,
    /// シンボル名の生成。ターゲットに依存しないので共有している。
    mangle: Mangler<'a>,
    pub(super) allocator: &'a oxc_allocator::Allocator,
}

impl<'a> AstBuildCtx<'a> {
    pub(super) fn new(
        hir: &'a Hir,
        interner: &'a IdentInterner,
        srcs: &'a SourceHolder,
        ext_pkgs: &'a [(PackageId, Arc<DepMetadata>)],
        allocator: &'a oxc_allocator::Allocator,
    ) -> Self {
        Self {
            hir,
            mangle: Mangler::new(hir, interner, srcs, ext_pkgs),
            allocator,
        }
    }

    // マングリングは [`Mangler`] に委譲する。
    // wasm バックエンドも同じものを使う。

    pub(super) fn get_value_mangled(&self, def_id: &ValDefId) -> String {
        self.mangle.get_value_mangled(def_id)
    }

    pub(super) fn get_type_mangled(&self, def_id: &TyDefId) -> String {
        self.mangle.get_type_mangled(def_id)
    }

    pub(super) fn str_of(&self, interned: &InternedIdent) -> &str {
        self.mangle.str_of(interned)
    }

    pub(super) fn get_package_name_of_type(&self, def_id: &TyDefId) -> &str {
        self.mangle.get_package_name_of_type(def_id)
    }

    pub(super) fn get_package_name_of_value(&self, def_id: &ValDefId) -> &str {
        self.mangle.get_package_name_of_value(def_id)
    }

    /// この値が scene か。
    ///
    /// scene は generator function として出力されるので、
    /// 呼び出し側は `yield*` で委譲しなければならない
    /// (そのまま呼ぶと本体が走らず generator オブジェクトが返るだけになる)。
    ///
    /// 判定できるのは自パッケージの scene だけである。
    /// 外部パッケージのシンボルは HIR に無く、`.biwameta` も
    /// 関数と scene を区別して持っていない。
    /// パッケージを跨いだ scene 呼び出しを解禁するときは、
    /// メタデータに種別を載せる必要がある。
    pub(super) fn is_scene(&self, def_id: &ValDefId) -> bool {
        matches!(self.hir.vals.get(def_id), Some(ValDefKind::NovelScene(_)))
    }
}

pub(super) struct FnAstBuildCtx<'a> {
    pub(super) var_tys: &'a HashMap<VarId, Ty>,
    /// 式の型。`match` の対象を受ける一時変数に注釈を付けるのに要る。
    pub(super) expr_tys: &'a HashMap<ExprId, Ty>,
    pub(super) stmts: Vec<oxc_ast::ast::Statement<'a>>,
    /// 一時変数の連番。`match` を式として使うときに要る。
    next_temp: usize,
}

impl<'a> FnAstBuildCtx<'a> {
    pub(super) fn new(var_tys: &'a HashMap<VarId, Ty>, expr_tys: &'a HashMap<ExprId, Ty>) -> Self {
        Self {
            var_tys,
            expr_tys,
            stmts: Vec::new(),
            next_temp: 0,
        }
    }

    /// 生成コード用の一時変数名。
    ///
    /// biwa の変数はマングルされて `_ZN..` になるので、この形と衝突しない。
    pub(super) fn alloc_temp(&mut self) -> String {
        let name = format!("__biwa_tmp{}", self.next_temp);
        self.next_temp += 1;
        name
    }
}
