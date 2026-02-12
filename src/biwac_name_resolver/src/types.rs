use biwac_parser::{PrimTyp, TypRepr, TypReprVal};

use crate::{AbsId, ModuleLevelTryResolve};

#[derive(Debug, Clone, PartialEq)]
pub enum Typ {
    Int,
    Float,
    Bool,
    Fn(FnTyp),
    Defined(AbsId),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnTyp {
    pub args: Vec<Typ>,
    pub ret: Box<Typ>,
    // pub genargs: Vec<String>,
}

impl ModuleLevelTryResolve<TypRepr> for Typ {
    fn try_resolve_in_module<'pctx>(
        value: TypRepr,
        mctx: &crate::context::ModLvlRslvCtx<'pctx>,
    ) -> crate::RsvResult<Self> {
        match value.val {
            TypReprVal::Primitive(p) => match p {
                PrimTyp::Int => Ok(Typ::Int),
                PrimTyp::Uint => Ok(Typ::Int),
                PrimTyp::Bool => Ok(Typ::Int),
            },
            TypReprVal::Defined(deftyp) => {
                Ok(Typ::Defined(mctx.try_resolve_deftyp(&deftyp.qualid)?))
            }
        }
    }
}
