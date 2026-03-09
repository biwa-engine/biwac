use biwac_hir::{AssocCallee, LocVarId, ValId};

pub(crate) mod fn_level;
pub(crate) mod impl_level;
pub(crate) mod module_level;

#[derive(Debug)]
pub(crate) enum ResolvedValue {
    Global(ValId),
    Local(LocVarId),
    Assoc(AssocCallee),
}
