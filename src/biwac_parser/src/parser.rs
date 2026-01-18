use biwac_lexer::token::{TkKind, TkVal, Token};

use crate::{DefTyp, ParseError, PrimTyp, TypRepr, symbols::QualifiedId};

pub(crate) struct TokenStream<'t> {
    tokens: std::iter::Peekable<std::slice::Iter<'t, Token>>,
}

impl<'t> TokenStream<'t> {
    pub(crate) fn new(tokens: std::iter::Peekable<std::slice::Iter<'t, Token>>) -> Self {
        Self { tokens }
    }

    pub(crate) fn next(&mut self) -> Option<&Token> {
        self.tokens.next()
    }

    pub(crate) fn peek(&mut self) -> Option<&&'t Token> {
        self.tokens.peek()
    }

    pub(crate) fn consume_next_if_match(&mut self, kinds: Vec<TkKind>) -> Option<&Token> {
        let t: &Token = self.peek()?;

        for kind in &kinds {
            if kind == &t.kind {
                self.next();
                return Some(t);
            }
        }

        None
    }

    pub(crate) fn must_consume_next(&mut self, kinds: Vec<TkKind>) -> Result<&Token, ParseError> {
        let t: &Token = self.next().ok_or(ParseError::InvalidEOF(kinds.clone()))?;

        for kind in &kinds {
            if kind == &t.kind {
                return Ok(t);
            }
        }

        Err(ParseError::InvalidToken(kinds, t.clone()))
    }

    pub(crate) fn must_consume_type_annotation(&mut self) -> Result<TypRepr, ParseError> {
        let t = self
            .peek()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::Colon]))?
            .to_owned();

        if let TkKind::Colon = t.kind {
            self.next();

            Ok(self.consume_type_representaion()?)
        } else {
            Err(ParseError::InvalidToken(vec![TkKind::Colon], t.clone()))
        }
    }

    pub(crate) fn opt_consume_type_annotation(&mut self) -> Result<Option<TypRepr>, ParseError> {
        let t = self
            .peek()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::Colon]))?
            .to_owned();

        if let TkKind::Colon = t.kind {
            self.next();

            Ok(Some(self.consume_type_representaion()?))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_type_representaion(&mut self) -> Result<TypRepr, ParseError> {
        if let Some(t) = self.peek() {
            if let TkKind::Uint = t.kind {
                self.next();
                Ok(TypRepr::Primitive(PrimTyp::Uint))
            } else if let TkKind::Int = t.kind {
                self.next();
                Ok(TypRepr::Primitive(PrimTyp::Int))
            } else if let TkKind::Bool = t.kind {
                self.next();
                Ok(TypRepr::Primitive(PrimTyp::Bool))
            } else if let TkKind::Ident = t.kind {
                // NOTE: idのみ得られた場合、ジェネリクス型(`T`)である可能性がある
                let qualid = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args()?;

                Ok(TypRepr::Defined(DefTyp { qualid, genargs }))
            } else if let TkKind::Package = t.kind {
                let qualid = self.consume_qualified_identifier()?;
                let genargs = self.opt_consume_generic_args()?;

                Ok(TypRepr::Defined(DefTyp { qualid, genargs }))
            } else {
                Err(ParseError::InvalidToken(
                    vec![TkKind::Uint, TkKind::Int, TkKind::Bool, TkKind::Ident],
                    t.clone().clone(),
                ))
            }
        } else {
            Err(ParseError::InvalidEOF(vec![
                TkKind::Uint,
                TkKind::Int,
                TkKind::Bool,
                TkKind::Ident,
                TkKind::Package,
            ]))
        }
    }

    /// Optionaly consumes tokens and parses to get generic arguments.
    /// We should use here:
    /// let a: foo::bar[Int] = ...
    ///                ^
    ///                |
    pub(crate) fn opt_consume_generic_args(&mut self) -> Result<Vec<TypRepr>, ParseError> {
        let mut genargs = vec![];
        if let Some(t) = self.peek()
            && matches!(t.kind, TkKind::LBracket)
        {
            self.next();
        } else {
            return Ok(genargs);
        }

        loop {
            if let Some(t) = self.peek()
                && let TkKind::RBracket = t.kind
            {
                self.next();

                return Ok(genargs);
            } else {
                genargs.push(self.consume_type_representaion()?);

                if let Some(t) = self.next() {
                    if let TkKind::RBracket = t.kind {
                        return Ok(genargs);
                    } else if let TkKind::Comma = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken(
                            vec![TkKind::RBracket, TkKind::Comma],
                            t.clone(),
                        ));
                    }
                } else {
                    return Err(ParseError::InvalidEOF(vec![
                        TkKind::RBracket,
                        TkKind::Comma,
                    ]));
                }
            }
        }
    }

    pub(crate) fn consume_identifier(&mut self) -> Result<String, ParseError> {
        let t = self
            .next()
            .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident]))?;

        if let TkKind::Ident = &t.kind {
            Ok(t.unwrap_string_value())
        } else {
            Err(ParseError::InvalidToken(vec![TkKind::Ident], t.clone()))
        }
    }

    pub(crate) fn consume_qualified_identifier(&mut self) -> Result<QualifiedId, ParseError> {
        let mut ids = vec![];
        let is_from_root = if let Some(t) = self.peek() {
            matches!(t.kind, TkKind::Package)
        } else {
            false
        };

        ids.push(self.consume_identifier()?);

        loop {
            if let Some(t) = self.peek() {
                if let TkKind::DoubleColon = t.kind {
                    self.next();
                    ids.push(self.consume_identifier()?);
                } else {
                    return Ok(QualifiedId {
                        is_from_root,
                        quals: ids[..ids.len() - 1].to_vec(),
                        id: ids.last().expect("no identifier parsed").clone(),
                    });
                }
            } else {
                return Ok(QualifiedId {
                    is_from_root,
                    quals: ids[..ids.len() - 1].to_vec(),
                    id: ids.last().expect("no identifier parsed").clone(),
                });
            }
        }
    }
}

