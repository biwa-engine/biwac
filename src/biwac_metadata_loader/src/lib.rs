mod error;

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};

use biwac_base::{DependedPackage, MetadataHolder, PackageMetadata, PackageName, PackageVersion};
use serde::Deserialize;

pub use error::PkgMetadataLoadError;

// pkg_root_path はdirであることが保証されている必要がある
pub fn try_load_package_metadata(
    pkg_root_path: PathBuf,
) -> Result<MetadataHolder, PkgMetadataLoadError> {
    let metadata_path = pkg_root_path.join(Path::new(biwac_base::METADATA_FILE_NAME));

    if !metadata_path.exists() || !metadata_path.is_file() {
        Err(PkgMetadataLoadError::MetadataFileNotFound)
    } else {
        let mut f = File::open(metadata_path.as_path()).unwrap();
        let mut contents = String::new();
        f.read_to_string(&mut contents).unwrap();

        let m: PkgMetadata =
            serde_json::from_str(&contents).map_err(|e| PkgMetadataLoadError::InvalidFormat {
                err_msg: e.to_string(),
                line: e.line(),
                column: e.column(),
            })?;

        Ok(MetadataHolder {
            metadata: PackageMetadata {
                name: PackageName::from_str(&m.name)
                    .map_err(PkgMetadataLoadError::PackageNameError)?,
                version: PackageVersion::from_str(m.version.as_str())
                    .map_err(PkgMetadataLoadError::PackageVersionError)?,
                description: m.description,
                dependencies: m
                    .dependencies
                    .into_iter()
                    .map(|d| {
                        Ok(DependedPackage {
                            name: PackageName::from_str(&d.name)
                                .map_err(PkgMetadataLoadError::PackageNameError)?,
                            min_version: PackageVersion::from_str(d.version.min.as_str())
                                .map_err(PkgMetadataLoadError::PackageVersionError)?,
                            max_version: d
                                .version
                                .max
                                .map(|v| PackageVersion::from_str(v.as_str()))
                                .transpose()
                                .map_err(PkgMetadataLoadError::PackageVersionError)?,
                        })
                    })
                    .collect::<Result<_, _>>()?,
            },
            src: contents,
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
