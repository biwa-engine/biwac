use biwac_base::Span;
use biwac_lexer::TkKind;

use crate::{Exprs, Ident, ParseError, parser::TokenStream, types::TypDecl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarDecl {
    pub typ: TypDecl,
    pub id: Ident,
    pub init: Exprs,
    pub span: Span,
}

impl<'t> TokenStream<'t> {
    // "let" <identifier> (":" <type-representation>)? "=" <expression> ";"
    pub(crate) fn consume_variable_declaration_statment(&mut self) -> Result<VarDecl, ParseError> {
        // "let"
        let begin = self.must_consume_next(vec![TkKind::Let])?.span.clone();
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
        let _ = self.must_consume_next(vec![TkKind::Assign])?;

        // <expression>
        let init = self.consume_expression()?;

        // ";"
        let end = self.must_consume_semicolon()?.span.clone();

        Ok(VarDecl {
            id,
            typ,
            init,
            span: Span::merge(&begin, &end),
        })
    }
}
