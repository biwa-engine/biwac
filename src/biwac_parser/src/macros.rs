use biwac_ast::{
    BoolLiteral, CompilerFlag, CompilerFlagArg, CompilerFlagLiteral, IntegerLiteral, StringLiteral,
};
use biwac_lexer::{TkKind, TkKindName};

use crate::{ParseError, TokenStream};

impl<'t, 'src> TokenStream<'t, 'src> {
    fn consume_compiler_flag_literal(&mut self) -> Result<CompilerFlagLiteral, ParseError<'src>> {
        if let Some(t) = self.next() {
            match t.kind {
                TkKind::LiteralInteger(val) => Ok(CompilerFlagLiteral::Integer(IntegerLiteral {
                    span: t.span.clone(),
                    val,
                })),
                TkKind::KwBoolTrue => Ok(CompilerFlagLiteral::Bool(BoolLiteral {
                    span: t.span.clone(),
                    val: true,
                })),
                TkKind::KwBoolFalse => Ok(CompilerFlagLiteral::Bool(BoolLiteral {
                    span: t.span.clone(),
                    val: false,
                })),
                TkKind::LiteralString(str) => Ok(CompilerFlagLiteral::String(StringLiteral {
                    span: t.span.clone(),
                    val: str.to_string(),
                })),
                _ => Err(ParseError::InvalidToken {
                    expecteds: vec![
                        TkKindName::LiteralInteger,
                        TkKindName::LiteralString,
                        TkKindName::KwBoolTrue,
                        TkKindName::KwBoolFalse,
                    ],
                    found: t.clone(),
                }),
            }
        } else {
            Err(ParseError::InvalidEOF {
                mod_id: self.mod_id,
                expecteds: vec![
                    TkKindName::LiteralInteger,
                    TkKindName::LiteralString,
                    TkKindName::KwBoolTrue,
                    TkKindName::KwBoolFalse,
                ],
            })
        }
    }

    fn consume_compiler_flag_args(&mut self) -> Result<Vec<CompilerFlagArg>, ParseError<'src>> {
        let _ = self.must_consume_next(vec![TkKindName::MarkLPare])?;

        let mut args = vec![];

        loop {
            if let Some(t) = self.peek().copied()
                && TkKind::MarkRPare == t.kind
            {
                self.next();
                return Ok(args);
            } else {
                let arg = self.consume_identifier()?;

                if let Some(t) = self.peek().copied()
                    && TkKind::MarkAssign == t.kind
                {
                    self.next();

                    let val = self.consume_compiler_flag_literal()?;

                    args.push(CompilerFlagArg {
                        arg,
                        val: Some(val),
                    });

                    if let Some(t) = self.peek().copied() {
                        if TkKind::MarkRPare == t.kind {
                            self.next();
                            return Ok(args);
                        } else if TkKind::MarkComma == t.kind {
                            self.next();
                            continue;
                        } else {
                            return Err(ParseError::InvalidToken {
                                expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                                found: t.clone(),
                            });
                        }
                    } else {
                        return Err(ParseError::InvalidEOF {
                            mod_id: self.mod_id,
                            expecteds: vec![TkKindName::MarkComma, TkKindName::MarkRPare],
                        });
                    }
                } else {
                    args.push(CompilerFlagArg { arg, val: None });
                }
            }
        }
    }

    fn opt_consume_compiler_flag(&mut self) -> Result<Option<CompilerFlag>, ParseError<'src>> {
        // "[" "[" <identifier> ( "(" ( <identifier> ( "=" <compiler-flag-literal> )? "," )* ")" )? "]" "]"
        if let Some(t) = self.peek().copied()
            && TkKind::MarkLBracket == t.kind
        {
            self.next();
            if let Some(t2) = self.peek().copied()
                && TkKind::MarkLBracket == t2.kind
            {
                self.next();
                let flag = self.consume_identifier()?;

                if let Some(t) = self.peek().copied()
                    && TkKind::MarkLPare == t.kind
                {
                    let args = self.consume_compiler_flag_args()?;

                    let _ = self.must_consume_next(vec![TkKindName::MarkRBracket])?;
                    let _ = self.must_consume_next(vec![TkKindName::MarkRBracket])?;

                    Ok(Some(CompilerFlag { flag, args }))
                } else {
                    let _ = self.must_consume_next(vec![TkKindName::MarkRBracket])?;
                    let _ = self.must_consume_next(vec![TkKindName::MarkRBracket])?;

                    Ok(Some(CompilerFlag { flag, args: vec![] }))
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_compiler_flags(&mut self) -> Result<Vec<CompilerFlag>, ParseError<'src>> {
        let mut flags = vec![];

        while let Some(flag) = self.opt_consume_compiler_flag()? {
            flags.push(flag);
        }

        Ok(flags)
    }
}
