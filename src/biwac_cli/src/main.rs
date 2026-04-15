use std::{path::Path, process::exit};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// package path (optional)
    #[arg(short, long)]
    package_path: Option<String>,
}

fn main() {
    let args = Args::parse();

    let pkg_root_path = Path::new(args.package_path.as_deref().unwrap_or("."));

    if !pkg_root_path.is_dir() {
        panic!(
            "Directory expected, but got file: `{}`",
            pkg_root_path
                .as_os_str()
                .to_str()
                .expect("broken package root path")
        );
    }

    let res = biwac_driver::compile(pkg_root_path.to_path_buf());

    match res {
        Ok(_) => exit(0),
        Err(_) => exit(1),
    }
}
