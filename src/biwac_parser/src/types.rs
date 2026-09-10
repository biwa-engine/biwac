use std::cell::OnceCell;

use biwac_ast::{
    PrimTyp, TypRepr, TypReprVal,
    symbols::globals::{GenArgDeclItem, GenArgsDecl},
};
use biwac_lexer::{TkKind, TkKindName};
use biwac_span::Span;

use crate::{ParseError, TokenStream};

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    pub(crate) fn must_consume_type_annotation(&mut self) -> Result<TypRepr, ParseError<'src>> {
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

            Ok(self.consume_type_representaion()?)
        } else {
            Err(ParseError::InvalidToken {
                expecteds: vec![TkKindName::MarkColon],
                found: t.clone(),
            })
        }
    }

    pub(crate) fn opt_consume_type_annotation(
        &mut self,
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

            Ok(Some(self.consume_type_representaion()?))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_type_representaion(&mut self) -> Result<TypRepr, ParseError<'src>> {
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
                let genargs = self.opt_consume_generic_args()?;

                Ok(TypRepr::new_def_typ(path, genargs))
            } else if let TkKind::KwPackage = t.kind {
                let path = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args()?;

                Ok(TypRepr::new_def_typ(path, genargs))
            } else if let TkKind::KwSelfTyp = t.kind {
                let span = t.span.clone();
                self.next();
                Ok(TypRepr {
                    val: TypReprVal::SelfTyp,
                    span,
                })
            } else {
                Err(ParseError::InvalidToken {
                    expecteds: vec![
                        TkKindName::KwUint,
                        TkKindName::KwInt,
                        TkKindName::KwBool,
                        TkKindName::Ident,
                        TkKindName::KwPackage,
                        TkKindName::KwSelfTyp,
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
                    TkKindName::KwSelfTyp,
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
    // "[" ( <identifier> ( ":" <typ> ( "&&" <typ> )* )? ","? )* "]"
    //
    // 制限 (`T: Gyao && Conv[Int]`) は trait でなければならないが、
    // 書かれ方は型とまったく同じなので、ここでは型として読む。
    // trait であることの検査は名前解決が行う。
    pub(crate) fn opt_consume_generic_argument_declaration<I>(
        &mut self,
    ) -> Result<Option<GenArgsDecl<I>>, ParseError<'src>> {
        let mod_id = self.mod_id;
        let begin = if let Some(t) = self.peek()
            && matches!(t.kind, TkKind::MarkLBracket)
        {
            let span = t.span.clone();
            self.next();
            span
        } else {
            return Ok(None);
        };

        let mut genargs: Vec<GenArgDeclItem<I>> = vec![];

        let end = loop {
            let t = self.peek().ok_or(ParseError::InvalidEOF {
                mod_id,
                expecteds: vec![TkKindName::MarkRBracket],
            })?;
            if let TkKind::MarkRBracket = t.kind {
                let end = t.span.clone();
                self.next();
                break end;
            }

            let id = self.consume_identifier()?;

            let mut bounds = vec![];
            if self
                .consume_next_if_match(vec![TkKindName::MarkColon])
                .is_some()
            {
                loop {
                    bounds.push(self.consume_type_representaion()?);
                    if self
                        .consume_next_if_match(vec![TkKindName::MarkAndAnd])
                        .is_none()
                    {
                        break;
                    }
                }
            }

            genargs.push(GenArgDeclItem::<I> {
                id,
                def_id: OnceCell::<I>::new(),
                bounds,
            });

            let t =
                self.must_consume_next(vec![TkKindName::MarkComma, TkKindName::MarkRBracket])?;
            if let TkKind::MarkRBracket = t.kind {
                break t.span.clone();
            }
        };

        Ok(Some(GenArgsDecl {
            genargs,
            span: Span::merge(&begin, &end),
        }))
    }

    /// Optionaly consumes tokens and parses to get generic arguments.
    /// We should use here:
    /// let a: foo::bar[Int] = ...
    ///                ^
    ///                |
    // pub(crate) fn opt_consume_generic_argument_assignment(
    pub(crate) fn opt_consume_generic_args(
        &mut self,
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
                genargs.push(self.consume_type_representaion()?);

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
