use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, hash_map::Entry},
};

pub(crate) mod def_id;
pub(crate) mod symbols;
pub(crate) mod types;

use biwac_base::{ModPath, PackageName};
use biwac_span::Span;

use crate::{
    AssocCallee, DefinedTy, FnDefContentBody, FnDefContentSignature, FnTy, GenTyId, HirError,
    HirResult, Ident, ImplValDefContentKind, InferTy, LocGenTyId, NativeCode, StructDefContent, Ty,
    TyDefContentKind, TyId, TyKind, ValDefContentKind, ValId,
};

// Progressive は漸進的に値が更新されていくことを示す
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progressive<Y, C> {
    NotYet(Y),
    Completed(C),
}

//  Hir は
//  High-level Intermediate Representation (高レベル中間表現) である
//
//  以下の漸進的な進行を、
//  パッケージ全体のすべての情報を集約して保持し、
//  この内部に変更を繰り返し加えることで結果を蓄積していくことで達成する
//
//  各進行状態を保持するために Progressive による不完全な状態が存在する
//  適切な順序で呼び出していくことが必要
//  不正な順序で呼び出すと直ちにコンパイラはエラー終了する
//
//  1. 名前解決
//      1. すべての型について
//          1. 型の存在だけ登録
//          2. 実体(シグニチャ)を登録
//      2. 各種関数、関連関数、メソッドについて
//          1. 存在を登録
//              - シグニチャの型を解決しつつ
//              - 重複を検査したうえで
//                  - 関数については、関数、変数の名前空間での名前の重複検査をする
//                  - 関連関数、メソッドについては、とりあえず実装対象の型の存在を確認する
//          2. 関数内の名前解決を行い(
//              関連関数呼び出しは、普通の関数 -> 型の関連関数の順でこの段階でも解決できる
//              またはケースの制約を導入するか。
//              メソッド呼び出しは、型推論が必要なためこの時点では解決しない
//              )、
//          登録する
//  2. 各種関数類の関数内の型推論
//      1. 関連関数、メソッドについて、実装対象の型についてimplの重複を検査する(変更は加えない)
//      2. 各種関数、関連関数、メソッドについて、内部の型推論を行う
#[derive(Debug, Clone)]
pub struct Hir {
    pub pkg_name: PackageName,

    // 値名前空間 value namespace 内の一意なシンボルの集合
    // - 関数
    // - グローバル変数(const)
    // が含まれる
    // 外部パッケージの値は予め登録される
    pub vals: HashMap<ValId, ValDefContentKind>,

    // 型の定義とその実装
    // e.g.) struct, enum
    // ほとんど、型名前空間 type namespace 内の一意なシンボルの集合と言える
    // 外部パッケージの値は予め登録される
    pub tys: HashMap<TyId, DefinedTyImpl>,

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

    // パッケージ内に存在するモジュールの集合
    pub modules: HashSet<ModPath>,

    pub module_global_natives: HashMap<ModPath, Vec<NativeCode>>,

    // 外部パッケージのシンボルで、
    // 使用されていることを確認したシンボル
    pub deps_recorder: RefCell<DepsRecorder>,
}

#[derive(Debug, Clone)]
pub struct DefinedTyImpl {
    // length of genargs
    pub ty_content: Progressive<TyExistence, TyDefContentKind>,
    // ある関連値名(メンバ名、関連関数名、関連定数名)と、
    // 各ジェネリック引数列に対する実装の実体、のマップ
    pub vals: HashMap<String, TyValImplList>,
}

#[derive(Debug, Clone)]
pub struct SpecialTyImpl {
    pub vals: HashMap<String, ImplValDefContentKind>,
}

// ジェネリック引数列と、実体の組のリスト
#[derive(Debug, Clone)]
pub struct TyValImplList {
    pub vals: HashMap<ImplValId, TyValImplGenargsContentPair>,
}

// 型に対する実装の値(関連値)のシンボル
// - 関連関数
// - メソッド
// - 関連定数(const)
// のうち同一の名前内で一意なid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImplValId(usize);

impl ImplValId {
    fn new(id: usize) -> Self {
        Self(id)
    }

    pub fn value(&self) -> usize {
        self.0
    }
}

