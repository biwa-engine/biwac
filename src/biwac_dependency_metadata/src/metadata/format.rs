/// .biwameta ファイルのディスク上フォーマット定義
///
/// 物理レイアウト:
///   [MAGIC: 4B][version: u32 LE]
///   [sym_hdr_count: u32][DiskSymbolHeader; sym_hdr_count]
///   [root_sym_idx: u32]
///   [sym_body_total_bytes: u32][body_data: sym_body_total_bytes B]
///     └─ 各エントリ: [body_size: u32][body: body_size B]
///   [source_file_count: u32][DiskSourceInfo; source_file_count]
///   [string_table_total_bytes: u32][string_data: string_table_total_bytes B]
///     └─ 各文字列: null 終端 UTF-8
///   [lang_item_count: u32][DiskLangItem; lang_item_count]
///   [svh: u64]
///   [dep_svh_count: u32][DiskDepSvh; dep_svh_count]
///   [ext_sym_count: u32][DiskExternalSymbol; ext_sym_count]
use super::codec::{DiskDecode, DiskEncode, DiskVec, impl_u32_newtype_codec};
use crate::error::DepMetadataError;

pub const BIWAC_DEPENDENCY_METADATA_MAGIC: &[u8; 4] = b"bwmt";
pub const BIWAC_DEPENDENCY_METADATA_FORMAT_VERSION: u32 = 5;

// --- インデックス / オフセット型 ---

/// sym_body_data 先頭からのバイトオフセット (body_size プレフィックスを指す)
#[derive(Debug, Clone, Copy)]
pub struct DiskBodyOffset(pub u32);

/// string_table_data 先頭からのバイトオフセット
#[derive(Debug, Clone, Copy)]
pub struct DiskStringOffset(pub u32);

/// source_file_table 内のインデックス
#[derive(Debug, Clone, Copy)]
pub struct DiskFileIndex(pub u32);

/// sym_hdr_table 内のインデックス
#[derive(Debug, Clone, Copy)]
pub struct DiskSymbolIndex(pub u32);

impl_u32_newtype_codec!(DiskBodyOffset);
impl_u32_newtype_codec!(DiskStringOffset);
impl_u32_newtype_codec!(DiskFileIndex);
impl_u32_newtype_codec!(DiskSymbolIndex);

// --- DiskSymbolKind ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DiskSymbolKind {
    Mod = 0,
    Struct = 1,
    Fn = 2,
    NativeTypeAlias = 3,
}

impl TryFrom<u32> for DiskSymbolKind {
    type Error = DepMetadataError;
    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Mod),
            1 => Ok(Self::Struct),
            2 => Ok(Self::Fn),
            3 => Ok(Self::NativeTypeAlias),
            _ => Err(DepMetadataError::UnknownSymbolKind(v)),
        }
    }
}

// --- DiskVisibility ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DiskVisibility {
    Private = 0,
    SuperModulePublic = 1, // pub(super)
    PackagePublic = 2,     // pub(package)
    Public = 3,            // pub
}

impl TryFrom<u32> for DiskVisibility {
    type Error = DepMetadataError;
    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Private),
            1 => Ok(Self::SuperModulePublic),
            2 => Ok(Self::PackagePublic),
            3 => Ok(Self::Public),
            _ => Err(DepMetadataError::UnknownVisibility(v)),
        }
    }
}

// --- DiskSymbolHeader (固定長 12B) ---
//
// sym_hdr_table の各エントリ。固定長なので O(1) ランダムアクセス可能。

#[derive(Debug, Clone, Copy)]
pub struct DiskSymbolHeader {
    pub kind: u32,              // DiskSymbolKind として解釈
    pub vis: u32,               // DiskVisibility として解釈
    pub offset: DiskBodyOffset, // sym_body_data 内の body_size プレフィックスへのオフセット
}

impl DiskSymbolHeader {
    pub const BYTE_SIZE: usize = 12;

    pub fn kind(&self) -> Result<DiskSymbolKind, DepMetadataError> {
        DiskSymbolKind::try_from(self.kind)
    }

    pub fn vis(&self) -> Result<DiskVisibility, DepMetadataError> {
        DiskVisibility::try_from(self.vis)
    }
}

