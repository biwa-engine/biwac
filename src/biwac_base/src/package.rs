use colored::Colorize;
use std::{fmt::Display, str::FromStr};

#[derive(Debug)]
pub struct MetadataHolder {
    pub metadata: PackageMetadata,
    pub src: String, // metadata file string content
}

/// パッケージが実行可能 (playable) かライブラリか。
///
/// `main.biwa` があれば playable、`lib.biwa` があれば library。
/// playable package はエントリポイント (`scene main`) を持たなければならない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Lib,
    /// Rust でいう binary package。ゲームとして遊べる成果物になる。
    Bin,
}

impl PackageKind {
    pub fn is_playable(&self) -> bool {
        matches!(self, Self::Bin)
    }
}

#[derive(Debug)]
pub struct PackageMetadata {
    pub name: PackageName,
    pub version: PackageVersion,
    pub description: Option<String>,
    pub dependencies: Vec<DependedPackage>,

    /// このパッケージが std に依存しないこと。
    ///
    /// std 自身は自分に依存できないため std は真になる。
    /// 真のパッケージは lang item の提供元となるため、
    /// ビルド時に全 lang item が揃っていることを検証する。
    pub no_std: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PackageName(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// 32 bit package id (simple increment).
/// This is unique in global scope (inter-package) and inter-session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackageId(u32);

impl PackageId {
    pub const SELF_PACKAGE: PackageId = PackageId(0);

    pub const BUILTIN_RESERVED_PACKAGE: PackageId = PackageId(1);

    pub const UNRESERVED_PACKAGE_MIN: u32 = 2;

    #[inline]
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    #[inline]
    pub fn is_self(&self) -> bool {
        self == &Self::SELF_PACKAGE
    }

    #[inline]
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl FromStr for PackageName {
    type Err = PackageNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 前後を `^`/`$` で固定しないと部分一致になり、
        // 例えば `"Invalid-Name!"` も (中の `"nvalid"` にマッチして) 通ってしまう。
        let re = regex::Regex::new("^[a-z][0-9a-z_]*$").unwrap();
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

impl Display for PackageName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PackageNameError {
    pub fn print_error_message(&self) {
        match self {
            Self::InvalidPackageName(name) => {
                println!(
                    r#"{} Invalid package name: `{name}`
Package name must be `[a-z][0-9a-z_]*`"#,
                    "Error:".red()
                )
            }
        }
    }
}

impl Display for PackageNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPackageName(name) => {
                write!(
                    f,
                    "Invalid package name: `{name}`. Package name must be `[a-z][0-9a-z_]*`",
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
    pub fn new(major: usize, minor: usize, patch: usize) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn major(&self) -> usize {
        self.major
    }

    pub fn minor(&self) -> usize {
        self.minor
    }

    pub fn patch(&self) -> usize {
        self.patch
    }

    /// `min..max` (max は inclusive、無指定なら上限無し) の範囲に収まるか。
    pub fn in_range(&self, min: &PackageVersion, max: Option<&PackageVersion>) -> bool {
        self >= min && max.is_none_or(|max| self <= max)
    }
}

impl Display for PackageVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl PackageVersionError {
    pub fn print_error_message(&self) {
        match self {
            Self::InvalidPackageVersion(version) => {
                println!(
                    r#"{} Invalid package version: `{version}`
Package version must be `[0-9].[0-9].[0-9]`"#,
                    "Error:".red()
                )
            }
        }
    }
}

impl Display for PackageVersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPackageVersion(version) => {
                write!(
                    f,
                    "Invalid package version: `{version}`. Package version must be `[0-9].[0-9].[0-9]`",
                )
            }
        }
    }
}
