use biwac_span::Span;

use crate::{AttrShape, AttrValueKind, KnownAttr, Target};

#[derive(Debug)]
pub enum AttrError {
    /// 一覧に無い属性名が書かれている。
    UnknownAttribute { name: String, span: Span },

    /// 属性の形態が一覧の宣言と一致しない。
    /// (`[[lang(write)]]` のようにリスト形式で書いた等)
    InvalidShape {
        attr: KnownAttr,
        expected: AttrShape,
        span: Span,
    },

    /// リスト形式の属性に一覧に無いキーが書かれている。
    UnknownKey {
        attr: KnownAttr,
        key: String,
        span: Span,
    },

    /// 値の型が一覧の宣言と一致しない。
    InvalidValueKind {
        attr: KnownAttr,
        key: Option<String>,
        expected: AttrValueKind,
        found: AttrValueKind,
        span: Span,
    },

    /// 値を取らないキーに値が書かれている。
    UnexpectedValue {
        attr: KnownAttr,
        key: String,
        span: Span,
    },

    /// 値を取るキーに値が書かれていない。
    MissingValue {
        attr: KnownAttr,
        key: String,
        span: Span,
    },

    /// 付与できない対象に属性が付いている。
    InvalidTarget {
        attr: KnownAttr,
        target: Target,
        span: Span,
    },

    /// 同一の属性が同じ定義に複数回付いている。
    DuplicatedAttribute {
        attr: KnownAttr,
        first: Span,
        span: Span,
    },

    /// ネイティブ定義に `[[native]]` が付いていない。
    ///
    /// parser は `{{ ... }}` ボディを見て Native* 系のノードを作るが、
    /// その判断自体が `[[native]]` の有無に基づくため、
    /// 本来この不整合は起こらない。
    /// 将来 parser を変更した際に静かに壊れないための不変条件チェックである。
    MissingNativeAttribute { target: Target, span: Span },
}

impl AttrError {
    pub fn span(&self) -> &Span {
        match self {
            Self::UnknownAttribute { span, .. }
            | Self::InvalidShape { span, .. }
            | Self::UnknownKey { span, .. }
            | Self::InvalidValueKind { span, .. }
            | Self::UnexpectedValue { span, .. }
            | Self::MissingValue { span, .. }
            | Self::InvalidTarget { span, .. }
            | Self::DuplicatedAttribute { span, .. }
            | Self::MissingNativeAttribute { span, .. } => span,
        }
    }

    /// 人間向けの一行メッセージ。
    ///
    /// 診断のレンダリング (ariadne によるソース抜粋付き表示) は
    /// 他のエラー系 crate と同様に未実装であり、
    /// 実装するまでの間はこのメッセージを使う。
    pub fn message(&self) -> String {
        match self {
            Self::UnknownAttribute { name, .. } => {
                let mut known: Vec<&str> = KnownAttr::ALL.iter().map(|a| a.name()).collect();
                known.sort_unstable();
                format!(
                    "unknown attribute `{}` (known attributes: {})",
                    name,
                    known.join(", ")
                )
            }
            Self::InvalidShape { attr, expected, .. } => format!(
                "attribute `{}` must be written as {}",
                attr.name(),
                shape_hint(attr.name(), expected)
            ),
            Self::UnknownKey { attr, key, .. } => {
                format!("attribute `{}` has no key `{}`", attr.name(), key)
            }
            Self::InvalidValueKind {
                attr,
                key,
                expected,
                found,
                ..
            } => match key {
                Some(key) => format!(
                    "key `{}` of attribute `{}` expects {}, found {}",
                    key,
                    attr.name(),
                    expected.name(),
                    found.name()
                ),
                None => format!(
                    "attribute `{}` expects {}, found {}",
                    attr.name(),
                    expected.name(),
                    found.name()
                ),
            },
            Self::UnexpectedValue { attr, key, .. } => format!(
                "key `{}` of attribute `{}` takes no value",
                key,
                attr.name()
            ),
            Self::MissingValue { attr, key, .. } => format!(
                "key `{}` of attribute `{}` requires a value",
                key,
                attr.name()
            ),
            Self::InvalidTarget { attr, target, .. } => format!(
                "attribute `{}` cannot be applied to {}",
                attr.name(),
                target.describe()
            ),
            Self::DuplicatedAttribute { attr, .. } => {
                format!("attribute `{}` is applied more than once", attr.name())
            }
            Self::MissingNativeAttribute { target, .. } => format!(
                "{} requires the `{}` attribute",
                target.describe(),
                crate::attr_names::Native
            ),
        }
    }
}

fn shape_hint(name: &str, shape: &AttrShape) -> String {
    match shape {
        AttrShape::Word => format!("`[[{}]]`", name),
        AttrShape::Value(kind) => format!("`[[{} = <{}>]]`", name, kind.name()),
        AttrShape::List(keys) => {
            let inner = keys
                .iter()
                .map(|(key, kind)| match kind {
                    Some(kind) => format!("{} = <{}>", key, kind.name()),
                    None => (*key).to_string(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("`[[{}({})]]`", name, inner)
        }
    }
}

impl biwac_base::BiwacError for AttrError {
    fn print_error_message(&self, _ctx: &biwac_base::ErrorContext) {
        // TODO: 他のエラー系 crate と同様、ariadne による
        //       ソース抜粋付きの診断表示は未実装。
        eprintln!("Error: {}", self.message());
    }
}
