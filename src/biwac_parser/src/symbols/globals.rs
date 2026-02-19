use biwac_base::Span;
use biwac_lexer::token::{TkKind, TkVal};

use crate::{
    CompilerFlag, Exprs, Ident, ParseError, QualifiedId, Stmt, TypRepr, VarDecl,
    parser::TokenStream, symbols::ExprOrStmt,
};

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
    NativeFnDef(NativeFnDef),
    MethodDef(MethodDef),
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub qualid: QualifiedId,
    pub span: Span,
}

// 型の関連関数の場合はtypがSome
// Selfは具体のTypReprによりパース時に解決される
#[derive(Debug, Clone)]
pub struct FnDef {
    pub self_typ: Option<TypRepr>,
    pub id: Ident,
    pub args: Vec<ArgDecl>,
    pub stmts: Vec<Stmt>,
    pub expr: Option<Exprs>,
    pub rtype: Option<TypRepr>, // None means void
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
}

#[derive(Debug, Clone)]
pub struct NativeFnDef {
    pub id: Ident,
    pub args: Vec<ArgDecl>,
    pub rtype: Option<TypRepr>, // None means void
    pub native: String,
    pub native_span: Span,
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgDecl {
    pub typ: TypRepr,
    pub id: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodDef {
    pub self_typ: TypRepr,
    pub self_ident: Ident,
    pub id: Ident,
    pub args: Vec<ArgDecl>, // 第一引数がselfであるのは自明なので含まない
    pub stmts: Vec<Stmt>,
    pub expr: Option<Exprs>,
    pub rtype: Option<TypRepr>, // None means void
    pub span: Span,
    pub flags: Vec<CompilerFlag>,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub typ_fns: Vec<FnDef>,
    pub methods: Vec<MethodDef>,
}

#[derive(Debug, Clone)]
pub enum TypeDef {
    Struct(StructDef),
    // Enum(EnumType),
    // Typedef(Box<Self>),
}

// FnParseCtx
// 関数内のパースをするときのコンテキスト
#[derive(Debug, Clone)]
pub(crate) struct FnParseCtx {
    // implブロック内の関連関数やメソッドのとき、Self型が何の型か
    // パース時点で型を決定してしまう
    // Noneのとき、Selfキーワードは出現するべきでないため、エラー
    pub(crate) self_typ: Option<TypRepr>,
    // is_method trueならselfキーワードを使える
    pub(crate) is_method: bool,
}

impl<'t> TokenStream<'t> {
    fn consume_function_or_method_definition(
        &mut self,
        flags: Vec<CompilerFlag>,
        self_typ: Option<TypRepr>,
    ) -> Result<Globals, ParseError> {
        let begin = self.must_consume_next(vec![TkKind::Fn])?.span.clone();
        self.next();

        let id = self.consume_identifier()?;

        // 実装対象の型typがSomeならメソッドである可能性がある
        let (args, self_ident) = if self_typ.is_some() {
            self.consume_method_argsdec(&self_typ)?
        } else {
            (self.consume_argsdec(&self_typ)?, None)
        };

        let rtype = if self.consume_next_if_match(vec![TkKind::Arrow]).is_some() {
            Some(self.consume_type_representaion(&self_typ)?)
        } else {
            None
        };

        if flags.iter().any(|f| &f.flag.id == "native") {
            if let Some(t) = self.next() {
                if TkKind::DslLiteral == t.kind {
                    Ok(Globals::NativeFnDef(NativeFnDef {
                        id,
                        args,
                        native: t.unwrap_string_value(),
                        rtype,
                        span: Span::merge(&begin, &t.span),
                        native_span: t.span.clone(),
                        flags,
                    }))
                } else {
                    Err(ParseError::InvalidToken(
                        vec![TkKind::DslLiteral],
                        t.clone(),
                    ))
                }
            } else {
                Err(ParseError::InvalidEOF(vec![TkKind::DslLiteral]))
            }
        } else {
            // Self型を表すtypをコンテキストとして渡してパースする
            let ctx = FnParseCtx {
                self_typ: self_typ.clone(),
                is_method: self_ident.is_some(),
            };
            let (stmts, expr, end) = match self.consume_block_expression_or_statement(&ctx)? {
                ExprOrStmt::Expr(block_expr) => {
                    (block_expr.stmts, Some(*block_expr.expr), block_expr.span)
                }
                ExprOrStmt::Stmt(block_stmt) => (block_stmt.stmts, None, block_stmt.span),
            };

            if let Some(self_ident) = self_ident {
                Ok(Globals::MethodDef(MethodDef {
                    // SAFETY: self_ident Someになるのはself_typ Someのときだけ
                    self_typ: self_typ.expect("compiler bug: not a method"),
                    self_ident,
                    id,
                    args,
                    stmts,
                    expr,
                    rtype,
                    span: Span::merge(&begin, &end),
                    flags,
                }))
            } else {
                Ok(Globals::FnDef(FnDef {
                    self_typ,
                    id,
                    args,
                    stmts,
                    expr,
                    rtype,
                    span: Span::merge(&begin, &end),
                    flags,
                }))
            }
        }
    }

    pub(super) fn opt_consume_global_symbols(&mut self) -> Result<Vec<Globals>, ParseError> {
        let flags = self.consume_compiler_flags()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::Import => {
                    // "import" <qualified-identifier> ";"
                    let begin = t.span.clone();
                    self.next();
                    let qualid = self.consume_qualified_identifier()?;

                    // ";"
                    let end = self.must_consume_semicolon()?.span.clone();

                    Ok(vec![Globals::Import(ImportDecl {
                        qualid,
                        span: Span::merge(&begin, &end),
                    })])
                }
                TkKind::Fn => Ok(vec![
                    self.consume_function_or_method_definition(flags, None)?,
                ]),
                TkKind::Let => Ok(vec![Globals::VarDecl(
                    // グローバル変数のパースには当然関数内の文脈を与えない
                    // constキーワードのみのほうが良いかも
                    self.consume_variable_declaration_statment(None)?,
                )]),
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

                            return Ok(vec![Globals::TypeDef(TypeDef::Struct(StructDef {
                                id,
                                members,
                            }))]);
                        } else {
                            let t = self
                                .next()
                                .ok_or(ParseError::InvalidEOF(vec![TkKind::Ident]))?;
                            if let TkKind::Ident = &t.kind
                                && let Some(TkVal::String(memberid)) = t.val.clone()
                            {
                                let t = t.clone();
                                // WARN: really?
                                let typ = self.must_consume_type_annotation(&None)?;

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
                                    return Ok(vec![Globals::TypeDef(TypeDef::Struct(
                                        StructDef { id, members },
                                    ))]);
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
                TkKind::Impl => {
                    // "impl" <type-representation> "{" ... "}"
                    self.next();

                    let self_typ = self.consume_type_representaion(&None)?;

                    self.must_consume_next(vec![TkKind::LBrace])?;

                    let mut type_impls = vec![];

                    loop {
                        if let Some(t) = self.peek().copied()
                            && TkKind::RBrace == t.kind
                        {
                            self.next();

                            return Ok(type_impls);
                        } else {
                            let flags = self.consume_compiler_flags()?;
                            let f = self.consume_function_or_method_definition(
                                flags,
                                Some(self_typ.clone()),
                            )?;

                            type_impls.push(f);
                        }
                    }
                }
                _ => Err(ParseError::InvalidToken(
                    vec![TkKind::Fn, TkKind::Let, TkKind::Struct],
                    t.to_owned().clone(),
                )),
            }
        } else {
            Ok(vec![])
        }
    }

    pub(crate) fn consume_argsdec(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<Vec<ArgDecl>, ParseError> {
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
                let typ = self.must_consume_type_annotation(self_typ)?;

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

    pub(crate) fn consume_method_argsdec(
        &mut self,
        self_typ: &Option<TypRepr>,
    ) -> Result<(Vec<ArgDecl>, Option<Ident>), ParseError> {
        // (args, self_ident)
        self.must_consume_next(vec![TkKind::LPare])?;

        let mut args = vec![];

        // first arg `self` or not
        let t = self.peek().ok_or(ParseError::InvalidEOF(vec![
            TkKind::RPare,
            TkKind::Ident,
            TkKind::SelfVar,
        ]))?;

        let self_ident = match t.kind {
            TkKind::RPare => {
                self.next();

                return Ok((args, None));
            }
            TkKind::SelfVar => {
                let span = t.span.clone();
                self.next();

                Some(Ident {
                    id: "self".to_string(),
                    span,
                })
            }
            _ => None,
        };

        loop {
            let t = self
                .next()
                .ok_or(ParseError::InvalidEOF(vec![TkKind::RPare, TkKind::Ident]))?
                .clone();

            if let TkKind::RPare = t.kind {
                return Ok((args, self_ident));
            } else if let TkKind::Ident = &t.kind
                && let Some(TkVal::String(arg)) = &t.val
            {
                let typ = self.must_consume_type_annotation(self_typ)?;

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
