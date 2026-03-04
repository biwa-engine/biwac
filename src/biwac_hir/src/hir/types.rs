use crate::{GenTyId, LocGenTyId, TyId};

// Ty はAST以降各種の検査を行う上での 型 を表す
//  1. 名前解決(biwac_name_resolver)によって、はじめてTyの形で現れる
//      この時点で本来明示的に型が書かれる部分は具体な型が(fnの定義, structのメンバの定義など)、
//      そうでない部分は推論の必要性を表す型などが割り当てられる
//  2. 型推論(biwac_type_inferrer)によって、すべてからTy::Infer(InferTy)が取り除かれる
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Int,
    Float,
    Bool,

    // Void は関数の戻り値などがない(空である, voidである)ことを表す
    // for function return type
    Void,

    // Fn は関数を表す
    //
    // 以下のようにラムダ関数(無名関数, クロージャ)として定義されたものも、
    // ```
    //  let f = (x, y) -> { x + y };
    // ```
    // 以下のようにグローバルシンボルな関数(関数, 関連関数, メソッド)として定義されたものも、
    // ```
    //  fn foo[T](x: Bar[T], y: Int) -> T { ... }
    // ```
    // いずれも型推論上の計算のための型表現としては FnTy に落とし込まれる
    //
    // なお、ラムダ関数は定義された位置において一意な型であり、ジェネリクスの概念はない
    // 型が書いていなくても多相性があるわけではなく、推論により一意に確定される必要があるだけである
    Fn(FnTy),

    // Defined は定義された型を表す
    // DefinedTy.TyIdでパッケージレベルの文脈から型の具体的な定義を取得できる
    Defined(DefinedTy),

    // Gen は型定義におけるジェネリック型を表す
    //
    // `Foo[T, Int]` のようにジェネリック引数列に型を代入している場合、
    // GenTyId -> Ty のマップが作られ、メンバなど各種型はそれにより解決される
    // これはその結果解決される型が`Int`のように完全に具体であるか、
    // `T`のようにジェネリック型(`Ty::LocGen(LocGenTyId)`)であるか、
    // 推論を必要とする型(`Ty::Infer(InferTy)`)であるか、
    // にかかわらず機能する
    //
    // e.g.) `T`, `U`
    // ```
    //  struct Foo[T, U] {
    //            ^^^^^^ 定義位置
    //      x: T,
    //         ^ 使用位置の例
    //      y: Bar[U],
    //            ^^^ 使用位置の例
    //      z: Int,
    //  }
    // ```
    Gen(GenTyId),

    // LocGen は(impl block や fn の)ローカルでのジェネリック型を表す
    // impl block レベルでの文脈と,
    // fn レベルでの文脈の、それぞれはそこで定義したジェネリック型のリストを持つ
    // impl block から fn へ実質的な通し番号である GenTyId が順に振られる
    //
    // e.g.) `T`, `U`
    // ```
    //  impl[T] Foo[T, Int] {
    //       ^      ^ 使用位置の例
    //       | 定義位置
    //      fn bar[U](self) -> (T, U) {
    //             ^           ^^^^^^ 使用位置の例
    //             | 定義位置
    //
    //          let x: U = baz;
    //                 ^ 使用位置の例
    //      }
    //  }
    // ```
    LocGen(LocGenTyId),

    // Infer は型推論で用いられる
    Infer(InferTy),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InferTy {
    // 型推論の途中で型変数が割り当てられていることを示す
    Var(TyVar),

    // 型がまったく未定で型推論を要することを示す
    Unknown,
}

// DefinedTy は
// ユーザ定義型を使用する側から見て、
// 使用する型情報を保持する
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefinedTy {
    pub tid: TyId,
    pub genargs: Vec<Ty>, // NOTE: Option ?
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TyVar(pub(crate) usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnTy {
    pub(crate) args: Vec<Ty>,

    // if the function does not return value ( = void function),
    // Ty::Void
    pub(crate) rty: Box<Ty>,

    pub(crate) genargs: Vec<Ty>,
}
