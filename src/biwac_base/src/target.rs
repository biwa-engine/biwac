//! コード生成のターゲット。
//!
//! この表がターゲットに関する語彙の唯一の情報源である。
//! `[[native(arch = "...")]]` の arch 名も、成果物の置き場所も、ここから引く。
//!
//! [`Target::ALL`] は **cargo feature に関係なく全変種**を持つ。
//! feature で絞られるのは「このビルドのコンパイラが生成できるか」であって、
//! 「そのターゲットが存在するか」ではないためである。
//! `typescript` feature だけでビルドしたコンパイラでも、
//! std に書かれた `arch = "wasm"` は「知らない arch」ではなく
//! 「このビルドでは使わない arch」でなければならない。
//!
//! 生成できるターゲットの一覧は `biwac_generator::available_targets` が返す。

use std::fmt;

macro_rules! target_table {
    ( $( $variant:ident, $name:literal, $ext:literal ; )* ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Target {
            $($variant,)*
        }

        impl Target {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            /// `--target` に渡す名前であり、`[[native(arch = "...")]]` の arch 名でもある。
            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }

            /// 成果物の拡張子。
            pub fn bin_extension(&self) -> &'static str {
                match self {
                    $(Self::$variant => $ext,)*
                }
            }
        }
    };
}

target_table!(
    // ブラウザで動くノベルゲームエンジンに載る形。
    // 生成物はパッケージごとの `.ts` で、依存の `.ts` を隣に並べて import する。
    TypeScript, "typescript", "ts";

    // VM や別プロセスに隔離して動かす形。
    // エンジンの機能は同期的なホスト関数の呼び出しで使う。
    // 単相化がプログラム全体の操作なので、生成物は playable パッケージに 1 つだけ出る。
    Wasm, "wasm", "wasm";
);

impl Target {
    /// このターゲットの成果物と中間生成物を入れるディレクトリ名。
    ///
    /// `arch` の切り落としでシンボルの集合がターゲットごとに変わるので、
    /// `.biwameta` も `.biwamir` もターゲット依存である。
    /// ターゲットを切り替えても互いのキャッシュを壊さないよう、
    /// すべてこの下に入れる。
    pub fn build_subdir(&self) -> &'static str {
        self.name()
    }

    /// ライブラリパッケージがこのターゲットで成果物を持つか。
    ///
    /// wasm は単相化を通す。単相化はプログラム全体の操作で、
    /// 根 (`scene main`) を持つのは playable パッケージだけなので、
    /// ライブラリからは何も出ない。
    pub fn library_produces_binary(&self) -> bool {
        match self {
            Self::TypeScript => true,
            Self::Wasm => false,
        }
    }

    /// 依存パッケージの成果物を自分の出力ディレクトリに集める必要があるか。
    ///
    /// TypeScript は import が `./<package>.ts` を指すので要る。
    /// wasm は単相化で 1 つにまとまるので要らない。
    pub fn collects_dependency_binaries(&self) -> bool {
        match self {
            Self::TypeScript => true,
            Self::Wasm => false,
        }
    }

    /// 依存パッケージの **関数の本体** を自分の出力に取り込むか。
    ///
    /// 取り込むターゲットでは、依存の `.biwamir` が変われば建て直さなければならない。
    /// TypeScript は `greeter.ts` に std の本体を入れないので要らない。
    pub fn consumes_dependency_mir(&self) -> bool {
        match self {
            Self::TypeScript => false,
            Self::Wasm => true,
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// 選択肢を人に見せる形に並べる。
pub fn describe_targets(targets: &[Target]) -> String {
    targets
        .iter()
        .map(|t| format!("`{}`", t.name()))
        .collect::<Vec<_>>()
        .join(", ")
}
