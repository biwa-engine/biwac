use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() == 2 {
        if let Some(rootpath) = args.get(1) {
            let pkg = biwac_packager::load(rootpath);
            // println!("pkg: {pkg:#?}");

            let validated_pkg = match biwac_validator::validate(&pkg) {
                Ok(v) => v,
                Err(e) => e.panic_with_error_message(),
            };
            // println!("validated_pkg: {validated_pkg:#?}");

            biwac_generator::arch::biwasm::generate(&validated_pkg);
        }
    } else {
        panic!("1 Argument Required: <package-path>")
    }
}
