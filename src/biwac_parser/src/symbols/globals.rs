use biwac_base::Span;
use biwac_lexer::token::{TkKind, TkVal};

use crate::{BlockStmt, Ident, ParseError, QualifiedId, TypRepr, VarDecl, parser::TokenStream};

#[derive(Debug, Clone)]
pub struct StructDef {
    pub id: Ident,
    pub members: Vec<(Ident, TypRepr)>,
}

#[derive(Debug)]
pub enum Globals {
    Import(ImportDecl),
    FnDef(FnDef),
    VarDecl(VarDecl),
    TypeDef(TypeDef),
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub qualid: QualifiedId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub id: Ident,
    pub args: Vec<ArgDecl>,
    pub body: BlockStmt,
    pub rtype: Option<TypRepr>, // None means void
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgDecl {
    pub typ: TypRepr,
    pub id: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TypeDef {
    Struct(StructDef),
    // Enum(EnumType),
    // Typedef(Box<Self>),
}

impl<'t> TokenStream<'t> {
    pub(super) fn opt_consume_global_symbol(&mut self) -> Result<Option<Globals>, ParseError> {
        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Import => {
                    // "import" <qualified-identifier> ";"
                    let begin = t.span.clone();
                    self.next();
                    let qualid = self.consume_qualified_identifier()?;

                    // ";"
                    let end = self.must_consume_semicolon()?.span.clone();

                    return Ok(Some(Globals::Import(ImportDecl {
                        qualid,
                        span: Span::merge(&begin, &end),
                    })));
                }
                TkKind::Fn => {
                    let begin = t.span.clone();
                    self.next();

                    let id = self.consume_identifier()?;
                    let args = self.consume_argsdec()?;

                    let rtype = if self.consume_next_if_match(vec![TkKind::Arrow]).is_some() {
                        Some(self.consume_type_representaion()?)
                    } else {
                        None
                    };

                    let body = self.consume_block_statement()?;
                    let end = body.span.clone();

                    return Ok(Some(Globals::FnDef(FnDef {
                        id,
                        args,
                        body,
                        rtype,
                        span: Span::merge(&begin, &end),
                    })));
                }
                TkKind::Let => {
                    return Ok(Some(Globals::VarDecl(
                        self.consume_variable_declaration_statment()?,
                    )));
                }
                TkKind::Struct => {
                    self.next();

                    let id = self.consume_identifier()?;

                    let _ = self.must_consume_next(vec![TkKind::LBrace])?;

                    let mut members = vec![];

                    loop {
                        let t = self
                            .peek()
                            .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident, TkKind::RBrace]))?;

                        if let TkKind::RBrace = t.kind {
                            self.next();

                            return Ok(Some(Globals::TypeDef(TypeDef::Struct(StructDef {
                                id,
                                members,
                            }))));
                        } else {
                            let t = self
                                .next()
                                .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident]))?;
                            if let TkKind::Ident = &t.kind
                                && let Some(TkVal::String(memberid)) = t.val.clone()
                            {
                                let t = t.clone();
                                let typ = self.must_consume_type_annotation()?;

                                members.push((
                                    Ident {
                                        id: memberid,
                                        span: t.span,
                                    },
                                    typ,
                                ));

                                let t =
                                    self.must_consume_next(vec![TkKind::Comma, TkKind::RBrace])?;
                                if let TkKind::Comma = t.kind {
                                    continue;
                                } else if let TkKind::RBrace = t.kind {
                                    return Ok(Some(Globals::TypeDef(TypeDef::Struct(
                                        StructDef { id, members },
                                    ))));
                                }
                            } else {
                                return Err(ParseError::InvalidToken(
                                    vec![TkKind::Ident],
                                    t.clone(),
                                ));
                            }
                        }
                    }
                }
                _ => {
                    return Err(ParseError::InvalidToken(
                        vec![TkKind::Fn, TkKind::Let, TkKind::Struct],
                        t.to_owned().clone(),
                    ));
                }
            }
        }

        Ok(None)
    }
    pub(crate) fn consume_argsdec(&mut self) -> Result<Vec<ArgDecl>, ParseError> {
        self.must_consume_next(vec![TkKind::LPare])?;

        let mut args = vec![];

        loop {
            let t = self
                .next()
                .ok_or(ParseError::InvalidEOF(vec![TkKind::RPare, TkKind::Ident]))?
                .clone();

            if let TkKind::RPare = t.kind {
                return Ok(args);
            } else if let TkKind::Ident = &t.kind
                && let Some(TkVal::String(arg)) = &t.val
            {
                let typ = self.must_consume_type_annotation()?;

                args.push(ArgDecl {
                    span: Span::merge(&t.span, &typ.span),
                    typ,
                    id: Ident {
                        id: arg.clone(),
                        span: t.span.clone(),
                    },
                });

                if let Some(t) = self.peek() {
                    if let TkKind::Comma = t.kind {
                        self.next();
                    } else if let TkKind::RPare = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken(
                            vec![TkKind::Comma, TkKind::RPare],
                            t.to_owned().clone(),
                        ));
                    }
                } else {
                    return Err(ParseError::InvalidEOF(vec![TkKind::Comma, TkKind::RPare]));
                }
            }
        }
    }
}
