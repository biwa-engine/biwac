use biwac_ast::{Ident, PrimTyp, TypRepr, TypReprVal};
use biwac_lexer::{TkKind, TkKindName};

use crate::{ParseError, TokenStream};

impl<'t, 'src> TokenStream<'t, 'src> {
    pub(crate) fn must_consume_type_annotation(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<TypRepr, ParseError<'src>> {
        let mod_id = self.mod_id;
        let t = self
            .peek()
            .ok_or(ParseError::InvalidEOF {
                mod_id,
                expecteds: vec![TkKindName::MarkColon],
            })?
            .to_owned();

        if let TkKind::MarkColon = t.kind {
            self.next();

            Ok(self.consume_type_representaion(self_typ)?)
        } else {
            Err(ParseError::InvalidToken {
                expecteds: vec![TkKindName::MarkColon],
                found: t.clone(),
            })
        }
    }

    pub(crate) fn opt_consume_type_annotation(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<Option<TypRepr>, ParseError<'src>> {
        let mod_id = self.mod_id;
        let t = self
            .peek()
            .ok_or(ParseError::InvalidEOF {
                mod_id,
                expecteds: vec![TkKindName::MarkColon],
            })?
            .to_owned();

        if let TkKind::MarkColon = t.kind {
            self.next();

            Ok(Some(self.consume_type_representaion(self_typ)?))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_type_representaion(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<TypRepr, ParseError<'src>> {
        if let Some(t) = self.peek() {
            if let TkKind::KwUint = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Uint),
                    span,
                })
            } else if let TkKind::KwInt = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Int),
                    span,
                })
            } else if let TkKind::KwFloat = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Float),
                    span,
                })
            } else if let TkKind::KwBool = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::Primitive(PrimTyp::Bool),
                    span,
                })
            } else if let TkKind::Ident(_) = t.kind {
                // NOTE: idのみ得られた場合、ジェネリクス型(`T`)である可能性がある
                let path = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args(self_typ)?;

                Ok(TypRepr::new_def_typ(path, genargs))
            } else if let TkKind::KwPackage = t.kind {
                let path = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args(self_typ)?;

                Ok(TypRepr::new_def_typ(path, genargs))
            } else if let TkKind::KwSelfTyp = t.kind
                && let Some(self_typ) = self_typ
            {
                // Self型がある場合のみSelfは有効
                self.next();
                Ok(self_typ.clone())
            } else {
                Err(ParseError::InvalidToken {
                    expecteds: vec![
                        TkKindName::KwUint,
                        TkKindName::KwInt,
                        TkKindName::KwBool,
                        TkKindName::Ident,
                    ],
                    found: t.to_owned().clone(),
                })
            }
        } else {
            Err(ParseError::InvalidEOF {
                mod_id: self.mod_id,
                expecteds: vec![
                    TkKindName::KwUint,
                    TkKindName::KwInt,
                    TkKindName::KwBool,
                    TkKindName::Ident,
                    TkKindName::KwPackage,
                ],
            })
        }
    }

    /// consume generic argument declaration
    /// ```biwa
    /// struct Hoge[T, U] { ... }
    ///            ^^^^^^
    ///
    /// fn hoge[T, U](t: T, i: Int) -> U { ... }
    ///        ^^^^^^
    ///
    /// impl[T, U] Hoge[T, U] { ... }
    ///     ^^^^^^
    /// ```
    /// Generic argument declaration is declaration of generic type which appears for the first
    /// time in the scope, so it only contains <identifier>.
    /// It has diffinitly different meaning with generic argument assignment which makes generic
    /// type argument concrete type.
    ///
    /// ジェネリック型引数宣言は、そのスコープで始めて現れるジェネリック型の宣言であり、
    /// その引数列には<identifier>しか含まれない。
    /// ジェネリック型を具体化するときのジェネリック型引数の代入列とは別の意味合いである。
    pub(crate) fn opt_consume_generic_argument_declaration(
        &mut self,
    ) -> Result<Vec<Ident>, ParseError<'src>> {
        let mut genargs = vec![];
        if let Some(t) = self.peek()
            && matches!(t.kind, TkKind::MarkLBracket)
        {
            self.next();
        } else {
            return Ok(genargs);
        }

        loop {
            if let Some(t) = self.peek()
                && let TkKind::MarkRBracket = t.kind
            {
                self.next();

                return Ok(genargs);
            } else {
                genargs.push(self.consume_identifier()?);

                if let Some(t) = self.next() {
                    if let TkKind::MarkRBracket = t.kind {
                        return Ok(genargs);
                    } else if let TkKind::MarkComma = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkRBracket, TkKindName::MarkComma],
                            found: t.clone(),
                        });
                    }
                } else {
                    return Err(ParseError::InvalidEOF {
                        mod_id: self.mod_id,
                        expecteds: vec![TkKindName::MarkRBracket, TkKindName::MarkComma],
                    });
                }
            }
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
    ) -> Result<Option<Vec<TypRepr>>, ParseError<'src>> {
        if let Some(t) = self.peek()
            && matches!(t.kind, TkKind::MarkLBracket)
        {
            self.next();
        } else {
            return Ok(None);
        }

        let mut genargs = vec![];
        loop {
            if let Some(t) = self.peek()
                && let TkKind::MarkRBracket = t.kind
            {
                self.next();

                return Ok(Some(genargs));
            } else {
                genargs.push(self.consume_type_representaion(self_typ)?);

                if let Some(t) = self.next() {
                    if let TkKind::MarkRBracket = t.kind {
                        return Ok(Some(genargs));
                    } else if let TkKind::MarkComma = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkRBracket, TkKindName::MarkComma],
                            found: t.clone(),
                        });
                    }
                } else {
                    return Err(ParseError::InvalidEOF {
                        mod_id: self.mod_id,
                        expecteds: vec![TkKindName::MarkRBracket, TkKindName::MarkComma],
                    });
                }
            }
        }
    }
}
