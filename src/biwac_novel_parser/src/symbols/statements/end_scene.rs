use biwac_ast::NovelEndSceneStmt;
use biwac_span::Span;

use crate::{NovelParseError, NovelSourceStream, token::NCodeTkKindName};

impl<'src> NovelSourceStream<'src> {
    // "endscene" <expression> <END_OF_LINE>
    pub(crate) fn consume_end_scene_statment(
        &mut self,
    ) -> Result<NovelEndSceneStmt, NovelParseError> {
        // "endscene"
        let begin = self
            .must_consume_next(vec![NCodeTkKindName::KwEndScene])?
            .span
            .clone();

        // <expression>
        let expr = self.consume_expression()?;

        // <END_OF_LINE>
        self.must_be_line_end()?;

        Ok(NovelEndSceneStmt {
            span: Span::merge(&begin, &expr.span()),
            expr,
        })
    }
}
