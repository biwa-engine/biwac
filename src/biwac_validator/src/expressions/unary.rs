use crate::{
    parser::symbols::expressions,
    validator::{
        expressions::{Exprs, UnOperator},
        types::AbsoluteType,
        Env, PrimitiveType, ValidateError,
    },
};

pub fn validate(
    op: &expressions::UnOperator,
    expr: &crate::packager::resolver::symbols::expressions::Exprs,
    env: &mut Env,
) -> Result<(AbsoluteType, super::Exprs), ValidateError> {
    let (rtyp, rexpr) = expr.validate(env)?;

    match op {
        expressions::UnOperator::Neg => {
            if let AbsoluteType::Primitive(prim) = rtyp {
                match prim {
                    PrimitiveType::Int => Ok((
                        AbsoluteType::Primitive(PrimitiveType::Int),
                        Exprs::Unary(UnOperator::Neg, Box::new(rexpr)),
                    )),
                    // NOTE: Uint から Int へ暗黙の型変換が行われる
                    PrimitiveType::Uint => Ok((
                        AbsoluteType::Primitive(PrimitiveType::Int),
                        Exprs::Unary(UnOperator::Neg, Box::new(rexpr)),
                    )),
                    _ => {
                        panic!("this type cannot get negative value");
                    }
                }
            } else {
                panic!("this type cannot get negative value");
            }
        }
    }
}
