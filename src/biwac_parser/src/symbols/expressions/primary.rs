use biwac_base::Span;
use biwac_lexer::TkKind;

use crate::{
    Ident, ParseError,
    parser::TokenStream,
    symbols::expressions::{
        BoolLiteral, Exprs, FnCall, IntegerLiteral, Literal, Primary, StringLiteral, StructLiteral,
    },
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
                    IntegerLiteral {
                        val: t.unwrap_integer_value(),
                        span: t.span.clone(),
                    },
                ))))
            }
            TkKind::StringLiteral => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::String(
                    StringLiteral {
                        val: t.unwrap_string_value(),
                        span: t.span.clone(),
                    },
                ))))
            }
            TkKind::BoolLiteralTrue => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(
                    BoolLiteral {
                        val: true,
                        span: t.span.clone(),
                    },
                ))))
            }
            TkKind::BoolLiteralFalse => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(
                    BoolLiteral {
                        val: false,
                        span: t.span.clone(),
                    },
                ))))
            }
            TkKind::Ident => {
                let qualed_id = self.consume_qualified_identifier()?;

                if let Some(t2) = self.peek() {
                    if let TkKind::LPare = t2.kind {
                        self.next();

                        let mut args: Vec<Exprs> = vec![];

                        while let Some(t3) = self.peek() {
                            if let TkKind::RPare = t3.kind {
                                let end = t3.span.clone();
                                self.next();
                                return Ok(Exprs::Primary(Primary::FnCall(FnCall {
                                    qualed_id,
                                    args,
                                    span: Span::merge(&t.span, &end),
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
                    } else if let TkKind::LBrace = t2.kind {
                        self.next();

                        let mut members: Vec<(Ident, Box<Exprs>)> = vec![];

                        while let Some(t3) = self.peek() {
                            if let TkKind::RBrace = t3.kind {
                                let end = t3.span.clone();
                                self.next();
                                return Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                                    StructLiteral {
                                        qualid: qualed_id,
                                        members,
                                        span: Span::merge(&t.span, &end),
                                    },
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
                        Ok(Exprs::Primary(Primary::Variable(Ident {
                            id: qualed_id.id,
                            span: t.span.clone(),
                        })))
                    }
                } else if !qualed_id.quals.is_empty() && !qualed_id.is_from_root {
                    panic!("variable cannot qualified");
                } else {
                    Ok(Exprs::Primary(Primary::Variable(Ident {
                        id: qualed_id.id,
                        span: t.span.clone(),
                    })))
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
