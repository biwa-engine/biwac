use std::collections::{HashMap, hash_map::Entry};

pub(crate) mod symbols;
pub(crate) mod types;

use biwac_base::Span;

use crate::{
    FnDefContentBody, HirError, HirResult, ImplValDefContentKind, LocVarId, Ty, TyDefContentKind,
    TyId, ValDefContentKind, ValId,
};

// Progressive は漸進的に値が更新されていくことを示す
#[derive(Debug, Clone)]
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
pub struct Hir {
    // 値名前空間 value namespace 内の一意なシンボルの集合
    // - 関数
    // - グローバル変数(const)
    // が含まれる
    vals: HashMap<ValId, ValDefContentKind>,

    // 型の定義とその実装
    // e.g.) struct, enum
    // ほとんど、型名前空間 type namespace 内の一意なシンボルの集合と言える
    tys: HashMap<TyId, DefinedTyImpl>,
    //
    // // プリミティブ型やジェネリック型など
    // // 特殊な型に対する実装
    // // ```
    // //  impl Int {
    // //      fn foo(self) { ... }
    // //  }
    // //
    // //  impl[T] T: Into[T] {
    // //      [[inline]]
    // //      fn into(self) { self }
    // //  }
    // // ```
    // special_ty_impls: HashMap<Ty, SpecialTyImpl>,
}

#[derive(Debug, Clone)]
pub struct DefinedTyImpl {
    pub ty_content: Progressive<Span, TyDefContentKind>,
    pub vals: HashMap<String, ImplValDefContentKind>,
}

#[derive(Debug, Clone)]
pub struct SpecialTyImpl {
    pub vals: HashMap<String, ImplValDefContentKind>,
}

impl Hir {
    // TODO:
    // - impl 重複チェック をしたうえで、関連関数、メソッドを登録
    // - 関数、関連関数、メソッドについて、その中での型推論をすべて行う(型推論のエントリーポイント)

    // 型の存在を登録する
    pub fn register_type_existence(&mut self, tid: TyId, span: Span) -> HirResult<()> {
        // NOTE: 型はすべて、その存在自体は値名前空間の登録よりも前に行われなければならない
        assert!(self.vals.is_empty());

        match self.tys.entry(tid.clone()) {
            Entry::Vacant(e) => {
                e.insert(DefinedTyImpl {
                    ty_content: Progressive::NotYet(span),
                    vals: HashMap::new(),
                });

                Ok(())
            }
            Entry::Occupied(e) => Err(HirError::DuplicatedTypeName {
                tid: Box::new(tid),
                defined_position1: Box::new(match &e.get().ty_content {
                    Progressive::NotYet(span) => span.clone(),
                    Progressive::Completed(ty_content) => match ty_content {
                        TyDefContentKind::Struct(struct_) => struct_.struct_name_ident.span.clone(),
                    },
                }),
                defined_position2: Box::new(span),
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
                defined_ty_impl.ty_content = Progressive::Completed(ty_content);

                Ok(())
            }
            Progressive::Completed(_) => {
                panic!("compiler bug: type content already registered")
            }
        }
    }

    // 値(fn, const)の存在およびシグニチャを登録する
    pub fn register_value_existence(
        &mut self,
        vid: ValId,
        val_content: ValDefContentKind,
    ) -> HirResult<()> {
        match self.vals.entry(vid.clone()) {
            Entry::Vacant(e) => {
                e.insert(val_content);

                Ok(())
            }
            Entry::Occupied(e) => Err(HirError::DuplicatedValueName {
                vid: Box::new(vid),
                defined_position1: Box::new(match &e.get() {
                    ValDefContentKind::Fn(f) => f.fn_name_span.clone(),
                    ValDefContentKind::Native(f) => f.fn_name_span.clone(),
                }),
                defined_position2: Box::new(match val_content {
                    ValDefContentKind::Fn(f) => f.fn_name_span.clone(),
                    ValDefContentKind::Native(f) => f.fn_name_span.clone(),
                }),
            }),
        }
    }

    // 値(fn, const)定義を登録する
    // 存在と定義が別のフェーズで登録されるのは関数のみ
    pub fn register_val_definition(
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
            ValDefContentKind::Native(_) => {
                panic!("compiler bug: native function cannot be registered its body")
            }
        }
    }

    // 関数内の変数の型を更新する
    pub fn update_variable_ty(&mut self, fid: &ValId, var: &LocVarId, ty: Ty) {
        if let Some(f) = self.vals.get_mut(fid) {
            match f {
                ValDefContentKind::Fn(f) => {
                    if let Some(var) = f.vars.get_mut(var) {
                        var.ty = ty;
                    }
                }
                ValDefContentKind::Native(_) => {
                    // nothing to do
                }
            }
        }
    }
}
