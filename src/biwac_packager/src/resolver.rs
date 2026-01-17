pub(crate) mod symbols;

use std::collections::HashMap;

use crate::{
    packager::{resolver::symbols::ModuleSymbols, ModulePath},
    parser::{
        symbols::{globals::Globals, ModAst, QualifiedId},
        types::Type,
    },
    validator::{types::AbsoluteType, AbsoluteId},
};

trait TryResolve<T>: Sized {
    fn try_resolve(
        value: T,
        imports: &[QualifiedId],
        modpath: &ModulePath,
    ) -> Result<Self, ResolveError>;
}

trait TryResolveWithId<T>: Sized {
    fn try_resolve(
        value: T,
        imports: &[QualifiedId],
        modpath: &ModulePath,
    ) -> Result<(AbsoluteId, Self), ResolveError>;
}

#[derive(Debug)]
pub enum ResolveError {
    IdentifierNotFound(QualifiedId),
}

impl TryResolve<Type> for AbsoluteType {
    fn try_resolve(
        value: Type,
        imports: &[QualifiedId],
        modpath: &ModulePath,
    ) -> Result<Self, ResolveError> {
        match value {
            Type::Primitive(p) => Ok(AbsoluteType::Primitive(p)),
            Type::List(t) => Ok(AbsoluteType::List(Box::new(Self::try_resolve(
                *t, imports, modpath,
            )?))),
            Type::Defined(qualid) => Ok(AbsoluteType::Defined(AbsoluteId::try_resolve(
                qualid, imports, modpath,
            )?)),
        }
    }
}

pub(super) fn try_resolve_imports(
    module: ModAst,
    modpath: &ModulePath,
) -> Result<HashMap<AbsoluteId, ModuleSymbols>, ResolveError> {
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
        .map(|g| ModuleSymbols::try_resolve(g, &imports, modpath))
        .flat_map(|g| g.transpose())
        .collect::<Result<HashMap<AbsoluteId, ModuleSymbols>, ResolveError>>()
}

impl AbsoluteId {
    fn try_resolve(
        qualid: QualifiedId,
        imports: &[QualifiedId],
        modpath: &ModulePath,
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
                    let quals: Vec<String> = [modpath.0.clone(), i.quals.clone()].concat();

                    Ok(Self::new(quals, qualid.id))
                }
            } else {
                // NOTE: ファイルローカルなシンボルの解決
                // なお、存在チェックは行われないことに注意
                Ok(Self::new(modpath.0.clone(), qualid.id))
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
                    [modpath.0.clone(), i.quals.clone(), qualid.quals].concat();

                Ok(Self::new(quals, qualid.id))
            }
        } else {
            Err(ResolveError::IdentifierNotFound(qualid))
        }
    }
}
