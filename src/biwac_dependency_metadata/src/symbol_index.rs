use std::collections::HashMap;

use biwac_base::ModPath;
use biwac_span::{GenDefId, LocalGenDefId, TyDefId, ValDefId};

/// パッケージローカルな DefId と、`.biwameta` 上のシンボル索引の対応。
///
/// `.biwameta` はシンボルを
/// `[struct][native type alias][assoc fn][top-level fn][mod]` の順に並べ直して
/// 0 から採番し、**消費側はその索引をそのまま [`biwac_span::PackageLocalDefId`] として使う**
/// ([`crate::metadata::module_view`] を参照)。
/// つまり「下流から見たこのパッケージのシンボルの DefId」を決めているのはこの表である。
///
/// `.biwamir` も同じ索引でシンボルを参照するので、
/// この表が `.biwameta` と `.biwamir` の唯一の情報源になる。
///
/// 表を作るのは [`crate::DepMetadata::new`] で、
/// エンコードの過程で決まった採番をそのまま記録して返す。
/// 採番規則を二重に書くことがないようにするためである。
#[derive(Debug, Clone, Default)]
pub struct SymbolIndexMap {
    ty: HashMap<TyDefId, u32>,
    val: HashMap<ValDefId, u32>,
    /// 型定義のジェネリック引数 → (所属シンボルの索引, 序数)
    ty_genarg: HashMap<GenDefId, (u32, u32)>,
    /// (所属する関数のシンボル索引, ローカルジェネリック引数) → 序数
    ///
    /// 序数は `impl_genargs ++ signature.genargs` の位置である。
    ///
    /// 所属する関数までを鍵にするのは、`.biwameta` が
    /// **関数ごとに独立した id を合成する**ためである。
    /// HIR では `impl[T] Vec[T] { fn new(); fn push(); }` の `T` は
    /// 1 つの [`LocalGenDefId`] を共有するが、
    /// メタデータ上は `new` の `T` と `push` の `T` が別の id になる
    /// (シグニチャを 1 つずつ自己完結させているため)。
    /// したがって「どの関数から見た `T` か」まで決めないと索引が定まらない。
    fn_genarg: HashMap<(u32, LocalGenDefId), u32>,
    module: HashMap<ModPath, u32>,
}

impl SymbolIndexMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ty(&self, def_id: &TyDefId) -> Option<u32> {
        self.ty.get(def_id).copied()
    }

    pub fn val(&self, def_id: &ValDefId) -> Option<u32> {
        self.val.get(def_id).copied()
    }

    /// (所属シンボルの索引, 序数)
    pub fn ty_genarg(&self, def_id: &GenDefId) -> Option<(u32, u32)> {
        self.ty_genarg.get(def_id).copied()
    }

    /// `owner` の関数から見たときの序数。
    pub fn fn_genarg(&self, owner: u32, def_id: &LocalGenDefId) -> Option<u32> {
        self.fn_genarg.get(&(owner, *def_id)).copied()
    }

    pub fn module(&self, path: &ModPath) -> Option<u32> {
        self.module.get(path).copied()
    }

    pub(crate) fn insert_ty(&mut self, def_id: TyDefId, sym_idx: u32) {
        self.ty.insert(def_id, sym_idx);
    }

    pub(crate) fn insert_val(&mut self, def_id: ValDefId, sym_idx: u32) {
        self.val.insert(def_id, sym_idx);
    }

    pub(crate) fn insert_ty_genarg(&mut self, def_id: GenDefId, owner: u32, ordinal: u32) {
        self.ty_genarg.insert(def_id, (owner, ordinal));
    }

    pub(crate) fn insert_fn_genarg(&mut self, owner: u32, def_id: LocalGenDefId, ordinal: u32) {
        self.fn_genarg.insert((owner, def_id), ordinal);
    }

    pub(crate) fn insert_module(&mut self, path: ModPath, sym_idx: u32) {
        self.module.insert(path, sym_idx);
    }
}
