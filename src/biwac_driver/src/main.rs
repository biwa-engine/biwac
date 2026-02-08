use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        let rootpath = args.get(1).unwrap();
        let pkg = biwac_package_loader::Pkg::try_load(rootpath).unwrap();
        // println!("pkg: {pkg:#?}");

        let pkg = biwac_name_resolver::PkgSymMap::try_resolve_symbol(pkg).unwrap();
        // println!("pkg: {pkg:#?}");

        let pkg = biwac_validator::Pkg::try_validate(pkg).unwrap();
        // println!("pkg: {pkg:#?}");

        biwac_generator::arch::biwasm::generate(&pkg);
    } else {
        panic!("1 Argument Required: <package-path>")
    }
}
