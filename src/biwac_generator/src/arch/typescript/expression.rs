use biwac_name_resolver::Expr;

use crate::arch::typescript::AsOxc;

impl<'a> AsOxc<oxc_ast::ast::Expression<'a>> for Expr {
    fn as_oxc(&self) -> oxc_ast::ast::Expression<'a> {
        todo!()
    }
}
