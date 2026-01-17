use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        matches,
        symbols::{
            consume_type_annotation,
            expressions::{self, Exprs},
        },
        types::Type,
        ParseError,
    },
};

#[derive(Debug, Clone)]
pub struct VarDec {
    pub typ: Type,
    pub name: String,
    pub init: Exprs,
}

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<VarDec, ParseError> {
    matches(tokens.next(), vec![TokenKind::Let])?;

    if let TokenKind::Identifier(id) =
        matches(tokens.next(), vec![TokenKind::Identifier("".to_string())])?
    {
        let typ =
            consume_type_annotation(tokens)?.expect("type anotation omission not implemented yet");

        // NOTE: 変数宣言時、初期化は必須
        // 代入漏れバリデーション能力が向上したら初期化しないパターンもサポートするかも
        matches(tokens.next(), vec![TokenKind::Assign])?;

        let init = expressions::consume(tokens)?;

        Ok(VarDec {
            typ,
            name: id.clone(),
            init,
        })
    } else {
        Err(ParseError::InvalidEOF(vec![TokenKind::Let]))
    }
}