// pub fn must_consume_next<'t>(
//     tokens: &'t mut TokenStream<'t>,
//     kinds: Vec<TkKind>,
// ) -> Result<&'t Token, ParseError> {
//     let t: &Token = tokens.next().ok_or(ParseError::InvalidEOF(kinds.clone()))?;
//
//     for kind in &kinds {
//         if kind == &t.kind {
//             return Ok(t);
//         }
//     }
//
//     Err(ParseError::InvalidToken(kinds, t.clone()))
// }

// pub fn matches(opt_t: Option<&Token>, kinds: Vec<TkKind>) -> Result<TkKind, ParseError> {
//     let t: &Token = opt_t.ok_or(ParseError::InvalidEOF(kinds.clone()))?;
//
//     for kind in &kinds {
//         if std::mem::discriminant(&t.kind) == std::mem::discriminant(kind) {
//             return Ok(t.kind.clone());
//         }
//     }
//
//     Err(ParseError::InvalidToken(kinds, t.clone()))
// }

// pub(crate) fn must_consume_type_annotation<'t>(
//     tokens: &mut TokenStream<'t>,
// ) -> Result<TypRepr, ParseError> {
//     let t = tokens
//         .peek()
//         .ok_or(ParseError::InvalidEOF(vec![TkKind::Colon]))?
//         .to_owned();
//
//     if let TkKind::Colon = t.kind {
//         tokens.next();
//
//         Ok(consume_type_representaion(tokens)?)
//     } else {
//         Err(ParseError::InvalidToken(vec![TkKind::Colon], t.clone()))
//     }
// }
//
// pub(crate) fn opt_consume_type_annotation<'t>(
//     tokens: &mut TokenStream<'t>,
// ) -> Result<Option<TypRepr>, ParseError> {
//     let t = tokens
//         .peek()
//         .ok_or(ParseError::InvalidEOF(vec![TkKind::Colon]))?
//         .to_owned();
//
//     if let TkKind::Colon = t.kind {
//         tokens.next();
//
//         Ok(Some(consume_type_representaion(tokens)?))
//     } else {
//         Ok(None)
//     }
// }
//
// fn consume_type_representaion<'t>(tokens: &mut TokenStream<'t>) -> Result<TypRepr, ParseError> {
//     if let Some(t) = tokens.peek() {
//         if let TkKind::Uint = t.kind {
//             tokens.next();
//             Ok(TypRepr::Primitive(PrimTyp::Uint))
//         } else if let TkKind::Int = t.kind {
//             tokens.next();
//             Ok(TypRepr::Primitive(PrimTyp::Int))
//         } else if let TkKind::Bool = t.kind {
//             tokens.next();
//             Ok(TypRepr::Primitive(PrimTyp::Bool))
//         } else if let TkKind::Ident = t.kind {
//             // NOTE: idのみ得られた場合、ジェネリクス型(`T`)である可能性がある
//             let qualid = consume_qualified_identifier(tokens)?;
//             let genargs = opt_consume_generic_args(tokens)?;
//
//             Ok(TypRepr::Defined(DefTyp { qualid, genargs }))
//         } else if let TkKind::Package = t.kind {
//             let qualid = consume_qualified_identifier(tokens)?;
//             let genargs = opt_consume_generic_args(tokens)?;
//
//             Ok(TypRepr::Defined(DefTyp { qualid, genargs }))
//         } else {
//             Err(ParseError::InvalidToken(
//                 vec![TkKind::Uint, TkKind::Int, TkKind::Bool, TkKind::Ident],
//                 t.clone().clone(),
//             ))
//         }
//     } else {
//         Err(ParseError::InvalidEOF(vec![
//             TkKind::Uint,
//             TkKind::Int,
//             TkKind::Bool,
//             TkKind::Ident,
//             TkKind::Package,
//         ]))
//     }
// }
//
// /// Optionaly consumes tokens and parses to get generic arguments.
// /// We should use here:
// /// let a: foo::bar[Int] = ...
// ///                ^
// ///                |
// fn opt_consume_generic_args<'t>(tokens: &mut TokenStream<'t>) -> Result<Vec<TypRepr>, ParseError> {
//     let mut genargs = vec![];
//     if let Some(t) = tokens.peek()
//         && matches!(t.kind, TkKind::LBracket)
//     {
//         tokens.next();
//     } else {
//         return Ok(genargs);
//     }
//
//     loop {
//         if let Some(t) = tokens.peek()
//             && let TkKind::RBracket = t.kind
//         {
//             tokens.next();
//
//             return Ok(genargs);
//         } else {
//             genargs.push(consume_type_representaion(tokens)?);
//
//             if let Some(t) = tokens.next() {
//                 if let TkKind::RBracket = t.kind {
//                     return Ok(genargs);
//                 } else if let TkKind::Comma = t.kind {
//                     continue;
//                 } else {
//                     return Err(ParseError::InvalidToken(
//                         vec![TkKind::RBracket, TkKind::Comma],
//                         t.clone(),
//                     ));
//                 }
//             } else {
//                 return Err(ParseError::InvalidEOF(vec![
//                     TkKind::RBracket,
//                     TkKind::Comma,
//                 ]));
//             }
//         }
//     }
// }

