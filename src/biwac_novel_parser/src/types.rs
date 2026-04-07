use biwac_ast::{DefTyp, PrimTyp, TypRepr, TypReprVal};

use crate::{
    NovelParseError, NovelSourceStream,
    token::{NCodeTkKind, NCodeTkKindName},
};

impl<'src> NovelSourceStream<'src> {
    pub(crate) fn must_consume_type_annotation(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<TypRepr, NovelParseError> {
        let t = self
            .peek_token()?
            .ok_or(NovelParseError::InvalidLineEnd {
                expecteds: vec![NCodeTkKindName::MarkColon],
                span: self.current_span(1),
            })?
            .to_owned();

        if let NCodeTkKind::MarkColon = t.kind {
            self.next_token()?;

            Ok(self.consume_type_representaion(self_typ)?)
        } else {
            Err(NovelParseError::InvalidToken {
                expecteds: vec![NCodeTkKindName::MarkColon],
                found: Box::new(t.clone()),
            })
        }
    }

    pub(crate) fn opt_consume_type_annotation(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<Option<TypRepr>, NovelParseError> {
        let t = self
            .peek_token()?
            .ok_or(NovelParseError::InvalidLineEnd {
                expecteds: vec![NCodeTkKindName::MarkColon],
                span: self.current_span(1),
            })?
            .to_owned();

        if let NCodeTkKind::MarkColon = t.kind {
            self.next_token()?;

            Ok(Some(self.consume_type_representaion(self_typ)?))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_type_representaion(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<TypRepr, NovelParseError> {
        if let Some(t) = self.peek_token()? {
            if let NCodeTkKind::KwUint = t.kind {
                let span = t.span.clone();
                self.next_token()?;
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Uint),
                    span,
                })
            } else if let NCodeTkKind::KwInt = t.kind {
                let span = t.span.clone();
                self.next_token()?;
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Int),
                    span,
                })
            } else if let NCodeTkKind::KwFloat = t.kind {
                let span = t.span.clone();
                self.next_token()?;
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Float),
                    span,
                })
            } else if let NCodeTkKind::KwBool = t.kind {
                let span = t.span.clone();
                self.next_token()?;
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Bool),
                    span,
                })
            } else if let NCodeTkKind::Ident(_) = t.kind {
                // NOTE: idのみ得られた場合、ジェネリクス型(`T`)である可能性がある
                let qualid = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args(self_typ)?;

                Ok(TypRepr {
                    span: qualid.span.clone(),
                    val: TypReprVal::Defined(DefTyp { qualid, genargs }),
                })
            } else if let NCodeTkKind::KwPackage = t.kind {
                let qualid = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args(self_typ)?;

                Ok(TypRepr {
                    span: qualid.span.clone(),
                    val: TypReprVal::Defined(DefTyp { qualid, genargs }),
                })
            } else {
                Err(NovelParseError::InvalidToken {
                    expecteds: vec![
                        NCodeTkKindName::KwUint,
                        NCodeTkKindName::KwInt,
                        NCodeTkKindName::KwBool,
                        NCodeTkKindName::Ident,
                    ],

                    found: Box::new(t.to_owned().clone()),
                })
            }
        } else {
            Err(NovelParseError::InvalidLineEnd {
                expecteds: vec![
                    NCodeTkKindName::KwUint,
                    NCodeTkKindName::KwInt,
                    NCodeTkKindName::KwBool,
                    NCodeTkKindName::Ident,
                    NCodeTkKindName::KwPackage,
                ],
                span: self.current_span(1),
            })
        }
    }

    /// Optionaly consumes tokens and parses to get generic arguments.
    /// We should use here:
    /// let a: foo::bar[Int] = ...
    ///                ^
    ///                |
    // pub(crate) fn opt_consume_generic_argument_assignment(
    pub(crate) fn opt_consume_generic_args(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<Option<Vec<TypRepr>>, NovelParseError> {
        if let Some(t) = self.peek_token()?
            && matches!(t.kind, NCodeTkKind::MarkLBracket)
        {
            self.next_token()?;
        } else {
            return Ok(None);
        }

        let mut genargs = vec![];
        loop {
            if let Some(t) = self.peek_token()?
                && let NCodeTkKind::MarkRBracket = t.kind
            {
                self.next_token()?;

                return Ok(Some(genargs));
            } else {
                genargs.push(self.consume_type_representaion(self_typ)?);

                if let Some(t) = self.next_token()? {
                    if let NCodeTkKind::MarkRBracket = t.kind {
                        return Ok(Some(genargs));
                    } else if let NCodeTkKind::MarkComma = t.kind {
                        continue;
                    } else {
                        return Err(NovelParseError::InvalidToken {
                            expecteds: vec![
                                NCodeTkKindName::MarkRBracket,
                                NCodeTkKindName::MarkComma,
                            ],
                            found: Box::new(t.clone()),
                        });
                    }
                } else {
                    return Err(NovelParseError::InvalidLineEnd {
                        expecteds: vec![NCodeTkKindName::MarkRBracket, NCodeTkKindName::MarkComma],
                        span: self.current_span(1),
                    });
                }
            }
        }
    }
}
