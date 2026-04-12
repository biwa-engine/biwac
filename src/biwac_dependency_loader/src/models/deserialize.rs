use biwac_base::{BiwacError, PackageName, PackageNameError, PackageVersion, PackageVersionError};

use serde::Deserialize;

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
