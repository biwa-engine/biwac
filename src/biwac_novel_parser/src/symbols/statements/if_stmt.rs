use biwac_span::Span;

use biwac_ast::{NovelBlockStmt, NovelIfStmt};

use crate::{
    NovelLineKind, NovelParseError, NovelSourceStream, symbols::statements::ParsedNovelStmt,
    token::NCodeTkKindName,
};

impl<'src> NovelSourceStream<'src> {
    //  "if" <expression> "{" <END_OF_LINE>
    //      <statement>*
    //  "}" ("else" "{" <END_OF_LINE>
    //      <statement>*
    //  "}" )?
    pub(super) fn consume_if_statement(&mut self) -> Result<NovelIfStmt, NovelParseError> {
        let begin = self
            .must_consume_next(vec![NCodeTkKindName::KwIf])?
            .span
            .clone();

        let cond = self.consume_condition_expression()?;

        // "{"
        let then_begin = self
            .must_consume_next(vec![NCodeTkKindName::MarkLBrace])?
            .span
            .clone();

        // <END_OF_LINE>
        self.must_be_line_end()?;

        let mut stmts = Vec::new();
        loop {
            let parsed = self.consume_statements()?;

            match parsed {
                ParsedNovelStmt::Stmts { stmts: ss } => {
                    stmts.extend(ss);
                }
                ParsedNovelStmt::ExitBlock { line_handler } => match line_handler.kind() {
                    NovelLineKind::BlockClose => {
                        // TODO: `}` 以降にトークンがないことを確認

                        let then_end = self.line_span(&line_handler);

                        return Ok(NovelIfStmt {
                            span: Span::merge(&begin, &then_end),
                            cond,
                            then: NovelBlockStmt {
                                stmts,
                                span: Span::merge(&then_begin, &then_end),
                            },
                            els: None,
                        });
                    }
                    _ => {
                        panic!("compiler bug");
                    }
                },
                ParsedNovelStmt::EndOfRange { span } => {
                    return Err(NovelParseError::CloseLineExpected { span });
                }
            }
        }
    }
}
