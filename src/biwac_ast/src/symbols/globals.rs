use biwac_base::Span;

use crate::{CompilerFlag, Exprs, Ident, QualifiedId, RetTypRepr, Stmt, TypRepr, VarDecl};

#[derive(Debug, Clone)]
pub struct StructDef {
    pub id: Ident,
    pub members: Vec<(Ident, TypRepr)>,
    pub genargs: Vec<Ident>,
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
    pub genargs: Vec<Ident>,
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
    pub genargs: Vec<Ident>,
    pub native: String,
    pub native_span: Span,
}

#[derive(Debug)]
pub enum Globals {
    Import(ImportDecl),
    FnDef(FnDef),
    VarDecl(VarDecl),
    TypeDef(TypeDef),
    NativeFnDef(NativeFnDef),
    MethodDef(MethodDef),
    NativeMethodDef(NativeMethodDef),
    NativeCode(NativeCode),
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub qualid: QualifiedId,
    pub span: Span,
}

// 型の関連関数の場合はtypがSome
// Selfは具体のTypReprによりパース時に解決される
#[derive(Debug, Clone)]
pub struct FnDef {
    pub impl_ctx: Option<ImplCtx>,
    pub id: Ident,
    pub args: ArgDeclList,
    pub stmts: Vec<Stmt>,
    pub expr: Option<Exprs>,
    pub rtype: RetTypRepr,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
    pub genargs: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub struct NativeFnDef {
    pub impl_ctx: Option<ImplCtx>,
    pub id: Ident,
    pub args: ArgDeclList,
    pub rtype: RetTypRepr,
    pub native: String,
    pub native_span: Span,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
    pub genargs: Vec<Ident>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgDecl {
    pub typ: TypRepr,
    pub id: Ident,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgDeclList {
    pub args: Vec<ArgDecl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodDef {
    pub impl_genargs: Vec<Ident>,
    pub self_typ: TypRepr,
    pub self_ident: Ident,
    pub id: Ident,
    pub args: ArgDeclList, // 第一引数がselfであるのは自明なので含まない
    pub stmts: Vec<Stmt>,
    pub expr: Option<Exprs>,
    pub rtype: RetTypRepr,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
    pub genargs: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub struct NativeMethodDef {
    pub impl_genargs: Vec<Ident>,
    pub self_typ: TypRepr,
    pub self_ident: Ident,
    pub id: Ident,
    pub args: ArgDeclList, // 第一引数がselfであるのは自明なので含まない
    pub rtype: RetTypRepr,
    pub native: String,
    pub native_span: Span,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
    pub genargs: Vec<Ident>,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub typ_fns: Vec<FnDef>,
    pub methods: Vec<MethodDef>,
    pub genargs: Vec<Ident>,
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

#[derive(Debug, Clone)]
pub struct ImplCtx {
    pub genargs: Vec<Ident>,
    pub self_typ: TypRepr,
}
