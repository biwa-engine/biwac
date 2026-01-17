pub mod expressions;
pub mod globals;
pub mod statements;

use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        matches,
        symbols::globals::Globals,
        types::{PrimitiveType, Type},
        ParseError,
    },
};

#[derive(Debug)]
pub struct ModAst {
    pub globals: Vec<Globals>, // pub fns: Vec<FnDec>,
}

impl ModAst {
    pub fn consume(
        tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
    ) -> Result<Self, ParseError> {
        let mut prog = Self { globals: vec![] };

        while let Some(global) = Globals::consume(tokens)? {
            prog.globals.push(global);
        }

        Ok(prog)
    }
}

#[derive(Debug, Clone)]
pub struct QualifiedId {
    pub is_from_root: bool,
    pub quals: Vec<String>,
    pub id: String,
}

pub fn consume_type_annotation(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Option<Type>, ParseError> {
    let t = tokens
        .peek()
        .ok_or(ParseError::InvalidEOF(vec![TokenKind::Colon]))?
        .to_owned();

    if let TokenKind::Colon = t.kind {
        tokens.next();

        let kind = matches(
            tokens.peek().copied(),
            vec![
                TokenKind::Uint,
                TokenKind::Int,
                TokenKind::Bool,
                TokenKind::Identifier("".to_string()),
                TokenKind::Package,
            ],
        )?;

        if let TokenKind::Uint = kind {
            tokens.next();
            Ok(Some(Type::Primitive(PrimitiveType::Uint)))
        } else if let TokenKind::Int = kind {
            tokens.next();
            Ok(Some(Type::Primitive(PrimitiveType::Int)))
        } else if let TokenKind::Bool = kind {
            tokens.next();
            Ok(Some(Type::Primitive(PrimitiveType::Bool)))
        } else if let TokenKind::Identifier(_) = kind {
            let qualed_id = consume_qualified_identifier(tokens)?;
            Ok(Some(Type::Defined(qualed_id)))
        } else if let TokenKind::Package = kind {
            let qualed_id = consume_qualified_identifier(tokens)?;
            Ok(Some(Type::Defined(qualed_id)))
        } else {
            Err(ParseError::InvalidToken(
                vec![
                    TokenKind::Uint,
                    TokenKind::Int,
                    TokenKind::Bool,
                    TokenKind::Identifier("".to_string()),
                ],
                t.clone().clone(),
            ))
        }
    } else {
        Ok(None)
    }
}

pub fn consume_type_annotation_which_must_annotate(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Type, ParseError> {
    matches(tokens.next(), vec![TokenKind::Colon])?;

    let kind = matches(
        tokens.peek().copied(),
        vec![
            TokenKind::Uint,
            TokenKind::Int,
            TokenKind::Bool,
            TokenKind::Identifier("".to_string()),
            TokenKind::Package,
        ],
    )?;

    if let TokenKind::Uint = kind {
        tokens.next();
        Ok(Type::Primitive(PrimitiveType::Uint))
    } else if let TokenKind::Int = kind {
        tokens.next();
        Ok(Type::Primitive(PrimitiveType::Int))
    } else if let TokenKind::Bool = kind {
        tokens.next();
        Ok(Type::Primitive(PrimitiveType::Bool))
    } else if let TokenKind::Identifier(_) = kind {
        let qualed_id = consume_qualified_identifier(tokens)?;
        Ok(Type::Defined(qualed_id))
    } else if let TokenKind::Package = kind {
        let qualed_id = consume_qualified_identifier(tokens)?;
        Ok(Type::Defined(qualed_id))
    } else {
        panic!("unreachable");
    }
}

fn consume_identifier(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<String, ParseError> {
    let t = tokens
        .next()
        .ok_or(ParseError::InvalidEOF(vec![TokenKind::Identifier(
            "".to_string(),
        )]))?;

    if let TokenKind::Identifier(id) = &t.kind {
        Ok(id.clone())
    } else {
        Err(ParseError::InvalidToken(
            vec![TokenKind::Identifier("".to_string())],
            t.clone(),
        ))
    }
}

pub fn consume_qualified_identifier(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<QualifiedId, ParseError> {
    let mut ids = vec![];
    let is_from_root = if let Some(t) = tokens.peek() {
        matches!(t.kind, TokenKind::Package)
    } else {
        false
    };

    ids.push(consume_identifier(tokens)?);

    loop {
        if let Some(t) = tokens.peek() {
            if let TokenKind::DoubleColon = t.kind {
                tokens.next();
                ids.push(consume_identifier(tokens)?);
            } else {
                return Ok(QualifiedId {
                    is_from_root,
                    quals: ids[..ids.len() - 1].to_vec(),
                    id: ids.last().expect("no identifier parsed").clone(),
                });
            }
        } else {
            return Ok(QualifiedId {
                is_from_root,
                quals: ids[..ids.len() - 1].to_vec(),
                id: ids.last().expect("no identifier parsed").clone(),
            });
        }
    }
}
