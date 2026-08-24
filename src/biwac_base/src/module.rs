use crate::BIWA_EXTENSION;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModPath {
    Main,
    Lib,
    Mod(Vec<String>),
}

impl ModPath {
    pub fn file_name(&self) -> String {
        match self {
            Self::Main => format!("main.{BIWA_EXTENSION}"),
            Self::Lib => format!("lib.{BIWA_EXTENSION}"),
            Self::Mod(path) => format!("{}.{BIWA_EXTENSION}", path.join("/")),
        }
    }

    /// [`Self::file_name`] の逆変換。
    ///
    /// 依存パッケージのメタデータはモジュールをファイル名で持つため、
    /// そこから ModPath を復元するのに使う。
    pub fn from_file_name(file_name: &str) -> Option<Self> {
        let stem = file_name.strip_suffix(&format!(".{BIWA_EXTENSION}"))?;

        match stem {
            "main" => Some(Self::Main),
            "lib" => Some(Self::Lib),
            _ => Some(Self::Mod(stem.split('/').map(|s| s.to_string()).collect())),
        }
    }

    pub fn push(self, child: String) -> Self {
        match self {
            Self::Main => Self::Mod([child].into()),
            Self::Lib => Self::Mod([child].into()),
            Self::Mod(mut m) => {
                m.push(child);

                Self::Mod(m)
            }
        }
    }

    pub fn extend(self, path: Vec<String>) -> Self {
        match self {
            Self::Main => Self::Mod(path),
            Self::Lib => Self::Mod(path),
            Self::Mod(mut m) => {
                m.extend(path);

                Self::Mod(m)
            }
        }
    }
}

impl From<ModPath> for Vec<String> {
    fn from(value: ModPath) -> Self {
        match value {
            ModPath::Main => vec![],
            ModPath::Lib => vec![],
            ModPath::Mod(m) => m,
        }
    }
}