// ジェネリック引数列と、実体の組
#[derive(Debug, Clone)]
pub struct TyValImplGenargsContentPair {
    pub impl_block_genargs: HashMap<String, (LocGenTyId, Span)>,
    pub genargs: Vec<Ty>,
    pub val_content: ImplValDefContentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TyExistence {
    pub ty_name_span: Span,
    pub genarg_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PkgId {
    name: PackageName,
}

impl Hir {
    pub fn new(
        pkg_name: PackageName,
        external_tys: HashMap<TyId, TyDefContentKind>,
        external_vals: HashMap<ValId, FnDefContentSignature>,
    ) -> Self {
        let mut vals = HashMap::new();
        let mut tys = HashMap::new();

        // NOTE:
        // std のコンパイル時にはlang itemは登録しない
        // FIXME:
        // もっとマシな方法で std であることを検出
        if pkg_name.value() != "std" {
            for item in crate::lang_item::default_lang_items() {
                match item.kind {
                    crate::lang_item::LangItemKind::Ty { tid, genarg_len } => {
                        tys.insert(
                            tid,
                            DefinedTyImpl {
                                // TODO: とりあえず struct ということにしている
                                // lang item 側により情報をもたせ、struct 以外も作れるようにする
                                ty_content: Progressive::Completed(TyDefContentKind::Struct(
                                    Box::new(StructDefContent {
                                        members: HashMap::new(),
                                        genargs: (0..genarg_len).map(GenTyId::new).collect(),
                                        struct_name_span: item.span.clone(),
                                    }),
                                )),
                                vals: HashMap::new(),
                            },
                        );
                    }
                    crate::lang_item::LangItemKind::Val { vid, val } => match val {
                        crate::lang_item::LangItemVal::Fn { signature } => {
                            vals.insert(vid, ValDefContentKind::ExternalFn(signature));
                        }
                    },
                }
            }
        }

        vals.extend(
            external_vals
                .into_iter()
                .map(|(vid, fsign)| (vid, ValDefContentKind::ExternalFn(Box::new(fsign)))),
        );

        tys.extend(external_tys.into_iter().map(|(tid, ty)| {
            (
                tid,
                DefinedTyImpl {
                    ty_content: Progressive::Completed(ty),
                    vals: HashMap::new(),
                },
            )
        }));

        Self {
            deps_recorder: RefCell::new(DepsRecorder::new(pkg_name.clone())),
            pkg_name,
            vals,
            tys,
            special_ty_impls: HashMap::new(),
            modules: HashSet::new(),
            module_global_natives: HashMap::new(),
        }
    }

    // モジュールの存在を登録する
    pub fn register_module_existence(&mut self, module: ModPath) -> HirResult<()> {
        self.modules.insert(module);

        Ok(())
    }

    // 型の存在を登録する
    // NOTE: 型はすべて、その存在自体は値名前空間の登録よりも前に行われなければならない
    pub fn register_type_existence(
        &mut self,
        tid: TyId,
        ty_existence: TyExistence,
    ) -> HirResult<()> {
        match self.tys.entry(tid.clone()) {
            Entry::Vacant(e) => {
                e.insert(DefinedTyImpl {
                    ty_content: Progressive::NotYet(ty_existence),
                    vals: HashMap::new(),
                });

                Ok(())
            }
            Entry::Occupied(e) => Err(HirError::DuplicatedTypeName {
                tid: Box::new(tid),
                defined_position1: Box::new(match &e.get().ty_content {
                    Progressive::NotYet(ty_existence) => ty_existence.ty_name_span.clone(),
                    Progressive::Completed(ty_content) => match ty_content {
                        TyDefContentKind::Struct(struct_) => struct_.struct_name_span.clone(),
                        TyDefContentKind::TypeAlias(alias) => alias.alias_name_span.clone(),
                        TyDefContentKind::NativeTypeAlias(native) => native.alias_name_span.clone(),
                    },
                }),
                defined_position2: Box::new(ty_existence.ty_name_span),
            }),
        }
    }

    // 型の実体を登録する
    pub fn register_type_content(
        &mut self,
        tid: &TyId,
        ty_content: TyDefContentKind,
    ) -> HirResult<()> {
        let defined_ty_impl = self.tys.get_mut(tid).expect("compiler bug: type not found");

        match defined_ty_impl.ty_content {
            Progressive::NotYet(_) => {
                // 依存関係を記録
                match &ty_content {
                    TyDefContentKind::Struct(struct_) => {
                        for m in struct_.members.values() {
                            self.deps_recorder.borrow_mut().depends_on_ty(m);
                        }
                    }
                    TyDefContentKind::TypeAlias(alias) => {
                        self.deps_recorder.borrow_mut().depends_on_ty(&alias.right);
                    }
                    TyDefContentKind::NativeTypeAlias(_) => {
                        // nothing to do
                    }
                }

                defined_ty_impl.ty_content = Progressive::Completed(ty_content);

                Ok(())
            }
            Progressive::Completed(_) => {
                panic!("compiler bug: type content already registered")
            }
        }
    }

    // 型の存在を取得する
    pub fn get_type_existence(&self, tid: &TyId) -> Option<TyExistence> {
        self.tys.get(tid)?.ty_content.as_type_existence()
    }

    // 値(fn, const)の存在およびシグニチャを登録する
    // TODO: モジュール名との重複を検査
    //  fn foo :: bar :: baz()
    //     ^^^^^^^^^^    ^^^
    //     module        fn
    //  foo :: bar :: baz.biwa
    //  ^^^^^^^^^^^^^^^^^
    //  module
    //  は衝突する
    pub fn register_value_existence(
        &mut self,
        vid: ValId,
        val_content: ValDefContentKind,
    ) -> HirResult<()> {
        // シグニチャに使われている型を依存として記録
        if let Some(fsign) = match &val_content {
            ValDefContentKind::Fn(f) => Some(&f.signature),
            ValDefContentKind::Native(f) => Some(&f.signature),
            ValDefContentKind::NovelScene(n) => Some(&n.signature),
            ValDefContentKind::ExternalFn(_) => None,
        } {
            self.deps_recorder.borrow_mut().register_from_fn_sign(fsign);
        }

        match self.vals.entry(vid.clone()) {
            Entry::Vacant(e) => {
                e.insert(val_content);

                Ok(())
            }
            Entry::Occupied(e) => Err(HirError::DuplicatedValueName {
                vid: Box::new(vid),
                defined_position1: Box::new(match &e.get() {
                    ValDefContentKind::Fn(f) => f.fn_name_span.clone().into(),
                    ValDefContentKind::Native(f) => f.fn_name_span.clone().into(),
                    ValDefContentKind::NovelScene(n) => n.scene_name_span.clone().into(),
                    ValDefContentKind::ExternalFn(f) => f.span.clone(),
                }),
                defined_position2: Box::new(match val_content {
                    ValDefContentKind::Fn(f) => f.fn_name_span.clone().into(),
                    ValDefContentKind::Native(f) => f.fn_name_span.clone().into(),
                    ValDefContentKind::NovelScene(n) => n.scene_name_span.clone().into(),
                    ValDefContentKind::ExternalFn(f) => f.span.clone(),
                }),
            }),
        }
    }

    // 値(fn, const)定義を登録する
    // 存在と定義が別のフェーズで登録されるのは関数のみ
    pub fn register_value_definition(
        &mut self,
        vid: &ValId,
        fn_body: FnDefContentBody,
    ) -> HirResult<()> {
        let val_def_content_kind = self
            .vals
            .get_mut(vid)
            .expect("compiler bug: type not found");

        match val_def_content_kind {
            ValDefContentKind::Fn(f) => match f.body {
                Progressive::NotYet(_) => {
                    f.body = Progressive::Completed(fn_body);

                    Ok(())
                }
                Progressive::Completed(_) => {
                    panic!("compiler bug: type content already registered")
                }
            },
            ValDefContentKind::NovelScene(f) => match f.body {
                Progressive::NotYet(_) => {
                    f.body = Progressive::Completed(fn_body);

                    Ok(())
                }
                Progressive::Completed(_) => {
                    panic!("compiler bug: type content already registered")
                }
            },
            ValDefContentKind::Native(_) => {
                panic!("compiler bug: native function cannot be registered its body")
            }
            ValDefContentKind::ExternalFn(_) => {
                panic!("compiler bug: external function cannot be registered its body")
            }
        }
    }

    // 型に対する
    // 値(fn, const)の実装の存在およびシグニチャを登録する
    // 型は、ジェネリック引数列が排他である場合は別とみなしてimplを登録する
    pub fn register_impl_value_existence(
        &mut self,
        ty: TyKind,
        impl_block_genargs: HashMap<String, (LocGenTyId, Span)>,
        ident: &biwac_ast::Ident,
        val_content: ImplValDefContentKind,
    ) -> HirResult<()> {
        // シグニチャに使われている型を依存として記録
        let fsign = match &val_content {
            ImplValDefContentKind::Fn(f) => &f.signature,
            ImplValDefContentKind::NativeFn(f) => &f.signature,
            ImplValDefContentKind::Method(f) => &f.signature,
            ImplValDefContentKind::NativeMethod(f) => &f.signature,
        };
        self.deps_recorder.borrow_mut().register_from_fn_sign(fsign);

        match ty {
            TyKind::Defined(defined_ty) => {
                // 型の存在を取得し、ジェネリック引数の長さの一致を検査
                let ty_existence = self
                    .get_type_existence(&defined_ty.tid)
                    .expect("compiler bug: type not found");

                if defined_ty.genargs.len() != ty_existence.genarg_len {
                    return Err(HirError::GenericArgLengthMismatched {
                        defined_ty: Box::new(defined_ty.clone()),
                        ty_existence: Box::new(ty_existence),
                    });
                }

                let defined_ty_impl = self
                    .tys
                    .get_mut(&defined_ty.tid)
                    .expect("compiler bug: type not found");

                // type alias なら解決した先の型に登録
                if let Some(ty) =
                    resolve_ty_alias(&defined_ty, defined_ty_impl.ty_content.expect_completed())?
                {
                    self.register_impl_value_existence(
                        ty.kind,
                        impl_block_genargs,
                        ident,
                        val_content,
                    )
                } else {
                    // すでに同名の関連値名(メンバ名、関連関数名、関連定数名)が登録されているとき、
                    // ジェネリック引数列の重複検査をして登録
                    if let Some(impl_list) = defined_ty_impl.vals.get_mut(&ident.id) {
                        // 既存のすべての実装に対し、ジェネリック引数列の重複検査
                        for impl_ in impl_list.vals.values() {
                            // SAFETY:
                            // 同じTyIdに対する登録なので、ジェネリック引数列の長さの同一は保証されている
                            // そもそも型のジェネリック引数列が長さ0のとき、常に重複。
                            // Iterator::all()
                            // は空のイテレータに対してはtrueを返すため、これは達成される
                            // https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.all
                            // > An empty iterator returns true.
                            if impl_
                                .genargs
                                .iter()
                                .zip(defined_ty.genargs.iter())
                                .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind))
                            {
                                // 引数列すべてが重複判定なら、重複
                                return Err(HirError::DuplicatedImplementationForType {
                                    defined_ty: Box::new(defined_ty),
                                    ty_existence: Box::new(ty_existence),
                                    val_content1: Box::new(impl_.val_content.clone()),
                                    val_content2: Box::new(val_content),
                                });
                            }
                        }

                        // 重複がなかった場合
                        impl_list.vals.insert(
                            ImplValId::new(impl_list.vals.len()),
                            TyValImplGenargsContentPair {
                                genargs: defined_ty.genargs,
                                val_content,
                                impl_block_genargs,
                            },
                        );

                        Ok(())
                    } else {
                        // その関連値名の登録が初めてであるとき、自明に重複検査を必要としない
                        defined_ty_impl.vals.insert(
                            ident.id.clone(),
                            TyValImplList {
                                vals: [(
                                    ImplValId::new(0),
                                    TyValImplGenargsContentPair {
                                        genargs: defined_ty.genargs,
                                        val_content,
                                        impl_block_genargs,
                                    },
                                )]
                                .into(),
                            },
                        );

                        Ok(())
                    }
                }
            }
            TyKind::Int | TyKind::Float | TyKind::Bool => {
                if !impl_block_genargs.is_empty() {
                    panic!("compiler bug: primitive type has no generic arguments")
                }

                if let Some(ty_impl) = self.special_ty_impls.get_mut(&ty) {
                    match ty_impl.vals.entry(ident.id.clone()) {
                        Entry::Vacant(e) => {
                            e.insert(val_content);

                            Ok(())
                        }
                        Entry::Occupied(e) => {
                            Err(HirError::DuplicatedImplementationForSpecialType {
                                ty: Box::new(ty),
                                val_content1: Box::new(e.get().clone()),
                                val_content2: Box::new(val_content),
                            })
                        }
                    }
                } else {
                    self.special_ty_impls.insert(
                        ty,
                        SpecialTyImpl {
                            vals: [(ident.id.clone(), val_content)].into(),
                        },
                    );

                    Ok(())
                }
            }
            TyKind::LocGen(_) => todo!(),
            TyKind::Void | TyKind::Fn(_) | TyKind::Gen(_) => {
                panic!("impl not supported for this type")
            }
            TyKind::Infer(_) => panic!("implementation target type must be absolute"),
        }
    }

