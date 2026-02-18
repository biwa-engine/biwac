mod expression;
mod globals;
mod statement;
mod types;

use std::cell::Cell;

use biwac_name_resolver::{AbsId, TypeDefContent};
use biwac_type_inferrer::{Sym, TyInfo, TypedPkg};

pub fn generate(pkg: &TypedPkg) -> String {
    let allocator = oxc_allocator::Allocator::default();

    let oxc_ast = oxc_ast::ast::Program {
        span: span(),
        source_type: oxc_ast::ast::SourceType::ts(), // TypeScript
        body: oxc_allocator::Vec::from_iter_in(
            pkg.syms.iter().map(|(id, sym)| match sym {
                Sym::FnDef(f) => f.as_oxc_global(id, &allocator),
                Sym::NativeFnDef(f) => f.as_oxc_global(id, &allocator),
                Sym::TypeDef(t) => match t {
                    TypeDefContent::Struct(s) => s.as_oxc_global(id, &allocator),
                },
                Sym::VarDecl(_) => todo!(),
            }),
            &allocator,
        ),
        directives: oxc_allocator::Vec::new_in(&allocator),
        hashbang: None,
        source_text: "",
        comments: oxc_allocator::Vec::new_in(&allocator),
        scope_id: Cell::new(None),
    };

    let result = oxc_codegen::Codegen::new().build(&oxc_ast);

    result.code
}

fn span() -> oxc_span::Span {
    oxc_span::Span::new(0, 0)
}

struct FnAstBuildEnv<'a> {
    pub(super) ty_info: &'a TyInfo,
    pub(super) stmts: Vec<oxc_ast::ast::Statement<'a>>,
}

trait AsOxcGlobal<'a, O> {
    fn as_oxc_global(&'a self, id: &AbsId, allocator: &'a oxc_allocator::Allocator) -> O;
}

trait AsOxc<'a, O> {
    fn as_oxc(&'a self, env: &mut FnAstBuildEnv<'a>, allocator: &'a oxc_allocator::Allocator) -> O;
}

trait IntoOxc<'a, O> {
    fn into_oxc(self, allocator: &'a oxc_allocator::Allocator) -> O;
}

trait Mangled {
    fn mangled(&self) -> String;
}

impl Mangled for AbsId {
    fn mangled(&self) -> String {
        let mut result = String::from("_Z");

        // ネストがある場合は N ... E で囲む
        if !self.quals.is_empty() {
            result.push('N');

            for q in &self.quals {
                result.push_str(&format!("{}{}", q.len(), q));
            }

            result.push_str(&format!("{}{}", self.id.len(), self.id));
            result.push('E');
        } else {
            // グローバル
            result.push_str(&format!("{}{}", self.id.len(), self.id));
        }

        result
    }
}
