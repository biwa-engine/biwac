//! シンボル名のマングリング。
//!
//! ターゲットに依存しない。TypeScript の識別子としても、
//! wasm モジュール内部の名前としても同じものを使う。
//! ホスト側の実装がターゲット間で対応付けやすくなる。
//!
//! 形は Itanium C++ ABI に倣った `_ZN<長さ><名前>...E` である。
//! 名前の長さを前置するので、区切り文字を使わずに一意に分解できる。
//!
//! 外部パッケージのシンボルは HIR に無く span もダミーなので、
//! 名前とモジュールパスは `.biwameta` から引く。

use std::sync::Arc;

use biwac_base::{IdentInterner, InternedIdent, ModPath, PackageId, SourceHolder};
use biwac_dependency_metadata::DepMetadata;
use biwac_hir::{AssocValDefKind, DefinedTy, Hir, Ident, Ty, TyDefKind, TyKind, ValDefKind};
use biwac_span::{DefId, Span, TraitDefId, TyDefId, ValDefId};

pub struct Mangler<'a> {
    hir: &'a Hir,
    interner: &'a IdentInterner,
    srcs: &'a SourceHolder,
    /// 依存パッケージのメタデータ。
    /// 外部シンボルは HIR に無く、span もダミーなので、
    /// マングリングに必要な名前とモジュールパスはここから引く。
    ext_pkgs: &'a [(PackageId, Arc<DepMetadata>)],
}

impl<'a> Mangler<'a> {
    pub fn new(
        hir: &'a Hir,
        interner: &'a IdentInterner,
        srcs: &'a SourceHolder,
        ext_pkgs: &'a [(PackageId, Arc<DepMetadata>)],
    ) -> Self {
        Self {
            hir,
            interner,
            srcs,
            ext_pkgs,
        }
    }

    fn find_ext_dep(&self, pkg_id: PackageId) -> Option<&Arc<DepMetadata>> {
        self.ext_pkgs
            .iter()
            .find(|(pid, _)| *pid == pkg_id)
            .map(|(_, d)| d)
    }

    /// 外部パッケージのシンボルの (名前, モジュールパス) を引く。
    fn ext_symbol_info(&self, def_id: &DefId) -> Option<(&str, ModPath)> {
        self.find_ext_dep(def_id.pkg())?
            .symbol_mangling_info(def_id.local_idx())
    }

    fn pkg_name_of(&self, pkg_id: PackageId) -> &str {
        let interned = self
            .hir
            .packages
            .get(&pkg_id)
            .expect("compiler bug: unknown package id");
        self.interner.get_str(interned).unwrap()
    }

    pub fn str_of(&self, interned: &InternedIdent) -> &str {
        self.interner.get_str(interned).unwrap()
    }
    pub fn get_package_name_of_type(&self, def_id: &TyDefId) -> &str {
        self.pkg_name_of(def_id.pkg())
    }

    pub fn get_package_name_of_value(&self, def_id: &ValDefId) -> &str {
        self.pkg_name_of(def_id.pkg())
    }

    fn get_value_ident(&self, def_id: &ValDefId) -> &Ident {
        if let Some(val) = self.hir.vals.get(def_id) {
            match val {
                ValDefKind::Fn(fn_def) => &fn_def.name,
                ValDefKind::Native(fn_def) => &fn_def.name,
                ValDefKind::NovelScene(scene_def) => &scene_def.name,
            }
        } else {
            let (ty_def_id, assoc_name) = self.hir.assoc_val_map.get(def_id).unwrap();
            match &self
                .hir
                .tys
                .get(ty_def_id)
                .unwrap()
                .vals
                .get(assoc_name)
                .unwrap()
                .vals
                .get(def_id)
                .unwrap()
                .val_content
            {
                AssocValDefKind::Fn(fn_def) => &fn_def.name,
                AssocValDefKind::NativeFn(fn_def) => &fn_def.name,
            }
        }
    }

