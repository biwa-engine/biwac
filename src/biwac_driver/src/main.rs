use std::{env, path::Path};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        let pkg_root_path = args.get(1).unwrap();
        let pkg_root_path = Path::new(pkg_root_path);

        if !pkg_root_path.is_dir() {
            panic!(
                "Directory expected, but got file: `{}`",
                pkg_root_path
                    .as_os_str()
                    .to_str()
                    .expect("broken package root path")
            );
        }

        let metadata =
            biwac_metadata_loader::try_load_package_metadata(pkg_root_path.to_path_buf()).unwrap();

        println!("metadata: {metadata:#?}");

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

        let pkg = biwac_package_loader::Pkg::try_load(pkg_root_path.to_path_buf()).unwrap();
        // println!("pkg: {pkg:#?}");

        let deps =
            biwac_dependency_loader::try_load_dependencies(build_dir_path.to_path_buf()).unwrap();
        println!("deps: {deps:#?}");

        let hir = biwac_name_resolver::ResolveCtx::new(&metadata, &deps)
            .unwrap()
            .try_resolve(pkg)
            .unwrap();
        // println!("pkg: {pkg:#?}");

        let hir = biwac_type_inferrer::TyCtx::new(hir).infer().unwrap();
        // println!("pkg: {pkg:#?}");

        let bin = biwac_generator::arch::typescript::generate(&hir);

        biwac_driver::write_bin(build_dir_path.to_path_buf(), &metadata.name, &bin).unwrap();
    } else {
        panic!("1 Argument Required: <package-path>")
    }
}
