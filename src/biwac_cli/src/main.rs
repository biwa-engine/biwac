use std::{path::Path, process::exit};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// package path (optional)
    #[arg(short, long)]
    package_path: Option<String>,

    /// Rebuild every package, ignoring cached build results.
    ///
    /// 差分ビルドは前回のフィンガープリントとの突き合わせで判定するので、
    /// キャッシュを疑ったときの逃げ道として用意しておく。
    #[arg(short, long)]
    rebuild: bool,

    /// Emit the given intermediate representation instead of the default output.
    ///
    /// rustc と同じく、既定の出力を置き換える。
    /// 中間表現は成果物ではなくキャッシュもされないので、
    /// 指定すると鮮度に関わらず建て直しになる。
    #[arg(long, value_enum)]
    emit: Option<EmitKind>,

    /// Code generation target.
    ///
    /// 選べるのはこのコンパイラが生成できるターゲット
    /// (cargo feature で決まる) のうちの 1 つである。
    /// 候補が 1 つしか無ければ省略できる。
    #[arg(long)]
    target: Option<String>,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum EmitKind {
    /// MIR のテキスト表現を `<build dir>/<target>/<package>.biwamir` に書き出す。
    Mir,
}

/// `--target` を解決する。
///
/// 候補が 1 つしか無ければ省略できる。
/// 複数あるのに省略されたら、どれを作りたいのか決められないのでエラーにする。
fn resolve_target(requested: Option<&str>) -> Result<biwac_base::Target, String> {
    let available = biwac_generator::available_targets();

    match requested {
        Some(name) => match biwac_base::Target::from_name(name) {
            Some(t) if available.contains(&t) => Ok(t),
            Some(t) => Err(format!(
                "target `{t}` is not available in this build of biwac (available: {})",
                biwac_base::describe_targets(&available)
            )),
            None => Err(format!(
                "unknown target `{name}` (available: {})",
                biwac_base::describe_targets(&available)
            )),
        },
        None => match available.as_slice() {
            [only] => Ok(*only),
            [] => Err("this build of biwac has no code generation target".to_string()),
            many => Err(format!(
                "--target is required because this build of biwac can produce {}",
                biwac_base::describe_targets(many)
            )),
        },
    }
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

    let target = match resolve_target(args.target.as_deref()) {
        Ok(t) => t,
        Err(message) => {
            eprintln!("Error: {message}");
            exit(1);
        }
    };

    let res = biwac_driver::compile(
        pkg_root_path.to_path_buf(),
        biwac_driver::BuildOptions {
            force_rebuild: args.rebuild,
            emit_mir: args.emit == Some(EmitKind::Mir),
            target,
        },
    );

    match res {
        Ok(_) => exit(0),
        Err(_) => exit(1),
    }
}