    fn get_type_ident(&self, def_id: &TyDefId) -> &Ident {
        match self
            .hir
            .tys
            .get(def_id)
            .unwrap()
            .ty_content
            .as_ref()
            .unwrap()
        {
            TyDefKind::Struct(struct_def) => &struct_def.name,
            TyDefKind::Enum(enum_def) => &enum_def.name,
            TyDefKind::NativeTypeAlias(alias_def) => &alias_def.name,
        }
    }
    // --- シンボル名のマングリング ---
    //
    // マングル名は「その定義がグローバルな名前ツリーのどこに当たるか」だけで決まる。
    // どこに書かれたか (どの package / module に impl ブロックがあるか) は使わない。
    //
    //  <symbol>  ::= "_Z" "N" <path> "E"
    //  <path>    ::= <pkg> <mod>* <name>              トップレベル関数 / 型定義
    //              | <owner> <name>                   関連関数 / メソッド
    //              | <owner> "R" <trait> <name>       trait impl の項目
    //  <owner>   ::= <pkg> <mod>* <name> <genargs>?   定義された型が対象
    //              | <prim>                           プリミティブ型が対象 (ツリーの根)
    //  <genargs> ::= "I" <type>+ "E"
    //  <type>    ::= <prim>
    //              | "T" <index> "_"                          未具体化のジェネリック引数
    //              | "N" <pkg> <mod>* <name> <genargs>? "E"   定義された型
    //              | "F" <type> <type>* "E"                   関数型 (戻り値, 引数列)
    //  <trait>   ::= <pkg> <mod>* <name>
    //  <prim>    ::= "i" | "f" | "b" | "v"
    //  <name>    ::= <len> <ident>
    //
    // trait 成分を挟むのは、別々のパッケージが同じ型に同じ名前を
    // 生やしたときに衝突するからである。
    //  ```
    //  package a: trait Foo { fn bar(self) -> Int; }  impl std::String: Foo { .. }
    //  package b: trait Baz { fn bar(self) -> Int; }  impl std::String: Baz { .. }
    //  ```
    // `R` は名前の長さ前置 (数字始まり) とも型タグとも衝突しない。
    //
    // ネストの N...E とジェネリック引数の I...E、長さ前置は
    // Itanium C++ ABI と Rust v0 に倣っている。
    // 長さは 1-9 で始まり型タグは数字で始まらないので、区切り文字なしで曖昧にならない。
    // 生成物は TypeScript の識別子になるため使う文字は [A-Za-z0-9_] に収めてある。

    pub fn get_value_mangled(&self, def_id: &ValDefId) -> String {
        let mut path = String::new();

        match self.impl_self_ty_of_assoc(def_id) {
            // 関連関数・メソッド: 対象型のツリー上の位置にぶら下がる
            Some(self_ty) => {
                self.push_owner(&mut path, &self_ty);
                if let Some(trait_def_id) = self.trait_of_assoc(def_id) {
                    path.push('R');
                    self.push_module_path_and_name(&mut path, &trait_def_id.def_id(), || {
                        self.get_trait_ident(&trait_def_id)
                    });
                }
                push_name(&mut path, self.assoc_val_name(def_id));
            }
            // トップレベル関数
            None => self.push_module_path_and_name(&mut path, &def_id.def_id(), || {
                self.get_value_ident(def_id)
            }),
        }

        format!("_ZN{path}E")
    }

    pub fn get_type_mangled(&self, def_id: &TyDefId) -> String {
        let mut path = String::new();
        self.push_module_path_and_name(&mut path, &def_id.def_id(), || self.get_type_ident(def_id));

        format!("_ZN{path}E")
    }

