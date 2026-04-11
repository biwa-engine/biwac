use std::str::FromStr;

use crate::BiwacError;

#[derive(Debug)]
pub struct PackageMetadata {
    pub name: PackageName,
    pub version: PackageVersion,
    pub description: Option<String>,
    pub dependencies: Vec<DependedPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageName(String);

#[derive(Debug)]
pub struct PackageVersion {
    major: usize,
    minor: usize,
    patch: usize,
}

#[derive(Debug)]
pub struct DependedPackage {
    pub name: PackageName,
    pub min_version: PackageVersion,
    pub max_version: Option<PackageVersion>,
}

#[derive(Debug)]
pub enum PackageNameError {
    InvalidPackageName(String),
}

#[derive(Debug)]
pub enum PackageVersionError {
    InvalidPackageVersion(String),
}

impl FromStr for PackageName {
    type Err = PackageNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = regex::Regex::new("[a-z][0-9a-z_]*").unwrap();
        if re.is_match(s) {
            Ok(Self(s.into()))
        } else {
            Err(PackageNameError::InvalidPackageName(s.into()))
        }
    }
}

impl PackageName {
    pub fn value(&self) -> &str {
        &self.0
    }
}

impl BiwacError for PackageNameError {
    fn error_message(&self) -> String {
        match self {
            Self::InvalidPackageName(name) => {
                format!(
                    r#"Invalid package name: `{name}`
Package name must be `[a-z][0-9a-z_]*`"#,
                )
            }
        }
    }
}

impl FromStr for PackageVersion {
    type Err = PackageVersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens: Vec<&str> = s.split(".").collect();

        if tokens.len() != 3 {
            Err(PackageVersionError::InvalidPackageVersion(s.to_string()))
        } else {
            let major = tokens[0]
                .parse()
                .map_err(|_| PackageVersionError::InvalidPackageVersion(s.to_string()))?;
            let minor = tokens[1]
                .parse()
                .map_err(|_| PackageVersionError::InvalidPackageVersion(s.to_string()))?;
            let patch = tokens[2]
                .parse()
                .map_err(|_| PackageVersionError::InvalidPackageVersion(s.to_string()))?;

            Ok(Self {
                major,
                minor,
                patch,
            })
        }
    }
}

impl PackageVersion {
    pub fn major(&self) -> usize {
        self.major
    }

    pub fn minor(&self) -> usize {
        self.minor
    }

    pub fn patch(&self) -> usize {
        self.patch
    }
}

impl BiwacError for PackageVersionError {
    fn error_message(&self) -> String {
        match self {
            Self::InvalidPackageVersion(version) => {
                format!(
                    r#"Invalid package version: `{version}`
Package version must be `[0-9].[0-9].[0-9]`"#,
                )
            }
        }
    }
}
