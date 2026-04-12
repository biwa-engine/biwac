use biwac_base::Span;

use biwac_ast::{NovelBlockStmt, NovelIfStmt};

use crate::{NovelLineKind, NovelParseError, NovelSourceStream, token::NCodeTkKindName};

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

        let cond = self.consume_expression()?;

        // "{"
        let then_begin = self
            .must_consume_next(vec![NCodeTkKindName::MarkLBrace])?
            .span
            .clone();

        // <END_OF_LINE>
        self.must_be_line_end()?;

        let mut stmts = Vec::new();
        loop {
            let ss = self.consume_statements()?;

            if ss.is_empty() {
                match self.line_kind() {
                    Some(NovelLineKind::BlockClose) => {
                        // TODO: `}` 以降にトークンがないことを確認

                        let then_end = self.current_span(1);

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
                    Some(_) => {
                        panic!("compiler bug");
                    }
                    None => {
                        return Err(NovelParseError::CloseLineExpected {
                            span: self.current_span(1),
                        });
                    }
                }
            } else {
                stmts.extend(ss);
            }
        }
    }
}
