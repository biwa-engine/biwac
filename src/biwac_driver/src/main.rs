use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        let rootpath = args.get(1).unwrap();
        let pkg = biwac_package_loader::Pkg::try_load(rootpath).unwrap();
        // println!("pkg: {pkg:#?}");

        let hir = biwac_name_resolver::ResolveCtx::new()
            .try_resolve(pkg)
            .unwrap();
        // println!("pkg: {pkg:#?}");

        let hir = biwac_type_inferrer::TyCtx::new(hir).infer().unwrap();
        // println!("pkg: {pkg:#?}");

        let bin = biwac_generator::arch::typescript::generate(&hir);

        biwac_driver::write_bin(rootpath, &bin).unwrap();
    } else {
        panic!("1 Argument Required: <package-path>")
    }
}
