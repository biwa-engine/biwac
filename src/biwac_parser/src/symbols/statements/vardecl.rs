use std::cell::OnceCell;

use biwac_lexer::TkKindName;
use biwac_span::Span;

use biwac_ast::{TypDecl, VarDecl};

use crate::{ParseError, TokenStream};

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    // "let" <identifier> (":" <type-representation>)? "=" <expression> ";"
    // グローバル変数に束縛できる値は限られる。リテラルだけでconstのみ許容でも良い
    pub(crate) fn consume_variable_declaration_statment(
        &mut self,
    ) -> Result<VarDecl, ParseError<'src>> {
        // "let"
        let begin = self
            .must_consume_next(vec![TkKindName::KwLet])?
            .span
            .clone();
        // <identifier>
        let id = self.consume_identifier()?;

        // (":" <type-representation>)?
        let typ = if let Some(t) = self.opt_consume_type_annotation()? {
            TypDecl::Typ(t)
        } else {
            TypDecl::Any
        };

        // "="
        // NOTE: 変数宣言時、初期化は必須
        // 代入漏れバリデーション能力が向上したら初期化しないパターンもサポートするかも
        let _ = self.must_consume_next(vec![TkKindName::MarkAssign])?;

        // <expression>
        let init = self.consume_expression()?;

        // ";"
        let end = self.must_consume_semicolon()?.span.clone();

        Ok(VarDecl {
            id,
            typ,
            init,
            span: Span::merge(&begin, &end),
            var_id: OnceCell::new(),
        })
    }
}
