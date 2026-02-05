use biwac_lexer::TkKind;

use crate::{
    ParseError,
    parser::TokenStream,
    symbols::expressions::{Exprs, FnCall, Literal, Primary},
};

// Primary = Literal | Identifier ( "(" ")" )? | "(" Exprs ")"
impl<'t> TokenStream<'t> {
    pub(super) fn consume_primary_expression(&mut self) -> Result<super::Exprs, ParseError> {
        // Primary = Literal | "(" Expr ")"
        let t = *self.peek().ok_or(ParseError::InvalidEOF(vec![
            TkKind::Ident,
            TkKind::IntegerLiteral,
        ]))?;

        match &t.kind {
            TkKind::IntegerLiteral => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Integer(
                    t.unwrap_integer_value(),
                ))))
            }
            TkKind::StringLiteral => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::String(
                    t.unwrap_string_value(),
                ))))
            }
            TkKind::BoolLiteralTrue => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(true))))
            }
            TkKind::BoolLiteralFalse => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(false))))
            }
            TkKind::Ident => {
                let qualed_id = self.consume_qualified_identifier()?;

                if let Some(t) = self.peek() {
                    if let TkKind::LPare = t.kind {
                        self.next();

                        let mut args: Vec<Exprs> = vec![];

                        while let Some(t) = self.peek() {
                            if let TkKind::RPare = t.kind {
                                self.next();
                                return Ok(Exprs::Primary(Primary::FnCall(FnCall {
                                    qualed_id,
                                    args,
                                })));
                            } else {
                                let expr = self.consume_expression()?;
                                args.push(expr);

                                if let Some(t) = self.peek() {
                                    if let TkKind::Comma = t.kind {
                                        self.next();
                                        continue;
                                    } else if let TkKind::RPare = t.kind {
                                        continue;
                                    } else {
                                        return Err(ParseError::InvalidToken(
                                            vec![TkKind::RPare, TkKind::Comma],
                                            t.to_owned().clone(),
                                        ));
                                    }
                                } else {
                                    return Err(ParseError::InvalidEOF(vec![
                                        TkKind::RPare,
                                        TkKind::Comma,
                                    ]));
                                }
                            }
                        }

                        Err(ParseError::InvalidEOF(vec![TkKind::RPare]))
                    } else if let TkKind::LBrace = t.kind {
                        self.next();

                        let mut members: Vec<(String, Box<Exprs>)> = vec![];

                        while let Some(t) = self.peek() {
                            if let TkKind::RBrace = t.kind {
                                self.next();
                                return Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                                    qualed_id, members,
                                ))));
                            } else {
                                let member = self.consume_identifier()?;
                                let _ = self.must_consume_next(vec![TkKind::Assign])?;
                                let expr = self.consume_expression()?;

                                members.push((member, Box::new(expr)));

                                if let Some(t) = self.peek() {
                                    if let TkKind::Comma = t.kind {
                                        self.next();
                                        continue;
                                    } else if let TkKind::RBrace = t.kind {
                                        continue;
                                    } else {
                                        return Err(ParseError::InvalidToken(
                                            vec![TkKind::RBrace, TkKind::Comma],
                                            t.to_owned().clone(),
                                        ));
                                    }
                                } else {
                                    return Err(ParseError::InvalidEOF(vec![
                                        TkKind::RBrace,
                                        TkKind::Comma,
                                    ]));
                                }
                            }
                        }

                        Err(ParseError::InvalidEOF(vec![TkKind::RBrace]))
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
            TkKind::LPare => {
                self.next();
                let expr = self.consume_expression()?;

                let _ = self.must_consume_next(vec![TkKind::RPare])?;

                Ok(expr)
            }
            _ => Err(ParseError::InvalidEOF(vec![
                TkKind::Ident,
                TkKind::IntegerLiteral,
            ])),
        }
    }
}
