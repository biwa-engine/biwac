pub(crate) mod symbols;
pub mod types;

use std::collections::HashMap;

use biwac_base::ModPath;
use biwac_parser::{Globals, ModAst, QualifiedId};

use crate::resolver::symbols::ModSym;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Callee {
    Var(String),
    Abs(AbsId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbsId {
    // pub package: enum Package { Internal, External(String)}
    pub quals: Vec<String>,
    pub id: String,
}

impl AbsId {
    pub fn new(quals: Vec<String>, id: String) -> Self {
        Self { quals, id }
    }
}

trait TryResolve<T>: Sized {
    fn try_resolve(
        value: T,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError>;
}

trait TryResolveWithId<T>: Sized {
    fn try_resolve(
        value: T,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<(AbsId, Self), ResolveError>;
}

#[derive(Debug)]
pub enum ResolveError {
    IdentifierNotFound(QualifiedId),
}

pub(super) fn try_resolve_imports(
    module: ModAst,
    modpath: &ModPath,
) -> Result<HashMap<AbsId, ModSym>, ResolveError> {
    let imports: Vec<QualifiedId> = module
        .globals
        .iter()
        .flat_map(|g| {
            if let Globals::Import(qualid) = g {
                Some(qualid.to_owned())
            } else {
                None
            }
        })
        .collect();

    module
        .globals
        .into_iter()
        .map(|g| ModSym::try_resolve(g, &imports, modpath))
        .flat_map(|g| g.transpose())
        .collect::<Result<HashMap<AbsId, ModSym>, ResolveError>>()
}

impl AbsId {
    fn try_resolve(
        qualid: QualifiedId,
        imports: &[QualifiedId],
        modpath: &ModPath,
    ) -> Result<Self, ResolveError> {
        if qualid.is_from_root {
            // `package::hoge::fuga` の場合、直ちにOk
            Ok(Self::new(qualid.quals, qualid.id))
        } else if qualid.quals.is_empty() {
            // `hoge` の場合
            if let Some(i) = imports.iter().find(|i| i.id == qualid.id) {
                // `import package::piyo::foo::hoge` の場合
                if i.is_from_root {
                    Ok(Self::new(qualid.quals, qualid.id))
                } else {
                    // `import piyo::foo::hoge` の場合

                    Ok(Self::new(
                        modpath.clone().extend(i.quals.clone()).into(),
                        qualid.id,
                    ))
                }
            } else {
                // NOTE: ファイルローカルなシンボルの解決
                // なお、存在チェックは行われないことに注意
                Ok(Self::new(modpath.clone().into(), qualid.id))
            }
        } else if let Some(i) = imports
            .iter()
            .find(|i| &i.id == qualid.quals.first().unwrap())
        {
            // `import hoge::fuga; fuga::piyo::foo` の場合

            if i.is_from_root {
                // `import package::hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> = [i.quals.clone(), qualid.quals].concat();

                Ok(Self::new(quals, qualid.id))
            } else {
                // `import hoge::fuga; fuga::piyo::foo` の場合
                let quals: Vec<String> =
                    [modpath.clone().into(), i.quals.clone(), qualid.quals].concat();

                Ok(Self::new(quals, qualid.id))
            }
        } else {
            Err(ResolveError::IdentifierNotFound(qualid))
        }
    }
}
