use colored::Colorize;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

use biwac_base::{IdentInterner, PackageName, SourceHolder};

pub fn compile(pkg_root_path: PathBuf) -> Result<(), ()> {
    println!("{}", "Compiling...".green().bold(),);

    // 空のソースファイルリストを作成
    let mut srcs = SourceHolder::default();
    // 空のインターンプールを生成
    let mut interner = IdentInterner::new();

    let metadata = biwac_metadata_loader::try_load_package_metadata(pkg_root_path.clone())
        .map_err(|e| {
            e.print_error_message();
            biwac_base::print_error_finish_message(1);
        })?;
    let package_name_interned = interner.get_or_insert(metadata.metadata.name.value());

    println!(
        "Package: {} v{}.{}.{}",
        metadata.metadata.name.value(),
        metadata.metadata.version.major(),
        metadata.metadata.version.minor(),
        metadata.metadata.version.patch()
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

    let pkg = biwac_package_loader::Pkg::try_load(
        &metadata,
        &mut interner,
        &mut srcs,
        pkg_root_path.to_path_buf(),
    )
    .map_err(|e| e.print_error_messages())?;
    // println!("pkg: {pkg:#?}");

    let deps =
        biwac_dependency_loader::try_load_dependencies(build_dir_path.to_path_buf()).unwrap();

    let hir = biwac_name_resolver::NameResolver::new(&metadata, &deps, package_name_interned, pkg)
        .unwrap()
        .try_resolve()
        .unwrap();
    // println!("pkg: {pkg:#?}");

    let hir = biwac_type_inferrer::TyCtx::new(hir).infer().unwrap();
    // println!("pkg: {pkg:#?}");

    let bin = biwac_generator::arch::typescript::generate(&hir, &interner, &srcs);

    write_bin(build_dir_path.to_path_buf(), &metadata.metadata.name, &bin).unwrap();

    println!("{}", "Finished!".green().bold(),);

    Ok(())
}

// build_dir_path はdirであることが保証されている必要がある
fn write_bin(
    build_dir_path: PathBuf,
    pkg_name: &PackageName,
    bin: &str,
) -> Result<(), std::io::Error> {
    if cfg!(feature = "typescript") {
        let dstpath = build_dir_path.join(Path::new("typescript"));
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
