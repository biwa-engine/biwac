use std::{
    io::Write,
    path::{Path, PathBuf},
};

// build_dir_path はdirであることが保証されている必要がある
pub fn write_bin(build_dir_path: PathBuf, bin: &str) -> Result<(), std::io::Error> {
    if cfg!(feature = "typescript") {
        let dstpath = build_dir_path
            .join(Path::new("typescript"))
            .join(Path::new("src"))
            .join(Path::new("generated"));
        if !dstpath.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(dstpath.clone())
                .unwrap();
        } else if !dstpath.is_dir() {
            panic!(
                "Destination directory broken, conflicted file found: `{}`",
                dstpath
                    .as_os_str()
                    .to_str()
                    .expect("broken build directory path")
            );
        }

        let binpath = dstpath.join(Path::new("dst.ts"));

        let mut f = std::fs::File::create(binpath).unwrap();

        f.write_all(bin.as_bytes())
    } else {
        todo!()
    }
}
