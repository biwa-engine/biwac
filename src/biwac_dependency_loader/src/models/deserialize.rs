use std::str::FromStr;

use biwac_base::{
    BiwacError, ModPath, PackageName, PackageNameError, PackageVersion, PackageVersionError,
};

use serde::Deserialize;

use crate::{DepsPkgInternalId, DepsSymId};

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

pub(super) fn deserialize_version<'de, D>(deserializer: D) -> Result<PackageVersion, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse()
        .map_err(|e: PackageVersionError| serde::de::Error::custom(e.error_message()))
}

pub(super) fn deserialize_package_name<'de, D>(deserializer: D) -> Result<PackageName, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse()
        .map_err(|e: PackageNameError| serde::de::Error::custom(e.error_message()))
}
