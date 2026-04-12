mod models;

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

pub use crate::models::*;

const BIWA_BUILD_DEPENDENCY_LIST_FILE_NAME: &str = "biwa-dependencies.json";

#[derive(Debug)]
pub enum PkgDepsLoadError {
    DepsPathIsNotFile,
    InvalidDepsFile(String),
}

pub fn try_load_dependencies(build_dir_path: PathBuf) -> Result<Deps, PkgDepsLoadError> {
    let deps_path = build_dir_path.join(Path::new(BIWA_BUILD_DEPENDENCY_LIST_FILE_NAME));

    if !deps_path.is_file() {
        Err(PkgDepsLoadError::DepsPathIsNotFile)
    } else if !deps_path.exists() {
        // TODO:
        // fetch dependencies infromation from package registry
        todo!()
    } else {
        let mut f = File::open(deps_path.as_path()).unwrap();
        let mut contents = String::new();
        f.read_to_string(&mut contents).unwrap();

        serde_json::from_str(&contents)
            .map_err(|e| PkgDepsLoadError::InvalidDepsFile(e.to_string()))
    }
}
