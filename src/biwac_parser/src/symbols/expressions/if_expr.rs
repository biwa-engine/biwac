use biwac_base::Span;

use crate::{BlockExpr, Exprs};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfExpr {
    pub cond: Box<Exprs>,
    pub then: BlockExpr,
    // pub else_ifs: Vec<(Exprs, BlockExpr)>,
    pub els: BlockExpr,
    pub span: Span,
}
