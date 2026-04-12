mod convert;
mod deserialize;

use biwac_base::{ModPath, PackageName, PackageVersion};

use serde::Deserialize;

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
    #[serde(default)]
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
    #[serde(default)]
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

        #[serde(default)]
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
