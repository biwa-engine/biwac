use crate::{
    packager::resolver::symbols::expressions,
    validator::{
        expressions::{Exprs, FnCall, Literal, MemberAccess, Primary},
        types::{AbsoluteType, PrimitiveType, TypeComarison},
        Env, ValidateError,
    },
};

pub fn validate(
    prim: &expressions::Primary,
    env: &mut Env,
) -> Result<(AbsoluteType, Exprs), ValidateError> {
    match prim {
        expressions::Primary::Literal(lit) => match lit {
            expressions::Literal::Uint(u) => Ok((
                AbsoluteType::Primitive(PrimitiveType::Uint),
                Exprs::Primary(Primary::Literal(Literal::Uint(*u))),
            )),
            // primary::Literal::Float(f) => Ok((
            //     // Type::Primitive(PrimitiveType::Float),
            //     Type::Primitive(PrimitiveType::Int),
            //     Primary::Literal(Literal::Float(*f)),
            // )),
            expressions::Literal::String(_) => {
                todo!()
                // if let Some(id) = env.global.string_literals.get(s) {
                //     Ok((
                //         Type::Array(Box::new(Type::Primitive(PrimitiveType::Char)), s.len()),
                //         Exprs::Primary(Primary::Literal(Literal::String(*id))),
                //     ))
                // } else {
                //     let id = env.global.string_literals.values().len();
                //     env.global.string_literals.insert(s.clone(), id);
                //     Ok((
                //         Type::Array(Box::new(Type::Primitive(PrimitiveType::Char)), s.len()),
                //         Exprs::Primary(Primary::Literal(Literal::String(id))),
                //     ))
                // }
            }
            expressions::Literal::Bool(b) => Ok((
                AbsoluteType::Primitive(PrimitiveType::Bool),
                Exprs::Primary(Primary::Literal(Literal::Bool(*b))),
            )),
            expressions::Literal::Struct(absid, members) => {
                if let Some(ssign) = env.get_structsign(absid) {
                    let mut member_typs = ssign.members.clone();
                    let mut member_inits = vec![];

                    for (member_id, expr) in members {
                        if let Some((dtyp, dindex)) = member_typs.get(member_id) {
                            let (styp, sexpr) = expr.validate(env)?;

                            match dtyp {
                                AbsoluteType::Primitive(dptyp) => {
                                    if let AbsoluteType::Primitive(sptyp) = styp {
                                        match dptyp.compare(&sptyp) {
                                            TypeComarison::Equal => {
                                                member_inits.push((*dindex, dtyp.clone(), sexpr));
                                            }
                                            TypeComarison::ImplicitlyConvertableFrom => {
                                                // TODO:
                                                // Ok(Some(Stmt::Assign(dst, Exprs::Unary( ConvertOp,
                                                // Box::new(sexpr)))
                                                member_inits.push((*dindex, dtyp.clone(), sexpr));
                                            }
                                            _ => panic!("types not equal"),
                                        }
                                    } else {
                                        panic!("types not equal");
                                    }
                                }
                                AbsoluteType::List(_) => {
                                    if dtyp.clone().equals(&styp) {
                                        member_inits.push((*dindex, dtyp.clone(), sexpr));
                                    } else {
                                        panic!("types not equal");
                                    }
                                }
                                AbsoluteType::Defined(_) => {
                                    if dtyp.clone().equals(&styp) {
                                        member_inits.push((*dindex, dtyp.clone(), sexpr));
                                    } else {
                                        panic!("types not equal");
                                    }
                                }
                            }

                            member_typs.remove(member_id);
                        } else {
                            return Err(ValidateError::StructMemberNotFound(
                                absid.clone(),
                                member_id.clone(),
                            ));
                        }
                    }

                    if !member_typs.is_empty() {
                        // TODO: return Err
                        panic!("not all members of struct initialized");
                    }

                    Ok((
                        AbsoluteType::Defined(absid.clone()),
                        Exprs::Primary(Primary::Literal(Literal::Struct(
                            absid.clone(),
                            member_inits,
                        ))),
                    ))
                } else {
                    Err(ValidateError::TypeNotFound(absid.clone()))
                }
            }
        },
        expressions::Primary::Variable(id) => {
            let var = env
                .get_var(id)
                .ok_or(ValidateError::VariableNotFound(id.clone()))?;

            Ok((
                var.typ.clone(),
                Exprs::Primary(Primary::Variable(var.clone())),
            ))
        }
        expressions::Primary::FnCall(fcalling) => {
            env.fn_exists(&fcalling.absid)?;

            let mut i = 0;
            let mut args = vec![];

            while let Some(acalling) = fcalling.args.get(i) {
                let (acalling_typ, acalling) = acalling.validate(env)?;

                let atyp;

                let fcallee = env.get_fnsign(&fcalling.absid).unwrap();
                if let Some((acallee_typ, _)) = fcallee.args.get(i) {
                    match acallee_typ.compare(&acalling_typ) {
                        TypeComarison::Equal => {
                            atyp = acalling_typ;
                        }
                        TypeComarison::ImplicitlyConvertableFrom => {
                            atyp = acallee_typ.clone();
                        }
                        _ => {
                            return Err(ValidateError::ArgumentMismatch(
                                Some(Box::new(acallee_typ.clone())),
                                Some(Box::new(acalling_typ)),
                            ));
                        }
                    }
                } else {
                    return Err(ValidateError::ArgumentMismatch(
                        None,
                        Some(Box::new(acalling_typ)),
                    ));
                }

                i += 1;
                args.push((atyp, acalling));
            }

            let fcallee = env.get_fnsign(&fcalling.absid).unwrap();
            if let Some((acallee_typ, _)) = fcallee.args.get(i) {
                Err(ValidateError::ArgumentMismatch(
                    Some(Box::new(acallee_typ.clone())),
                    None,
                ))
            } else {
                Ok((
                    fcallee.rtype.clone().expect("function returns void"),
                    Exprs::Primary(Primary::FnCall(FnCall {
                        absid: fcalling.absid.clone(),
                        args,
                        rtype: fcallee.rtype.clone(),
                    })),
                ))
            }
        }
        // expressions::Primary::LanglibfnCall(fcalling) => {
        //     // TODO: when not found, return LanglibFunctionNotFound error
        //     let fcallee_rtype = env
        //         .global
        //         .signtree
        //         .langlibfns
        //         .get(&fcalling.id)
        //         .ok_or(ValidateError::FunctionNotFound(fcalling.id.clone()))?;
        //
        //     let args = fcalling
        //         .args
        //         .iter()
        //         .map(|a| a.validate(env))
        //         .collect::<Result<Vec<(AbsoluteType, Exprs)>, ValidateError>>()?;
        //
        //     Ok((
        //         fcallee_rtype
        //             .clone()
        //             .expect("function returns void")
        //             .into_abs(env)?,
        //         Exprs::Primary(Primary::LanglibfnCall(LanglibfnCall {
        //             id: fcalling.id.clone(),
        //             args,
        //             rtype: fcallee_rtype
        //                 .clone()
        //                 .map(|typ| typ.into_abs(env))
        //                 .transpose()?,
        //         })),
        //     ))
        // }
        expressions::Primary::MemberAccess(access) => {
            let (ltyp, left) = access.left.validate(env)?;

            if let AbsoluteType::Defined(absid) = ltyp.clone() {
                let ssign = env
                    .get_structsign(&absid)
                    .ok_or(ValidateError::FunctionNotFound(absid.clone()))?;

                if let Some((typ, index)) = ssign.members.get(&access.member) {
                    Ok((
                        typ.clone(),
                        Exprs::Primary(Primary::MemberAccess(MemberAccess {
                            str: (ltyp, Box::new(left)),
                            member: (typ.clone(), *index),
                        })),
                    ))
                } else {
                    Err(ValidateError::StructMemberNotFound(
                        absid,
                        access.member.clone(),
                    ))
                }
            } else {
                Err(ValidateError::TypeAndOperatorNotSupported(
                    ltyp.to_string(),
                    ".".to_string(),
                ))
            }
        } // expressions::Primary::MethodCall(_) => {}
    }
}
