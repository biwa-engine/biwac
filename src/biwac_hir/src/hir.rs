use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

pub(crate) mod symbols;
pub(crate) mod types;

use biwac_base::ModId;
use biwac_base::{InternedIdent, PackageId, PackageName};
use biwac_span::{
    LocalGenDefId, Span, TraitAssocDefId, TraitDefId, TyDefId, ValDefId, VariantDefId,
};

use crate::{
    AssocValDefKind, DefinedTy, NativeCode, TraitAssocOwner, TraitDef, Ty, TyDefKind, TyKind,
    TypeAliasDef, ValDefKind, VariantOwner,
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

    /// バリアントから親の enum への逆引き。
    ///
    /// `VariantDefId` だけでは親も添字も分からない。
    /// `Color::Red` ならパスから親を辿れるが、
    /// `import ..::Color::Red;` して `Red` と書いた形では辿れないので、
    /// 経路を 1 本にするために常にこの表を使う。
    ///
    /// 外部パッケージ分は `.biwameta` から遅延で引く
    /// (`biwac_type_inferrer::TyCtx`)。
    pub variant_owners: HashMap<VariantDefId, VariantOwner>,

    /// 自パッケージで宣言された trait。
    pub traits: HashMap<TraitDefId, TraitDef>,

    /// trait の項目から親の trait への逆引き。
    /// `variant_owners` と同じく [`Hir::new`] が `traits` から作る。
    pub trait_assoc_owners: HashMap<TraitAssocDefId, TraitAssocOwner>,

    /// モジュールごとの、名前解決に使える trait。
    ///
    /// そのモジュールで宣言されたものと `import` されたものの和である。
    /// trait 越しの関連関数・メソッドは、この一覧にある trait からしか引けない。
    ///
    /// 自パッケージのモジュールだけを持つ。
    /// 外部パッケージのコードは既に解決済みなので要らない。
    pub trait_scopes: HashMap<ModId, Vec<TraitDefId>>,

    pub module_global_natives: Vec<NativeCode>,

    /// self package の assoc fn ValDefId -> (TyDefId, method 名 InternedIdent) マップ
    pub assoc_val_map: HashMap<ValDefId, (TyDefId, InternedIdent)>,

    // 外部パッケージのシンボルで、
    // 使用されていることを確認したシンボル
    pub deps_recorder: RefCell<DepsRecorder>,
}

