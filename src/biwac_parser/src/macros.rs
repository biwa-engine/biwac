use biwac_ast::{
    BoolLiteral, CompilerFlag, CompilerFlagArg, CompilerFlagLiteral, IntegerLiteral, StringLiteral,
};
use biwac_lexer::TkKind;

use crate::{ParseError, TokenStream};

impl<'t> TokenStream<'t> {
    fn consume_compiler_flag_literal(&mut self) -> Result<CompilerFlagLiteral, ParseError> {
        if let Some(t) = self.next() {
            match t.kind {
                TkKind::IntegerLiteral => Ok(CompilerFlagLiteral::Integer(IntegerLiteral {
                    span: t.span.clone(),
                    val: t.unwrap_integer_value(),
                })),
                TkKind::BoolLiteralTrue => Ok(CompilerFlagLiteral::Bool(BoolLiteral {
                    span: t.span.clone(),
                    val: true,
                })),
                TkKind::BoolLiteralFalse => Ok(CompilerFlagLiteral::Bool(BoolLiteral {
                    span: t.span.clone(),
                    val: false,
                })),
                TkKind::StringLiteral => Ok(CompilerFlagLiteral::String(StringLiteral {
                    span: t.span.clone(),
                    val: t.unwrap_string_value(),
                })),
                _ => Err(ParseError::InvalidToken(
                    vec![
                        TkKind::IntegerLiteral,
                        TkKind::BoolLiteralTrue,
                        TkKind::BoolLiteralFalse,
                        TkKind::StringLiteral,
                    ],
                    t.clone(),
                )),
            }
        } else {
            Err(ParseError::InvalidEOF(vec![
                TkKind::IntegerLiteral,
                TkKind::BoolLiteralTrue,
                TkKind::BoolLiteralFalse,
                TkKind::StringLiteral,
            ]))
        }
    }

    fn consume_compiler_flag_args(&mut self) -> Result<Vec<CompilerFlagArg>, ParseError> {
        let _ = self.must_consume_next(vec![TkKind::LPare])?;

        let mut args = vec![];

        loop {
            if let Some(t) = self.peek().copied()
                && TkKind::RPare == t.kind
            {
                self.next();
                return Ok(args);
            } else {
                let arg = self.consume_identifier()?;

                if let Some(t) = self.peek().copied()
                    && TkKind::Assign == t.kind
                {
                    self.next();

                    let val = self.consume_compiler_flag_literal()?;

                    args.push(CompilerFlagArg {
                        arg,
                        val: Some(val),
                    });

                    if let Some(t) = self.peek().copied() {
                        if TkKind::RPare == t.kind {
                            self.next();
                            return Ok(args);
                        } else if TkKind::Comma == t.kind {
                            self.next();
                            continue;
                        } else {
                            return Err(ParseError::InvalidToken(
                                vec![TkKind::Comma, TkKind::RPare],
                                t.clone(),
                            ));
                        }
                    } else {
                        return Err(ParseError::InvalidEOF(vec![TkKind::Comma, TkKind::RPare]));
                    }
                } else {
                    args.push(CompilerFlagArg { arg, val: None });
                }
            }
        }
    }

    fn opt_consume_compiler_flag(&mut self) -> Result<Option<CompilerFlag>, ParseError> {
        // "[" "[" <identifier> ( "(" ( <identifier> ( "=" <compiler-flag-literal> )? "," )* ")" )? "]" "]"
        if let Some(t) = self.peek().copied()
            && TkKind::LBracket == t.kind
        {
            self.next();
            if let Some(t2) = self.peek().copied()
                && TkKind::LBracket == t2.kind
            {
                self.next();
                let flag = self.consume_identifier()?;

                if let Some(t) = self.peek().copied()
                    && TkKind::LPare == t.kind
                {
                    let args = self.consume_compiler_flag_args()?;

                    let _ = self.must_consume_next(vec![TkKind::RBracket])?;
                    let _ = self.must_consume_next(vec![TkKind::RBracket])?;

                    Ok(Some(CompilerFlag { flag, args }))
                } else {
                    let _ = self.must_consume_next(vec![TkKind::RBracket])?;
                    let _ = self.must_consume_next(vec![TkKind::RBracket])?;

                    Ok(Some(CompilerFlag { flag, args: vec![] }))
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub(crate) fn consume_compiler_flags(&mut self) -> Result<Vec<CompilerFlag>, ParseError> {
        let mut flags = vec![];

        while let Some(flag) = self.opt_consume_compiler_flag()? {
            flags.push(flag);
        }

        Ok(flags)
    }
}