    // 型に対する
    // 値(fn)の実装の実体(関数のボディ)を登録する
    pub fn register_impl_value_definition(
        &mut self,
        tid: &TyId,
        value_name: &str,
        impl_vid: &ImplValId,
        fn_body: FnDefContentBody,
    ) -> HirResult<()> {
        let defined_ty_impl = self.tys.get_mut(tid).expect("compiler bug: type not found");
        // alias なら解決先の型について探索する
        if let Some(ty) = resolve_ty_alias(
            &DefinedTy {
                tid: tid.clone(),
                genargs: vec![
                    // alias を検索するのにしか使われない
                    // genargs のため、適当な値を入れる
                    Ty {
                        kind: TyKind::Infer(InferTy::Unknown),
                        span: SSpan::External { pkg: self.pkg_name.clone(), modu: ModPath::Lib },
                    };
                    defined_ty_impl
                        .ty_content
                        .as_type_existence()
                        .unwrap()
                        .genarg_len
                ],
            },
            defined_ty_impl.ty_content.expect_completed(),
        )? {
            match ty.kind {
                TyKind::Defined(aliased_defined_ty) => {
                    self.register_impl_value_definition(
                        &aliased_defined_ty.tid,
                        value_name,
                        impl_vid,
                        fn_body,
                    )?;
                }
                _ => {
                    self.register_special_impl_value_definition(&ty.kind, value_name, fn_body)?;
                }
            }
        } else {
            let impl_list = defined_ty_impl
                .vals
                .get_mut(value_name)
                .expect("compiler bug: implementation for type not found");
            let impl_content = impl_list
                .vals
                .get_mut(impl_vid)
                .expect("compiler bug: implementation for type not found");
            match &mut impl_content.val_content {
                ImplValDefContentKind::Fn(f) => match f.body {
                    Progressive::NotYet(_) => {
                        f.body = Progressive::Completed(fn_body);
                    }
                    Progressive::Completed(_) => {
                        panic!("compiler bug: already registered function body")
                    }
                },
                ImplValDefContentKind::Method(m) => match m.body {
                    Progressive::NotYet(_) => {
                        m.body = Progressive::Completed(fn_body);
                    }
                    Progressive::Completed(_) => {
                        panic!("compiler bug: already registered function body")
                    }
                },
                ImplValDefContentKind::NativeFn(_) => {
                    panic!(
                        "compiler bug: native associated function must not be registered its body"
                    )
                }
                ImplValDefContentKind::NativeMethod(_) => {
                    panic!("compiler bug: native method must not be registered its body")
                }
            }
        }

        Ok(())
    }