// pub(crate) fn consume_type_annotation<'t>(
//     tokens: &mut TokenStream<'t>,
// ) -> Result<Option<Type>, ParseError> {
//     let t = tokens
//         .peek()
//         .ok_or(ParseError::InvalidEOF(vec![TkKind::Colon]))?
//         .to_owned();
//
//     if let TkKind::Colon = t.kind {
//         tokens.next();
//
//         let kind = matches(
//             tokens.peek().copied(),
//             vec![
//                 TkKind::Uint,
//                 TkKind::Int,
//                 TkKind::Bool,
//                 TkKind::Ident,
//                 TkKind::Package,
//             ],
//         )?;
//
//         if let TkKind::Uint = kind {
//             tokens.next();
//             Ok(Some(Type::Primitive(PrimitiveType::Uint)))
//         } else if let TkKind::Int = kind {
//             tokens.next();
//             Ok(Some(Type::Primitive(PrimitiveType::Int)))
//         } else if let TkKind::Bool = kind {
//             tokens.next();
//             Ok(Some(Type::Primitive(PrimitiveType::Bool)))
//         } else if let TkKind::Ident = kind {
//             let qualid = consume_qualified_identifier(tokens)?;
//             Ok(Some(Type::Defined(qualid)))
//         } else if let TkKind::Package = kind {
//             let qualid = consume_qualified_identifier(tokens)?;
//             Ok(Some(Type::Defined(qualid)))
//         } else {
//             Err(ParseError::InvalidToken(
//                 vec![TkKind::Uint, TkKind::Int, TkKind::Bool, TkKind::Ident],
//                 t.clone().clone(),
//             ))
//         }
//     } else {
//         Ok(None)
//     }
// }
//
// pub(crate) fn consume_type_annotation_which_must_annotate<'t>(
//     tokens: &mut TokenStream<'t>,
// ) -> Result<Type, ParseError> {
//     matches(tokens.next(), vec![TkKind::Colon])?;
//
//     let kind = matches(
//         tokens.peek().copied(),
//         vec![
//             TkKind::Uint,
//             TkKind::Int,
//             TkKind::Bool,
//             TkKind::Ident,
//             TkKind::Package,
//         ],
//     )?;
//
//     if let TkKind::Uint = kind {
//         tokens.next();
//         Ok(Type::Primitive(PrimitiveType::Uint))
//     } else if let TkKind::Int = kind {
//         tokens.next();
//         Ok(Type::Primitive(PrimitiveType::Int))
//     } else if let TkKind::Bool = kind {
//         tokens.next();
//         Ok(Type::Primitive(PrimitiveType::Bool))
//     } else if let TkKind::Ident = kind {
//         let qualed_id = consume_qualified_identifier(tokens)?;
//         Ok(Type::Defined(qualed_id))
//     } else if let TkKind::Package = kind {
//         let qualed_id = consume_qualified_identifier(tokens)?;
//         Ok(Type::Defined(qualed_id))
//     } else {
//         panic!("unreachable");
//     }
// }
//
// pub(crate) fn consume_identifier<'t>(tokens: &mut TokenStream<'t>) -> Result<String, ParseError> {
//     let t = tokens
//         .next()
//         .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident]))?;
//
//     if let TkKind::Ident = &t.kind
//         && let Some(TkVal::String(id)) = &t.val
//     {
//         Ok(id.clone())
//     } else {
//         Err(ParseError::InvalidToken(vec![TkKind::Ident], t.clone()))
//     }
// }
//
// pub(crate) fn consume_qualified_identifier<'t>(
//     tokens: &mut TokenStream<'t>,
// ) -> Result<QualifiedId, ParseError> {
//     let mut ids = vec![];
//     let is_from_root = if let Some(t) = tokens.peek() {
//         matches!(t.kind, TkKind::Package)
//     } else {
//         false
//     };
//
//     ids.push(consume_identifier(tokens)?);
//
//     loop {
//         if let Some(t) = tokens.peek() {
//             if let TkKind::DoubleColon = t.kind {
//                 tokens.next();
//                 ids.push(consume_identifier(tokens)?);
//             } else {
//                 return Ok(QualifiedId {
//                     is_from_root,
//                     quals: ids[..ids.len() - 1].to_vec(),
//                     id: ids.last().expect("no identifier parsed").clone(),
//                 });
//             }
//         } else {
//             return Ok(QualifiedId {
//                 is_from_root,
//                 quals: ids[..ids.len() - 1].to_vec(),
//                 id: ids.last().expect("no identifier parsed").clone(),
//             });
//         }
//     }
// }
