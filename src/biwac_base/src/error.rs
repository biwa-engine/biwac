use colored::Colorize;

use crate::{IdentInterner, MetadataHolder, SourceHolder};

pub trait BiwacError {
    type ErrorContext;
    fn print_error_message(&self, ctx: &Self::ErrorContext);
}

#[derive(Debug)]
pub struct ErrorHolder<'a, E: BiwacError> {
    pub errs: Vec<E>,
    pub srcs: &'a SourceHolder,
    pub metadata: &'a MetadataHolder,
    pub interner: &'a IdentInterner,
}

impl<'a, C, E: BiwacError<ErrorContext = C>> ErrorHolder<'a, E> {
    pub fn print_error_messages(&self, ctx: &C) {
        for e in &self.errs {
            e.print_error_message(ctx);
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
