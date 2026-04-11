use std::{
    io::Write,
    path::{Path, PathBuf},
};

// pkg_root_path はdirであることが保証されている必要がある
pub fn write_bin(pkg_root_path: PathBuf, bin: &str) -> Result<(), std::io::Error> {
    if cfg!(feature = "typescript") {
        let dstpath = pkg_root_path
            .join(Path::new(".biwa_build"))
            .join(Path::new("typescript"))
            .join(Path::new("src"))
            .join(Path::new("generated"));
        if !dstpath.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(dstpath.clone())
                .unwrap();
        } else if !dstpath.is_dir() {
            panic!("Destination directory broken, conflicted file found: `{dstpath:?}`");
        }

        let binpath = dstpath.join(Path::new("dst.ts"));

        let mut f = std::fs::File::create(binpath).unwrap();

        f.write_all(bin.as_bytes())
    } else {
        todo!()
    }
}