    /// `<pkg> <mod>* <name>` を積む。自パッケージは HIR の span から、
    /// 外部パッケージはメタデータから引く。
    fn push_module_path_and_name<'i>(
        &'i self,
        out: &mut String,
        def_id: &DefId,
        self_pkg_ident: impl FnOnce() -> &'i Ident,
    ) {
        if let Some((name, modu)) = self.ext_symbol_info(def_id) {
            push_pkg_and_mods(out, self.pkg_name_of(def_id.pkg()), &modu);
            push_name(out, name);
            return;
        }

        let ident = self_pkg_ident();
        let module = self.srcs.mods.get(&ident.span.module()).unwrap();
        let pkg_name_interned = self.hir.packages.get(&module.pkg_id).unwrap();

        push_pkg_and_mods(
            out,
            self.interner.get_str(pkg_name_interned).unwrap(),
            &module.modu,
        );
        push_name(out, self.interner.get_str(&ident.id).unwrap());
    }

    /// impl の対象型を `<owner>` として積む。
    fn push_owner(&self, out: &mut String, self_ty: &Ty) {
        match &self_ty.kind {
            TyKind::Defined(dt) => {
                self.push_module_path_and_name(out, &dt.def_id.def_id(), || {
                    self.get_type_ident(&dt.def_id)
                });
                self.push_genargs(out, &dt.genargs, &mut PlaceholderNumbering::default());
            }
            // プリミティブ型は言語組み込みでどの package にも属さないため、
            // ツリーの根に位置する。package / module は付かない。
            // これにより `impl Int { fn sqrt }` はグローバルに一意な名前になる。
            _ => out.push_str(prim_tag(&self_ty.kind).unwrap_or_else(|| {
                panic!(
                    "compiler bug: cannot implement methods on {:?}",
                    self_ty.kind
                )
            })),
        }
    }

    fn push_genargs(&self, out: &mut String, genargs: &[Ty], nums: &mut PlaceholderNumbering) {
        if genargs.is_empty() {
            return;
        }

        out.push('I');
        for g in genargs {
            self.push_type(out, g, nums);
        }
        out.push('E');
    }

    fn push_type(&self, out: &mut String, ty: &Ty, nums: &mut PlaceholderNumbering) {
        if let Some(tag) = prim_tag(&ty.kind) {
            out.push_str(tag);
            return;
        }

        match &ty.kind {
            TyKind::Defined(dt) => {
                out.push('N');
                self.push_module_path_and_name(out, &dt.def_id.def_id(), || {
                    self.get_type_ident(&dt.def_id)
                });
                self.push_genargs(out, &dt.genargs, nums);
                out.push('E');
            }
            // まだ具体化されていないジェネリック引数。
            // monomorphization を実装すればここは具体型で埋まり、T<n>_ は現れなくなる。
            TyKind::Gen(id) => out.push_str(&nums.placeholder(id.def_id())),
            TyKind::LocGen(id) => out.push_str(&nums.placeholder(id.def_id())),
            TyKind::Fn(f) => {
                out.push('F');
                self.push_type(out, &f.rty, nums);
                for a in &f.args {
                    self.push_type(out, a, nums);
                }
                out.push('E');
            }
            TyKind::Infer(_) => {
                panic!("compiler bug: failed to infer type appearing in a symbol name")
            }
            TyKind::Int | TyKind::Float | TyKind::Bool | TyKind::Void => unreachable!(),
        }
    }

    /// 関連関数・メソッドなら impl の self 型を返す。トップレベル関数なら `None`。
    fn impl_self_ty_of_assoc(&self, def_id: &ValDefId) -> Option<Ty> {
        if !def_id.pkg().is_self() {
            return self
                .find_ext_dep(def_id.pkg())?
                .assoc_impl_self_ty(def_id.local_idx(), def_id.pkg());
        }

        // 自パッケージ: 所属する型と impl 対象ジェネリック引数から組み立てる。
        let (ty_def_id, name) = self.hir.assoc_val_map.get(def_id)?;
        let genargs = &self
            .hir
            .tys
            .get(ty_def_id)?
            .vals
            .get(name)?
            .vals
            .get(def_id)?
            .genargs;

        Some(Ty::new(
            prim_ty_kind(ty_def_id).unwrap_or_else(|| {
                TyKind::Defined(DefinedTy {
                    def_id: *ty_def_id,
                    genargs: genargs.clone(),
                })
            }),
            Span::dummy(),
        ))
    }

    /// その関連アイテムが trait impl のものなら、その trait。
    ///
    /// 直接の impl なら `None`。マングル名に trait 成分を挟むかどうかを決める。
    fn trait_of_assoc(&self, def_id: &ValDefId) -> Option<TraitDefId> {
        if !def_id.pkg().is_self() {
            return self
                .find_ext_dep(def_id.pkg())?
                .assoc_trait_of(def_id.local_idx(), def_id.pkg());
        }

        let (ty_def_id, name) = self.hir.assoc_val_map.get(def_id)?;
        self.hir
            .tys
            .get(ty_def_id)?
            .vals
            .get(name)?
            .vals
            .get(def_id)?
            .trait_of
    }

    fn get_trait_ident(&self, def_id: &TraitDefId) -> &Ident {
        &self
            .hir
            .traits
            .get(def_id)
            .expect("compiler bug: trait not found for mangling")
            .name
    }

    fn assoc_val_name(&self, def_id: &ValDefId) -> &str {
        if let Some((name, _)) = self.ext_symbol_info(&def_id.def_id()) {
            return name;
        }

        self.interner
            .get_str(&self.get_value_ident(def_id).id)
            .unwrap()
    }
}

