use crate::{
    packager::resolver::{
        symbols::expressions::{Exprs, Primary},
        ResolveError, TryResolve,
    },
    parser,
    validator::types::AbsoluteType,
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
    pub typ: AbsoluteType,
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

impl TryResolve<parser::symbols::statements::Stmt> for Stmt {
    fn try_resolve(
        value: parser::symbols::statements::Stmt,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
    ) -> Result<Self, crate::packager::resolver::ResolveError> {
        match value {
            parser::symbols::statements::Stmt::If(i) => Ok(Self::If(Box::new(
                IfStmt::try_resolve(*i, imports, modpath)?,
            ))),
            parser::symbols::statements::Stmt::While(w) => Ok(Self::While(Box::new(
                WhileStmt::try_resolve(*w, imports, modpath)?,
            ))),
            parser::symbols::statements::Stmt::Block(stmts) => Ok(Self::Block(
                stmts
                    .into_iter()
                    .map(|stmt| Stmt::try_resolve(stmt, imports, modpath))
                    .collect::<Result<Vec<Stmt>, ResolveError>>()?,
            )),
            parser::symbols::statements::Stmt::Expr(expr) => {
                Ok(Self::Expr(Exprs::try_resolve(expr, imports, modpath)?))
            }
            parser::symbols::statements::Stmt::Return(expr) => {
                Ok(Self::Return(Exprs::try_resolve(expr, imports, modpath)?))
            }
            parser::symbols::statements::Stmt::VarDec(var) => {
                Ok(Self::VarDec(VarDec::try_resolve(var, imports, modpath)?))
            }
            parser::symbols::statements::Stmt::Assign(dst, src) => Ok(Self::Assign(
                Primary::try_resolve(dst, imports, modpath)?,
                Exprs::try_resolve(src, imports, modpath)?,
            )),
        }
    }
}

impl TryResolve<parser::symbols::statements::if_stmt::IfStmt> for IfStmt {
    fn try_resolve(
        value: parser::symbols::statements::if_stmt::IfStmt,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
    ) -> Result<Self, crate::packager::resolver::ResolveError> {
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

impl TryResolve<parser::symbols::statements::while_stmt::WhileStmt> for WhileStmt {
    fn try_resolve(
        value: parser::symbols::statements::while_stmt::WhileStmt,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
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

impl TryResolve<parser::symbols::statements::vardec::VarDec> for VarDec {
    fn try_resolve(
        value: parser::symbols::statements::vardec::VarDec,
        imports: &[parser::symbols::QualifiedId],
        modpath: &crate::packager::ModulePath,
    ) -> Result<Self, ResolveError> {
        Ok(Self {
            typ: AbsoluteType::try_resolve(value.typ, imports, modpath)?,
            id: value.name,
            init: Exprs::try_resolve(value.init, imports, modpath)?,
        })
    }
}
