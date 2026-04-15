use colored::Colorize;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use biwac_base::{BiwacError, MetadataHolder, PackageName, SourceHolder};

pub fn compile(pkg_root_path: PathBuf) {
    // 空のソースファイルリストを作成
    let mut srcs = SourceHolder::default();
    // 空のメタデータを作成
    let mut metadata = MetadataHolder::default();

    match biwac_metadata_loader::try_load_package_metadata(&mut metadata, pkg_root_path.clone()) {
        Ok(metadata) => metadata,
        Err(e) => {
            e.print_error_message(&metadata, &srcs);
            panic!()
        }
    };

    let meta = metadata.metadata.as_ref().unwrap();
    println!(
        "{} {} v{}.{}.{}",
        "Compiling".green().bold(),
        meta.name.value(),
        meta.version.major(),
        meta.version.minor(),
        meta.version.patch()
    );

    // build directory preparation
    let build_dir_path = pkg_root_path.join(Path::new(biwac_base::BIWA_BUILD_DIRECTORY_NAME));
    if !build_dir_path.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .create(build_dir_path.clone())
            .unwrap();
    } else if !build_dir_path.is_dir() {
        panic!(
            "Destination directory broken, conflicted file found: `{}`",
            build_dir_path
                .as_os_str()
                .to_str()
                .expect("broken build directory path")
        );
    }

    let pkg = match biwac_package_loader::Pkg::try_load(
        &metadata,
        &mut srcs,
        pkg_root_path.to_path_buf(),
    ) {
        Ok(pkg) => pkg,
        Err(e) => {
            e.panic_with_error_messages();
        }
    };
    // println!("pkg: {pkg:#?}");

    let deps =
        biwac_dependency_loader::try_load_dependencies(build_dir_path.to_path_buf()).unwrap();

    let hir = biwac_name_resolver::ResolveCtx::new(&metadata, &deps)
        .unwrap()
        .try_resolve(pkg)
        .unwrap();
    // println!("pkg: {pkg:#?}");

    let hir = biwac_type_inferrer::TyCtx::new(hir).infer().unwrap();
    // println!("pkg: {pkg:#?}");

    let bin = biwac_generator::arch::typescript::generate(&hir);

    write_bin(build_dir_path.to_path_buf(), &meta.name, &bin).unwrap();

    println!("{}", "Finished!".green().bold(),);
}

// build_dir_path はdirであることが保証されている必要がある
fn write_bin(
    build_dir_path: PathBuf,
    pkg_name: &PackageName,
    bin: &str,
) -> Result<(), std::io::Error> {
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

        let binpath = dstpath.join(Path::new(&format!("{}.ts", pkg_name.value())));

        let mut f = std::fs::File::create(binpath).unwrap();

        f.write_all(bin.as_bytes())
    } else {
        todo!()
    }
}
