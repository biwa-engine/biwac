use std::collections::{HashMap, hash_map::Entry};

use biwac_lexer::token::{TkKind, TkVal};

use crate::{
    ParseError, QualifiedId, Stmt, TypRepr, parser::TokenStream,
    symbols::statements::vardec::VarDec,
};

#[derive(Debug, Clone)]
pub struct StructDef {
    pub id: String,
    pub members: HashMap<String, (TypRepr, usize)>,
}

#[derive(Debug)]
pub enum Globals {
    Import(QualifiedId),
    FnDef(FnDef),
    VarDec(VarDec),
    TypeDef(TypeDef),
}

#[derive(Debug)]
pub struct FnDef {
    pub name: String,
    pub args: Vec<(TypRepr, String)>,
    pub stmts: Vec<Stmt>,
    pub rtype: Option<TypRepr>, // None means void
}

#[derive(Debug)]
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
                    self.next();
                    let qualed_id = self.consume_qualified_identifier()?;

                    return Ok(Some(Globals::Import(qualed_id)));
                }
                TkKind::Fn => {
                    self.next();

                    let id = self.consume_identifier()?;
                    let args = self.consume_argsdec()?;

                    let rtype = if self.consume_next_if_match(vec![TkKind::Arrow]).is_some() {
                        Some(self.consume_type_representaion()?)
                    } else {
                        None
                    };

                    return Ok(Some(Globals::FnDef(FnDef {
                        name: id,
                        args,
                        stmts: self.consume_block_statement()?,
                        rtype,
                    })));
                }
                TkKind::Let => {
                    return Ok(Some(Globals::VarDec(
                        self.consume_variable_declaration_statment()?,
                    )));
                }
                TkKind::Struct => {
                    self.next();

                    let id = self.consume_identifier()?;

                    let _ = self.must_consume_next(vec![TkKind::LBrace])?;

                    let mut members = HashMap::new();
                    let mut member_index = 0;

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

                                match members.entry(memberid.to_owned()) {
                                    Entry::Occupied(_) => {
                                        return Err(ParseError::StructMemberConflict(
                                            id,
                                            memberid.to_owned(),
                                            Box::new(t),
                                        ));
                                    }
                                    Entry::Vacant(e) => {
                                        e.insert((typ, member_index));
                                    }
                                }

                                let t =
                                    self.must_consume_next(vec![TkKind::Comma, TkKind::RBrace])?;
                                if let TkKind::Comma = t.kind {
                                    self.next();
                                    member_index += 1;
                                    continue;
                                } else if let TkKind::RBrace = t.kind {
                                    continue;
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
    pub(crate) fn consume_argsdec(&mut self) -> Result<Vec<(TypRepr, String)>, ParseError> {
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

                args.push((typ, arg.clone()));

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
