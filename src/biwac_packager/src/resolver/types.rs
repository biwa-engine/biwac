use biwac_base::ModPath;
use biwac_parser::{PrimTyp, QualifiedId, TypRepr};

use crate::{
    AbsId,
    resolver::{ResolveError, TryResolve},
};

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

impl TryResolve<TypRepr> for Typ {
    fn try_resolve(
        value: TypRepr,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        match value {
            TypRepr::Primitive(p) => match p {
                PrimTyp::Int => Ok(Typ::Int),
                PrimTyp::Uint => Ok(Typ::Int),
                PrimTyp::Bool => Ok(Typ::Int),
            },
            TypRepr::Defined(deftyp) => Ok(Typ::Defined(AbsId::try_resolve(
                deftyp.qualid,
                imports,
                modpath,
            )?)),
        }
    }
}