    // プリミティブ型など特殊な型に対する
    // 値(fn)の実装の実体(関数のボディ)を登録する
    pub fn register_special_impl_value_definition(
        &mut self,
        ty: &TyKind,
        value_name: &str,
        fn_body: FnDefContentBody,
    ) -> HirResult<()> {
        match self
            .special_ty_impls
            .get_mut(ty)
            .expect("compiler bug: implementation not registered for this type")
            .vals
            .get_mut(value_name)
            .expect("compiler bug: implementation not registered for this type")
        {
            ImplValDefContentKind::Fn(f) => match f.body {
                Progressive::NotYet(_) => {
                    f.body = Progressive::Completed(fn_body);
                }
                Progressive::Completed(_) => {
                    panic!("compiler bug: already registered function body")
                }
            },
            ImplValDefContentKind::Method(m) => match m.body {
                Progressive::NotYet(_) => {
                    m.body = Progressive::Completed(fn_body);
                }
                Progressive::Completed(_) => {
                    panic!("compiler bug: already registered function body")
                }
            },
            ImplValDefContentKind::NativeFn(_) => {
                panic!("compiler bug: native associated function must not be registered its body")
            }
            ImplValDefContentKind::NativeMethod(_) => {
                panic!("compiler bug: native method must not be registered its body")
            }
        }

        Ok(())
    }

