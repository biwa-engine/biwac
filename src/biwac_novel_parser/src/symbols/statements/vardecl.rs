use biwac_base::Span;

use biwac_ast::{TypDecl, VarDecl};

use crate::{NovelParseError, NovelSourceStream, token::NCodeTkKindName};

impl<'src> NovelSourceStream<'src> {
    // "let" <identifier> (":" <type-representation>)? "=" <expression> ";"
    // グローバル変数の初期化はctx None
    // グローバル変数に束縛できる値は限られる。リテラルだけでconstのみ許容でも良い
    pub(crate) fn consume_variable_declaration_statment(
        &mut self,
    ) -> Result<VarDecl, NovelParseError> {
        // "let"
        let begin = self
            .must_consume_next(vec![NCodeTkKindName::KwLet])?
            .span
            .clone();
        // <identifier>
        let id = self.consume_identifier()?;

        // (":" <type-representation>)?
        let typ = if let Some(t) = self.opt_consume_type_annotation(&None)? {
            TypDecl::Typ(t)
        } else {
            TypDecl::Any
        };

        // "="
        // NOTE: 変数宣言時、初期化は必須
        // 代入漏れバリデーション能力が向上したら初期化しないパターンもサポートするかも
        let _ = self.must_consume_next(vec![NCodeTkKindName::MarkAssign])?;

        // <expression>
        let init = self.consume_expression()?;

        // <END_OF_LINE>
        self.must_be_line_end()?;

        Ok(VarDecl {
            id,
            typ,
            init,
            span: Span::merge(&begin, &init.span()),
        })
    }
}