#[derive(Debug, Clone)]
pub struct DefinedTyImpl {
    // None = プリミティブ型 (型情報は TyKind が持つ)
    // Some = ユーザ定義型 (Struct / NativeTypeAlias)
    pub ty_content: Option<TyDefKind>,
    // ある関連値名(メンバ名、関連関数名、関連定数名)と、
    // 各ジェネリック引数列に対する実装の実体、のマップ
    //
    // trait impl の項目もここに入る。
    // 直接の impl と区別するのは
    // `TyValImplGenargsContentPair::trait_of` である。
    pub vals: HashMap<InternedIdent, TyValImplList>,
    /// この型に対する trait impl の索引。
    /// 項目の実体は `vals` の側にあり、ここは名前から引くための表でしかない。
    pub trait_impls: Vec<TyTraitImpl>,
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
    /// trait impl の項目なら、その trait。直接の impl なら `None`。
    ///
    /// 直接の探索 (パス解決の `TyNameTree::children` と
    /// `TyCtx::get_method_def_id`) は `None` のものだけを見る。
    /// `Some` のものはスコープにある trait を経由してしか引けない。
    /// これが import 規則の実体である。
    pub trait_of: Option<TraitDefId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TyExistence {
    pub ty_name_span: Span,
    pub genarg_len: usize,
}

//  ある型に対する 1 つの trait impl。
//
//  ```biwa
//  impl[T, U] Foo[T]: Bar[T, U] {
//  //  ^^^^^^    ^^^     ^^^^^^
//  //  |         |       trait_genargs
//  //  |         ty_genargs
//  //  impl_block_genargs
//    ...
//  }
//  ```
//
//  項目の実体は `DefinedTyImpl::vals` にある。ここは索引でしかない。
//
//  type inferrer は `<expression>.foo()` に対して
//  1. `<expression>` の型を推論する
//  2. メソッド `.foo()` を直接の impl から探す
//  --- 見つからなければ trait solver に委譲
//  3. その型に impl された trait の一覧 (`DefinedTyImpl::trait_impls`) を引く
//  4. そのスコープで使える trait と `TraitDefId` で積を取る
//  5. `TraitDefId` から項目名を引き、`foo` に一致するものを集める
//  6. `ty_genargs` が適合するものに絞る
//  7. ちょうど 1 つ残れば成功
//
//  biwac では path の名前解決は型推論とブートストラップしないので、
//  name resolver も `<type>::foo()` に対して同じ手順を踏める。
#[derive(Debug, Clone)]
pub struct TyTraitImpl {
    pub trait_def_id: TraitDefId,
    pub impl_block_genargs: HashMap<InternedIdent, (LocalGenDefId, Span)>,
    pub ty_genargs: Vec<Ty>,
    pub trait_genargs: Vec<Ty>,
    /// 項目名 -> 実装の `ValDefId`。実体は `DefinedTyImpl::vals` の側にある。
    pub vals: HashMap<InternedIdent, ValDefId>,
    /// `impl` の行の span。エラー表示に使う。
    pub span: Span,
}

impl Hir {
    pub fn new(
        pkg_name: PackageName,
        pkg_names: HashMap<PackageId, InternedIdent>,
        tys: HashMap<TyDefId, DefinedTyImpl>,
        vals: HashMap<ValDefId, ValDefKind>,
        ty_aliases: HashMap<TyDefId, TypeAliasDef>,
        native_codes: Vec<NativeCode>,
        traits: HashMap<TraitDefId, TraitDef>,
        trait_scopes: HashMap<ModId, Vec<TraitDefId>>,
    ) -> Self {
        // 所属する型ではなく、**関連アイテム自身の `ValDefId`** で
        // このパッケージのものかを決める。
        //
        // 外部パッケージの型に trait を実装できるので、
        // 「型が自パッケージか」では取りこぼす
        // (`.biwameta` の書き出しも同じ基準を使っている)。
        let mut assoc_val_map = HashMap::new();
        for (ty_def_id, ty_impl) in &tys {
            for (method_id, impl_list) in &ty_impl.vals {
                for val_def_id in impl_list.vals.keys() {
                    if !val_def_id.pkg().is_self()
                        && val_def_id.pkg() != PackageId::BUILTIN_RESERVED_PACKAGE
                    {
                        continue;
                    }
                    assoc_val_map.insert(*val_def_id, (*ty_def_id, *method_id));
                }
            }
        }

        // バリアントの逆引きは型定義から導けるので、外から渡さずここで作る。
        let mut variant_owners = HashMap::new();
        for (ty_def_id, ty_impl) in &tys {
            let Some(TyDefKind::Enum(enum_def)) = &ty_impl.ty_content else {
                continue;
            };
            for (index, variant) in enum_def.variants.iter().enumerate() {
                variant_owners.insert(
                    variant.def_id,
                    VariantOwner {
                        enum_def_id: *ty_def_id,
                        index: index as u32,
                    },
                );
            }
        }

        // trait の項目の逆引きも、バリアントと同じく宣言から導ける。
        let mut trait_assoc_owners = HashMap::new();
        for (trait_def_id, trait_def) in &traits {
            for (index, item) in trait_def.items.iter().enumerate() {
                trait_assoc_owners.insert(
                    item.def_id,
                    TraitAssocOwner {
                        trait_def_id: *trait_def_id,
                        index: index as u32,
                    },
                );
            }
        }

        Self {
            deps_recorder: RefCell::new(DepsRecorder::new()),
            pkg_name,
            packages: pkg_names,
            vals,
            tys,
            ty_aliases,
            variant_owners,
            traits,
            trait_assoc_owners,
            trait_scopes,
            assoc_val_map,
            module_global_natives: native_codes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DepsRecorder {
    depended_tys: HashSet<TyDefId>,
    depended_vals: HashSet<ValDefId>,
}

impl DepsRecorder {
    fn new() -> Self {
        Self {
            depended_tys: HashSet::new(),
            depended_vals: HashSet::new(),
        }
    }

    fn depends_on_defined_ty(&mut self, defined_ty: &DefinedTy) {
        // 記録するのは「他パッケージのシンボルへの依存」である。
        // codegen はこれを import 文の生成に使うため、
        // 自パッケージのシンボルを入れると自分自身を import してしまう。
        if !defined_ty.def_id.pkg().is_self() {
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

    pub fn depends_on_val(&mut self, vid: &ValDefId) {
        if !vid.pkg().is_self() {
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
