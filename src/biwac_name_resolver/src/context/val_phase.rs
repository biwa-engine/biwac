use biwac_hir::{AssocCallee, LocVarId, ValId};

pub(crate) mod fn_level;
pub(crate) mod impl_level;
pub(crate) mod module_level;

#[derive(Debug)]
pub(crate) enum ResolvedValue {
    Global(ValId),

    #[allow(dead_code)]
    Local(LocVarId),

    Assoc(AssocCallee),
}
