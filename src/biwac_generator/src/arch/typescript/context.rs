use std::collections::HashMap;
use std::sync::Arc;

use biwac_base::{IdentInterner, InternedIdent, PackageId, SourceHolder};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{ExprId, Hir, Ty, ValDefKind};
use biwac_lang_item::{LangItem, LangItemTable};
use biwac_span::{TyDefId, ValDefId, VarId};

use crate::mangle::Mangler;

pub(super) struct AstBuildCtx<'a> {
    hir: &'a Hir,
    /// シンボル名の生成。ターゲットに依存しないので共有している。
    mangle: Mangler<'a>,
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
            mangle: Mangler::new(hir, interner, srcs, ext_pkgs),
            lang_items,
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
