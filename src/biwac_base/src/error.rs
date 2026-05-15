use colored::Colorize;

use crate::{IdentInterner, MetadataHolder, SourceHolder};

pub trait BiwacError {
    fn print_error_message(&self, ctx: &ErrorContext);
}

#[derive(Debug)]
pub struct ErrorContext<'a> {
    pub metadata: &'a MetadataHolder,
    pub srcs: &'a SourceHolder,
    pub interner: &'a IdentInterner,
}

#[derive(Debug)]
pub struct ErrorHolder<'c, E: BiwacError> {
    pub errs: Vec<E>,
    pub ctx: ErrorContext<'c>,
}

impl<'a, E: BiwacError> ErrorHolder<'a, E> {
    pub fn print_error_messages(&self) {
        for e in &self.errs {
            e.print_error_message(&self.ctx);
        }

        print_error_finish_message(self.errs.len());
    }
}

pub fn print_error_finish_message(err_count: usize) {
    println!(
        "{} Compile failed because of {} previous error(s).",
        "Error!".red().bold(),
        err_count
    );
}
