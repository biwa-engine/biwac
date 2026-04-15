mod error;
mod lexer;
pub mod token;

#[cfg(test)]
mod tests;

use crate::lexer::{PreTkKind, divide_regions, pre_lex, try_get_dec_integer, try_get_prefixed_int};
use biwac_base::ModId;

pub use error::TokenizeError;
pub use token::{TkKind, TkKindName, TkVal, Token};

pub fn lex<'src>(file_id: ModId, src: &'src str) -> Result<Vec<Token<'src>>, TokenizeError> {
    let regions = divide_regions(file_id, src)?;

    let pretokens = pre_lex(file_id, src, regions);

    let tokens = pretokens
        .into_iter()
        .map(|p| match p.kind {
            PreTkKind::Word => {
                let w = &src[p.span.begin()..p.span.end()];

                let (kind, val) = match w {
                    "TRUE" => (TkKind::KwBoolTrue, None),
                    "FALSE" => (TkKind::KwBoolFalse, None),
                    "import" => (TkKind::KwImport, None),
                    "package" => (TkKind::KwPackage, None),
                    "fn" => (TkKind::KwFn, None),
                    "type" => (TkKind::KwType, None),
                    "let" => (TkKind::KwLet, None),
                    "if" => (TkKind::KwIf, None),
                    "else" => (TkKind::KwElse, None),
                    "while" => (TkKind::KwWhile, None),
                    "return" => (TkKind::KwReturn, None),
                    "Uint" => (TkKind::KwUint, None),
                    "Int" => (TkKind::KwInt, None),
                    "Float" => (TkKind::KwFloat, None),
                    "Bool" => (TkKind::KwBool, None),
                    "struct" => (TkKind::KwStruct, None),
                    "impl" => (TkKind::KwImpl, None),
                    "Self" => (TkKind::KwSelfTyp, None),
                    "self" => (TkKind::KwSelfVar, None),
                    "scene" => (TkKind::KwScene, None),
                    _ => {
                        if let Some(i) = try_get_dec_integer(w) {
                            (TkKind::LiteralInteger(i), Some(TkVal::Integer(i)))
                        } else if let Some(i) = try_get_prefixed_int(w) {
                            (TkKind::LiteralInteger(i), Some(TkVal::Integer(i)))
                        } else {
                            (TkKind::Ident(w), Some(TkVal::String(w.to_string())))
                        }
                    }
                };

                Token {
                    kind,
                    span: p.span,
                    val,
                }
            }
            PreTkKind::Mark(kind) => Token {
                kind,
                span: p.span,
                val: None,
            },
            PreTkKind::StringLiteral => Token {
                kind: TkKind::LiteralString(&src[p.span.begin() + 1..p.span.end() - 1]),
                span: p.span.clone(),
                val: Some(TkVal::String(
                    src[p.span.begin() + 1..p.span.end() - 1].to_string(),
                )),
            },
            PreTkKind::Dsl => Token {
                kind: TkKind::DslLiteral,
                span: p.span.clone(),
                val: Some(TkVal::String(src[p.span.begin()..p.span.end()].to_string())),
            },
        })
        .collect();

    Ok(tokens)
}
