use crate::SourceHolder;

pub trait BiwacError {
    fn print_error_message(&self, srcs: &SourceHolder);
}

#[derive(Debug)]
pub struct ErrorHolder<E: BiwacError> {
    pub errs: Vec<E>,
    pub srcs: SourceHolder,
}

impl<E: BiwacError> ErrorHolder<E> {
    pub fn panic_with_error_messages(&self) -> ! {
        for e in &self.errs {
            e.print_error_message(&self.srcs);
        }

        panic!()
    }
}
