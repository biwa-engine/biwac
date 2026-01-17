use crate::{
    lexer::token::{Token, TokenKind},
    parser::{symbols::expressions::unary::Unary, Parse, ParseError},
};

use super::equality::EqualityExpr;

// AssignExpr = (Unary "=")* EqualityExpr
#[derive(Debug, Clone)]
pub struct AssignExpr {
    pub lefts: Vec<AssignExprNode>,
    pub right: EqualityExpr,
}

#[derive(Debug, Clone)]
pub struct AssignExprNode {
    pub left: Unary,
    pub op: AssignOperator,
}

// now, only = can be used, but +=, -=, and so on will be used in the future
#[derive(Debug, Clone, Copy)]
pub enum AssignOperator {
    Assign, // =
}

impl Parse for AssignExpr {
    type SelfType = Self;

    fn consume(
        tokens: &mut std::iter::Peekable<std::slice::Iter<'_, Token>>,
    ) -> Result<Self::SelfType, ParseError> {
        let mut lefts: Vec<AssignExprNode> = vec![];
        let mut last_tokens = tokens.clone();

        while let Ok(unary) = Unary::consume(tokens) {
            if let Some(t) = tokens.peek() {
                match t.kind {
                    TokenKind::Assign => {
                        tokens.next();

                        lefts.push(AssignExprNode {
                            left: unary,
                            op: AssignOperator::Assign,
                        });

                        last_tokens = tokens.clone();
                    }
                    _ => {
                        break;
                    }
                }
            }
        }

        *tokens = last_tokens;

        let right = EqualityExpr::consume(tokens)?;

        Ok(Self { lefts, right })
    }
}
