use biwac_span::Span;

use biwac_ast::{
    BoolLiteral, Exprs, FnCall, Ident, IntegerLiteral, Literal, Primary, StringLiteral,
    StructLiteral, Variable,
};

use crate::{
    NovelParseError, NovelSourceStream,
    token::{NCodeTkKind, NCodeTkKindName},
};

// Primary = Literal | Identifier ( "(" ")" )? | "(" Exprs ")"
impl<'src> NovelSourceStream<'src> {
    pub(super) fn consume_primary_expression(&mut self) -> Result<Exprs, NovelParseError> {
        // Primary = Literal | "(" Expr ")"
        let t = self
            .peek_token()?
            .cloned()
            .ok_or(NovelParseError::InvalidLineEnd {
                expecteds: vec![NCodeTkKindName::Ident, NCodeTkKindName::LiteralInteger],
                span: self.current_span(1),
            })?;

        match &t.kind {
            NCodeTkKind::LiteralInteger(int) => {
                self.next_token()?;
                Ok(Exprs::Primary(Primary::Literal(Literal::Integer(
                    IntegerLiteral {
                        val: *int,
                        span: t.span,
                    },
                ))))
            }
            NCodeTkKind::LiteralString(str) => {
                self.next_token()?;
                Ok(Exprs::Primary(Primary::Literal(Literal::String(
                    StringLiteral {
                        val: str.to_string(),
                        span: t.span,
                    },
                ))))
            }
            NCodeTkKind::KwTrue => {
                self.next_token()?;
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(
                    BoolLiteral {
                        val: true,
                        span: t.span,
                    },
                ))))
            }
            NCodeTkKind::KwFalse => {
                self.next_token()?;
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(
                    BoolLiteral {
                        val: false,
                        span: t.span,
                    },
                ))))
            }
            // `package::` から始まる絶対パスも式に書ける。
            // `package` は識別子ではなくキーワードなので、ここで拾わないと
            // `consume_qualified_identifier` に辿り着けない。
            NCodeTkKind::Ident(_) | NCodeTkKind::KwPackage => {
                let begin = t.span.clone();
                let path = self.consume_qualified_identifier()?;

                if let Some(t2) = self.peek_token()? {
                    if let NCodeTkKind::MarkLPare = t2.kind {
                        let (args, span) = self.consume_arguments()?;

                        Ok(Exprs::Primary(Primary::FnCall(FnCall {
                            path,
                            args,
                            span: Span::merge(&begin, &span),
                        })))
                    } else if let NCodeTkKind::MarkLBrace = t2.kind {
                        let (members, span) = self.consume_struct_members()?;

                        Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                            StructLiteral {
                                path,
                                members,
                                span: Span::merge(&begin, &span),
                            },
                        ))))
                    } else {
                        Ok(Exprs::Primary(Primary::Variable(Variable::Path(path))))
                    }
                } else {
                    Ok(Exprs::Primary(Primary::Variable(Variable::Path(path))))
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
                span: t.span,
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
