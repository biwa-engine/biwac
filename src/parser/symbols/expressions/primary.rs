use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        matches,
        symbols::{
            consume_identifier, consume_qualified_identifier,
            expressions::{self, Exprs, FnCall, LanglibfnCall, Literal, Primary},
        },
        ParseError,
    },
};

// Primary = Literal | Identifier ( "(" ")" )? | "(" ArithmExpr ")"

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<super::Exprs, ParseError> {
    // Primary = Literal | "(" Expr ")"
    if let Some(t) = tokens.peek() {
        match &t.kind {
            TokenKind::IntLiteral(i) => {
                tokens.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Uint(*i))))
            }
            TokenKind::StringLiteral(s) => {
                tokens.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::String(s.clone()))))
            }
            TokenKind::BoolLiteralTrue => {
                tokens.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(true))))
            }
            TokenKind::BoolLiteralFalse => {
                tokens.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(false))))
            }
            TokenKind::Identifier(_) => {
                let qualed_id = consume_qualified_identifier(tokens)?;

                if let Some(t) = tokens.peek() {
                    if let TokenKind::LPare = t.kind {
                        tokens.next();

                        let mut args: Vec<Exprs> = vec![];

                        while let Some(t) = tokens.peek() {
                            if let TokenKind::RPare = t.kind {
                                tokens.next();
                                return Ok(Exprs::Primary(Primary::FnCall(FnCall {
                                    qualed_id,
                                    args,
                                })));
                            } else {
                                let expr = expressions::consume(tokens)?;
                                args.push(expr);

                                let kind = matches(
                                    tokens.peek().copied(),
                                    vec![TokenKind::RPare, TokenKind::Comma],
                                )?;
                                if let TokenKind::Comma = kind {
                                    tokens.next();
                                    continue;
                                } else if let TokenKind::RPare = kind {
                                    continue;
                                }
                            }
                        }

                        Err(ParseError::InvalidEOF(vec![TokenKind::RPare]))
                    } else if let TokenKind::LBrace = t.kind {
                        tokens.next();

                        let mut members: Vec<(String, Box<Exprs>)> = vec![];

                        while let Some(t) = tokens.peek() {
                            if let TokenKind::RBrace = t.kind {
                                tokens.next();
                                return Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                                    qualed_id, members,
                                ))));
                            } else {
                                let member = consume_identifier(tokens)?;
                                matches(tokens.next(), vec![TokenKind::Assign])?;
                                let expr = expressions::consume(tokens)?;

                                members.push((member, Box::new(expr)));

                                let kind = matches(
                                    tokens.peek().copied(),
                                    vec![TokenKind::RBrace, TokenKind::Comma],
                                )?;
                                if let TokenKind::Comma = kind {
                                    tokens.next();
                                    continue;
                                } else if let TokenKind::RBrace = kind {
                                    continue;
                                }
                            }
                        }

                        Err(ParseError::InvalidEOF(vec![TokenKind::RBrace]))
                    } else if !qualed_id.quals.is_empty() && !qualed_id.is_from_root {
                        panic!("variable cannot qualified");
                    } else {
                        Ok(Exprs::Primary(Primary::Variable(qualed_id.id)))
                    }
                } else if !qualed_id.quals.is_empty() && !qualed_id.is_from_root {
                    panic!("variable cannot qualified");
                } else {
                    Ok(Exprs::Primary(Primary::Variable(qualed_id.id)))
                }
            }
            TokenKind::LPare => {
                tokens.next();
                let expr = expressions::consume(tokens)?;

                if let TokenKind::RPare = matches(tokens.next(), vec![TokenKind::RPare])? {
                    Ok(expr)
                } else {
                    Err(ParseError::InvalidToken(
                        vec![
                            TokenKind::Identifier("".to_string()),
                            TokenKind::IntLiteral(0),
                        ],
                        tokens.peek().unwrap().to_owned().clone(),
                    ))
                }
            }
            // langlibfn::id(a0, 1, a2)
            TokenKind::Langlibfn => {
                tokens.next();

                matches(tokens.next(), vec![TokenKind::DoubleColon])?;
                let id = consume_identifier(tokens)?;
                matches(tokens.next(), vec![TokenKind::LPare])?;

                let mut args: Vec<Exprs> = vec![];

                while let Some(t) = tokens.peek() {
                    if let TokenKind::RPare = t.kind {
                        tokens.next();
                        return Ok(Exprs::Primary(Primary::LanglibfnCall(LanglibfnCall {
                            id,
                            args,
                        })));
                    } else {
                        let expr = expressions::consume(tokens)?;
                        args.push(expr);

                        let kind = matches(
                            tokens.peek().copied(),
                            vec![TokenKind::RPare, TokenKind::Comma],
                        )?;
                        if let TokenKind::Comma = kind {
                            tokens.next();
                            continue;
                        } else if let TokenKind::RPare = kind {
                            continue;
                        }
                    }
                }

                Err(ParseError::InvalidEOF(vec![TokenKind::RPare]))
            }
            _ => Err(ParseError::InvalidEOF(vec![
                TokenKind::Identifier("".to_string()),
                TokenKind::IntLiteral(0),
            ])),
        }
    } else {
        Err(ParseError::InvalidEOF(vec![
            TokenKind::Identifier("".to_string()),
            TokenKind::IntLiteral(0),
        ]))
    }
}
