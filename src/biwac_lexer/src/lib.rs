mod error;
mod lexer;
mod number;
pub mod token;

#[cfg(test)]
mod tests;

use crate::lexer::{PreTkKind, divide_regions, pre_lex};
use biwac_base::{IdentInterner, ModId};
use biwac_span::Span;

pub use error::TokenizeError;
pub use token::{TkKind, TkKindName, Token};

pub fn lex<'src>(
    interner: &mut IdentInterner,
    file_id: ModId,
    src: &'src str,
) -> Result<Vec<Token<'src>>, TokenizeError> {
    let regions = divide_regions(file_id, src)?;

    let pretokens = pre_lex(file_id, src, regions)?;

    let tokens: Vec<Token<'src>> = pretokens
        .into_iter()
        .map(|p| -> Result<Token<'src>, TokenizeError> {
            Ok(match p.kind {
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
                PreTkKind::StringLiteral => {
                    // `"` をトリムしてから展開する。
                    // 対応表は `biwac_base` にあり、ノベル DSL の字句解析
                    // (biwac_novel_parser) と共有している。
                    let body_begin = p.span.begin() + 1;
                    let body = &src[body_begin..p.span.end() - 1];

                    let val = biwac_base::unescape(body).map_err(|e| match e {
                        biwac_base::EscapeError::Unknown { offset, found } => {
                            TokenizeError::UnknownEscape {
                                found,
                                span: Span::new(
                                    file_id,
                                    body_begin + offset,
                                    body_begin + offset + 2,
                                ),
                            }
                        }
                        biwac_base::EscapeError::Trailing { offset } => {
                            TokenizeError::UnknownEscape {
                                found: '"',
                                span: Span::new(
                                    file_id,
                                    body_begin + offset,
                                    body_begin + offset + 1,
                                ),
                            }
                        }
                    })?;

                    Token {
                        kind: TkKind::LiteralString(val),
                        span: p.span.clone(),
                    }
                }
                PreTkKind::Dsl => Token {
                    kind: TkKind::DslLiteral(&src[p.span.begin()..p.span.end()]),
                    span: p.span.clone(),
                },
            })
        })
        .collect::<Result<_, _>>()?;

    Ok(tokens)
}
