mod expression;

use biwac_type_inferrer::TypedPkg;

pub fn generate(pkg: &TypedPkg) -> &str {
    todo!()
}

fn span() -> oxc_span::Span {
    oxc_span::Span::new(0, 0)
}

trait AsOxc<O> {
    fn as_oxc(&self) -> O;
}
