use crate::SourceHolder;

pub trait BiwacError {
    fn error_message(&self, srcs: &SourceHolder) -> String;
}

#[derive(Debug)]
pub struct ErrorHolder<E: BiwacError> {
    pub errs: Vec<E>,
    pub srcs: SourceHolder,
}

impl<E: BiwacError> ErrorHolder<E> {
    pub fn panic_with_error_messages(&self) -> ! {
        for e in &self.errs {
            println!("{}", e.error_message(&self.srcs))
        }

        panic!()
    }
}
