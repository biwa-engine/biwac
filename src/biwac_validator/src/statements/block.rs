use crate::validator::{statements::Stmt, Env, ValidateError};

pub fn validate(
    stmts: &Vec<crate::packager::resolver::symbols::statements::Stmt>,
    env: &mut Env,
) -> Result<Vec<Stmt>, ValidateError> {
    env.begin_scope();

    let stmts = stmts
        .iter()
        .map(|stmt| stmt.validate(env))
        .collect::<Result<Vec<Stmt>, ValidateError>>()?;

    env.end_scope();

    Ok(stmts)
}
