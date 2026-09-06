mod error;
mod inferrer;

#[cfg(test)]
mod tests;

pub use crate::error::{TyError, TyErrorReport, TyNames};
pub(crate) use crate::error::{error_ty_def_ids, error_tys};
pub use crate::inferrer::context::TyCtx;

pub type TyResult<T> = Result<T, TyError>;
