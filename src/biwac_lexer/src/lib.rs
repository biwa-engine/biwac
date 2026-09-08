mod error;
mod lexer;
mod number;
pub mod token;

#[cfg(test)]
mod tests;

use crate::lexer::{PreTkKind, divide_regions, pre_lex};
use biwac_base::{IdentInterner, ModId};

pub use error::TokenizeError;
pub use token::{TkKind, TkKindName, Token};

pub fn lex<'src>(
    interner: &mut IdentInterner,
    file_id: ModId,
    src: &'src str,
) -> Result<Vec<Token<'src>>, TokenizeError> {
    let regions = divide_regions(file_id, src)?;

    let pretokens = pre_lex(file_id, src, regions)?;

    let tokens = pretokens
        .into_iter()
        .map(|p| match p.kind {
            PreTkKind::Word => {
                let w = &src[p.span.begin()..p.span.end()];

                let kind = match w {
                    "TRUE" => TkKind::KwBoolTrue,
                    "FALSE" => TkKind::KwBoolFalse,
                    "import" => TkKind::KwImport,
                    "package" => TkKind::KwPackage,
                    "fn" => TkKind::KwFn,
                    "type" => TkKind::KwType,
                    "let" => TkKind::KwLet,
                    "if" => TkKind::KwIf,
                    "else" => TkKind::KwElse,
                    "while" => TkKind::KwWhile,
                    "return" => TkKind::KwReturn,
                    "Uint" => TkKind::KwUint,
                    "Int" => TkKind::KwInt,
                    "Float" => TkKind::KwFloat,
                    "Bool" => TkKind::KwBool,
                    "struct" => TkKind::KwStruct,
                    "enum" => TkKind::KwEnum,
                    "match" => TkKind::KwMatch,
                    // `_` 単体だけがワイルドカードである。`_foo` は識別子のまま。
                    "_" => TkKind::KwUnderscore,
                    "impl" => TkKind::KwImpl,
                    "trait" => TkKind::KwTrait,
                    "Self" => TkKind::KwSelfTyp,
                    "self" => TkKind::KwSelfVar,
                    "scene" => TkKind::KwScene,
                    // 数値リテラルは pre_lex が読み切っているので、
                    // ここに来る語は必ず識別子である
                    // (biwa の識別子は数字始まりになりえない)。
                    _ => TkKind::Ident(interner.get_or_insert(w)),
                };

                Token { kind, span: p.span }
            }
            PreTkKind::Mark(kind) | PreTkKind::Number(kind) => Token { kind, span: p.span },
            PreTkKind::StringLiteral => Token {
                // `"` をトリムする
                kind: TkKind::LiteralString(&src[p.span.begin() + 1..p.span.end() - 1]),
                span: p.span.clone(),
            },
            PreTkKind::Dsl => Token {
                kind: TkKind::DslLiteral(&src[p.span.begin()..p.span.end()]),
                span: p.span.clone(),
            },
        })
        .collect();

    Ok(tokens)
}