    // ある型に対する値の実装のidを取得する
    // 種類(関連関数、メソッド、関連定数)は問わない
    pub fn get_impl_value_id_of_type(
        &self,
        ty: &TyKind,
        value_name: &String,
    ) -> HirResult<Option<ImplValId>> {
        match ty {
            TyKind::Defined(defined_ty) => {
                let defined_ty_impl = self
                    .tys
                    .get(&defined_ty.tid)
                    .expect("compiler bug: type not found");

                // alias なら解決先の型について探索する
                if let Some(ty) =
                    resolve_ty_alias(defined_ty, defined_ty_impl.ty_content.expect_completed())?
                {
                    self.get_impl_value_id_of_type(&ty.kind, value_name)
                } else {
                    // ジェネリック引数列が重複する(一致する)ものを探す
                    defined_ty_impl
                        .vals
                        .get(value_name)
                        .and_then(|impl_list| {
                            impl_list
                                .vals
                                .iter()
                                .find(|(_, impl_)| {
                                    impl_.genargs.iter().zip(defined_ty.genargs.iter()).all(
                                        |(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind),
                                    )
                                })
                                .map(|(impl_vid, impl_)| match &impl_.val_content {
                                    ImplValDefContentKind::Fn(_) => Ok(*impl_vid),
                                    ImplValDefContentKind::Method(_) => Ok(*impl_vid),
                                    ImplValDefContentKind::NativeFn(_) => Ok(*impl_vid),
                                    ImplValDefContentKind::NativeMethod(_) => Ok(*impl_vid),
                                })
                        })
                        .transpose()
                }
            }
            _ => Ok(None),
        }
    }

