mod lexer;
pub mod token;

use crate::lexer::{PreTkKind, divide_regions, pre_lex, try_get_dec_integer, try_get_prefixed_int};
use biwac_base::ModPath;

pub use token::{TkKind, TkVal, Token};

#[derive(Debug, Clone)]
pub enum TokenizeError {
    SingleQuoteCloseNotFound,
    DoubleQuoteCloseNotFound,
}

pub fn lex(modu: ModPath, src: &str) -> Result<Vec<Token>, TokenizeError> {
    let regions = divide_regions(modu.clone(), src)?;

    let pretokens = pre_lex(modu, src, regions);

    let mut lines = src.lines();
    let tokens = pretokens
        .into_iter()
        .map(|p| match p.kind {
            PreTkKind::Word => {
                let w = &lines.nth(p.span.begin().line()).unwrap()
                    [p.span.begin().idx()..p.span.end().idx()];

                let (kind, val) = match w {
                    "TRUE" => (TkKind::BoolLiteralTrue, None),
                    "FALSE" => (TkKind::BoolLiteralFalse, None),
                    "import" => (TkKind::Import, None),
                    "package" => (TkKind::Package, None),
                    "fn" => (TkKind::Fn, None),
                    "let" => (TkKind::Let, None),
                    "if" => (TkKind::If, None),
                    "else" => (TkKind::Else, None),
                    "while" => (TkKind::While, None),
                    "return" => (TkKind::Return, None),
                    "Uint" => (TkKind::Uint, None),
                    "Int" => (TkKind::Int, None),
                    "Bool" => (TkKind::Bool, None),
                    "struct" => (TkKind::Struct, None),
                    _ => {
                        if let Some(i) = try_get_dec_integer(w) {
                            (TkKind::IntegerLiteral, Some(TkVal::Integer(i)))
                        } else if let Some(i) = try_get_prefixed_int(w) {
                            (TkKind::IntegerLiteral, Some(TkVal::Integer(i)))
                        } else {
                            (TkKind::Ident, Some(TkVal::String(w.to_string())))
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
                kind: TkKind::StringLiteral,
                span: p.span.clone(),
                val: Some(TkVal::String(
                    lines.nth(p.span.begin().line()).unwrap()
                        [p.span.begin().idx() + 1..p.span.end().idx() - 1]
                        .to_string(),
                )),
            },
        })
        .collect();

    Ok(tokens)
}
