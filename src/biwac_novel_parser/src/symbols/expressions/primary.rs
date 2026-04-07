use biwac_base::Span;

use biwac_ast::{
    BoolLiteral, Exprs, FnCall, Ident, IntegerLiteral, Literal, Primary, StringLiteral,
    StructLiteral,
};

use crate::{
    NovelParseError, NovelSourceStream,
    token::{NCodeTkKind, NCodeTkKindName},
};

// Primary = Literal | Identifier ( "(" ")" )? | "(" Exprs ")"
impl<'src> NovelSourceStream<'src> {
    pub(super) fn consume_primary_expression(&mut self) -> Result<Exprs, NovelParseError> {
        // Primary = Literal | "(" Expr ")"
        let t = self.peek_token()?.ok_or(NovelParseError::InvalidLineEnd {
            expecteds: vec![NCodeTkKindName::Ident, NCodeTkKindName::LiteralInteger],
            span: self.current_span(1),
        })?;

        match &t.kind {
            NCodeTkKind::LiteralInteger(int) => {
                self.next_token()?;
                Ok(Exprs::Primary(Primary::Literal(Literal::Integer(
                    IntegerLiteral {
                        val: *int,
                        span: t.span.clone(),
                    },
                ))))
            }
            NCodeTkKind::LiteralString(str) => {
                self.next_token()?;
                Ok(Exprs::Primary(Primary::Literal(Literal::String(
                    StringLiteral {
                        val: str.to_string(),
                        span: t.span.clone(),
                    },
                ))))
            }
            NCodeTkKind::KwTrue => {
                self.next_token()?;
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(
                    BoolLiteral {
                        val: true,
                        span: t.span.clone(),
                    },
                ))))
            }
            NCodeTkKind::KwFalse => {
                self.next_token()?;
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(
                    BoolLiteral {
                        val: false,
                        span: t.span.clone(),
                    },
                ))))
            }
            NCodeTkKind::Ident(_) => {
                let begin = t.span.clone();
                let qualed_id = self.consume_qualified_identifier()?;

                if let Some(t2) = self.peek_token()? {
                    if let NCodeTkKind::MarkLPare = t2.kind {
                        let (args, span) = self.consume_arguments()?;

                        Ok(Exprs::Primary(Primary::FnCall(FnCall {
                            qualed_id,
                            args,
                            span: Span::merge(&begin, &span),
                        })))
                    } else if let NCodeTkKind::MarkLBrace = t2.kind {
                        let (members, span) = self.consume_struct_members()?;

                        Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                            StructLiteral {
                                qualid: qualed_id,
                                members,
                                span: Span::merge(&begin, &span),
                            },
                        ))))
                    } else if !qualed_id.quals.is_empty() && !qualed_id.is_from_root {
                        Err(NovelParseError::InvalidToken {
                            expecteds: vec![
                                NCodeTkKindName::MarkLPare,
                                NCodeTkKindName::MarkLBrace,
                            ],
                            found: Box::new(t2.to_owned().clone()),
                        })
                    } else {
                        Ok(Exprs::Primary(Primary::Variable(Ident {
                            id: qualed_id.id,
                            span: t.span.clone(),
                        })))
                    }
                } else if !qualed_id.quals.is_empty() && !qualed_id.is_from_root {
                    Err(NovelParseError::InvalidLineEnd {
                        expecteds: vec![NCodeTkKindName::MarkLPare, NCodeTkKindName::MarkLBrace],
                        span: t.span.clone(),
                    })
                } else {
                    Ok(Exprs::Primary(Primary::Variable(Ident {
                        id: qualed_id.id,
                        span: t.span.clone(),
                    })))
                }
            }
            NCodeTkKind::MarkLPare => {
                self.next_token()?;
                let expr = self.consume_expression()?;

                let _ = self.must_consume_next(vec![NCodeTkKindName::MarkRPare])?;

                Ok(expr)
            }
            _ => Err(NovelParseError::InvalidLineEnd {
                expecteds: vec![NCodeTkKindName::Ident, NCodeTkKindName::LiteralInteger],
                span: t.span.clone(),
            }),
        }
    }

    pub(super) fn consume_arguments(&mut self) -> Result<(Vec<Exprs>, Span), NovelParseError> {
        let begin = self
            .must_consume_next(vec![NCodeTkKindName::MarkLPare])?
            .span
            .clone();
        let mut span = begin.clone();

        let mut args: Vec<Exprs> = vec![];

        while let Some(t3) = self.peek_token()? {
            if let NCodeTkKind::MarkRPare = t3.kind {
                let end = t3.span.clone();
                span = Span::merge(&begin, &end);

                self.next_token()?;
                break;
            } else {
                let expr = self.consume_expression()?;
                args.push(expr);

                if let Some(t) = self.peek_token()? {
                    if let NCodeTkKind::MarkComma = t.kind {
                        self.next_token()?;
                        continue;
                    } else if let NCodeTkKind::MarkRPare = t.kind {
                        continue;
                    } else {
                        return Err(NovelParseError::InvalidToken {
                            expecteds: vec![NCodeTkKindName::MarkRPare, NCodeTkKindName::MarkComma],
                            found: Box::new(t.to_owned().clone()),
                        });
                    }
                } else {
                    return Err(NovelParseError::InvalidLineEnd {
                        expecteds: vec![NCodeTkKindName::MarkRPare, NCodeTkKindName::MarkComma],
                        span: self.current_span(1),
                    });
                }
            }
        }

        Ok((args, span))
    }

    fn consume_struct_members(
        &mut self,
    ) -> Result<(Vec<(Ident, Box<Exprs>)>, Span), NovelParseError> {
        let begin = self
            .must_consume_next(vec![NCodeTkKindName::MarkLBrace])?
            .span
            .clone();
        let mut span = begin.clone();

        let mut members: Vec<(Ident, Box<Exprs>)> = vec![];

        while let Some(t3) = self.peek_token()? {
            if let NCodeTkKind::MarkRBrace = t3.kind {
                let end = t3.span.clone();
                span = Span::merge(&begin, &end);
                self.next_token()?;
                break;
            } else {
                let member = self.consume_identifier()?;
                let _ = self.must_consume_next(vec![NCodeTkKindName::MarkAssign])?;
                let expr = self.consume_expression()?;

                members.push((member, Box::new(expr)));

                if let Some(t) = self.peek_token()? {
                    if let NCodeTkKind::MarkComma = t.kind {
                        self.next_token()?;
                        continue;
                    } else if let NCodeTkKind::MarkRBrace = t.kind {
                        continue;
                    } else {
                        return Err(NovelParseError::InvalidToken {
                            expecteds: vec![
                                NCodeTkKindName::MarkRBrace,
                                NCodeTkKindName::MarkComma,
                            ],
                            found: Box::new(t.to_owned().clone()),
                        });
                    }
                } else {
                    return Err(NovelParseError::InvalidLineEnd {
                        expecteds: vec![NCodeTkKindName::MarkRBrace, NCodeTkKindName::MarkComma],
                        span: self.current_span(1),
                    });
                }
            }
        }

        Ok((members, span))
    }
}
