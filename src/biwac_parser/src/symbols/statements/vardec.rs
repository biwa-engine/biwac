use biwac_lexer::{TkKind, TkVal};

use crate::{Exprs, ParseError, parser::TokenStream, types::TypDecl};

#[derive(Debug, Clone)]
pub struct VarDec {
    pub typ: TypDecl,
    pub name: String,
    pub init: Exprs,
}

impl<'t> TokenStream<'t> {
    pub(crate) fn consume_variable_declaration_statment(&mut self) -> Result<VarDec, ParseError> {
        let _ = self.must_consume_next(vec![TkKind::Let])?;
        let t = self.must_consume_next(vec![TkKind::Ident])?.clone();

        if let TkKind::Ident = t.kind
            && let Some(TkVal::String(id)) = t.val
        {
            let typ = if let Some(t) = self.opt_consume_type_annotation()? {
                TypDecl::Typ(t)
            } else {
                TypDecl::Any
            };

            // NOTE: 変数宣言時、初期化は必須
            // 代入漏れバリデーション能力が向上したら初期化しないパターンもサポートするかも
            let _ = self.must_consume_next(vec![TkKind::Assign])?;

            let init = self.consume_expression()?;

            Ok(VarDec {
                typ,
                name: id.clone(),
                init,
            })
        } else {
            Err(ParseError::InvalidToken(vec![TkKind::Let], t.clone()))
        }
    }
}
