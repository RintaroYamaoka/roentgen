//! 言語ごとの抽出器が返す共通の中間表現(IR)。グラフはこれから作る。
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Import { pub spec: String, pub names: Vec<(String, String)>, pub line: u32, pub namespace: Option<String>, pub default: Option<String> }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Call { pub name: String, pub receiver: Option<String>, pub line: u32, /// 第 1 引数が文字列ならその値
    pub arg: Option<String> }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Func {
    pub name: String, pub line: u32, pub end_line: u32, pub exported: bool, pub signature: String, pub doc: String,
    /// fn | method | arrow | class(ctor)
    pub kind: String, pub class: Option<String>, pub calls: Vec<Call>, pub strings: Vec<String>, pub body: String, pub decorators: Vec<String>, pub is_async: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TypeDef {
    pub name: String, pub line: u32, pub end_line: u32, pub exported: bool,
    /// interface | class | enum | type | struct | trait | protocol | dataclass
    pub kind: String, pub extends: Vec<String>, pub implements: Vec<String>, pub doc: String, pub text: String, pub refs: Vec<String>, pub methods: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileIR {
    pub path: String, pub lang: String, pub lines: u32,
    pub imports: Vec<Import>, pub functions: Vec<Func>, pub types: Vec<TypeDef>,
    /// 関数の外(モジュールレベル)の呼び出しと文字列
    pub top_calls: Vec<Call>, pub top_strings: Vec<String>,
    pub directives: Vec<String>, pub env_keys: Vec<String>, pub doc: String,
    /// export { a, b } / export default などで公開された名前(宣言と別の場所)
    pub extra_exports: Vec<String>,
    /// const X = f / const X = wrap(f) / export { f as X } の別名(X → f)
    pub aliases: Vec<(String, String)>,
    pub parse_error: bool,
}

impl FileIR {
    pub fn all_strings(&self) -> impl Iterator<Item = &String> { self.top_strings.iter().chain(self.functions.iter().flat_map(|f| f.strings.iter())) }
    pub fn all_calls(&self) -> impl Iterator<Item = &Call> { self.top_calls.iter().chain(self.functions.iter().flat_map(|f| f.calls.iter())) }
}
