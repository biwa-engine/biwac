use biwac_parser::{PrimTyp, TypRepr};

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
    pub(crate) args: Vec<Typ>,
    pub(crate) ret: Box<Typ>,
    // pub(crate) genargs: Vec<String>,
}

impl ModuleLevelTryResolve<TypRepr> for Typ {
    fn try_resolve<'pctx>(
        value: TypRepr,
        mctx: &crate::context::ModLvlRslvCtx<'pctx>,
    ) -> crate::RsvResult<Self> {
        match value {
            TypRepr::Primitive(p) => match p {
                PrimTyp::Int => Ok(Typ::Int),
                PrimTyp::Uint => Ok(Typ::Int),
                PrimTyp::Bool => Ok(Typ::Int),
            },
            TypRepr::Defined(deftyp) => Ok(Typ::Defined(mctx.try_resolve_deftyp(&deftyp.qualid)?)),
        }
    }
}
