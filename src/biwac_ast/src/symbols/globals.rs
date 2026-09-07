use std::cell::OnceCell;

use biwac_span::{
    GenDefId, ImplId, LocalGenDefId, Span, TraitAssocDefId, TraitDefId, TyDefId, ValDefId, VarId,
    VariantDefId,
};

use crate::{Attrs, Exprs, Ident, NovelStmt, Path, RetTypRepr, Stmt, TypRepr, VarDecl};

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
    pub attrs: Attrs,
}

//  enum
//  ```
//  enum Color {
//    Red,
//    Rgb(Int, Int, Int),
//    Named { name: String, alpha: Int },
//  }
//  ```
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub id: Ident,
    pub def_id: OnceCell<TyDefId>,
    /// 宣言順。添字がそのままタグの値になるので、並べ替えてはならない。
    pub variants: Vec<VariantDecl>,
    pub genargs: Option<GenArgsDecl<GenDefId>>,
    pub attrs: Attrs,
}

#[derive(Debug, Clone)]
pub struct VariantDecl {
    pub id: Ident,
    pub def_id: OnceCell<VariantDefId>,
    pub fields: VariantFieldsDecl,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum VariantFieldsDecl {
    /// `Red`
    Unit,
    /// `Rgb(Int, Int, Int)`
    ///
    /// 名前は `_0`, `_1` に正規化してある。
    /// 名前を持つ形に揃えておけば、MIR も backend も struct と同じ経路を通れる。
    /// interner を持っているのはパーサだけなので、そこで付ける。
    Tuple(Vec<(Ident, TypRepr)>),
    /// `Named { name: String }`
    Struct(Vec<(Ident, TypRepr)>),
}

impl VariantFieldsDecl {
    pub fn shape(&self) -> VariantShape {
        match self {
            Self::Unit => VariantShape::Unit,
            Self::Tuple(_) => VariantShape::Tuple,
            Self::Struct(_) => VariantShape::Struct,
        }
    }
}

/// バリアントの書き方。
///
/// 宣言と、構築・パターンの書き方が一致しているかの検査にだけ使う。
/// 型としての意味は持たない (フィールドはどの形でも名前を持つ形に正規化される)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantShape {
    Unit,
    Tuple,
    Struct,
}

impl std::fmt::Display for VariantShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unit => "unit",
            Self::Tuple => "tuple",
            Self::Struct => "struct",
        })
    }
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
    pub attrs: Attrs,
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
    pub attrs: Attrs,
}

#[derive(Debug)]
pub enum Globals {
    Import(ImportDecl),
    FnDef(FnDef),
    VarDecl(VarDecl),
    TypeDef(TypeDef),
    TraitDef(TraitDef),
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
    pub attrs: Attrs,
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
    pub attrs: Attrs,
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
    pub attrs: Attrs,
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
    pub attrs: Attrs,
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
    /// `impl[T] Foo[T]: Bar[T, Int]` の `Bar[T, Int]`。
    /// trait を実装しない普通の impl なら `None`。
    pub trait_typ: Option<TypRepr>,
    pub span: Span,
}

//  trait 宣言
//  ```biwa
//  trait Gyao {
//    fn gyao(self) -> Gyoe;
//
//    fn guee(aaa: Aaa) -> Self;
//  }
//  ```
//
//  項目は本体を持たない。`;` で終わる。
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub id: Ident,
    pub def_id: OnceCell<TraitDefId>,
    /// `Self` を表す暗黙のジェネリック引数。
    ///
    /// trait の宣言の中では `Self` はまだ何の型でもないので、
    /// ジェネリック引数として扱っておく。
    /// 名前解決 (`TraitDefResolveCtx`) が採番する。
    pub self_gen: OnceCell<GenDefId>,
    /// 宣言順。添字がそのまま `TraitAssocOwner::index` になるので
    /// 並べ替えてはならない。
    pub items: Vec<TraitItemDecl>,
    pub genargs: Option<GenArgsDecl<GenDefId>>,
    pub attrs: Attrs,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TraitItemDecl {
    pub id: Ident,
    pub def_id: OnceCell<TraitAssocDefId>,
    pub args: TraitItemArgs,
    pub rtype: RetTypRepr,
    pub genargs: Option<GenArgsDecl<LocalGenDefId>>,
    pub attrs: Attrs,
    pub span: Span,
}

/// trait の項目が取る引数。
///
/// `self` を取るならメソッド形式、取らないなら関連関数形式である。
/// どちらであるかは実装側と一致していなければならない。
#[derive(Debug, Clone)]
pub enum TraitItemArgs {
    /// `fn guee(aaa: Aaa) -> Self;`
    Assoc(ArgDeclList),
    /// `fn gyao(self) -> Gyoe;`
    Method(MethodArgDeclList),
}

#[derive(Debug, Clone)]
pub enum TypeDef {
    Struct(StructDef),
    Enum(EnumDef),
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
    pub attrs: Attrs,
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
    pub attrs: Attrs,
}
