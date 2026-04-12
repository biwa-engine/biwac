mod convert;
mod deserialize;

use biwac_base::{BiwacError, ModPath, PackageName, PackageVersion};

use serde::Deserialize;
use std::str::FromStr;

use deserialize::{deserialize_package_name, deserialize_version};

#[derive(Debug, Deserialize)]
pub struct Deps {
    #[serde(rename = "dependencies")]
    pub deps_pkgs: Vec<DepsPkg>,
}

#[derive(Debug, Deserialize)]
pub struct DepsPkg {
    #[serde(deserialize_with = "deserialize_package_name")]
    pub name: PackageName,

    #[serde(deserialize_with = "deserialize_version")]
    pub version: PackageVersion,

    pub symbols: Vec<DepsSymbol>,
}

#[derive(Debug, Deserialize)]
pub struct DepsSymbol {
    pub id: DepsPkgInternalId,
    pub body: DepsSymbolKind,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum DepsSymbolKind {
    #[serde(rename = "function")]
    Function(DepsFunction),
    #[serde(rename = "struct")]
    Struct(DepsStruct),
}

#[derive(Debug, Deserialize)]
pub struct DepsFunction {
    #[serde(skip_deserializing)]
    pub genargs: Vec<String>,

    pub args: Vec<DepsArgDecl>,

    pub ret: DepsTy,
}

#[derive(Debug, Deserialize)]
pub struct DepsArgDecl {
    pub id: String,

    #[serde(rename = "type")]
    pub ty: DepsTy,
}

#[derive(Debug, Deserialize)]
pub struct DepsStruct {
    #[serde(skip_deserializing)]
    pub genargs: Vec<String>,

    pub members: Vec<DepsStructMember>,
}

#[derive(Debug, Deserialize)]
pub struct DepsStructMember {
    pub id: String,

    #[serde(rename = "type")]
    pub ty: DepsTy,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum DepsTy {
    #[serde(rename = "int")]
    Int,

    #[serde(rename = "float")]
    Float,

    #[serde(rename = "bool")]
    Bool,

    #[serde(rename = "void")]
    Void,

    #[serde(rename = "defined")]
    Defined {
        id: DepsSymId,

        #[serde(skip_deserializing)]
        genargs: Vec<DepsTy>,
    },
}

#[derive(Debug)]
pub struct DepsSymId {
    pub pkg: PackageName,
    pub modu: ModPath,
    pub id: String,
}

#[derive(Debug)]
pub struct DepsPkgInternalId {
    pub modu: ModPath,
    pub id: String,
}

impl<'de> Deserialize<'de> for DepsSymId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DepsSymId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for DepsSymId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.split("::").collect();

        let re = regex::Regex::new("[a-zA-Z][0-9a-zA-Z_]*|_[0-9a-zA-Z_]+").unwrap();
        if parts.len() == 2 && re.is_match(parts[0]) && re.is_match(parts[1]) {
            Ok(Self {
                pkg: PackageName::from_str(parts[0]).map_err(|e| e.error_message())?,
                modu: ModPath::Lib,
                id: parts[1].to_string(),
            })
        } else if parts.len() > 2 && parts.iter().all(|pat| re.is_match(pat)) {
            Ok(Self {
                pkg: PackageName::from_str(parts[0]).map_err(|e| e.error_message())?,
                modu: ModPath::Mod(
                    parts[1..parts.len() - 1]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                ),
                id: parts.last().unwrap().to_string(),
            })
        } else {
            Err("ID must be `[a-z][0-9a-z_]*(::[a-zA-Z][0-9a-zA-Z_]*|_[0-9a-zA-Z_]+)+`".into())
        }
    }
}

impl<'de> Deserialize<'de> for DepsPkgInternalId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DepsPkgInternalId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for DepsPkgInternalId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with("::") {
            Err("ID must start with `::`".into())
        } else if s.len() < 3 {
            Err("ID must be `(::[a-zA-Z][0-9a-zA-Z_]*|_[0-9a-zA-Z_]+)+`".into())
        } else {
            let parts: Vec<_> = s[2..].split("::").collect();

            let re = regex::Regex::new("[a-zA-Z][0-9a-zA-Z_]*|_[0-9a-zA-Z_]+").unwrap();
            if parts.len() == 1 && re.is_match(parts[0]) {
                Ok(Self {
                    modu: ModPath::Lib,
                    id: parts[0].to_string(),
                })
            } else if parts.iter().all(|pat| re.is_match(pat)) {
                Ok(Self {
                    modu: ModPath::Mod(
                        parts[..parts.len() - 1]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                    id: parts.last().unwrap().to_string(),
                })
            } else {
                Err("ID must be `(::[a-zA-Z][0-9a-zA-Z_]*|_[0-9a-zA-Z_]+)+`".into())
            }
        }
    }
}
