use biwac_base::ModPath;
use biwac_parser::types::TypDecl;

use crate::{
    Exprs, Primary, Typ,
    resolver::{ResolveError, TryResolve},
};

#[derive(Debug)]
pub struct IfStmt {
    pub cond: Exprs,
    pub then: Vec<Stmt>,
    pub els: Option<Vec<Stmt>>,
}

#[derive(Debug)]
pub struct WhileStmt {
    pub cond: Exprs,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct VarDec {
    pub typ: Option<Typ>,
    pub id: String,
    pub init: Exprs,
}

#[derive(Debug)]
pub enum Stmt {
    Block(Vec<Stmt>),
    Expr(Exprs),
    Return(Exprs),
    If(Box<IfStmt>),
    While(Box<WhileStmt>),
    VarDec(VarDec),
    Assign(Primary, Exprs), // dst, src
}

impl TryResolve<biwac_parser::Stmt> for Stmt {
    fn try_resolve(
        value: biwac_parser::Stmt,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        match value {
            biwac_parser::Stmt::If(i) => Ok(Self::If(Box::new(IfStmt::try_resolve(
                *i, imports, modpath,
            )?))),
            biwac_parser::Stmt::While(w) => Ok(Self::While(Box::new(WhileStmt::try_resolve(
                *w, imports, modpath,
            )?))),
            biwac_parser::Stmt::Block(stmts) => Ok(Self::Block(
                stmts
                    .into_iter()
                    .map(|stmt| Stmt::try_resolve(stmt, imports, modpath))
                    .collect::<Result<Vec<Stmt>, ResolveError>>()?,
            )),
            biwac_parser::Stmt::Expr(expr) => {
                Ok(Self::Expr(Exprs::try_resolve(expr, imports, modpath)?))
            }
            biwac_parser::Stmt::Return(expr) => {
                Ok(Self::Return(Exprs::try_resolve(expr, imports, modpath)?))
            }
            biwac_parser::Stmt::VarDec(var) => {
                Ok(Self::VarDec(VarDec::try_resolve(var, imports, modpath)?))
            }
            biwac_parser::Stmt::Assign(dst, src) => Ok(Self::Assign(
                Primary::try_resolve(dst, imports, modpath)?,
                Exprs::try_resolve(src, imports, modpath)?,
            )),
        }
    }
}

impl TryResolve<biwac_parser::IfStmt> for IfStmt {
    fn try_resolve(
        value: biwac_parser::IfStmt,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        Ok(Self {
            cond: Exprs::try_resolve(value.cond, imports, modpath)?,
            then: value
                .then
                .into_iter()
                .map(|stmt| Stmt::try_resolve(stmt, imports, modpath))
                .collect::<Result<Vec<Stmt>, ResolveError>>()?,
            els: value
                .els
                .map(|els| {
                    els.into_iter()
                        .map(|stmt| Stmt::try_resolve(stmt, imports, modpath))
                        .collect()
                })
                .transpose()?,
        })
    }
}

impl TryResolve<biwac_parser::WhileStmt> for WhileStmt {
    fn try_resolve(
        value: biwac_parser::WhileStmt,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        Ok(Self {
            cond: Exprs::try_resolve(value.cond, imports, modpath)?,
            stmts: value
                .stmts
                .into_iter()
                .map(|stmt| Stmt::try_resolve(stmt, imports, modpath))
                .collect::<Result<Vec<Stmt>, ResolveError>>()?,
        })
    }
}

impl TryResolve<biwac_parser::VarDec> for VarDec {
    fn try_resolve(
        value: biwac_parser::VarDec,
        imports: &[biwac_parser::QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        Ok(Self {
            typ: match value.typ {
                TypDecl::Typ(typ) => Some(Typ::try_resolve(typ, imports, modpath)?),
                TypDecl::Any => None,
            },
            id: value.name,
            init: Exprs::try_resolve(value.init, imports, modpath)?,
        })
    }
}
