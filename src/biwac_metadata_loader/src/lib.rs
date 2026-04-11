use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use biwac_base::{
    DependedPackage, PackageMetadata, PackageName, PackageNameError, PackageVersion,
    PackageVersionError,
};
use serde::Deserialize;

#[derive(Debug)]
pub enum PkgMetadataLoadError {
    MetadataFileNotFound,
    InvalidFormat(String),
    PackageNameError(PackageNameError),
    PackageVersionError(PackageVersionError),
}

// pkg_root_path はdirであることが保証されている必要がある
pub fn try_load_package_metadata(
    pkg_root_path: PathBuf,
) -> Result<PackageMetadata, PkgMetadataLoadError> {
    let metadata_path = pkg_root_path.join(Path::new("biwa-package.json"));

    if !metadata_path.exists() || !metadata_path.is_file() {
        Err(PkgMetadataLoadError::MetadataFileNotFound)
    } else {
        let mut f = File::open(metadata_path.as_path()).unwrap();
        let mut contents = String::new();
        f.read_to_string(&mut contents).unwrap();

        let metadata: PkgMetadata = serde_json::from_str(&contents)
            .map_err(|e| PkgMetadataLoadError::InvalidFormat(e.to_string()))?;

        Ok(PackageMetadata {
            name: PackageName::new(metadata.name)
                .map_err(PkgMetadataLoadError::PackageNameError)?,
            version: PackageVersion::try_from(metadata.version.as_str())
                .map_err(PkgMetadataLoadError::PackageVersionError)?,
            description: metadata.description,
            dependencies: metadata
                .dependencies
                .into_iter()
                .map(|d| {
                    Ok(DependedPackage {
                        name: PackageName::new(d.name)
                            .map_err(PkgMetadataLoadError::PackageNameError)?,
                        min_version: PackageVersion::try_from(d.version.min.as_str())
                            .map_err(PkgMetadataLoadError::PackageVersionError)?,
                        max_version: d
                            .version
                            .max
                            .map(|v| PackageVersion::try_from(v.as_str()))
                            .transpose()
                            .map_err(PkgMetadataLoadError::PackageVersionError)?,
                    })
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct PkgMetadata {
    name: String,
    version: String,
    description: Option<String>,
    dependencies: Vec<DependedPkg>,
}

#[derive(Debug, Deserialize)]
struct DependedPkg {
    name: String,
    version: DependedPkgVersion,
}

#[derive(Debug, Deserialize)]
struct DependedPkgVersion {
    min: String,
    max: Option<String>,
}
