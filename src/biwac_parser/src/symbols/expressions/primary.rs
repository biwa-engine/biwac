use biwac_base::Span;
use biwac_lexer::TkKind;

use biwac_ast::{
    BoolLiteral, Exprs, FnCall, Ident, IntegerLiteral, Literal, Primary, QualifiedId,
    StringLiteral, StructLiteral,
};

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

// Primary = Literal | Identifier ( "(" ")" )? | "(" Exprs ")"
impl<'t> TokenStream<'t> {
    pub(super) fn consume_primary_expression(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<super::Exprs, ParseError> {
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
                let begin = t.span.clone();
                let qualed_id = self.consume_qualified_identifier()?;

                if let Some(t2) = self.peek() {
                    if let TkKind::LPare = t2.kind {
                        let (args, span) = self.consume_arguments(ctx)?;

                        Ok(Exprs::Primary(Primary::FnCall(FnCall {
                            qualed_id,
                            args,
                            span: Span::merge(&begin, &span),
                        })))
                    } else if let TkKind::LBrace = t2.kind {
                        let (members, span) = self.consume_struct_members(ctx)?;

                        Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                            StructLiteral {
                                qualid: qualed_id,
                                members,
                                span: Span::merge(&begin, &span),
                            },
                        ))))
                    } else if !qualed_id.quals.is_empty() && !qualed_id.is_from_root {
                        Err(ParseError::InvalidToken(
                            vec![TkKind::LPare, TkKind::LBrace],
                            t2.to_owned().clone(),
                        ))
                    } else {
                        Ok(Exprs::Primary(Primary::Variable(Ident {
                            id: qualed_id.id,
                            span: t.span.clone(),
                        })))
                    }
                } else if !qualed_id.quals.is_empty() && !qualed_id.is_from_root {
                    Err(ParseError::InvalidEOF(vec![TkKind::LPare, TkKind::LBrace]))
                } else {
                    Ok(Exprs::Primary(Primary::Variable(Ident {
                        id: qualed_id.id,
                        span: t.span.clone(),
                    })))
                }
            }
            TkKind::SelfTyp => {
                let begin = t.span.clone();

                if let Some(self_typ) = &ctx.self_typ {
                    // "Self" (
                    //   ( "::" <identifier> "(" ... ")" ) |
                    //   ( "{" ... "}" )?
                    // )
                    self.next();

                    if let Some(t) = self.peek().copied() {
                        match t.kind {
                            TkKind::DoubleColon => {
                                self.next();

                                let ident = self.consume_identifier()?;

                                let (args, span) = self.consume_arguments(ctx)?;

                                Ok(Exprs::Primary(Primary::FnCall(FnCall {
                                    qualed_id: QualifiedId::new_type_impl(
                                        self_typ,
                                        ident.id,
                                        Span::merge(&begin, &ident.span),
                                    ),
                                    args,
                                    span: Span::merge(&begin, &span),
                                })))
                            }
                            TkKind::LBrace => {
                                let (members, span) = self.consume_struct_members(ctx)?;

                                Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                                    StructLiteral {
                                        members,
                                        span: Span::merge(&begin, &span),
                                        qualid: QualifiedId::from_type(self_typ, begin),
                                    },
                                ))))
                            }
                            _ => Err(ParseError::InvalidToken(
                                vec![TkKind::DoubleColon, TkKind::LBrace],
                                t.clone(),
                            )),
                        }
                    } else {
                        Err(ParseError::InvalidEOF(vec![
                            TkKind::DoubleColon,
                            TkKind::LBrace,
                        ]))
                    }
                } else {
                    Err(ParseError::InvalidEOF(vec![
                        TkKind::Ident,
                        TkKind::IntegerLiteral,
                    ]))
                }
            }
            TkKind::SelfVar => {
                let begin = t.span.clone();

                if ctx.is_method {
                    // "self"
                    self.next();
                    Ok(Exprs::Primary(Primary::Variable(Ident {
                        // WARN: really?
                        id: "self".to_string(),
                        span: begin,
                    })))
                } else {
                    Err(ParseError::InvalidEOF(vec![
                        TkKind::Ident,
                        TkKind::IntegerLiteral,
                    ]))
                }
            }
            TkKind::LPare => {
                self.next();
                let expr = self.consume_expression(ctx)?;

                let _ = self.must_consume_next(vec![TkKind::RPare])?;

                Ok(expr)
            }
            _ => Err(ParseError::InvalidEOF(vec![
                TkKind::Ident,
                TkKind::IntegerLiteral,
            ])),
        }
    }

    pub(super) fn consume_arguments(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<(Vec<Exprs>, Span), ParseError> {
        let begin = self.must_consume_next(vec![TkKind::LPare])?.span.clone();
        let mut span = begin.clone();

        let mut args: Vec<Exprs> = vec![];

        while let Some(t3) = self.peek() {
            if let TkKind::RPare = t3.kind {
                let end = t3.span.clone();
                span = Span::merge(&begin, &end);

                self.next();
                break;
            } else {
                let expr = self.consume_expression(ctx)?;
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
                    return Err(ParseError::InvalidEOF(vec![TkKind::RPare, TkKind::Comma]));
                }
            }
        }

        Ok((args, span))
    }

    fn consume_struct_members(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<(Vec<(Ident, Box<Exprs>)>, Span), ParseError> {
        let begin = self.must_consume_next(vec![TkKind::LBrace])?.span.clone();
        let mut span = begin.clone();

        let mut members: Vec<(Ident, Box<Exprs>)> = vec![];

        while let Some(t3) = self.peek() {
            if let TkKind::RBrace = t3.kind {
                let end = t3.span.clone();
                span = Span::merge(&begin, &end);
                self.next();
                break;
            } else {
                let member = self.consume_identifier()?;
                let _ = self.must_consume_next(vec![TkKind::Assign])?;
                let expr = self.consume_expression(ctx)?;

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
                    return Err(ParseError::InvalidEOF(vec![TkKind::RBrace, TkKind::Comma]));
                }
            }
        }

        Ok((members, span))
    }
}
