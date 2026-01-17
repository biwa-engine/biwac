use std::path::PathBuf;

pub struct ModSrc {
    path: PathBuf,
    src: String,
}

pub struct PkgSrc {
    mods: Vec<ModSrc>,
}
