use std::collections::{hash_map::Entry, HashMap};

use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        matches,
        symbols::{
            consume_identifier, consume_qualified_identifier, consume_type_annotation,
            consume_type_annotation_which_must_annotate,
            statements::{
                block,
                vardec::{self, VarDec},
                Stmt,
            },
            QualifiedId,
        },
        types::Type,
        ParseError,
    },
};

#[derive(Debug, Clone)]
pub struct StructDef {
    pub id: String,
    pub members: HashMap<String, (Type, usize)>,
}

#[derive(Debug)]
pub enum Globals {
    Import(QualifiedId),
    LanglibfnDec(LanglibfnDec),
    FnDef(FnDef),
    VarDec(VarDec),
    TypeDef(TypeDef),
}

impl Globals {
    pub fn consume(
        tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
    ) -> Result<Option<Self>, ParseError> {
        if let Some(t) = tokens.peek() {
            match t.kind {
                // langlibfn id(): rtype
                TokenKind::Langlibfn => {
                    tokens.next();

                    let id = consume_identifier(tokens)?;

                    matches(tokens.next(), vec![TokenKind::LPare])?;
                    matches(tokens.next(), vec![TokenKind::RPare])?;

                    let rtype = consume_type_annotation(tokens)?;

                    return Ok(Some(Self::LanglibfnDec(LanglibfnDec { id, rtype })));
                }
                TokenKind::Import => {
                    tokens.next();
                    let qualed_id = consume_qualified_identifier(tokens)?;

                    return Ok(Some(Self::Import(qualed_id)));
                }
                TokenKind::Fn => {
                    tokens.next();
                    // let typ = consume_scalar_type(base, tokens);

                    let id = consume_identifier(tokens)?;
                    let args = consume_argsdec(tokens)?;

                    let rtype = consume_type_annotation(tokens)?;

                    return Ok(Some(Self::FnDef(FnDef {
                        name: id,
                        args,
                        stmts: block::consume(tokens)?,
                        rtype,
                    })));
                }
                TokenKind::Let => {
                    return Ok(Some(Self::VarDec(vardec::consume(tokens)?)));
                }
                TokenKind::Struct => {
                    tokens.next();

                    let id = consume_identifier(tokens)?;

                    matches(tokens.next(), vec![TokenKind::LBrace])?;
                    let mut members = HashMap::new();
                    let mut member_index = 0;

                    loop {
                        let t = tokens.peek().ok_or(ParseError::InvalidEOF(vec![
                            TokenKind::Identifier("".to_string()),
                            TokenKind::RBrace,
                        ]))?;

                        if let TokenKind::RBrace = t.kind {
                            tokens.next();

                            return Ok(Some(Self::TypeDef(TypeDef::Struct(StructDef {
                                id,
                                members,
                            }))));
                        } else {
                            let t = tokens.next().ok_or(ParseError::InvalidEOF(vec![
                                TokenKind::Identifier("".to_string()),
                            ]))?;
                            if let TokenKind::Identifier(memberid) = &t.kind {
                                let typ = consume_type_annotation_which_must_annotate(tokens)?;

                                match members.entry(memberid.to_owned()) {
                                    Entry::Occupied(_) => {
                                        return Err(ParseError::StructMemberConflict(
                                            id,
                                            memberid.to_owned(),
                                            t.range.clone(),
                                        ))
                                    }
                                    Entry::Vacant(e) => {
                                        e.insert((typ, member_index));
                                    }
                                }

                                let kind = matches(
                                    tokens.peek().copied(),
                                    vec![TokenKind::Comma, TokenKind::RBrace],
                                )?;
                                if let TokenKind::Comma = kind {
                                    tokens.next();
                                    member_index += 1;
                                    continue;
                                } else if let TokenKind::RBrace = kind {
                                    continue;
                                }
                            } else {
                                return Err(ParseError::InvalidToken(
                                    vec![TokenKind::Identifier("".to_string())],
                                    t.clone(),
                                ));
                            }
                        }
                    }
                }
                _ => {
                    return Err(ParseError::InvalidToken(
                        vec![TokenKind::Fn, TokenKind::Let, TokenKind::Struct],
                        t.to_owned().clone(),
                    ));
                }
            }
        }

        Ok(None)
    }
}

#[derive(Debug)]
pub struct LanglibfnDec {
    pub id: String,
    // pub args: Vec<Type>,
    // NOTE: args type check not supported now, we must pay attention to langlibfn call
    // because it is unsafe
    pub rtype: Option<Type>, // None means void
}

#[derive(Debug)]
pub struct FnDef {
    pub name: String,
    pub args: Vec<(Type, String)>,
    pub stmts: Vec<Stmt>,
    pub rtype: Option<Type>, // None means void
}

#[derive(Debug)]
pub enum TypeDef {
    Struct(StructDef),
    // Enum(EnumType),
    // Typedef(Box<Self>),
}

pub fn consume_argsdec(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Vec<(Type, String)>, ParseError> {
    matches(tokens.next(), vec![TokenKind::LPare])?;

    let mut args = vec![];

    loop {
        let t = tokens.next().ok_or(ParseError::InvalidEOF(vec![
            TokenKind::RPare,
            TokenKind::Identifier("".to_string()),
        ]))?;

        if let TokenKind::RPare = t.kind {
            return Ok(args);
        } else if let TokenKind::Identifier(arg) = &t.kind {
            let typ = consume_type_annotation_which_must_annotate(tokens)?;

            args.push((typ, arg.clone()));

            let kind = matches(
                tokens.peek().copied(),
                vec![TokenKind::Comma, TokenKind::RPare],
            )?;
            if let TokenKind::Comma = kind {
                tokens.next();
            } else if let TokenKind::RPare = kind {
                continue;
            }
        }
    }
}
