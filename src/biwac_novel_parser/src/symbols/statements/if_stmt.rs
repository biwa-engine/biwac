use biwac_base::Span;

use biwac_ast::{BlockStmt, IfStmt};

use crate::{NovelLineKind, NovelParseError, NovelSourceStream, token::NCodeTkKindName};

impl<'src> NovelSourceStream<'src> {
    //  "if" <expression> "{" <END_OF_LINE>
    //      <statement>*
    //  "}" ("else" "{" <END_OF_LINE>
    //      <statement>*
    //  "}" )?
    pub(super) fn consume_if_statement(&mut self) -> Result<IfStmt, NovelParseError<'src>> {
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
            match self.next_line() {
                Some(NovelLineKind::BlockClose) => {
                    // TODO: `}` 以降にトークンがないことを確認

                    let then_end = self.current_span(1);

                    return Ok(IfStmt {
                        span: Span::merge(&begin, &then_end),
                        cond,
                        then: BlockStmt {
                            stmts,
                            span: Span::merge(&then_begin, &then_end),
                        },
                        els: None,
                    });
                }
                Some(_) => match self.consume_statement()? {
                    Some(stmt) => {
                        stmts.push(stmt);
                    }
                    None => {
                        // error
                        todo!()
                    }
                },
                None => {
                    // error
                    todo!()
                }
            }
        }
    }
}
