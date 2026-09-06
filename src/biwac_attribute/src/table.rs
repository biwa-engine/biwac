use crate::Target;

/// 属性が取れる形態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrShape {
    /// `[[foo]]`
    Word,

    /// `[[foo = "bar"]]`
    Value(AttrValueKind),

    /// `[[foo(a = 1, b)]]`
    ///
    /// 各要素は `(キー名, 値の種別)`。
    /// 値の種別が `None` のキーは値を取らない (`[[foo(bar)]]`)。
    List(&'static [(&'static str, Option<AttrValueKind>)]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrValueKind {
    Integer,
    String,
    Bool,
}

impl AttrValueKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::String => "string",
            Self::Bool => "boolean",
        }
    }

    pub fn of(val: &biwac_ast::AttrValue) -> Self {
        match val {
            biwac_ast::AttrValue::Integer(_) => Self::Integer,
            biwac_ast::AttrValue::String(_) => Self::String,
            biwac_ast::AttrValue::Bool(_) => Self::Bool,
        }
    }
}

// 既知の属性の一覧。
//
// rustc の `language_item_table!` と同じ流儀で、
// 属性名・形態・付与できる対象を 1 箇所に宣言する。
// この表が属性に関する唯一の情報源であり、
// biwac_ast (構文表現) も biwac_parser (パース) もこの知識を持たない。
macro_rules! attribute_table {
    ( $( $variant:ident, $name:literal, $shape:expr, $targets:expr ; )* ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum KnownAttr {
            $($variant,)*
        }

        pub mod attr_names {
            $(
                #[allow(non_upper_case_globals)]
                pub const $variant: &str = $name;
            )*
        }

        impl KnownAttr {
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }

            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            pub fn shape(&self) -> AttrShape {
                match self {
                    $(Self::$variant => $shape,)*
                }
            }

            /// この属性を付与できる対象の一覧。
            pub fn targets(&self) -> &'static [Target] {
                match self {
                    $(Self::$variant => $targets,)*
                }
            }

            pub fn accepts(&self, target: Target) -> bool {
                self.targets().contains(&target)
            }

            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];
        }
    };
}

attribute_table!(
    // ネイティブ実装。
    //
    // これは文法自体に影響する唯一の属性である。
    // parser は [[native]] の有無で `{{ ... }}` ネイティブボディを期待するかを決めるため、
    // 検証時点では既に Native* 系の AST ノードとして構築されている。
    // したがって付与対象は Native* 系のみが正当となる。
    //
    //  ```biwa
    //  [[native(arch="typescript")]]
    //  fn write(msg: String) {{ showMessage(msg); }}
    //  ```
    Native, "native",
        AttrShape::List(&[("arch", Some(AttrValueKind::String))]),
        &[
            Target::NativeFn,
            Target::NativeMethod,
            Target::NativeTypeAlias,
            Target::NativeCode,
        ];

    // lang item の宣言。
    //
    // キーの妥当性 (その文字列が実在する lang item か) と
    // 付与対象の種別・ジェネリクス個数の整合は
    // biwac_lang_item 側の回収パスが検証する。
    // ここでは「文字列値を 1 つ取る」形態と、
    // 定義に付けられていることだけを見る。
    //
    //  ```biwa
    //  [[lang="game"]]
    //  struct Game[C, S] { ... }
    //  ```
    Lang, "lang",
        AttrShape::Value(AttrValueKind::String),
        &[
            Target::Struct,
            Target::Enum,
            Target::TypeAlias,
            Target::NativeTypeAlias,
            Target::Fn,
            Target::NativeFn,
            Target::Method,
            Target::NativeMethod,
        ];
);
