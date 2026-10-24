//! 知識グラフの型。ノード(file / function / type / table / route / job / external / package)と辺(imports / calls / reads / writes / implements / extends / handles / triggers / uses)。
//! ID は安定(path + 名前)で、HTML の anchor・差分・notes のキーになる。
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind { File, Function, Type, Table, Route, Job, External, Package, Env, Layer }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind { Imports, Calls, Reads, Writes, Implements, Extends, Handles, Triggers, Uses, Contains, References, Exports }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub name: String,
    /// repo 相対 path(file / function / type / route のハンドラ)
    pub path: String,
    #[serde(default)] pub line: u32,
    #[serde(default)] pub end_line: u32,
    /// 層(src/adapter/ のような 1 段目)
    #[serde(default)] pub layer: String,
    /// 言語(ts / py / rs / go / sql)
    #[serde(default)] pub lang: String,
    /// コード本文のハッシュ(notes の鮮度・差分の「変化」判定)
    #[serde(default)] pub hash: String,
    /// 公開されているか(export / pub / 大文字)
    #[serde(default)] pub exported: bool,
    /// シグネチャ(function)/ 定義の先頭(type)
    #[serde(default)] pub signature: String,
    /// docstring / JSDoc / コメント(作者の言葉)
    #[serde(default)] pub doc: String,
    /// 種類の細目(function: fn|method|arrow|class / type: interface|class|enum|type|struct|trait|protocol / route: http|server_action|webhook|cron|cli|queue / external: host|sdk|llm)
    #[serde(default)] pub subkind: String,
    /// 追加の属性(route: method, auth / table: columns, rls / job: schedule / external: category)
    #[serde(default)] pub attrs: BTreeMap<String, serde_json::Value>,
    /// コード抜粋(入口のハンドラ・型の定義本文。長すぎるものは先頭だけ)
    #[serde(default)] pub excerpt: String,
}

impl Default for NodeKind { fn default() -> Self { NodeKind::File } }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    /// 呼び出し行など
    #[serde(default)] pub line: u32,
    /// 解決できなかった呼び先(名前だけ)
    #[serde(default)] pub unresolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn node(&self, id: &str) -> Option<&Node> { self.nodes.iter().find(|n| n.id == id) }
    pub fn index(&self) -> BTreeMap<&str, usize> { self.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect() }
}

/// ノード ID の作り方(安定):kind:path#name
pub fn id_of(kind: NodeKind, path: &str, name: &str) -> String {
    let k = match kind { NodeKind::File => "file", NodeKind::Function => "fn", NodeKind::Type => "type", NodeKind::Table => "table", NodeKind::Route => "route", NodeKind::Job => "job", NodeKind::External => "ext", NodeKind::Package => "pkg", NodeKind::Env => "env", NodeKind::Layer => "layer" };
    if name.is_empty() { format!("{k}:{path}") } else { format!("{k}:{path}#{name}") }
}

/// HTML の anchor に使える短い ID(英数字と - だけ)
pub fn anchor(id: &str) -> String {
    let h = blake3::hash(id.as_bytes()).to_hex();
    let slug: String = id.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>().trim_matches('-').chars().take(60).collect();
    format!("{slug}-{}", &h[..8])
}