impl DiskDecode for DiskSymbolHeader {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        if bytes.len() < Self::BYTE_SIZE {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: Self::BYTE_SIZE,
                available: bytes.len(),
            });
        }
        Ok((
            Self {
                kind: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
                vis: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
                offset: DiskBodyOffset(u32::from_le_bytes(bytes[8..12].try_into().unwrap())),
            },
            Self::BYTE_SIZE,
        ))
    }
}

impl DiskEncode for DiskSymbolHeader {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.kind.encode(buf);
        self.vis.encode(buf);
        self.offset.encode(buf);
    }
}

// --- DiskSpan (固定長 12B) ---

#[derive(Debug, Clone, Copy)]
pub struct DiskSpan {
    pub file: DiskFileIndex,
    pub begin: u32,
    pub end: u32,
}

impl DiskSpan {
    pub const BYTE_SIZE: usize = 12;
}

impl DiskDecode for DiskSpan {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        if bytes.len() < Self::BYTE_SIZE {
            return Err(DepMetadataError::UnexpectedEnd {
                needed: Self::BYTE_SIZE,
                available: bytes.len(),
            });
        }
        Ok((
            Self {
                file: DiskFileIndex(u32::from_le_bytes(bytes[0..4].try_into().unwrap())),
                begin: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
                end: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            },
            Self::BYTE_SIZE,
        ))
    }
}

impl DiskEncode for DiskSpan {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.file.encode(buf);
        self.begin.encode(buf);
        self.end.encode(buf);
    }
}

// --- DiskTyKind ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DiskTyKind {
    Int = 0,
    Float = 1,
    Bool = 2,
    Void = 3,
    Defined = 4, // sym_id が参照先シンボルを指す
    Gen = 5,     // struct/type 定義のジェネリクス引数 (sym_id = 宣言シンボル)
    LocGen = 6,  // fn/impl のローカルジェネリクス引数 (sym_id = 宣言シンボル)
    Fn = 7,      // 関数型 (genargs はないが args + rty が続く)
    /// 依存パッケージで定義された型。
    ///
    /// `Defined` の `sym_id` が「このファイル内の」シンボルインデックスなのに対し、
    /// こちらの `sym_id` は **ext_sym_table のインデックス** である。
    /// そこから `(PackageId, DiskSymbolIndex)` を引く。
    ///
    /// `DiskTyHeader` は `sym_id` を u32 1 個しか持てず、
    /// `(パッケージ, シンボル)` の組を直接埋め込むと固定長 20B が崩れるため、
    /// 表を 1 段挟んでいる。同じ外部シンボルへの複数の参照が 1 エントリを共有できる利点もある。
    ExternalDefined = 8,
}

impl TryFrom<u32> for DiskTyKind {
    type Error = DepMetadataError;
    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Int),
            1 => Ok(Self::Float),
            2 => Ok(Self::Bool),
            3 => Ok(Self::Void),
            4 => Ok(Self::Defined),
            5 => Ok(Self::Gen),
            6 => Ok(Self::LocGen),
            7 => Ok(Self::Fn),
            8 => Ok(Self::ExternalDefined),
            _ => Err(DepMetadataError::UnknownTyKind(v)),
        }
    }
}

// --- DiskTyHeader (固定長 20B) ---

#[derive(Debug, Clone, Copy)]
pub struct DiskTyHeader {
    pub kind: u32, // DiskTyKind として解釈
    // 意味は kind ごとに異なる:
    //   Defined         → このファイル内のシンボルインデックス
    //   ExternalDefined → ext_sym_table のインデックス
    //   Gen / LocGen    → 宣言側の genargs 内での序数
    //   Fn              → 引数の数
    //   それ以外        → 0
    pub sym_id: DiskSymbolIndex,
    pub span: DiskSpan,
}

impl DiskTyHeader {
    pub const BYTE_SIZE: usize = 4 + 4 + DiskSpan::BYTE_SIZE; // 20

    pub fn kind(&self) -> Result<DiskTyKind, DepMetadataError> {
        DiskTyKind::try_from(self.kind)
    }
}

impl DiskDecode for DiskTyHeader {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (kind, n) = u32::decode(&bytes[pos..])?;
        pos += n;
        let (sym_id, n) = DiskSymbolIndex::decode(&bytes[pos..])?;
        pos += n;
        let (span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        Ok((Self { kind, sym_id, span }, pos))
    }
}

impl DiskEncode for DiskTyHeader {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.kind.encode(buf);
        self.sym_id.encode(buf);
        self.span.encode(buf);
    }
}

