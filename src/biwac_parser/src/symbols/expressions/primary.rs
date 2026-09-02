use biwac_lexer::{TkKind, TkKindName};
use biwac_span::Span;

use biwac_ast::{
    AbsolutePathHeader, BoolLiteral, Exprs, FnCall, Ident, IntegerLiteral, Literal, Path, Primary,
    SelfTypHeader, StringLiteral, StructLiteral, Variable,
};

use crate::{ParseError, TokenStream};

// Primary = Literal | Identifier ( "(" ")" )? | "(" Exprs ")"
impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    pub(super) fn consume_primary_expression(&mut self) -> Result<Exprs, ParseError<'src>> {
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
            // `package::` から始まる絶対パスも式に書ける。
            // `package` は識別子ではなくキーワードなので、ここで拾わないと
            // `consume_qualified_identifier` に辿り着けない。
            TkKind::Ident(_) | TkKind::KwPackage => {
                let begin = t.span.clone();
                let path = self.consume_qualified_identifier()?;

                if let Some(t2) = self.peek() {
                    if let TkKind::MarkLPare = t2.kind {
                        let (args, span) = self.consume_arguments()?;

                        Ok(Exprs::Primary(Primary::FnCall(FnCall {
                            path,
                            args,
                            span: Span::merge(&begin, &span),
                        })))
                    } else if let TkKind::MarkLBrace = t2.kind {
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
            TkKind::KwSelfTyp => {
                let begin = t.span.clone();

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

                            let (args, span) = self.consume_arguments()?;

                            Ok(Exprs::Primary(Primary::FnCall(FnCall {
                                path: Path::new(
                                    Some(AbsolutePathHeader::SelfTyp(SelfTypHeader::new(
                                        begin.clone(),
                                    ))),
                                    vec![ident.into()],
                                ),
                                args,
                                span: Span::merge(&begin, &span),
                            })))
                        }
                        TkKind::MarkLBrace => {
                            let (members, span) = self.consume_struct_members()?;

                            Ok(Exprs::Primary(Primary::Literal(Literal::Struct(
                                StructLiteral {
                                    members,
                                    span: Span::merge(&begin, &span),
                                    path: Path::new(
                                        Some(AbsolutePathHeader::SelfTyp(SelfTypHeader::new(
                                            begin.clone(),
                                        ))),
                                        Vec::new(),
                                    ),
                                },
                            ))))
                        }
                        _ => Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkDoubleColon, TkKindName::MarkLBrace],
                            found: t.clone(),
                        }),
                    }
                } else {
                    Err(ParseError::InvalidEOF {
                        mod_id,
                        expecteds: vec![TkKindName::MarkDoubleColon, TkKindName::MarkLBrace],
                    })
                }
            }
            TkKind::KwSelfVar => {
                let self_span = t.span.clone();

                // "self"
                self.next();
                Ok(Exprs::Primary(Primary::Variable(Variable::SelfVar(
                    self_span,
                ))))
            }
            TkKind::MarkLPare => {
                self.next();
                let expr = self.consume_expression()?;

                let _ = self.must_consume_next(vec![TkKindName::MarkRPare])?;

                Ok(expr)
            }
            // 実際にはトークンがある。EOF として報告すると
            // 位置がファイル末尾になって原因が追えないので、そのトークンを指す。
            _ => Err(ParseError::InvalidToken {
                expecteds: vec![
                    TkKindName::Ident,
                    TkKindName::LiteralInteger,
                    TkKindName::LiteralString,
                    TkKindName::KwBoolTrue,
                    TkKindName::KwBoolFalse,
                    TkKindName::MarkLPare,
                ],
                found: t.clone(),
            }),
        }
    }

    pub(super) fn consume_arguments(&mut self) -> Result<(Vec<Exprs>, Span), ParseError<'src>> {
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
                let expr = self.consume_expression()?;
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
                let expr = self.consume_expression()?;

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
