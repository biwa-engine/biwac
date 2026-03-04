use std::collections::HashMap;

use biwac_base::Span;
use biwac_parser::Ident;

use crate::{DecledVar, Expr, ExprId, LocVarId, Progressive, Stmt, Ty};

// 型名前空間のシンボルを
// 識別するid
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TyId {
    // pub pkg: enum Package { Internal, External(String)}
    quals: Vec<String>,
    id: String,
}

// 型定義側で
// 宣言されるジェネリクス型に割り当てられるid
// GenTyIdに対するTyの割り当て(HashMap<GenTyId, Ty>)を保持することで、
// あるジェネリック型の使用箇所におけるのメンバなどへの型付けを計算できる
//  ```
//  struct Foo[T, U] {
//            ^^^^^^
//      x: T,
//      y: U,
//      z: Int,
//  }
//  ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenTyId(usize);

// impl block や fn のローカルなスコープで宣言された
// ジェネリック型に通しで振られるid
// ```
//  impl[T] Foo[T, Int] {
//      ^^^
//      fn bar[U](self) -> Baz[T, U] {
//            ^^^
//          ...
//      }
//  }
// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocGenTyId(usize);

// 値名前空間のシンボル
// - 関数
// - グローバル変数(const)
// を識別するid
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ValId {
    // pub pkg: enum Package { Internal, External(String)}
    quals: Vec<String>,
    id: String,
}

// 値名前空間のシンボル
#[derive(Debug, Clone)]
pub enum ValDefContentKind {
    Fn(Box<FnDefContent>),
    Native(Box<NativeFnDefContent>),
}

// impl block 内での
// 値名前空間のシンボル
#[derive(Debug, Clone)]
pub enum ImplValDefContentKind {
    Fn(Box<FnDefContent>),
    Method(Box<MethodDefContent>),
}

#[derive(Debug, Clone)]
pub struct FnDefContent {
    pub fn_name_span: Span,

    // signature
    pub signature: FnDefContentSignature,

    // body
    // needs type inferrence
    pub body: Progressive<(), FnDefContentBody>,

    // 関数内で宣言された変数のマップ
    // 一意なid: LocVarIdを割り当てる
    pub vars: HashMap<LocVarId, DecledVar>,

    // 型推論された結果の式に対する型が記録される
    pub expr_tys: HashMap<ExprId, Ty>,
}

// ```
//  fn foo[T](x: T, y: Int) -> Bar[T] { ... }
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^
//        signature
// ```
#[derive(Debug, Clone)]
pub struct FnDefContentSignature {
    // TODO:
    // pub args: Vec<DecledArg>,
    pub args: Vec<(Ident, Ty)>,

    // if the function does not return value ( = void function),
    // Ty::Void
    pub rty: Ty,

    pub genargs: Vec<(Ident, LocGenTyId)>,
    // TODO:
    // pub genargs: HashMap<LocGenTyId, Ident>,
    // pub genargs: HashMap<String, (Ident, LocGenTyId)>,
}

#[derive(Debug, Clone)]
pub struct FnDefContentBody {
    pub stmts: Vec<Stmt>,
    pub expr: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct NativeFnDefContent {
    // signature
    pub signature: FnDefContentSignature,

    pub native_body: String,

    pub fn_name_span: Span,
    pub native_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFnArgDecl {
    pub ty: Ty,
    pub id: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DecledArg {
    pub id: LocVarId,
}

// TODO: FnDefContent と同じで済むなら同じに
#[derive(Debug, Clone)]
pub struct MethodDefContent {
    pub fn_name_span: Span,

    // NOTE: selfは0が割り当てられることを決めてしまう?
    // pub self_id: LocVarId,

    // signature
    pub signature: FnDefContentSignature,

    // body
    // needs type inferrence
    pub body: Progressive<(), FnDefContentBody>,

    // 関数内で宣言された変数のマップ
    // 一意なid: LocVarIdを割り当てる
    pub vars: HashMap<LocVarId, DecledVar>,

    // 型推論された結果の式に対する型が記録される
    pub expr_tys: HashMap<ExprId, Ty>,
}

// 各種の型の定義
// e.g.) struct, enum
#[derive(Debug, Clone)]
pub enum TyDefContentKind {
    Struct(Box<StructDefContent>),
    // Enum(EnumDefContent),
    // TypeAlias(Box<Self>),
}

#[derive(Debug, Clone)]
pub struct StructDefContent {
    pub members: HashMap<String, (Ty, Span)>,
    pub genargs: Vec<GenTyId>,
    // TODO: その他各種情報
    pub(crate) struct_name_ident: Ident,
}
