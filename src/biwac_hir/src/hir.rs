use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

pub(crate) mod symbols;
pub(crate) mod types;

use biwac_base::{InternedIdent, ModPath, PackageId, PackageName};
use biwac_span::{LocalGenDefId, Span, TyDefId, ValDefId};

use crate::{
    AssocValDefKind, DefinedTy, NativeCode, Ty, TyDefKind, TyKind, TypeAliasDef, ValDefKind,
};

///  Hir は
///  High-level Intermediate Representation (高レベル中間表現) である
///
///  名前解決完了後、対象パッケージ自身と、依存するパッケージのシグニチャを保持し、
///  以降の解析,コード生成のためのデータを提供する
///
///  ## 以降の解析,コード生成
///
///  1. analysis unit (各種関数類) 単位での型推論
///
///  2. 型推論完了後、 HIR を元にコード生成
#[derive(Debug, Clone)]
pub struct Hir {
    pub pkg_name: PackageName,

    pub packages: HashMap<PackageId, InternedIdent>,

    // 値名前空間 value namespace 内の一意なシンボルの集合
    // - 関数
    // - グローバル変数(const)
    // が含まれる
    // 外部パッケージの値は予め登録される
    pub vals: HashMap<ValDefId, ValDefKind>,

    // 型の定義とその実装
    // e.g.) struct, enum
    // ほとんど、型名前空間 type namespace 内の一意なシンボルの集合と言える
    // 外部パッケージの値は予め登録される
    pub tys: HashMap<TyDefId, DefinedTyImpl>,

    pub ty_aliases: HashMap<TyDefId, TypeAliasDef>,

    // プリミティブ型やジェネリック型など
    // 特殊な型に対する実装
    // ```
    //  impl Int {
    //      fn foo(self) { ... }
    //  }
    //
    //  impl[T] T: Into[T] {
    //      [[inline]]
    //      fn into(self) { self }
    //  }
    // ```
    pub special_ty_impls: HashMap<TyKind, SpecialTyImpl>,

    pub module_global_natives: HashMap<ModPath, Vec<NativeCode>>,

    // 外部パッケージのシンボルで、
    // 使用されていることを確認したシンボル
    pub deps_recorder: RefCell<DepsRecorder>,
}

#[derive(Debug, Clone)]
pub struct DefinedTyImpl {
    // length of genargs
    pub ty_content: TyDefKind,
    // ある関連値名(メンバ名、関連関数名、関連定数名)と、
    // 各ジェネリック引数列に対する実装の実体、のマップ
    pub vals: HashMap<InternedIdent, TyValImplList>,
}

#[derive(Debug, Clone)]
pub struct SpecialTyImpl {
    pub vals: HashMap<InternedIdent, AssocValDefKind>,
}

// ジェネリック引数列と、実体の組のリスト
#[derive(Debug, Clone)]
pub struct TyValImplList {
    pub vals: HashMap<ValDefId, TyValImplGenargsContentPair>,
}

// ジェネリック引数列と、実体の組
#[derive(Debug, Clone)]
pub struct TyValImplGenargsContentPair {
    pub impl_block_genargs: HashMap<InternedIdent, (LocalGenDefId, Span)>,
    pub genargs: Vec<Ty>,
    pub val_content: AssocValDefKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TyExistence {
    pub ty_name_span: Span,
    pub genarg_len: usize,
}

impl Hir {
    pub fn new(
        pkg_name: PackageName,
        // external_tys: HashMap<TyId, TyDefContentKind>,
        // external_vals: HashMap<ValId, FnDefContentSignature>,
    ) -> Self {
        let vals = HashMap::new();
        let tys = HashMap::new();
        let ty_aliases = HashMap::new();

        Self {
            deps_recorder: RefCell::new(DepsRecorder::new(pkg_name.clone())),
            pkg_name,
            packages: HashMap::new(),
            vals,
            tys,
            ty_aliases,
            special_ty_impls: HashMap::new(),
            module_global_natives: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DepsRecorder {
    pkg_name: PackageName,
    depended_tys: HashSet<TyDefId>,
    depended_vals: HashSet<ValDefId>,
}

impl DepsRecorder {
    fn new(pkg_name: PackageName) -> Self {
        Self {
            pkg_name,
            depended_tys: HashSet::new(),
            depended_vals: HashSet::new(),
        }
    }

    fn depends_on_defined_ty(&mut self, defined_ty: &DefinedTy) {
        if defined_ty.def_id.pkg().is_self() {
            self.depended_tys.insert(defined_ty.def_id);
        }

        for g in &defined_ty.genargs {
            self.depends_on_ty(g);
        }
    }

    pub fn depends_on_ty(&mut self, ty: &Ty) {
        match &ty.kind {
            TyKind::Defined(defined_ty) => {
                self.depends_on_defined_ty(defined_ty);
            }
            TyKind::Fn(fty) => {
                for a in &fty.args {
                    self.depends_on_ty(a);
                }
                self.depends_on_ty(&fty.rty);
            }
            TyKind::Int
            | TyKind::Float
            | TyKind::Bool
            | TyKind::Void
            | TyKind::Gen(_)
            | TyKind::LocGen(_)
            | TyKind::Infer(_) => {}
        }
    }

    // fn register_from_fn_sign(&mut self, fsign: &FnDefContentSignature) {
    //     for (_, aty) in &fsign.args {
    //         self.depends_on_ty(aty);
    //     }
    //     self.depends_on_ty(&fsign.rty);
    // }

    pub fn depends_on_val(&mut self, vid: &ValDefId) {
        if vid.pkg().is_self() {
            self.depended_vals.insert(*vid);
        }
    }

    pub fn depended_tys(&self) -> &HashSet<TyDefId> {
        &self.depended_tys
    }

    pub fn depended_vals(&self) -> &HashSet<ValDefId> {
        &self.depended_vals
    }
}
