use crate::{
    parser::symbols::expressions,
    validator::{
        expressions::{BinOperator, Exprs},
        types::{AbsoluteType, PrimitiveType, TypeComarison},
        Env, ValidateError,
    },
};

impl BinOperator {
    pub fn from(value: &expressions::BinOperator, typ: &PrimitiveType) -> (Self, PrimitiveType) {
        // also returns which primitive type returned by op

        // TODO: type based operator
        match value {
            expressions::BinOperator::Add => match typ {
                PrimitiveType::Int => (Self::Add, typ.clone()),
                PrimitiveType::Uint => (Self::Add, typ.clone()),
                // PrimitiveType::Float => (Self::FAdd, typ.clone()),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Sub => match typ {
                PrimitiveType::Int => (Self::Sub, typ.clone()),
                PrimitiveType::Uint => (Self::Sub, typ.clone()),
                // PrimitiveType::Float => Self::FSub,
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Mul => match typ {
                PrimitiveType::Int => (Self::SMul, typ.clone()),
                PrimitiveType::Uint => (Self::UMul, typ.clone()),
                // PrimitiveType::Float => (Self::FMul,typ.clone()),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Div => match typ {
                PrimitiveType::Int => (Self::SDiv, typ.clone()),
                PrimitiveType::Uint => (Self::UDiv, typ.clone()),
                // PrimitiveType::Float => (Self::FDiv,typ.clone()),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Mod => match typ {
                PrimitiveType::Int => (Self::SMod, typ.clone()), // WARN: is it true?
                PrimitiveType::Uint => (Self::UMod, typ.clone()),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Lt => match typ {
                PrimitiveType::Int => (Self::SLt, PrimitiveType::Bool),
                PrimitiveType::Uint => (Self::ULt, PrimitiveType::Bool),
                // PrimitiveType::Float => (Self::FLt,PrimitiveType::Bool),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Gt => match typ {
                PrimitiveType::Int => (Self::SGt, PrimitiveType::Bool),
                PrimitiveType::Uint => (Self::UGt, PrimitiveType::Bool),
                // PrimitiveType::Float => (Self::FGt,PrimitiveType::Bool),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Le => match typ {
                PrimitiveType::Int => (Self::SLe, PrimitiveType::Bool),
                PrimitiveType::Uint => (Self::ULe, PrimitiveType::Bool),
                // PrimitiveType::Float => (Self::FLe,PrimitiveType::Bool),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Ge => match typ {
                PrimitiveType::Int => (Self::SGe, PrimitiveType::Bool),
                PrimitiveType::Uint => (Self::UGe, PrimitiveType::Bool),
                // PrimitiveType::Float => (Self::FGe,PrimitiveType::Bool),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Eq => match typ {
                PrimitiveType::Int => (Self::Eq, PrimitiveType::Bool),
                PrimitiveType::Uint => (Self::Eq, PrimitiveType::Bool),
                // PrimitiveType::Float => (Self::FEq,PrimitiveType::Bool),
                _ => panic!("not supported"),
            },
            expressions::BinOperator::Ne => match typ {
                PrimitiveType::Int => (Self::Ne, PrimitiveType::Bool),
                PrimitiveType::Uint => (Self::Ne, PrimitiveType::Bool),
                // PrimitiveType::Float => Self::FNe,
                _ => panic!("not supported"),
            },
        }
    }
}

pub fn validate(
    op: &expressions::BinOperator,
    left: &crate::packager::resolver::symbols::expressions::Exprs,
    right: &crate::packager::resolver::symbols::expressions::Exprs,
    env: &mut Env,
) -> Result<(AbsoluteType, Exprs), ValidateError> {
    let (ltyp, lexpr) = left.validate(env)?;
    let (rtyp, rexpr) = right.validate(env)?;

    if let AbsoluteType::Primitive(lptyp) = &ltyp {
        if let AbsoluteType::Primitive(rptyp) = &rtyp {
            match lptyp.compare(rptyp) {
                TypeComarison::Equal => {
                    let (op, ptyp) = BinOperator::from(op, lptyp);

                    Ok((
                        AbsoluteType::Primitive(ptyp),
                        Exprs::Binary(op, Box::new(lexpr), Box::new(rexpr)),
                    ))
                }
                TypeComarison::ImplicitlyConvertableTo => {
                    // TODO: if rtyp is Float, unary uitofp, sitofp

                    let (op, ptyp) = BinOperator::from(op, rptyp);

                    Ok((
                        AbsoluteType::Primitive(ptyp),
                        Exprs::Binary(op, Box::new(lexpr), Box::new(rexpr)),
                    ))
                }
                TypeComarison::ImplicitlyConvertableFrom => {
                    // TODO: if ltyp is Float, unary uitofp, sitofp

                    let (op, ptyp) = BinOperator::from(op, lptyp);

                    Ok((
                        AbsoluteType::Primitive(ptyp),
                        Exprs::Binary(op, Box::new(lexpr), Box::new(rexpr)),
                    ))
                }
                _ => {
                    todo!()
                }
            }
        } else {
            Err(ValidateError::TypeAndOperatorNotSupported(
                ltyp.to_string(),
                "TODO: BinOperator::to_string()".to_string(),
            ))
        }
    } else {
        Err(ValidateError::TypeAndOperatorNotSupported(
            ltyp.to_string(),
            "TODO: BinOperator::to_string()".to_string(),
        ))
    }
}
