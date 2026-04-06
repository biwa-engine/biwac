use biwac_base::Span;
use biwac_lexer::TkKind;

use biwac_ast::{TypDecl, VarDecl};

use crate::{ParseError, TokenStream, symbols::globals::FnParseCtx};

impl<'t> TokenStream<'t> {
    // "let" <identifier> (":" <type-representation>)? "=" <expression> ";"
    // グローバル変数の初期化はctx None
    // グローバル変数に束縛できる値は限られる。リテラルだけでconstのみ許容でも良い
    pub(crate) fn consume_variable_declaration_statment(
        &mut self,
        ctx: Option<&FnParseCtx>,
    ) -> Result<VarDecl, ParseError> {
        // "let"
        let begin = self.must_consume_next(vec![TkKind::Let])?.span.clone();
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
        let _ = self.must_consume_next(vec![TkKind::Assign])?;

        // <expression>
        let init =
            self.consume_expression(ctx.expect("global variable parse not supported yet"))?;

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
