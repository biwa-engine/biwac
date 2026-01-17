use crate::{parser, validator::AbsoluteId};

pub type PrimitiveType = parser::types::PrimitiveType;

impl PrimitiveType {
    pub fn compare(&self, other: &Self) -> TypeComarison {
        match self {
            Self::Uint => match other {
                Self::Uint => TypeComarison::Equal,
                Self::Int => TypeComarison::ImplicitlyConvertableTo,
                // Self::Float => TypeComarison::ImplicitlyConvertableTo,
                // Self::String => TypeComarison::ImplicitlyConvertableTo,
                _ => TypeComarison::ImplicitlyUnconvertable,
            },
            Self::Int => match other {
                Self::Int => TypeComarison::Equal,
                Self::Uint => TypeComarison::ImplicitlyConvertableFrom,
                // Self::Float => TypeComarison::ImplicitlyConvertableTo,
                // Self::String => TypeComarison::ImplicitlyConvertableTo,
                _ => TypeComarison::ImplicitlyUnconvertable,
            },
            // Self::Float => match other_prim {
            //     Self::Uint => TypeComarison::ImplicitlyConvertableFrom,
            //     Self::Int => TypeComarison::ImplicitlyConvertableFrom,
            //     Self::Float => TypeComarison::Equal,
            // },
            Self::Bool => match other {
                Self::Bool => TypeComarison::Equal,
                // Self::String => TypeComarison::ImplicitlyConvertableTo,
                _ => TypeComarison::ImplicitlyUnconvertable,
            },
            // Self::String => match other {
            //     Self::String => TypeComarison::Equal,
            //     _ => TypeComarison::ImplicitlyConvertableFrom,
            // },
            // Self::Path => match other {
            //     Self::Path => TypeComarison::Equal,
            //     Self::String => TypeComarison::ImplicitlyConvertableTo,
            //     _ => TypeComarison::ImplicitlyUnconvertable,
            // },
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeComarison {
    Equal,
    ImplicitlyConvertableTo,
    ImplicitlyConvertableFrom,
    ImplicitlyUnconvertable,
}

#[derive(Debug, Clone)]
pub enum AbsoluteType {
    Primitive(PrimitiveType),
    List(Box<AbsoluteType>),
    Defined(AbsoluteId),
}

// #[derive(Debug, Clone)]
// pub enum DefinedType {
//     Struct(String),
//     // Interface(String),
// }

impl AbsoluteType {
    pub fn compare(&self, other: &Self) -> TypeComarison {
        // ただし、Listについてはその識別子単体が渡されたと考える
        match self {
            Self::Primitive(prim) => match other {
                Self::Primitive(other_prim) => prim.compare(other_prim),
                _ => TypeComarison::ImplicitlyUnconvertable,
            },
            Self::List(ltyp) => match other {
                Self::List(other_ltyp) => {
                    if ltyp.equals(other_ltyp) {
                        TypeComarison::Equal
                    } else {
                        TypeComarison::ImplicitlyUnconvertable
                    }
                }
                _ => TypeComarison::ImplicitlyUnconvertable,
            },
            Self::Defined(defed) => match other {
                Self::Defined(other) => {
                    if defed.equals(other) {
                        TypeComarison::Equal
                    } else {
                        TypeComarison::ImplicitlyUnconvertable
                    }
                }
                _ => TypeComarison::ImplicitlyUnconvertable,
            },
        }
    }

    pub fn equals(&self, other: &Self) -> bool {
        match self {
            Self::Primitive(p) => match other {
                Self::Primitive(other_p) => p == other_p,
                _ => false,
            },
            Self::List(ltyp) => match other {
                Self::List(other_ltyp) => ltyp.equals(other_ltyp),
                _ => false,
            },
            Self::Defined(defed) => match other {
                Self::Defined(other) => defed.equals(other),
                _ => false,
            },
        }
    }
}
