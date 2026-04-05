use std::{io::Write, path::Path};

pub fn write_bin(rootpath: &str, bin: &str) -> Result<(), std::io::Error> {
    let root = Path::new(rootpath);

    if !root.is_dir() {
        panic!("Directory expected, but got file: `{rootpath}`");
    }

    if cfg!(feature = "typescript") {
        let dstpath = root
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
