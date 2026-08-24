use biwac_ast::{
    AttrArg, AttrBody, AttrValue, Attribute, Attrs, BoolLiteral, IntegerLiteral, StringLiteral,
};
use biwac_lexer::{TkKind, TkKindName, Token};
use biwac_span::Span;

use crate::{ParseError, TokenStream};

// 属性のパース。
//
//  <attribute>       ::= "[" "[" <identifier> <attribute-body>? "]" "]"
//  <attribute-body>  ::= "=" <attribute-value>
//                      | "(" ( <identifier> ( "=" <attribute-value> )? ","? )* ")"
//  <attribute-value> ::= <integer> | <string> | <boolean>
//
// ここでは構文だけを見る。
// 属性名が既知かどうか、キーが正しいか、付与対象が妥当かは
// biwac_attribute の検証パスの責務である。

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    /// 消費せずに先頭 2 トークンを覗く。
    ///
    /// `[[` の判定には 2 トークン必要だが `Peekable` は 1 トークンしか
    /// 先読みできないため、イテレータを複製して読む。
    fn peek_two(&self) -> (Option<&'t Token<'src>>, Option<&'t Token<'src>>) {
        let mut it = self.tokens.clone();
        let first = it.next();
        let second = it.next();
        (first, second)
    }

    fn consume_attr_value(&mut self) -> Result<AttrValue, ParseError<'src>> {
        let expecteds = vec![
            TkKindName::LiteralInteger,
            TkKindName::LiteralString,
            TkKindName::KwBoolTrue,
            TkKindName::KwBoolFalse,
        ];

        let mod_id = self.mod_id;
        let t = self.next().ok_or(ParseError::InvalidEOF {
            mod_id,
            expecteds: expecteds.clone(),
        })?;

        match t.kind {
            TkKind::LiteralInteger(val) => Ok(AttrValue::Integer(IntegerLiteral {
                span: t.span.clone(),
                val,
            })),
            TkKind::LiteralString(str) => Ok(AttrValue::String(StringLiteral {
                span: t.span.clone(),
                val: str.to_string(),
            })),
            TkKind::KwBoolTrue => Ok(AttrValue::Bool(BoolLiteral {
                span: t.span.clone(),
                val: true,
            })),
            TkKind::KwBoolFalse => Ok(AttrValue::Bool(BoolLiteral {
                span: t.span.clone(),
                val: false,
            })),
            _ => Err(ParseError::InvalidToken {
                expecteds,
                found: t.clone(),
            }),
        }
    }

    /// `"(" ( <identifier> ( "=" <attribute-value> )? ","? )* ")"` を消費する。
    /// 呼び出し時点で次のトークンは `"("` である。
    fn consume_attr_args(&mut self) -> Result<Vec<AttrArg>, ParseError<'src>> {
        let _ = self.must_consume_next(vec![TkKindName::MarkLPare])?;

        let mut args = vec![];

        loop {
            let mod_id = self.mod_id;
            let t = *self.peek().ok_or(ParseError::InvalidEOF {
                mod_id,
                expecteds: vec![TkKindName::Ident, TkKindName::MarkRPare],
            })?;

            if TkKind::MarkRPare == t.kind {
                self.next();
                return Ok(args);
            }

            let key = self.consume_identifier()?;

            // 値は省略できる: `[[foo(bar)]]`
            let val = if let Some(t) = self.peek().copied()
                && TkKind::MarkAssign == t.kind
            {
                self.next();
                Some(self.consume_attr_value()?)
            } else {
                None
            };

            let span = match &val {
                Some(v) => Span::merge(&key.span, v.span()),
                None => key.span.clone(),
            };
            args.push(AttrArg { key, val, span });

            // 値の有無にかかわらず、区切りは "," か ")" のいずれかである。
            let t = self.must_consume_next(vec![TkKindName::MarkComma, TkKindName::MarkRPare])?;
            if TkKind::MarkRPare == t.kind {
                return Ok(args);
            }
        }
    }

    fn opt_consume_attribute(&mut self) -> Result<Option<Attribute>, ParseError<'src>> {
        // `[[` が揃っていることを確認してから消費する。
        // 片方だけ消費して戻るとトークンを落としてしまう。
        let (first, second) = self.peek_two();
        match (first, second) {
            (Some(f), Some(s))
                if TkKind::MarkLBracket == f.kind && TkKind::MarkLBracket == s.kind => {}
            _ => return Ok(None),
        }

        let begin = self.next().expect("peeked").span.clone();
        self.next();

        let name = self.consume_identifier()?;

        let body = match self.peek().copied() {
            Some(t) if TkKind::MarkAssign == t.kind => {
                self.next();
                AttrBody::Value(self.consume_attr_value()?)
            }
            Some(t) if TkKind::MarkLPare == t.kind => AttrBody::List(self.consume_attr_args()?),
            _ => AttrBody::Word,
        };

        let _ = self.must_consume_next(vec![TkKindName::MarkRBracket])?;
        let end = self
            .must_consume_next(vec![TkKindName::MarkRBracket])?
            .span
            .clone();

        Ok(Some(Attribute {
            name,
            body,
            span: Span::merge(&begin, &end),
        }))
    }

    pub(crate) fn consume_attributes(&mut self) -> Result<Attrs, ParseError<'src>> {
        let mut attrs = vec![];

        while let Some(attr) = self.opt_consume_attribute()? {
            attrs.push(attr);
        }

        Ok(Attrs::new(attrs))
    }
}

impl<'t, 'src, 'i> TokenStream<'t, 'src, 'i> {
    /// `[[native]]` が付いているか。
    ///
    /// `native` は文法自体に影響する唯一の属性であり、
    /// 付いていれば `{{ ... }}` のネイティブボディを期待する。
    /// 属性の一覧と検証は biwac_attribute が持つが、
    /// パースの分岐にはこの判定だけが必要なので名前定数を参照する。
    pub(crate) fn has_native_attr(&mut self, attrs: &Attrs) -> bool {
        let native = self
            .interner
            .get_or_insert(biwac_attribute::attr_names::Native);
        attrs.iter().any(|a| a.name.id == native)
    }
}
