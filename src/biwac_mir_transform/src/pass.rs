use std::fmt;

use biwac_mir::{Body, Mir, MirItem, ValidationError};

/// 関数 1 つ分に掛ける変換。
pub trait MirPass {
    fn name(&self) -> &'static str;

    fn run(&self, body: &mut Body);
}

/// パスが MIR を壊した。
#[derive(Debug)]
pub struct PassError {
    pub pass: &'static str,
    pub errors: Vec<ValidationError>,
}

impl fmt::Display for PassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "MIR pass `{}` produced invalid MIR:", self.pass)?;
        for e in &self.errors {
            writeln!(f, "  {e}")?;
        }
        Ok(())
    }
}

/// パスを順に走らせる。
///
/// 各パスの後に不変条件を検査する。
/// 壊れているのはコンパイラのバグなので、どのパスが壊したかを添えて返す。
pub fn run_passes(mir: &mut Mir, passes: &[&dyn MirPass]) -> Result<(), PassError> {
    for pass in passes {
        for item in mir.items.values_mut() {
            if let MirItem::Body(body) = item {
                pass.run(body);
            }
        }

        let errors = biwac_mir::validate(mir);
        if !errors.is_empty() {
            return Err(PassError {
                pass: pass.name(),
                errors,
            });
        }
    }
    Ok(())
}

/// `.biwamir` を書く前に掛けるパス列。
///
/// 今は畳むだけである。定数伝播やインライン化はまだ入れていない。
pub fn default_passes() -> &'static [&'static dyn MirPass] {
    &[&crate::SimplifyCfg]
}
