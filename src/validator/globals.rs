use crate::{
    packager::resolver::symbols::globals::FnDefContent,
    validator::{statements::Stmt, types::AbsoluteType, Env, ValidateError},
};

#[derive(Debug)]
pub struct Function {
    pub stmts: Vec<Stmt>,
    pub args: Vec<(AbsoluteType, String)>,
    pub rtype: Option<AbsoluteType>,
}
// 次のcodegenで、関数はただのラベルに続けてインストラクションを並べただけなのでいらない
// args: Vec<Type>,
// rtype: Type,

impl FnDefContent {
    pub fn validate(&self, env: &mut Env) -> Result<Function, ValidateError> {
        let stmts = self
            .stmts
            .iter()
            .map(|stmt| stmt.validate(env))
            .collect::<Result<Vec<Stmt>, ValidateError>>()?;

        if env.local.is_some() {
            Ok(Function {
                stmts,
                args: self.args.clone(),
                rtype: self.rtype.clone(),
            })
        } else {
            Err(ValidateError::OutOfScopes)
        }
    }
}