/// 未具体化のジェネリック引数への添字割り当て。
///
/// impl 対象ジェネリック引数列を左から見て初出順に 0, 1, ... を振る。
/// 添字が無いと `impl[T,U] Pair[T,U]` と `impl[T] Pair[T,T]` が同名になってしまう。
///
/// 宣言順の序数ではなく初出順にしているのは、
/// 自パッケージ側と依存側で番号の取得経路が別になり規約がずれるのを避けるため。
/// 初出順ならどちらもジェネリック引数列 1 本だけから決まる。
#[derive(Default)]
struct PlaceholderNumbering {
    assigned: Vec<DefId>,
}

impl PlaceholderNumbering {
    fn placeholder(&mut self, id: DefId) -> String {
        let idx = match self.assigned.iter().position(|x| *x == id) {
            Some(i) => i,
            None => {
                self.assigned.push(id);
                self.assigned.len() - 1
            }
        };

        format!("T{idx}_")
    }
}

fn prim_tag(kind: &TyKind) -> Option<&'static str> {
    match kind {
        TyKind::Int => Some("i"),
        TyKind::Float => Some("f"),
        TyKind::Bool => Some("b"),
        TyKind::Void => Some("v"),
        _ => None,
    }
}

/// 予約済みの [`TyDefId`] ならプリミティブの [`TyKind`] に戻す。
fn prim_ty_kind(def_id: &TyDefId) -> Option<TyKind> {
    match *def_id {
        TyDefId::INT_TY_DEF_ID => Some(TyKind::Int),
        TyDefId::FLOAT_TY_DEF_ID => Some(TyKind::Float),
        TyDefId::BOOL_TY_DEF_ID => Some(TyKind::Bool),
        TyDefId::VOID_TY_DEF_ID => Some(TyKind::Void),
        _ => None,
    }
}

fn push_name(out: &mut String, name: &str) {
    out.push_str(&format!("{}{}", name.len(), name));
}

fn push_pkg_and_mods(out: &mut String, pkg_name: &str, module_path: &ModPath) {
    push_name(out, pkg_name);

    match module_path {
        ModPath::Main | ModPath::Lib => {}
        ModPath::Mod(path) => {
            for p in path {
                push_name(out, p);
            }
        }
    }
}