// --- DiskTy (可変長、再帰的) ---
//
// ディスク上レイアウト:
//   [DiskTyHeader (20B)]
//   [genargs_len: u32]
//   [DiskTy; genargs_len]  ← 再帰
//
// `DiskVec<DiskTy>` は使わず手動再帰デコードすることで
// size_of::<DiskTy>() への依存を完全に排除している。

#[derive(Debug, Clone)]
pub struct DiskTy {
    pub hdr: DiskTyHeader,
    pub genargs: Vec<DiskTy>,
}

impl DiskDecode for DiskTy {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (hdr, n) = DiskTyHeader::decode(&bytes[pos..])?;
        pos += n;
        let (genargs_len, n) = u32::decode(&bytes[pos..])?;
        pos += n;
        let mut genargs = Vec::with_capacity(genargs_len as usize);
        for _ in 0..genargs_len {
            let (ty, n) = DiskTy::decode(&bytes[pos..])?;
            pos += n;
            genargs.push(ty);
        }
        Ok((DiskTy { hdr, genargs }, pos))
    }
}

impl DiskEncode for DiskTy {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.hdr.encode(buf);
        (self.genargs.len() as u32).encode(buf);
        for ty in &self.genargs {
            ty.encode(buf);
        }
    }
}

// --- DiskGenArg (固定長 16B): ジェネリクス引数名と宣言位置 ---

#[derive(Debug, Clone, Copy)]
pub struct DiskGenArg {
    pub name: DiskStringOffset,
    pub name_span: DiskSpan,
}

impl DiskGenArg {
    pub const BYTE_SIZE: usize = 4 + DiskSpan::BYTE_SIZE; // 16
}

impl DiskDecode for DiskGenArg {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (name, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (name_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        Ok((Self { name, name_span }, pos))
    }
}

impl DiskEncode for DiskGenArg {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.name.encode(buf);
        self.name_span.encode(buf);
    }
}

// --- DiskArg (可変長): 関数引数 ---
//
// DiskTy が可変長のため #[repr(C)] を使わず、
// DiskDecode で順次フィールドを読む。

#[derive(Debug, Clone)]
pub struct DiskArg {
    pub name: DiskStringOffset,
    pub name_span: DiskSpan,
    pub ty: DiskTy,
}

impl DiskDecode for DiskArg {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (name, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (name_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (ty, n) = DiskTy::decode(&bytes[pos..])?;
        pos += n;
        Ok((
            Self {
                name,
                name_span,
                ty,
            },
            pos,
        ))
    }
}

impl DiskEncode for DiskArg {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.name.encode(buf);
        self.name_span.encode(buf);
        self.ty.encode(buf);
    }
}

// --- DiskStructMember (可変長) ---

#[derive(Debug, Clone)]
pub struct DiskStructMember {
    pub name: DiskStringOffset,
    pub name_span: DiskSpan,
    pub ty: DiskTy,
}

impl DiskDecode for DiskStructMember {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (name, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (name_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (ty, n) = DiskTy::decode(&bytes[pos..])?;
        pos += n;
        Ok((
            Self {
                name,
                name_span,
                ty,
            },
            pos,
        ))
    }
}

impl DiskEncode for DiskStructMember {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.name.encode(buf);
        self.name_span.encode(buf);
        self.ty.encode(buf);
    }
}

// --- シンボルボディ ---
//
// sym_body_data 内の各エントリ:
//   [body_size: u32][body bytes: body_size B]
//
// body bytes の内容はシンボル種別 (DiskSymbolKind) によって異なる。

// --- DiskFnData ---

#[derive(Debug)]
pub struct DiskFnData {
    pub name: DiskStringOffset,
    pub name_span: DiskSpan,
    pub def_raw_code: DiskStringOffset, // シグニチャ部分のソース文字列
    pub def_span: DiskSpan,             // def_raw_code のspan
    pub genargs: DiskVec<DiskGenArg>,
    pub args: DiskVec<DiskArg>,
    pub rty: DiskTy,
    /// impl の self 型。トップレベル関数なら空、関連関数・メソッドなら 1 要素。
    ///
    /// `impl Pair[Int, Int]` なら Defined(Pair) + genargs [Int, Int] になる。
    /// DiskTy はプリミティブも定義された型も表現できるので、
    /// 「どの型の impl か」と「impl の対象ジェネリック引数」をこれ 1 つで運べる。
    ///
    /// codegen のシンボル名マングリングと、
    /// 特殊化された impl のメソッド解決に使う。
    pub impl_self_ty: DiskVec<DiskTy>,
}

