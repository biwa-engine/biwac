use colored::Colorize;

use crate::{MetadataHolder, SourceHolder};

pub trait BiwacError {
    fn print_error_message(&self, metadata: &MetadataHolder, srcs: &SourceHolder);
}

#[derive(Debug)]
pub struct ErrorHolder<'a, E: BiwacError> {
    pub errs: Vec<E>,
    pub srcs: &'a SourceHolder,
    pub metadata: &'a MetadataHolder,
}

impl<'a, E: BiwacError> ErrorHolder<'a, E> {
    pub fn print_error_messages(&self) {
        for e in &self.errs {
            e.print_error_message(self.metadata, self.srcs);
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
