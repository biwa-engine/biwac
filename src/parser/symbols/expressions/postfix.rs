use crate::{
    lexer::token::{Token, TokenKind},
    parser::{
        symbols::{
            consume_identifier,
            expressions::{primary, Exprs, MemberAccess, Primary},
        },
        ParseError,
    },
};

pub fn consume(
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Exprs, ParseError> {
    let expr = primary::consume(tokens)?;

    consume_postfix_after_expr(expr, tokens)
}

fn consume_postfix_after_expr(
    expr: Exprs,
    tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
) -> Result<Exprs, ParseError> {
    if let Some(t) = tokens.peek() {
        match t.kind {
            TokenKind::Dot => {
                tokens.next();

                let mem_or_method = consume_identifier(tokens)?;

                if let Some(t) = tokens.peek() {
                    if let TokenKind::LPare = t.kind {
                        todo!()
                        // ISSUE: 何らかのデリミタを用意しないと、
                        // expr.method() と
                        // expr.member (anotherexpr)
                        // が区別できない
                    } else {
                        Ok(consume_postfix_after_expr(
                            Exprs::Primary(Primary::MemberAccess(MemberAccess {
                                left: Box::new(expr),
                                member: mem_or_method.clone(),
                            })),
                            tokens,
                        )?)
                    }
                } else {
                    Ok(consume_postfix_after_expr(
                        Exprs::Primary(Primary::MemberAccess(MemberAccess {
                            left: Box::new(expr),
                            member: mem_or_method.clone(),
                        })),
                        tokens,
                    )?)
                }
            }
            _ => Ok(expr),
        }
    } else {
        Ok(expr)
    }
}
