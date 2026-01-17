pub mod block;

use crate::validator::{
    expressions::{AssignableExprs, Exprs, MemberAccess},
    types::{AbsoluteType, PrimitiveType, TypeComarison},
    Env, ValidateError,
};

#[derive(Debug)]
pub enum Stmt {
    Compound(Vec<Stmt>),
    Expr(Exprs),
    Return(Exprs),
    Branch(Box<BranchStmt>),
    Loop(Box<LoopStmt>),
    VarDec(String, AbsoluteType, Exprs),
    Assign(AssignableExprs, Exprs),
}

#[derive(Debug)]
pub struct BranchStmt {
    pub cond: Exprs,
    pub then: Vec<Stmt>,
    pub els: Option<Vec<Stmt>>,
}

#[derive(Debug)]
pub struct LoopStmt {
    pub cond: Exprs,
    pub stmts: Vec<Stmt>,
}

impl crate::packager::resolver::symbols::statements::Stmt {
    pub fn validate(&self, env: &mut Env) -> Result<Stmt, ValidateError> {
        match self {
            Self::Block(stmts) => Ok(Stmt::Compound(block::validate(stmts, env)?)),
            Self::Expr(expr) => Ok(Stmt::Expr(expr.validate(env)?.1)),
            Self::Return(expr) => {
                let (expr_typ, expr) = expr.validate(env)?;

                if let Some(local) = &env.local {
                    if let Some(rtype) = &local.rtype {
                        match rtype.compare(&expr_typ) {
                            TypeComarison::Equal => Ok(Stmt::Return(expr)),
                            TypeComarison::ImplicitlyConvertableFrom => Ok(Stmt::Return(expr)),
                            _ => Err(ValidateError::Mismatch(
                                Box::new(rtype.clone()),
                                Box::new(expr_typ),
                            )),
                        }
                    } else {
                        // WARN: fix it,
                        // no argument type check also mismatch with None
                        Err(ValidateError::ArgumentMismatch(
                            None,
                            Some(Box::new(expr_typ)),
                        ))
                    }
                } else {
                    Err(ValidateError::OutOfScopes)
                }
            }
            Self::If(if_stmt) => {
                let (typ, cond) = if_stmt.cond.validate(env)?;

                if !matches!(typ, AbsoluteType::Primitive(PrimitiveType::Bool)) {
                    return Err(ValidateError::Mismatch(
                        Box::new(AbsoluteType::Primitive(PrimitiveType::Bool)),
                        Box::new(typ),
                    ));
                }

                Ok(Stmt::Branch(Box::new(BranchStmt {
                    cond,
                    then: block::validate(&if_stmt.then, env)?,
                    els: if_stmt
                        .els
                        .as_ref()
                        .map(|els| block::validate(els, env))
                        .transpose()?,
                })))
            }
            Self::While(while_stmt) => {
                let (typ, cond) = while_stmt.cond.validate(env)?;

                if !matches!(typ, AbsoluteType::Primitive(PrimitiveType::Bool)) {
                    return Err(ValidateError::Mismatch(
                        Box::new(AbsoluteType::Primitive(PrimitiveType::Bool)),
                        Box::new(typ),
                    ));
                }

                Ok(Stmt::Loop(Box::new(LoopStmt {
                    cond,
                    stmts: block::validate(&while_stmt.stmts, env)?,
                })))
            }
            Self::VarDec(var) => {
                env.insert_var(var.id.clone(), var.typ.clone())?;

                let (init_typ, init) = var.init.validate(env)?;

                // NOTE: 変数の型がIntでかつ初期化の右辺値がUintと判定されるときを除き、
                // 型が一致していなければエラー
                if !(var.typ.clone().equals(&init_typ)
                    || (matches!(init_typ, AbsoluteType::Primitive(PrimitiveType::Uint))
                        && matches!(var.typ, AbsoluteType::Primitive(PrimitiveType::Int))))
                {
                    return Err(ValidateError::Mismatch(
                        Box::new(var.typ.clone()),
                        Box::new(init_typ),
                    ));
                }

                Ok(Stmt::VarDec(var.id.clone(), var.typ.clone(), init))
            }
            Self::Assign(dst, src) => {
                // 左辺値が代入可能であることのバリデーション
                let (dtyp, assignable) = match dst {
                    crate::packager::resolver::symbols::expressions::Primary::Variable(var) => {
                        let var = env
                            .get_var(var)
                            .ok_or(ValidateError::VariableNotFound(var.clone()))?;

                        (var.typ.clone(), AssignableExprs::Variable(var))
                    }
                    crate::packager::resolver::symbols::expressions::Primary::MemberAccess(
                        access,
                    ) => {
                        let (strtyp, strexpr) = access.left.validate(env)?;

                        if let AbsoluteType::Defined(absid) = strtyp.clone() {
                            let ssign = env
                                .get_structsign(&absid)
                                .ok_or(ValidateError::TypeNotFound(absid.clone()))?;

                            if let Some((typ, index)) = ssign.members.get(&access.member) {
                                (
                                    typ.clone(),
                                    AssignableExprs::MemberAccess(MemberAccess {
                                        str: (strtyp, Box::new(strexpr)),
                                        member: (typ.clone(), *index),
                                    }),
                                )
                            } else {
                                return Err(ValidateError::StructMemberNotFound(
                                    absid,
                                    access.member.clone(),
                                ));
                            }
                        } else {
                            return Err(ValidateError::TypeAndOperatorNotSupported(
                                strtyp.to_string(),
                                ".".to_string(),
                            ));
                        }
                    }
                    _ => {
                        panic!("other than variable or struct member cannot be assigned");
                    }
                };

                // 右辺値が左辺値に代入可能な型であることのバリデーション
                let (styp, sexpr) = src.validate(env)?;
                match dtyp {
                    AbsoluteType::Primitive(dptyp) => {
                        if let AbsoluteType::Primitive(sptyp) = styp {
                            match dptyp.compare(&sptyp) {
                                TypeComarison::Equal => Ok(Stmt::Assign(assignable, sexpr)),
                                TypeComarison::ImplicitlyConvertableFrom => {
                                    // TODO:
                                    // Ok(Some(Stmt::Assign(dst, Exprs::Unary( ConvertOp,
                                    // Box::new(sexpr)))
                                    Ok(Stmt::Assign(assignable, sexpr))
                                }
                                _ => panic!("types not equal"),
                            }
                        } else {
                            panic!("types not equal");
                        }
                    }
                    AbsoluteType::List(_) => {
                        if dtyp.equals(&styp) {
                            Ok(Stmt::Assign(assignable, sexpr))
                        } else {
                            panic!("types not equal");
                        }
                    }
                    AbsoluteType::Defined(_) => {
                        if dtyp.equals(&styp) {
                            Ok(Stmt::Assign(assignable, sexpr))
                        } else {
                            panic!("types not equal");
                        }
                    }
                }
            }
        }
    }
}