    // メソッドのシグニチャ(FnTy)を取得
    // ただし、第一引数selfはその型自体であり、型推論時に必要ないので含まない
    // Ident は caller 側の場所を保持したIdent
    pub fn get_method_of_type(&self, ty: &TyKind, method: &Ident) -> HirResult<Option<Ty>> {
        match ty {
            TyKind::Defined(defined_ty) => {
                let defined_ty_impl = self
                    .tys
                    .get(&defined_ty.tid)
                    .expect("compiler bug: type not found");

                if let Some(impl_list) = defined_ty_impl.vals.get(&method.id) {
                    // ジェネリック引数列が重複する(一致する)ものを探す
                    impl_list
                        .vals
                        .values()
                        .find(|impl_| {
                            impl_
                                .genargs
                                .iter()
                                .zip(defined_ty.genargs.iter())
                                .all(|(t1, t2)| t1.kind.is_duplicated_for_impl_genarg(&t2.kind))
                        })
                        .map(|impl_| match &impl_.val_content {
                            ImplValDefContentKind::Fn(_) => {
                                Err(HirError::ImplementedValueIsNotMethod {
                                    ty: Box::new(ty.clone()),
                                    method: Box::new(method.clone()),
                                    val_content: Box::new(impl_.val_content.clone()),
                                })
                            }
                            ImplValDefContentKind::NativeFn(_) => {
                                Err(HirError::ImplementedValueIsNotMethod {
                                    ty: Box::new(ty.clone()),
                                    method: Box::new(method.clone()),
                                    val_content: Box::new(impl_.val_content.clone()),
                                })
                            }
                            ImplValDefContentKind::Method(m) => {
                                // LocGenTyId -> Ty の割り当てがあれば具体化する
                                let mut assigns = HashMap::new();
                                for (t1, t2) in impl_.genargs.iter().zip(defined_ty.genargs.iter())
                                {
                                    if let TyKind::LocGen(lgid) = &t1.kind {
                                        assigns.insert(*lgid, t2.kind.clone());
                                    }
                                }

                                Ok(Ty::new(
                                    TyKind::Fn(FnTy {
                                        args: m
                                            .signature
                                            .args
                                            .iter()
                                            .map(|(_, ty)| {
                                                ty.clone().embody_by_loc_gen_ty_id(&assigns)
                                            })
                                            .collect(),
                                        rty: Box::new(
                                            m.signature
                                                .rty
                                                .clone()
                                                .embody_by_loc_gen_ty_id(&assigns),
                                        ),
                                        genargs: m
                                            .signature
                                            .genargs
                                            .iter()
                                            .map(|(_, lgid)| *lgid)
                                            .collect(),
                                    }),
                                    m.signature.span.clone(),
                                ))
                            }
                            ImplValDefContentKind::NativeMethod(m) => {
                                // LocGenTyId -> Ty の割り当てがあれば具体化する
                                let mut assigns = HashMap::new();
                                for (t1, t2) in impl_.genargs.iter().zip(defined_ty.genargs.iter())
                                {
                                    if let TyKind::LocGen(lgid) = &t1.kind {
                                        assigns.insert(*lgid, t2.kind.clone());
                                    }
                                }

                                Ok(Ty::new(
                                    TyKind::Fn(FnTy {
                                        args: m
                                            .signature
                                            .args
                                            .iter()
                                            .map(|(_, ty)| {
                                                ty.clone().embody_by_loc_gen_ty_id(&assigns)
                                            })
                                            .collect(),
                                        rty: Box::new(
                                            m.signature
                                                .rty
                                                .clone()
                                                .embody_by_loc_gen_ty_id(&assigns),
                                        ),
                                        genargs: m
                                            .signature
                                            .genargs
                                            .iter()
                                            .map(|(_, lgid)| *lgid)
                                            .collect(),
                                    }),
                                    m.signature.span.clone(),
                                ))
                            }
                        })
                        .transpose()
                } else {
                    Ok(None)
                }
            }
            TyKind::Int | TyKind::Float | TyKind::Bool => {
                if let Some(ty_impl) = self.special_ty_impls.get(ty)
                    && let Some(val) = ty_impl.vals.get(&method.id)
                {
                    match val {
                        ImplValDefContentKind::Fn(_) => {
                            Err(HirError::ImplementedValueIsNotMethod {
                                ty: Box::new(ty.clone()),
                                method: Box::new(method.clone()),
                                val_content: Box::new(val.clone()),
                            })
                        }
                        ImplValDefContentKind::NativeFn(_) => {
                            Err(HirError::ImplementedValueIsNotMethod {
                                ty: Box::new(ty.clone()),
                                method: Box::new(method.clone()),
                                val_content: Box::new(val.clone()),
                            })
                        }
                        ImplValDefContentKind::Method(m) => Ok(Some(Ty::new(
                            TyKind::Fn(FnTy {
                                args: m.signature.args.iter().map(|(_, ty)| ty).cloned().collect(),
                                rty: Box::new(m.signature.rty.clone()),
                                genargs: m
                                    .signature
                                    .genargs
                                    .iter()
                                    .map(|(_, lgid)| *lgid)
                                    .collect(),
                            }),
                            m.signature.span.clone(),
                        ))),
                        ImplValDefContentKind::NativeMethod(m) => Ok(Some(Ty::new(
                            TyKind::Fn(FnTy {
                                args: m.signature.args.iter().map(|(_, ty)| ty).cloned().collect(),
                                rty: Box::new(m.signature.rty.clone()),
                                genargs: m
                                    .signature
                                    .genargs
                                    .iter()
                                    .map(|(_, lgid)| *lgid)
                                    .collect(),
                            }),
                            m.signature.span.clone(),
                        ))),
                    }
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }

    // ある型に対する関連関数のシグニチャ(FnTy)を取得する
    pub fn get_assoc_of_type(&self, assoc_callee: &AssocCallee, span: &Span) -> HirResult<Ty> {
        match &assoc_callee.ty.kind {
            TyKind::Defined(defined_ty) => {
                let defined_ty_impl = self
                    .tys
                    .get(&defined_ty.tid)
                    .expect("compiler bug: type not found");

                // alias なら解決先の型について探索する
                if let Some(ty) =
                    resolve_ty_alias(defined_ty, defined_ty_impl.ty_content.expect_completed())?
                {
                    let assoc_callee = AssocCallee {
                        ty,
                        assoc: assoc_callee.assoc.clone(),
                        impl_vid: assoc_callee.impl_vid,
                    };

                    self.get_assoc_of_type(&assoc_callee, span)
                } else {
                    let impl_list = defined_ty_impl
                        .vals
                        .get(&assoc_callee.assoc)
                        .expect("compiler bug: implemented value not found for this name");
                    let impl_ = impl_list
                        .vals
                        .get(&assoc_callee.impl_vid)
                        .expect("compiler bug: implemented value not found");

                    match &impl_.val_content {
                        ImplValDefContentKind::Fn(f) => {
                            Ok(f.signature.as_ty())

                            // WARN: really?
                            //
                            // // LocGenTyId -> Ty の割り当てがあれば具体化する
                            // let mut assigns = HashMap::new();
                            // for (t1, t2) in impl_.genargs.iter().zip(defined_ty.genargs.iter()) {
                            //     if let Ty::LocGen(lgid) = t1 {
                            //         assigns.insert(*lgid, t2.clone());
                            //     }
                            // }
                            //
                            // Ok(FnTy {
                            //     args: f
                            //         .signature
                            //         .args
                            //         .iter()
                            //         .map(|(_, ty)| ty.clone().embody_by_loc_gen_ty_id(&assigns))
                            //         .collect(),
                            //     rty: Box::new(
                            //         f.signature.rty.clone().embody_by_loc_gen_ty_id(&assigns),
                            //     ),
                            //     genargs: f
                            //         .signature
                            //         .genargs
                            //         .iter()
                            //         .map(|(_, lgid)| *lgid)
                            //         .collect(),
                            // })
                        }
                        ImplValDefContentKind::NativeFn(f) => Ok(f.signature.as_ty()),
                        ImplValDefContentKind::Method(_)
                        | ImplValDefContentKind::NativeMethod(_) => {
                            Err(HirError::ImplementedValueIsNotAssoc {
                                assoc_callee: Box::new(assoc_callee.clone()),
                                caller_span: Box::new(span.clone().into()),
                                val_content: Box::new(impl_.val_content.clone()),
                            })
                        }
                    }
                }
            }
            TyKind::Int | TyKind::Float | TyKind::Bool => {
                let val_content = self
                    .special_ty_impls
                    .get(&assoc_callee.ty.kind)
                    .expect("compiler bug: ty impl not found")
                    .vals
                    .get(&assoc_callee.assoc)
                    .expect("compiler bug: ty imple value not found");

                match &val_content {
                    ImplValDefContentKind::Fn(f) => Ok(f.signature.as_ty()),
                    ImplValDefContentKind::NativeFn(f) => Ok(f.signature.as_ty()),
                    ImplValDefContentKind::Method(_) | ImplValDefContentKind::NativeMethod(_) => {
                        Err(HirError::ImplementedValueIsNotAssoc {
                            assoc_callee: Box::new(assoc_callee.clone()),
                            caller_span: Box::new(span.clone().into()),
                            val_content: Box::new(val_content.clone()),
                        })
                    }
                }
            }
            TyKind::Void
            | TyKind::Gen(_)
            | TyKind::LocGen(_)
            | TyKind::Infer(_)
            | TyKind::Fn(_) => {
                // error
                todo!()
            }
        }
    }

    // 型の実体を取得する
    pub fn get_type_definition(&self, tid: &TyId) -> Option<&TyDefContentKind> {
        match &self.tys.get(tid)?.ty_content {
            Progressive::NotYet(_) => panic!("compiler bug: type definition not registered yet"),
            Progressive::Completed(ty_content) => Some(ty_content),
        }
    }

    pub fn register_module_native_code(
        &mut self,
        modpath: ModPath,
        native: &biwac_ast::NativeCode,
    ) {
        if let Some(e) = self.module_global_natives.get_mut(&modpath) {
            e.push(NativeCode::from(native));
        } else {
            self.module_global_natives
                .insert(modpath, vec![NativeCode::from(native)]);
        }
    }

    pub fn get_fn_sign(&self, vid: &ValId) -> Option<&FnDefContentSignature> {
        // 依存関係を記録
        self.deps_recorder.borrow_mut().depends_on_val(vid);

        self.vals.get(vid).map(|val| match &val {
            ValDefContentKind::Fn(f) => &f.signature,
            ValDefContentKind::Native(f) => &f.signature,
            ValDefContentKind::NovelScene(n) => &n.signature,
            ValDefContentKind::ExternalFn(f) => f,
        })
    }
}

impl<Y, C> Progressive<Y, C> {
    pub fn expect_completed(&self) -> &C {
        match self {
            Self::NotYet(_) => {
                panic!("compiler bug: progressive registration not yet")
            }
            Self::Completed(c) => c,
        }
    }
}

// alias なら解決先の型を返す
fn resolve_ty_alias(
    defined_ty: &DefinedTy,
    ty_content: &TyDefContentKind,
) -> HirResult<Option<Ty>> {
    if let TyDefContentKind::TypeAlias(alias) = ty_content {
        if alias.genargs.len() == defined_ty.genargs.len() {
            let assigns = alias
                .genargs
                .iter()
                .cloned()
                .zip(defined_ty.genargs.iter().map(|ty| ty.kind.clone()))
                .collect::<HashMap<_, _>>();

            Ok(Some(alias.right.clone().embody_by_gen_ty_id(&assigns)))
        } else {
            Err(HirError::GenericArgLengthMismatched {
                defined_ty: Box::new(defined_ty.clone()),
                ty_existence: Box::new(TyExistence {
                    ty_name_span: alias.alias_name_span.clone(),
                    genarg_len: alias.genargs.len(),
                }),
            })
        }
    } else {
        Ok(None)
    }
}

impl Progressive<TyExistence, TyDefContentKind> {
    fn as_type_existence(&self) -> Option<TyExistence> {
        match &self {
            Progressive::NotYet(ty_existence) => Some(ty_existence.clone()),
            Progressive::Completed(ty_content) => match ty_content {
                TyDefContentKind::Struct(struct_) => Some(TyExistence {
                    ty_name_span: struct_.struct_name_span.clone(),
                    genarg_len: struct_.genargs.len(),
                }),
                TyDefContentKind::TypeAlias(alias) => Some(TyExistence {
                    ty_name_span: alias.alias_name_span.clone(),
                    genarg_len: alias.genargs.len(),
                }),
                TyDefContentKind::NativeTypeAlias(native) => Some(TyExistence {
                    ty_name_span: native.alias_name_span.clone(),
                    genarg_len: native.genargs.len(),
                }),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct DepsRecorder {
    pkg_name: PackageName,
    depended_tys: HashSet<TyId>,
    depended_vals: HashSet<ValId>,
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
        if defined_ty.tid.pkg().name() != &self.pkg_name {
            self.depended_tys.insert(defined_ty.tid.clone());
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

    fn register_from_fn_sign(&mut self, fsign: &FnDefContentSignature) {
        for (_, aty) in &fsign.args {
            self.depends_on_ty(aty);
        }
        self.depends_on_ty(&fsign.rty);
    }

    pub fn depends_on_val(&mut self, vid: &ValId) {
        if vid.pkg().name() != &self.pkg_name {
            self.depended_vals.insert(vid.clone());
        }
    }

    pub fn depended_tys(&self) -> &HashSet<TyId> {
        &self.depended_tys
    }

    pub fn depended_vals(&self) -> &HashSet<ValId> {
        &self.depended_vals
    }
}

impl PkgId {
    pub fn new(name: PackageName) -> Self {
        Self { name }
    }

    pub fn name(&self) -> &PackageName {
        &self.name
    }
}
