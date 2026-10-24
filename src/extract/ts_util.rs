//! tree-sitter の共通ヘルパ。
use tree_sitter::{Node, Parser, Tree};

pub fn parse(lang: &tree_sitter::Language, src: &str) -> Option<Tree> {
    let mut p = Parser::new();
    p.set_language(lang).ok()?;
    p.parse(src, None)
}

pub fn text<'a>(n: Node, src: &'a str) -> &'a str { n.utf8_text(src.as_bytes()).unwrap_or("") }

pub fn field<'t>(n: Node<'t>, name: &str) -> Option<Node<'t>> { n.child_by_field_name(name) }

pub fn field_text(n: Node, name: &str, src: &str) -> String { field(n, name).map(|c| text(c, src).to_string()).unwrap_or_default() }

pub fn children<'t>(n: Node<'t>) -> Vec<Node<'t>> { let mut c = n.walk(); n.children(&mut c).collect() }

pub fn named_children<'t>(n: Node<'t>) -> Vec<Node<'t>> { let mut c = n.walk(); n.named_children(&mut c).collect() }

pub fn line(n: Node) -> u32 { n.start_position().row as u32 + 1 }
pub fn end_line(n: Node) -> u32 { n.end_position().row as u32 + 1 }

/// 直前の兄弟コメントを docstring として集める(JSDoc / /// / # )。
pub fn doc_before(n: Node, src: &str) -> String {
    // 文レベルの先祖まで上がる(export / const x = / decorator / impl の中のメソッド は宣言の前にコメントが付く)
    let mut top = n;
    while let Some(p) = top.parent() { if matches!(p.kind(), "export_statement" | "lexical_declaration" | "variable_declaration" | "variable_declarator" | "decorated_definition" | "parenthesized_expression" | "call_expression" | "arguments") { top = p; } else { break; } }
    let mut docs: Vec<String> = Vec::new();
    let mut cur = top.prev_sibling();
    while let Some(c) = cur {
        if !c.is_named() { cur = c.prev_sibling(); continue; }   // `export` などのトークンは飛ばす
        if c.kind() == "comment" || c.kind() == "line_comment" || c.kind() == "block_comment" {
            if c.end_position().row + 2 < top.start_position().row && !docs.is_empty() { break; }
            if c.end_position().row + 2 < top.start_position().row { break; }
            let t = text(c, src);
            if c.kind() == "line_comment" && !(t.starts_with("///") || t.starts_with("//!")) { break; }
            docs.push(clean_comment(t));
            cur = c.prev_sibling();
        } else { break; }
    }
    docs.reverse();
    let s = docs.join("\n");
    let s = s.trim();
    if s.chars().count() > 600 { format!("{}…", s.chars().take(600).collect::<String>()) } else { s.to_string() }
}

pub fn clean_comment(c: &str) -> String {
    let c = c.trim();
    let c = c.strip_prefix("/**").or_else(|| c.strip_prefix("/*")).unwrap_or(c);
    let c = c.strip_suffix("*/").unwrap_or(c);
    c.lines().map(|l| { let l = l.trim(); let l = l.strip_prefix("///").or_else(|| l.strip_prefix("//!")).or_else(|| l.strip_prefix("//")).or_else(|| l.strip_prefix("#")).or_else(|| l.strip_prefix("*")).unwrap_or(l); l.trim() })
        .filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// 文字列リテラルの中身(クォートを外す)。template string は ${} を {} にする。
pub fn string_value(n: Node, src: &str) -> String {
    let t = text(n, src);
    // 先頭のクォートまでの prefix(r / b / f / r# など)を落とし、両端のクォートを外す
    let start = t.find(|c| c == '"' || c == '\'' || c == '`').unwrap_or(0);
    let t = &t[start..];
    let t = t.trim_matches('#');
    let t = t.trim_matches(|c| c == '"' || c == '\'' || c == '`');
    t.to_string()
}

/// 先頭 max 行の抜粋(共通インデントを落とす)。
pub fn excerpt(src: &str, start: u32, end: u32, max: usize) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let s = (start as usize).saturating_sub(1).min(lines.len());
    let e = (end as usize).min(lines.len()).max(s);
    let total = e - s;
    let take = total.min(max);
    let out: Vec<&str> = lines[s..s + take].to_vec();
    let indent = out.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let mut text = out.iter().map(|l| if l.len() >= indent { &l[indent..] } else { l.trim_start() }).collect::<Vec<_>>().join("\n");
    if total > take { text.push_str(&format!("\n… ({} 行省略)", total - take)); }
    text
}

/// 引数の最初の文字列リテラル(テーブル名・path・SQL などの検出に使う)。
pub fn first_string_arg(args: Option<Node>, src: &str) -> Option<String> {
    let a = args?;
    let first = named_children(a).into_iter().next()?;
    if matches!(first.kind(), "string" | "template_string" | "string_literal" | "raw_string_literal" | "interpreted_string_literal" | "concatenated_string") { let v = string_value(first, src); if !v.is_empty() && v.len() < 400 { return Some(v); } }
    None
}

pub fn hash(s: &str) -> String { blake3::hash(s.as_bytes()).to_hex()[..16].to_string() }

/// ノード以下を深さ優先で巡る(関数の境界で止めるかは呼び手が決める)。
pub fn walk<'t>(n: Node<'t>, f: &mut dyn FnMut(Node<'t>) -> bool) {
    if !f(n) { return; }
    for c in named_children(n) { walk(c, f); }
}

pub const ENV_RX: &str = r"process\.env\.([A-Z][A-Z0-9_]+)|process\.env\[['\x22]([A-Z][A-Z0-9_]+)['\x22]\]|import\.meta\.env\.([A-Z][A-Z0-9_]+)|os\.environ(?:\.get)?\(?\[?['\x22]([A-Z][A-Z0-9_]+)['\x22]|os\.getenv\(\s*['\x22]([A-Z][A-Z0-9_]+)['\x22]|env::var(?:_os)?\(\s*\x22([A-Z][A-Z0-9_]+)\x22|env!\(\s*\x22([A-Z][A-Z0-9_]+)\x22|os\.Getenv\(\s*\x22([A-Z][A-Z0-9_]+)\x22|Deno\.env\.get\(\s*['\x22]([A-Z][A-Z0-9_]+)['\x22]";

pub fn env_keys(src: &str) -> Vec<String> {
    static RX: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(ENV_RX).unwrap());
    let mut out: Vec<String> = RX.captures_iter(src).filter_map(|c| (1..=9).find_map(|i| c.get(i)).map(|m| m.as_str().to_string())).collect();
    out.sort(); out.dedup(); out
}
