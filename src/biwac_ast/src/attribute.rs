use biwac_span::Span;

use crate::{BoolLiteral, Ident, IntegerLiteral, StringLiteral};

// このモジュールは属性の *構文表現* のみを持つ。
//
// 「どの属性名が存在するか」「どのキーを取るか」「どの対象に付けられるか」
// といった知識は一切持たない。
// それらは biwac_attribute crate が一覧として定義し、
// AST から HIR への lowering までの間に走る検証パスが突き合わせる。
//
// パーサは書かれたものをそのままここに載せるだけであり、
// 未知の属性名であってもパースは成功する。

/// 1 つの属性。
///
///  ```biwa
///  [[native(arch="typescript")]]
///  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ span
///    ^^^^^^                      name
///           ^^^^^^^^^^^^^^^^^^   body (AttrBody::List)
///  ```
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: Ident,
    pub body: AttrBody,
    pub span: Span,
}

/// 属性の形態。rustc の `MetaItemKind` と同じ 3 形態を取る。
#[derive(Debug, Clone)]
pub enum AttrBody {
    /// 引数なし
    ///  ```biwa
    ///  [[foo]]
    ///  ```
    Word,

    /// 単一の値を取る
    ///  ```biwa
    ///  [[lang="write"]]
    ///  ```
    Value(AttrValue),

    /// キー付き引数列を取る
    ///  ```biwa
    ///  [[native(arch="typescript")]]
    ///  ```
    List(Vec<AttrArg>),
}

/// `AttrBody::List` の要素。値は省略できる (`[[foo(bar)]]`)。
#[derive(Debug, Clone)]
pub struct AttrArg {
    pub key: Ident,
    pub val: Option<AttrValue>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    Integer(IntegerLiteral),
    String(StringLiteral),
    Bool(BoolLiteral),
}

impl AttrValue {
    pub fn span(&self) -> &Span {
        match self {
            Self::Integer(l) => &l.span,
            Self::String(l) => &l.span,
            Self::Bool(l) => &l.span,
        }
    }

    /// 文字列値であればその中身を返す。
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(l) => Some(l.val.as_str()),
            _ => None,
        }
    }
}

/// 1 つの定義に付与された属性の列。
///
/// 検証済みかどうかをこの型は区別しない。
/// 属性名による検索は biwac_attribute 側の型付きアクセサを通して行い、
/// ここでは走査のための最小限の API のみを提供する。
#[derive(Debug, Clone, Default)]
pub struct Attrs(Vec<Attribute>);

impl Attrs {
    pub fn new(attrs: Vec<Attribute>) -> Self {
        Self(attrs)
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Attribute> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 指定した名前の属性を探す。
    /// 同名の属性が複数あった場合は最初のものを返す
    /// (重複自体は検証パスがエラーとして報告する)。
    pub fn find(&self, name: &str, interner: &biwac_base::IdentInterner) -> Option<&Attribute> {
        self.iter()
            .find(|a| interner.get_str(&a.name.id) == Some(name))
    }

    pub fn has(&self, name: &str, interner: &biwac_base::IdentInterner) -> bool {
        self.find(name, interner).is_some()
    }
}

impl<'a> IntoIterator for &'a Attrs {
    type Item = &'a Attribute;
    type IntoIter = std::slice::Iter<'a, Attribute>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
