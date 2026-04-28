use biwac_base::Span;

use crate::Path;

// NOTE:
// 以下のTypDeclなどはいずれも、
// 変数などの型の宣言を表す(決してユーザ定義型そのものの定義を表すものではない)。
// 型推論において、
// 型Tは、型変数?Xであるか、型名Nであるかだが、
// 任意の型名Nは多相パラメータ<>を持つと考える。
// T = ?X
//   | N<T1, ..., Tn>
// しかし、このパースの段階においては
// パラメータを持つのはユーザ定義型のみであるとわかっているので、
// `struct DefTyp`のみ`genargs`を持つ

/// Type declaration, especially for variable declataion.
/// Variables often do not have explicit type representation.
/// If not, we mark as `Any` and must inter its type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypDecl {
    Any,
    Typ(TypRepr),
}

/// Representation of type.
/// fn foo[T](idx: Uint, vec: Vec[T]) -> T? { let b: Bool = FALSE; ... }
///                ^^^^       ^^^^^^     ^^          ^^^^
///                |          |          |           |
/// All of them are representation of types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypRepr {
    pub val: TypReprVal,
    pub span: Span,
}

impl TypRepr {
    pub fn new_def_typ(path: Path, genargs: Option<Vec<TypRepr>>) -> Self {
        TypRepr {
            span: if let Some(genargs) = &genargs
                && !genargs.is_empty()
            {
                Span::merge(&path.span(), &genargs.last().unwrap().span)
            } else {
                path.span()
            },
            val: TypReprVal::Defined(DefTyp { path, genargs }),
        }
    }
}

/// RetTypRepr は関数の戻り値の表明子
/// 無い場合、つまり Void の場合、
/// その位置を示すspanのみ持つ
///  ```biwa
///  fn foo() -> Int { ... }
///              ^^^
///  fn bar() { ... }
///          ^
///  ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetTypRepr {
    Typ(TypRepr),
    Void(Span),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypReprVal {
    Primitive(PrimTyp),
    Defined(DefTyp),
    // NOTE:
    // `T`のようなジェネリクス型も、
    // Path が1つの <identifier> のみのDefTypとしてパースされる
    // (パース時にはその意味論は決定できない)
}

/// Primitive(built-in) types like `Int`, `Uint`, `Float`, `Bool` ...
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrimTyp {
    Uint,
    Int,
    Float,
    Bool,
    // String,
}

/// User-defined types such as `struct Foo`, `enum Bar`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefTyp {
    pub path: Path,
    pub genargs: Option<Vec<TypRepr>>,
}

/// Generic argument like `T`
#[derive(Debug, Clone)]
pub struct GenArg {
    pub id: String,
}
