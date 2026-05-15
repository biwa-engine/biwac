use std::cell::OnceCell;

use biwac_lexer::{TkKindName, token::TkKind};
use biwac_span::Span;

use biwac_ast::{
    ArgDecl, ArgDeclList, CompilerFlag, FnDef, Globals, Ident, ImplBlock, ImportDecl,
    MethodArgDeclList, MethodDef, NativeCode, NativeFnDef, NativeMethodDef, NativeTypeAlias,
    NovelScene, RetTypRepr, StructDef, TypeAlias, TypeDef,
};

use crate::{ExprOrStmt, ParseError, TokenStream};

enum FnOrMethod<F, M> {
    Fn(F),
    Method(M),
}

enum CodeOrNative<C, N> {
    Code(C),
    Native(N),
}

impl<'t, 'src> TokenStream<'t, 'src> {
    fn consume_return_type(
        &mut self,
        arg_decl_span: &Span,
    ) -> Result<RetTypRepr, ParseError<'src>> {
        if self
            .consume_next_if_match(vec![TkKindName::MarkArrow])
            .is_some()
        {
            Ok(RetTypRepr::Typ(self.consume_type_representaion()?))
        } else {
            Ok(RetTypRepr::Void(Span::new(
                arg_decl_span.module(),
                arg_decl_span.end(),
                arg_decl_span.end(),
            )))
        }
    }

    fn consume_function(
        &mut self,
        flags: Vec<CompilerFlag>,
    ) -> Result<CodeOrNative<FnDef, NativeFnDef>, ParseError<'src>> {
        let begin = self.must_consume_next(vec![TkKindName::KwFn])?.span.clone();

        let id = self.consume_identifier()?;

        let genargs = self.opt_consume_generic_argument_declaration()?;

        let interned_str_native = self.interner.get_or_insert("native");

        let args = self.consume_argsdec()?;
        let rtype = self.consume_return_type(&args.span)?;
        if flags.iter().any(|f| f.flag.id == interned_str_native) {
            if let Some(t) = self.next() {
                if let TkKind::DslLiteral(str) = t.kind {
                    Ok(CodeOrNative::Native(NativeFnDef {
                        id,
                        def_id: OnceCell::new(),
                        args,
                        native: str.to_string(),
                        rtype,
                        span: Span::merge(&begin, &t.span),
                        native_span: t.span.clone(),
                        flags,
                        genargs,
                    }))
                } else {
                    Err(ParseError::InvalidToken {
                        expecteds: vec![TkKindName::DslLiteral],
                        found: t.clone(),
                    })
                }
            } else {
                Err(ParseError::InvalidEOF {
                    mod_id: self.mod_id,
                    expecteds: vec![TkKindName::DslLiteral],
                })
            }
        } else {
            let (stmts, expr, end) = match self.consume_block_expression_or_statement()? {
                ExprOrStmt::Expr(block_expr) => {
                    (block_expr.stmts, Some(*block_expr.expr), block_expr.span)
                }
                ExprOrStmt::Stmt(block_stmt) => (block_stmt.stmts, None, block_stmt.span),
            };

            Ok(CodeOrNative::Code(FnDef {
                id,
                def_id: OnceCell::new(),
                args,
                stmts,
                expr,
                rtype,
                span: Span::merge(&begin, &end),
                flags,
                genargs,
            }))
        }
    }

    fn consume_function_or_method_definition(
        &mut self,
        flags: Vec<CompilerFlag>,
    ) -> Result<
        FnOrMethod<CodeOrNative<FnDef, NativeFnDef>, CodeOrNative<MethodDef, NativeMethodDef>>,
        ParseError<'src>,
    > {
        let begin = self.must_consume_next(vec![TkKindName::KwFn])?.span.clone();

        let id = self.consume_identifier()?;

        let genargs = self.opt_consume_generic_argument_declaration()?;

        let interned_str_native = self.interner.get_or_insert("native");

        match self.consume_method_argsdec()? {
            FnOrMethod::Fn(args) => {
                let rtype = self.consume_return_type(&args.span)?;
                if flags.iter().any(|f| f.flag.id == interned_str_native) {
                    if let Some(t) = self.next() {
                        if let TkKind::DslLiteral(str) = t.kind {
                            Ok(FnOrMethod::Fn(CodeOrNative::Native(NativeFnDef {
                                id,
                                def_id: OnceCell::new(),
                                args,
                                native: str.to_string(),
                                rtype,
                                span: Span::merge(&begin, &t.span),
                                native_span: t.span.clone(),
                                flags,
                                genargs,
                            })))
                        } else {
                            Err(ParseError::InvalidToken {
                                expecteds: vec![TkKindName::DslLiteral],
                                found: t.clone(),
                            })
                        }
                    } else {
                        Err(ParseError::InvalidEOF {
                            mod_id: self.mod_id,
                            expecteds: vec![TkKindName::DslLiteral],
                        })
                    }
                } else {
                    let (stmts, expr, end) = match self.consume_block_expression_or_statement()? {
                        ExprOrStmt::Expr(block_expr) => {
                            (block_expr.stmts, Some(*block_expr.expr), block_expr.span)
                        }
                        ExprOrStmt::Stmt(block_stmt) => (block_stmt.stmts, None, block_stmt.span),
                    };

                    Ok(FnOrMethod::Fn(CodeOrNative::Code(FnDef {
                        id,
                        def_id: OnceCell::new(),
                        args,
                        stmts,
                        expr,
                        rtype,
                        span: Span::merge(&begin, &end),
                        flags,
                        genargs,
                    })))
                }
            }
            FnOrMethod::Method(args) => {
                let rtype = self.consume_return_type(&args.span)?;
                if flags.iter().any(|f| f.flag.id == interned_str_native) {
                    if let Some(t) = self.next() {
                        if let TkKind::DslLiteral(str) = t.kind {
                            Ok(FnOrMethod::Method(CodeOrNative::Native(NativeMethodDef {
                                id,
                                def_id: OnceCell::new(),
                                args,
                                rtype,
                                native: str.to_string(),
                                native_span: t.span.clone(),
                                span: Span::merge(&begin, &t.span),
                                flags,
                                genargs,
                            })))
                        } else {
                            Err(ParseError::InvalidToken {
                                expecteds: vec![TkKindName::DslLiteral],
                                found: t.clone(),
                            })
                        }
                    } else {
                        Err(ParseError::InvalidEOF {
                            mod_id: self.mod_id,
                            expecteds: vec![TkKindName::DslLiteral],
                        })
                    }
                } else {
                    let (stmts, expr, end) = match self.consume_block_expression_or_statement()? {
                        ExprOrStmt::Expr(block_expr) => {
                            (block_expr.stmts, Some(*block_expr.expr), block_expr.span)
                        }
                        ExprOrStmt::Stmt(block_stmt) => (block_stmt.stmts, None, block_stmt.span),
                    };
                    Ok(FnOrMethod::Method(CodeOrNative::Code(MethodDef {
                        id,
                        def_id: OnceCell::new(),
                        args,
                        stmts,
                        expr,
                        rtype,
                        span: Span::merge(&begin, &end),
                        flags,
                        genargs,
                    })))
                }
            }
        }
    }

    pub(super) fn opt_consume_global_symbols(
        &mut self,
    ) -> Result<Option<Globals>, ParseError<'src>> {
        let mod_id = self.mod_id;
        let flags = self.consume_compiler_flags()?;

        if let Some(t) = self.peek() {
            match t.kind {
                TkKind::KwImport => {
                    // "import" <qualified-identifier> ";"
                    let begin = t.span.clone();
                    self.next();
                    let path = self.consume_qualified_identifier()?;

                    // ";"
                    let end = self.must_consume_semicolon()?.span.clone();

                    Ok(Some(Globals::Import(ImportDecl {
                        path,
                        span: Span::merge(&begin, &end),
                    })))
                }
                TkKind::KwFn => Ok(match self.consume_function(flags)? {
                    CodeOrNative::Code(f) => Some(Globals::FnDef(f)),
                    CodeOrNative::Native(f) => Some(Globals::NativeFnDef(f)),
                }),
                TkKind::KwStruct => {
                    self.next();

                    let id = self.consume_identifier()?;

                    let genargs = self.opt_consume_generic_argument_declaration()?;

                    let _ = self.must_consume_next(vec![TkKindName::MarkLBrace])?;

                    let mut members = vec![];

                    loop {
                        let t = self.peek().ok_or(ParseError::InvalidEOF {
                            mod_id,
                            expecteds: vec![TkKindName::Ident, TkKindName::MarkRBrace],
                        })?;

                        if let TkKind::MarkRBrace = t.kind {
                            self.next();

                            return Ok(Some(Globals::TypeDef(TypeDef::Struct(StructDef {
                                id,
                                def_id: OnceCell::new(),
                                members,
                                genargs,
                            }))));
                        } else {
                            let t = self
                                .next()
                                .ok_or(ParseError::InvalidEOF {
                                    mod_id,
                                    expecteds: vec![TkKindName::Ident],
                                })?
                                .to_owned();
                            if let TkKind::Ident(member_id) = &t.kind {
                                let t = t.clone();
                                // WARN: really?
                                let typ = self.must_consume_type_annotation()?;

                                members.push((
                                    Ident {
                                        id: *member_id,
                                        span: t.span,
                                    },
                                    typ,
                                ));

                                let t = self.must_consume_next(vec![
                                    TkKindName::MarkComma,
                                    TkKindName::MarkRBrace,
                                ])?;
                                if let TkKind::MarkComma = t.kind {
                                    continue;
                                } else if let TkKind::MarkRBrace = t.kind {
                                    return Ok(Some(Globals::TypeDef(TypeDef::Struct(
                                        StructDef {
                                            id,
                                            def_id: OnceCell::new(),
                                            members,
                                            genargs,
                                        },
                                    ))));
                                }
                            } else {
                                return Err(ParseError::InvalidToken {
                                    expecteds: vec![TkKindName::Ident],
                                    found: t.clone(),
                                });
                            }
                        }
                    }
                }
                TkKind::KwType => {
                    let interned_str_native = self.interner.get_or_insert("native");
                    if flags.iter().any(|f| f.flag.id == interned_str_native) {
                        // "type" <identifier> ( <generic-argument-declaration> )?
                        //     "=" {{
                        //         native type implementation
                        //     }} ";"

                        self.next();

                        let ident = self.consume_identifier()?;

                        let genargs = self.opt_consume_generic_argument_declaration()?;

                        let _ = self.must_consume_next(vec![TkKindName::MarkAssign])?;

                        let t = self.next().ok_or(ParseError::InvalidEOF {
                            mod_id,
                            expecteds: vec![TkKindName::DslLiteral],
                        })?;
                        if let TkKind::DslLiteral(str) = t.kind {
                            let native = str.to_string();
                            let native_span = t.span.clone();

                            let _ = self.must_consume_next(vec![TkKindName::MarkSemiColon])?;

                            Ok(Some(Globals::TypeDef(TypeDef::NativeTypeAlias(
                                NativeTypeAlias {
                                    ident,
                                    def_id: OnceCell::new(),
                                    genargs,
                                    native,
                                    native_span,
                                },
                            ))))
                        } else {
                            Err(ParseError::InvalidEOF {
                                mod_id,
                                expecteds: vec![TkKindName::DslLiteral],
                            })
                        }
                    } else {
                        // "type" <identifier> ( <generic-argument-declaration> )? "=" <type-representation> ";"
                        self.next();

                        let ident = self.consume_identifier()?;

                        let genargs = self.opt_consume_generic_argument_declaration()?;

                        let _ = self.must_consume_next(vec![TkKindName::MarkAssign])?;

                        let right = self.consume_type_representaion()?;

                        let _ = self.must_consume_next(vec![TkKindName::MarkSemiColon])?;

                        Ok(Some(Globals::TypeDef(TypeDef::TypeAlias(TypeAlias {
                            ident,
                            def_id: OnceCell::new(),
                            genargs,
                            right,
                        }))))
                    }
                }
                TkKind::KwImpl => {
                    // "impl" ( <generic-argument-declaration> )? <type-representation> "{" ... "}"
                    self.next();

                    let genargs_decl = self.opt_consume_generic_argument_declaration()?;

                    let self_typ = self.consume_type_representaion()?;

                    self.must_consume_next(vec![TkKindName::MarkLBrace])?;

                    let mut assoc_fns = vec![];
                    let mut methods = vec![];
                    let mut native_assoc_fns = vec![];
                    let mut native_methods = vec![];

                    loop {
                        if let Some(t) = self.peek().copied()
                            && TkKind::MarkRBrace == t.kind
                        {
                            self.next();

                            return Ok(Some(Globals::ImplBlock(ImplBlock {
                                assoc_fns,
                                methods,
                                native_assoc_fns,
                                native_methods,
                                genargs_decl,
                                self_typ,
                            })));
                        } else {
                            let flags = self.consume_compiler_flags()?;
                            let f = self.consume_function_or_method_definition(flags)?;

                            match f {
                                FnOrMethod::Fn(CodeOrNative::Code(f)) => {
                                    assoc_fns.push(f);
                                }
                                FnOrMethod::Fn(CodeOrNative::Native(f)) => {
                                    native_assoc_fns.push(f);
                                }
                                FnOrMethod::Method(CodeOrNative::Code(f)) => {
                                    methods.push(f);
                                }
                                FnOrMethod::Method(CodeOrNative::Native(f)) => {
                                    native_methods.push(f);
                                }
                            }
                        }
                    }
                }
                TkKind::DslLiteral(str) => {
                    let native = str.to_string();
                    let native_span = t.span.clone();
                    self.next();

                    Ok(Some(Globals::NativeCode(NativeCode {
                        native,
                        native_span,
                        flags,
                    })))
                }
                TkKind::KwScene => {
                    let begin = t.span.clone();
                    self.next();

                    let id = self.consume_identifier()?;

                    let args = self.consume_argsdec()?;

                    let rtype = if self
                        .consume_next_if_match(vec![TkKindName::MarkArrow])
                        .is_some()
                    {
                        RetTypRepr::Typ(self.consume_type_representaion()?)
                    } else {
                        RetTypRepr::Void(Span::new(
                            args.span.module(),
                            args.span.end(),
                            args.span.end(),
                        ))
                    };

                    let t = self.next().ok_or(ParseError::InvalidEOF {
                        mod_id,
                        expecteds: vec![TkKindName::DslLiteral],
                    })?;
                    let end = t.span.clone();
                    if let TkKind::DslLiteral(str) = t.kind {
                        let novel_stmts = biwac_novel_parser::NovelSourceStream::new(
                            str,
                            t.span.clone(),
                            self.interner,
                        )
                        .parse()
                        .map_err(ParseError::NovelParseError)?;

                        Ok(Some(Globals::NovelScene(NovelScene {
                            id,
                            def_id: OnceCell::new(),
                            args,
                            rtype,
                            stmts: novel_stmts,
                            span: Span::merge(&begin, &end),
                            flags,
                        })))
                    } else {
                        Err(ParseError::InvalidEOF {
                            mod_id,
                            expecteds: vec![TkKindName::DslLiteral],
                        })
                    }
                }
                _ => Err(ParseError::InvalidToken {
                    expecteds: vec![
                        TkKindName::KwFn,
                        TkKindName::KwStruct,
                        TkKindName::KwType,
                        TkKindName::KwImport,
                        TkKindName::KwImpl,
                        TkKindName::KwScene,
                    ],
                    found: t.to_owned().clone(),
                }),
            }
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_argsdec(&mut self) -> Result<ArgDeclList, ParseError<'src>> {
        let mod_id = self.mod_id;
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLPare])?
            .span
            .clone();

        let mut args = vec![];

        loop {
            let t = self
                .next()
                .ok_or(ParseError::InvalidEOF {
                    mod_id,
                    expecteds: vec![TkKindName::MarkRPare, TkKindName::Ident],
                })?
                .clone();

            if let TkKind::MarkRPare = t.kind {
                return Ok(ArgDeclList {
                    args,
                    span: Span::merge(&begin, &t.span),
                });
            } else if let TkKind::Ident(arg) = &t.kind {
                let typ = self.must_consume_type_annotation()?;

                args.push(ArgDecl {
                    span: Span::merge(&t.span, &typ.span),
                    typ,
                    id: Ident {
                        id: *arg,
                        span: t.span.clone(),
                    },
                });

                if let Some(t) = self.peek() {
                    if let TkKind::MarkComma = t.kind {
                        self.next();
                    } else if let TkKind::MarkRPare = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                            found: t.to_owned().clone(),
                        });
                    }
                } else {
                    return Err(ParseError::InvalidEOF {
                        mod_id,
                        expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                    });
                }
            }
        }
    }

    pub(crate) fn consume_method_argsdec(
        &mut self,
    ) -> Result<FnOrMethod<ArgDeclList, MethodArgDeclList>, ParseError<'src>> {
        let mod_id = self.mod_id;

        // (args, self_ident)
        let begin = self
            .must_consume_next(vec![TkKindName::MarkLPare])?
            .span
            .clone();

        let mut args = vec![];

        // first arg `self` or not
        let t = self.peek().ok_or(ParseError::InvalidEOF {
            mod_id,
            expecteds: vec![
                TkKindName::MarkRPare,
                TkKindName::Ident,
                TkKindName::KwSelfVar,
            ],
        })?;

        let opt_self_span = match t.kind {
            TkKind::MarkRPare => {
                let end = t.span.clone();
                self.next();

                return Ok(FnOrMethod::Fn(ArgDeclList {
                    args,
                    span: Span::merge(&begin, &end),
                }));
            }
            TkKind::KwSelfVar => {
                let span = t.span.clone();
                self.next();

                Some(span)
            }
            _ => None,
        };

        loop {
            let t = self
                .next()
                .ok_or(ParseError::InvalidEOF {
                    mod_id,
                    expecteds: vec![TkKindName::MarkRPare, TkKindName::Ident],
                })?
                .clone();

            if let TkKind::MarkRPare = t.kind {
                match opt_self_span {
                    Some(self_span) => {
                        return Ok(FnOrMethod::Method(MethodArgDeclList {
                            self_span,
                            args,
                            span: Span::merge(&begin, &t.span),
                        }));
                    }
                    None => {
                        return Ok(FnOrMethod::Fn(ArgDeclList {
                            args,
                            span: Span::merge(&begin, &t.span),
                        }));
                    }
                }
            } else if let TkKind::Ident(arg) = &t.kind {
                let typ = self.must_consume_type_annotation()?;

                args.push(ArgDecl {
                    span: Span::merge(&t.span, &typ.span),
                    typ,
                    id: Ident {
                        id: *arg,
                        span: t.span.clone(),
                    },
                });

                if let Some(t) = self.peek() {
                    if let TkKind::MarkComma = t.kind {
                        self.next();
                    } else if let TkKind::MarkRPare = t.kind {
                        continue;
                    } else {
                        return Err(ParseError::InvalidToken {
                            expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                            found: t.to_owned().clone(),
                        });
                    }
                } else {
                    return Err(ParseError::InvalidEOF {
                        mod_id,
                        expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                    });
                }
            }
        }
    }
}
