use std::cell::OnceCell;

use biwac_span::{GenDefId, ImplId, LocalGenDefId, Span, TyDefId, ValDefId, VarId};

use crate::{CompilerFlag, Exprs, Ident, NovelStmt, Path, RetTypRepr, Stmt, TypRepr, VarDecl};

#[derive(Debug, Clone)]
pub struct GenArgDeclItem<I> {
    pub id: Ident,
    pub def_id: OnceCell<I>,
}

#[derive(Debug, Clone)]
pub struct GenArgsDecl<I> {
    pub genargs: Vec<GenArgDeclItem<I>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub id: Ident,
    pub def_id: OnceCell<TyDefId>,
    pub members: Vec<(Ident, TypRepr)>,
    pub genargs: Option<GenArgsDecl<GenDefId>>,
}

//  type alias
//  ```
//  type Foo[T] = Bar[T, Int];
//       ^^^
//       |  ^^^ genargs
//       ident
//  ```
#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub ident: Ident,
    pub def_id: OnceCell<TyDefId>,
    pub genargs: Option<GenArgsDecl<GenDefId>>,
    pub right: TypRepr,
}

//  native type alias
//  ```
//  type Foo[T] = {{
//      native type implementation
//  }};
//  ```
#[derive(Debug, Clone)]
pub struct NativeTypeAlias {
    pub ident: Ident,
    pub def_id: OnceCell<TyDefId>,
    pub genargs: Option<GenArgsDecl<GenDefId>>,
    pub native: String,
    pub native_span: Span,
}

#[derive(Debug)]
pub enum Globals {
    Import(ImportDecl),
    FnDef(FnDef),
    VarDecl(VarDecl),
    TypeDef(TypeDef),
    ImplBlock(ImplBlock),
    NativeFnDef(NativeFnDef),
    NativeCode(NativeCode),
    NovelScene(NovelScene),
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub path: Path,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub id: Ident,
    pub def_id: OnceCell<ValDefId>,
    pub args: ArgDeclList,
    pub stmts: Vec<Stmt>,
    pub expr: Option<Exprs>,
    pub rtype: RetTypRepr,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
    pub genargs: Option<GenArgsDecl<LocalGenDefId>>,
}

#[derive(Debug, Clone)]
pub struct NativeFnDef {
    pub id: Ident,
    pub def_id: OnceCell<ValDefId>,
    pub args: ArgDeclList,
    pub rtype: RetTypRepr,
    pub native: String,
    pub native_span: Span,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
    pub genargs: Option<GenArgsDecl<LocalGenDefId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgDecl {
    pub typ: TypRepr,
    pub id: Ident,
    pub span: Span,
    pub var_id: OnceCell<VarId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgDeclList {
    pub args: Vec<ArgDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodArgDeclList {
    pub self_span: Span,
    pub args: Vec<ArgDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodDef {
    pub def_id: OnceCell<ValDefId>,
    pub id: Ident,
    pub args: MethodArgDeclList, // 第一引数がselfであるのは自明なので含まない
    pub stmts: Vec<Stmt>,
    pub expr: Option<Exprs>,
    pub rtype: RetTypRepr,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
    pub genargs: Option<GenArgsDecl<LocalGenDefId>>,
}

#[derive(Debug, Clone)]
pub struct NativeMethodDef {
    pub def_id: OnceCell<ValDefId>,
    pub id: Ident,
    pub args: MethodArgDeclList, // 第一引数がselfであるのは自明なので含まない
    pub rtype: RetTypRepr,
    pub native: String,
    pub native_span: Span,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
    pub genargs: Option<GenArgsDecl<LocalGenDefId>>,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub impl_id: OnceCell<ImplId>,
    pub assoc_fns: Vec<FnDef>,
    pub methods: Vec<MethodDef>,
    pub native_assoc_fns: Vec<NativeFnDef>,
    pub native_methods: Vec<NativeMethodDef>,
    pub genargs_decl: Option<GenArgsDecl<LocalGenDefId>>,
    pub self_typ: TypRepr,
}

#[derive(Debug, Clone)]
pub enum TypeDef {
    Struct(StructDef),
    // Enum(EnumType),
    TypeAlias(TypeAlias),
    NativeTypeAlias(NativeTypeAlias),
}

/// NativeCode
/// 以下のようにネイティブコードを直接書きたく、
/// かつそれが他のbiwaコード自体からは名前で参照されないようなもの
/// の場合に使われる
/// 元々のネイティブコードにおいて順序がどうなるべきか、
/// biwaのレベルではわからないため、
/// 必ずファイルの先頭に展開されることを保証する
/// つまりimportなどに使用できることになる
/// 逆にネイティブコードにおいて他のシンボルとの順序関係が重視されるものに関しては
/// そもそもこの NativeCode 方式を使うべきでないし、
/// 型や関数のnative実装はサポートされているためそれで事足りるはずである
///  ```biwa
///  [[native(arch="arch_name")]]
///  {{
///      ...
///  }}
///  ```
#[derive(Debug, Clone)]
pub struct NativeCode {
    pub native: String,
    pub native_span: Span,
    pub flags: Vec<CompilerFlag>,
}

// biwa言語がノベルゲーム記述用言語であるための
// 最も特徴的な機能として scene がある。
// scene 内では、
// - 直接記述したテキストがメッセージウィンドウに出力され、
// - プレフィックスに続いて通常のsyntaxに近いコードの制御命令が使える
//
//  ```biwa
//  scene scene1(game: MyGame) -> MyGame {{
//      Hello!
//      #foo()
//      #if cond {
//          By the way...
//      }
//  }}
//  ```
// novel scene は通常のASTにおける 文 <statement>
// の列と同様であり、
// これはパース段階で変換できる
#[derive(Debug, Clone)]
pub struct NovelScene {
    pub id: Ident,
    pub def_id: OnceCell<ValDefId>,
    pub args: ArgDeclList,
    pub rtype: RetTypRepr,
    pub stmts: Vec<NovelStmt>,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
}