impl DiskDecode for DiskFnData {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (name, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (name_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (def_raw_code, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (def_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (genargs, n) = DiskVec::<DiskGenArg>::decode(&bytes[pos..])?;
        pos += n;
        let (args, n) = DiskVec::<DiskArg>::decode(&bytes[pos..])?;
        pos += n;
        let (rty, n) = DiskTy::decode(&bytes[pos..])?;
        pos += n;
        let (impl_self_ty, n) = DiskVec::<DiskTy>::decode(&bytes[pos..])?;
        pos += n;
        Ok((
            Self {
                name,
                name_span,
                def_raw_code,
                def_span,
                genargs,
                args,
                rty,
                impl_self_ty,
            },
            pos,
        ))
    }
}

impl DiskEncode for DiskFnData {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.name.encode(buf);
        self.name_span.encode(buf);
        self.def_raw_code.encode(buf);
        self.def_span.encode(buf);
        self.genargs.encode(buf);
        self.args.encode(buf);
        self.rty.encode(buf);
        self.impl_self_ty.encode(buf);
    }
}

// --- DiskStructData ---

#[derive(Debug)]
pub struct DiskStructData {
    pub name: DiskStringOffset,
    pub name_span: DiskSpan,
    pub def_raw_code: DiskStringOffset,
    pub def_span: DiskSpan,
    pub genargs: DiskVec<DiskGenArg>,
    pub members: DiskVec<DiskStructMember>,
    /// 関連関数・メソッドのシンボルインデックス
    pub assoc_symbols: DiskVec<DiskSymbolIndex>,
}

impl DiskDecode for DiskStructData {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (name, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (name_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (def_raw_code, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (def_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (genargs, n) = DiskVec::<DiskGenArg>::decode(&bytes[pos..])?;
        pos += n;
        let (members, n) = DiskVec::<DiskStructMember>::decode(&bytes[pos..])?;
        pos += n;
        let (assoc_symbols, n) = DiskVec::<DiskSymbolIndex>::decode(&bytes[pos..])?;
        pos += n;
        Ok((
            Self {
                name,
                name_span,
                def_raw_code,
                def_span,
                genargs,
                members,
                assoc_symbols,
            },
            pos,
        ))
    }
}

impl DiskEncode for DiskStructData {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.name.encode(buf);
        self.name_span.encode(buf);
        self.def_raw_code.encode(buf);
        self.def_span.encode(buf);
        self.genargs.encode(buf);
        self.members.encode(buf);
        self.assoc_symbols.encode(buf);
    }
}

// --- DiskNativeTypeAliasData ---
//
// native type alias (`type String = {{ string }};`) のボディ。
//
// struct と同じくパッケージ外から参照される型定義であり、
// impl block を持てる (`impl String { fn concat(..) }`) ため
// assoc_symbols を持つ。

#[derive(Debug)]
pub struct DiskNativeTypeAliasData {
    pub name: DiskStringOffset,
    pub name_span: DiskSpan,
    /// `{{ ... }}` の中身 (ターゲット言語のコード)
    pub native: DiskStringOffset,
    pub native_span: DiskSpan,
    pub genargs: DiskVec<DiskGenArg>,
    /// 関連関数・メソッドのシンボルインデックス
    pub assoc_symbols: DiskVec<DiskSymbolIndex>,
}

impl DiskDecode for DiskNativeTypeAliasData {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (name, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (name_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (native, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (native_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (genargs, n) = DiskVec::<DiskGenArg>::decode(&bytes[pos..])?;
        pos += n;
        let (assoc_symbols, n) = DiskVec::<DiskSymbolIndex>::decode(&bytes[pos..])?;
        pos += n;
        Ok((
            Self {
                name,
                name_span,
                native,
                native_span,
                genargs,
                assoc_symbols,
            },
            pos,
        ))
    }
}

impl DiskEncode for DiskNativeTypeAliasData {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.name.encode(buf);
        self.name_span.encode(buf);
        self.native.encode(buf);
        self.native_span.encode(buf);
        self.genargs.encode(buf);
        self.assoc_symbols.encode(buf);
    }
}

// --- DiskModData ---

#[derive(Debug)]
pub struct DiskModData {
    pub name: DiskStringOffset,
    pub name_span: DiskSpan,
    /// 子シンボル (子モジュール、型、関数など) のインデックスリスト
    pub children: DiskVec<DiskSymbolIndex>,
}

impl DiskDecode for DiskModData {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (name, n) = DiskStringOffset::decode(&bytes[pos..])?;
        pos += n;
        let (name_span, n) = DiskSpan::decode(&bytes[pos..])?;
        pos += n;
        let (children, n) = DiskVec::<DiskSymbolIndex>::decode(&bytes[pos..])?;
        pos += n;
        Ok((
            Self {
                name,
                name_span,
                children,
            },
            pos,
        ))
    }
}

impl DiskEncode for DiskModData {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.name.encode(buf);
        self.name_span.encode(buf);
        self.children.encode(buf);
    }
}

// --- DiskDepSvh (固定長 12B): 依存パッケージの SVH ---
//
// このパッケージをビルドしたとき、依存グラフの推移閉包に居た各パッケージの
// `(PackageId, Svh)` を記録する。
//
// 名前ではなく id で持てるのは、PackageId が (name, version) から導出される
// 安定ハッシュだからである (biwac_span::PackageHashId を参照)。
// 誰がいつビルドしても同じ id になるので、
// 「このファイルの中でだけ通じるパッケージ番号」を名前で解決し直す必要がない。
//
// 用途は 2 つ:
//   - ロード時の整合性検査 (rustc の CrateDep::hash に相当)
//   - 差分ビルドの鮮度判定。前回ビルド時の依存の SVH と今回のそれを突き合わせる
//
// 推移閉包すべてを載せるのは、パッケージのビルドが直接依存だけでなく
// 推移閉包すべてのメタデータを読むからである
// (推移的な依存が定義した lang item も取り込む)。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskDepSvh {
    /// 依存パッケージの [`biwac_base::PackageId`] の生値
    pub pkg: u32,
    /// そのパッケージのインタフェースのハッシュ
    pub svh: u64,
}

impl DiskDepSvh {
    pub const BYTE_SIZE: usize = 12;
}

impl DiskDecode for DiskDepSvh {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (pkg, n) = u32::decode(&bytes[pos..])?;
        pos += n;
        let (svh, n) = u64::decode(&bytes[pos..])?;
        pos += n;
        Ok((Self { pkg, svh }, pos))
    }
}

impl DiskEncode for DiskDepSvh {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.pkg.encode(buf);
        self.svh.encode(buf);
    }
}

// --- DiskExternalSymbol (固定長 8B): 外部シンボル表のエントリ ---
//
// 依存パッケージのシンボル 1 件への参照。
// `DiskTyKind::ExternalDefined` の sym_id がこの表を指す。
//
// `pkg` は [`biwac_base::PackageId`] の生値そのもの。
// `sym` は **相手の .biwameta 内での** シンボルインデックスである。
// したがって依存が再ビルドされて採番が変われば、このファイルの外部参照は無効になる。
// それを検出するのが上の dep_svh_table で、
// SVH にはシンボルインデックスが含まれる (DepMetadata::compute_svh を参照)。

#[derive(Debug, Clone, Copy)]
pub struct DiskExternalSymbol {
    pub pkg: u32,
    pub sym: DiskSymbolIndex,
}

impl DiskExternalSymbol {
    pub const BYTE_SIZE: usize = 8;
}

impl DiskDecode for DiskExternalSymbol {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let mut pos = 0;
        let (pkg, n) = u32::decode(&bytes[pos..])?;
        pos += n;
        let (sym, n) = DiskSymbolIndex::decode(&bytes[pos..])?;
        pos += n;
        Ok((Self { pkg, sym }, pos))
    }
}

impl DiskEncode for DiskExternalSymbol {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.pkg.encode(buf);
        self.sym.encode(buf);
    }
}

// --- DiskSourceInfo (固定長 4B) ---

#[derive(Debug, Clone, Copy)]
pub struct DiskSourceInfo {
    pub file_path: DiskStringOffset,
}

impl DiskSourceInfo {
    pub const BYTE_SIZE: usize = 4;
}

impl DiskDecode for DiskSourceInfo {
    fn decode(bytes: &[u8]) -> Result<(Self, usize), DepMetadataError> {
        let (file_path, n) = DiskStringOffset::decode(bytes)?;
        Ok((Self { file_path }, n))
    }
}

impl DiskEncode for DiskSourceInfo {
    fn encode(&self, buf: &mut Vec<u8>) {
        self.file_path.encode(buf);
    }
}
