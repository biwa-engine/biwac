use biwac_base::Span;
use biwac_lexer::{TkKind, TkKindName};

use biwac_ast::{
    AbsolutePathHeader, BoolLiteral, Exprs, FnCall, Ident, IntegerLiteral, Literal, Path, Primary,
    StringLiteral, StructLiteral, Variable,
};

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

// Primary = Literal | Identifier ( "(" ")" )? | "(" Exprs ")"
impl<'t, 'src> TokenStream<'t, 'src> {
    pub(super) fn consume_primary_expression(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<Exprs, ParseError<'src>> {
        let mod_id = self.mod_id;

        // Primary = Literal | "(" Expr ")"
        let t = *self.peek().ok_or(ParseError::InvalidEOF {
            mod_id,
            expecteds: vec![TkKindName::Ident, TkKindName::LiteralInteger],
        })?;

        match &t.kind {
            TkKind::LiteralInteger(val) => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Integer(
                    IntegerLiteral {
                        val: *val,
                        span: t.span.clone(),
                    },
                ))))
            }
            TkKind::LiteralString(str) => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::String(
                    StringLiteral {
                        val: str.to_string(),
                        span: t.span.clone(),
                    },
                ))))
            }
            TkKind::KwBoolTrue => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(
                    BoolLiteral {
                        val: true,
                        span: t.span.clone(),
                    },
                ))))
            }
            TkKind::KwBoolFalse => {
                self.next();
                Ok(Exprs::Primary(Primary::Literal(Literal::Bool(
                    BoolLiteral {
                        val: false,
                        span: t.span.clone(),
                    },
                ))))
            }
            TkKind::Ident(_) => {
                let begin = t.span.clone();
                let path = self.consume_qualified_identifier()?;
                let qualed_id = self.consume_qualified_identifier()?;

                if let Some(t2) = self.peek() {
                    if let TkKind::MarkLPare = t2.kind {
                        let (args, span) = self.consume_arguments(ctx)?;

                        Ok(Exprs::Primary(Primary::FnCall(FnCall {
                            path,
                            args,
                            span: Span::merge(&begin, &span),
                        })))
                    } else if let TkKind::MarkLBrace = t2.kind {
                        let (members, span) = self.consume_struct_members(ctx)?;

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
            TkKind::KwSelfTyp => {
                let begin = t.span.clone();

                if let Some(self_typ) = &ctx.self_typ {
                    // "Self" (
                    //   ( "::" <identifier> "(" ... ")" ) |
                    //   ( "{" ... "}" )?
                    // )
                    self.next();

                    if let Some(t) = self.peek().copied() {
                        match t.kind {
                            TkKind::MarkDoubleColon => {
                                self.next();

                                let ident = self.consume_identifier()?;

                                let (args, span) = self.consume_arguments(ctx)?;

                                Ok(Exprs::Primary(Primary::FnCall(FnCall {
                                    path: Path::new(
                                        Some(AbsolutePathHeader::SelfTyp(begin.clone())),
                                        vec![ident.into()],
                                    ),
                                    args,
                                    span: Span::merge(&begin, &span),
                                })))
                            }
                            TkKind::MarkLBrace => {
                                let (members, span) = self.consume_struct_members(ctx)?;

                                Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                                    StructLiteral {
                                        members,
                                        span: Span::merge(&begin, &span),
                                        path: Path::new(
                                            Some(AbsolutePathHeader::SelfTyp(begin.clone())),
                                            Vec::new(),
                                        ),
                                    },
                                ))))
                            }
                            _ => Err(ParseError::InvalidToken {
                                expecteds: vec![
                                    TkKindName::MarkDoubleColon,
                                    TkKindName::MarkLBrace,
                                ],
                                found: t.clone(),
                            }),
                        }
                    } else {
                        Err(ParseError::InvalidEOF {
                            mod_id,
                            expecteds: vec![TkKindName::MarkDoubleColon, TkKindName::MarkLBrace],
                        })
                    }
                } else {
                    Err(ParseError::InvalidToken {
                        expecteds: vec![
                            TkKindName::Ident,
                            TkKindName::LiteralInteger,
                            TkKindName::LiteralString,
                            TkKindName::KwBoolTrue,
                            TkKindName::KwBoolFalse,
                            TkKindName::MarkLPare,
                        ],
                        found: t.clone(),
                    })
                }
            }
            TkKind::KwSelfVar => {
                let self_span = t.span.clone();

                if ctx.is_method {
                    // "self"
                    self.next();
                    Ok(Exprs::Primary(Primary::Variable(Variable::SelfVar(
                        self_span,
                    ))))
                } else {
                    Err(ParseError::InvalidToken {
                        expecteds: vec![
                            TkKindName::Ident,
                            TkKindName::LiteralInteger,
                            TkKindName::LiteralString,
                            TkKindName::KwBoolTrue,
                            TkKindName::KwBoolFalse,
                            TkKindName::MarkLPare,
                        ],
                        found: t.clone(),
                    })
                }
            }
            TkKind::MarkLPare => {
                self.next();
                let expr = self.consume_expression(ctx)?;

                let _ = self.must_consume_next(vec![TkKindName::MarkRPare])?;

                Ok(expr)
            }
            _ => Err(ParseError::InvalidEOF {
                mod_id,
                expecteds: vec![
                    TkKindName::Ident,
                    TkKindName::LiteralInteger,
                    TkKindName::LiteralString,
                    TkKindName::KwBoolTrue,
                    TkKindName::KwBoolFalse,
                    TkKindName::MarkLPare,
                ],
            }),
        }
    }

    pub(super) fn consume_arguments(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<(Vec<Exprs>, Span), ParseError<'src>> {
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLPare])?
            .span
            .clone();
        let mut span = begin.clone();

        let mut args: Vec<Exprs> = vec![];

        while let Some(t3) = self.peek() {
            if let TkKind::MarkRPare = t3.kind {
                let end = t3.span.clone();
                span = Span::merge(&begin, &end);

                self.next();
                break;
            } else {
                let expr = self.consume_expression(ctx)?;
                args.push(expr);

                if let Some(t) = self.peek() {
                    if let TkKind::MarkComma = t.kind {
                        self.next();
                        continue;
                    } else if let TkKind::MarkRPare = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkRPare, TkKindName::MarkComma],
                            found: t.to_owned().clone(),
                        });
                    }
                } else {
                    return Err(ParseError::InvalidEOF {
                        mod_id: self.mod_id,
                        expecteds: vec![TkKindName::MarkRPare, TkKindName::MarkComma],
                    });
                }
            }
        }

        Ok((args, span))
    }

    fn consume_struct_members(
        &mut self,
        ctx: &FnParseCtx,
    ) -> Result<(Vec<(Ident, Box<Exprs>)>, Span), ParseError<'src>> {
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLBrace])?
            .span
            .clone();
        let mut span = begin.clone();

        let mut members: Vec<(Ident, Box<Exprs>)> = vec![];

        while let Some(t3) = self.peek() {
            if let TkKind::MarkRBrace = t3.kind {
                let end = t3.span.clone();
                span = Span::merge(&begin, &end);
                self.next();
                break;
            } else {
                let member = self.consume_identifier()?;
                let _ = self.must_consume_next(vec![TkKindName::MarkAssign])?;
                let expr = self.consume_expression(ctx)?;

                members.push((member, Box::new(expr)));

                if let Some(t) = self.peek() {
                    if let TkKind::MarkComma = t.kind {
                        self.next();
                        continue;
                    } else if let TkKind::MarkRBrace = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkRBrace, TkKindName::MarkComma],
                            found: t.to_owned().clone(),
                        });
                    }
                } else {
                    return Err(ParseError::InvalidEOF {
                        mod_id: self.mod_id,
                        expecteds: vec![TkKindName::MarkRBrace, TkKindName::MarkComma],
                    });
                }
            }
        }

        Ok((members, span))
    }
}
