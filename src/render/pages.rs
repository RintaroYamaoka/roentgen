//! 19 ページの組み立て。data/*.json(事実)+ narrative.json(LLM の章の解説)+ .roentgen/notes(ノードの解説)から HTML を出す。
use super::html::*;
use super::svg;
use crate::analyze::Analysis;
use crate::model::*;
use crate::repo::role_of;
use anyhow::Result;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

pub struct Ctx {
    pub meta: Value, pub g: Graph, pub a: Analysis, pub cfg: Value, pub tables: Value, pub diff: Option<Value>, pub nar: Value, pub notes: BTreeMap<String, crate::notes::Note>,
    pub idx: HashMap<String, usize>, pub out_edges: HashMap<String, Vec<usize>>, pub in_edges: HashMap<String, Vec<usize>>, pub stamp: String, pub changed: Vec<String>,
}

pub fn build(out: &Path) -> Result<()> {
    let d = out.join("data");
    let meta: Value = crate::store::read_json(&d.join("meta.json")).unwrap_or(Value::Null);
    let g: Graph = crate::store::read_json(&d.join("graph.json")).ok_or_else(|| anyhow::anyhow!("graph.json が無い"))?;
    let a: Analysis = crate::store::read_json(&d.join("analysis.json")).unwrap_or_default();
    let cfg: Value = crate::store::read_json(&d.join("config.json")).unwrap_or(Value::Null);
    let tables: Value = crate::store::read_json(&d.join("tables.json")).unwrap_or(Value::Null);
    let diff: Option<Value> = crate::store::read_json(&d.join("diff.json"));
    let nar: Value = crate::store::read_json(&d.join("narrative.json")).unwrap_or(Value::Null);
    let root = meta.get("root").and_then(|r| r.as_str()).map(std::path::PathBuf::from).unwrap_or_default();
    let notes = if root.exists() { crate::notes::load(&root, &g) } else { BTreeMap::new() };
    let idx: HashMap<String, usize> = g.nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
    let mut out_edges: HashMap<String, Vec<usize>> = HashMap::new(); let mut in_edges: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, e) in g.edges.iter().enumerate() { out_edges.entry(e.from.clone()).or_default().push(i); in_edges.entry(e.to.clone()).or_default().push(i); }
    let stamp = format!("{} {} {}", s(&meta, "name"), s(&meta, "sha"), s(&meta, "analyzed"));
    let changed: Vec<String> = diff.as_ref().and_then(|d| d.get("changed")).and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    let ctx = Ctx { meta, g, a, cfg, tables, diff, nar, notes, idx, out_edges, in_edges, stamp, changed };
    let pages: Vec<(&str, fn(&Ctx) -> String)> = vec![("index", p_index), ("01", p_tree), ("02", p_arch), ("03", p_data), ("04", p_entry), ("05", p_callgraph), ("06", p_modules), ("07", p_types), ("08", p_deps), ("09", p_hotspots), ("10", p_debt), ("11", p_dev), ("12", p_integrations), ("13", p_authz), ("14", p_jobs), ("15", p_llm), ("16", p_diff), ("17", p_glossary), ("99", p_unknown)];
    for (k, f) in pages { std::fs::write(out.join(file_of(k)), f(&ctx))?; }
    Ok(())
}

// ---------- helpers ----------
fn s<'a>(v: &'a Value, k: &str) -> &'a str { v.get(k).and_then(|x| x.as_str()).unwrap_or("") }
fn n_u64(v: &Value, k: &str) -> u64 { v.get(k).and_then(|x| x.as_u64()).unwrap_or(0) }
fn nar<'a>(c: &'a Ctx, ch: &str) -> &'a Value { c.nar.get(ch).unwrap_or(&Value::Null) }
fn nar_list(c: &Ctx, ch: &str, k: &str) -> Option<Vec<String>> { nar(c, ch).get(k).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()) }
fn html_meta(c: &Ctx) -> Meta<'_> { Meta { name: s(&c.meta, "name"), sha: s(&c.meta, "sha"), date: s(&c.meta, "date"), analyzed: s(&c.meta, "analyzed"), src_files: n_u64(&c.meta, "src_files") as usize, src_lines: n_u64(&c.meta, "src_lines") } }
fn node<'a>(c: &'a Ctx, id: &str) -> Option<&'a Node> { c.idx.get(id).map(|&i| &c.g.nodes[i]) }
fn nodes_of(c: &Ctx, k: NodeKind) -> Vec<&Node> { c.g.nodes.iter().filter(|n| n.kind == k).collect() }
fn outs<'a>(c: &'a Ctx, id: &str, k: EdgeKind) -> Vec<&'a Edge> { c.out_edges.get(id).into_iter().flatten().map(|&i| &c.g.edges[i]).filter(|e| e.kind == k).collect() }
fn ins<'a>(c: &'a Ctx, id: &str, k: EdgeKind) -> Vec<&'a Edge> { c.in_edges.get(id).into_iter().flatten().map(|&i| &c.g.edges[i]).filter(|e| e.kind == k).collect() }
fn href_of(c: &Ctx, id: &str) -> String { match node(c, id) { Some(n) => match n.kind { NodeKind::Function => format!("{}#{}", file_of("06"), anchor(id)), NodeKind::Type => format!("{}#{}", file_of("07"), anchor(id)), NodeKind::Table => format!("{}#table-{}", file_of("03"), svg::id(&n.name)), NodeKind::Route => format!("{}#{}", file_of("04"), anchor(id)), NodeKind::File => format!("{}#{}", file_of("06"), anchor(id)), NodeKind::External => if n.subkind == "llm" { format!("{}#callers", file_of("15")) } else { format!("{}#{}", file_of("12"), anchor(id)) }, NodeKind::Package => file_of("08").into(), NodeKind::Env => file_of("11").into(), NodeKind::Job => file_of("14").into(), NodeKind::Layer => format!("{}#layer-{}", file_of("02"), svg::id(&n.name)) }, None => String::new() } }
fn link(c: &Ctx, id: &str) -> String { match node(c, id) { Some(n) => format!("<a href=\"{}\">{}</a>", href_of(c, id), esc(&n.name)), None => esc(id) } }
fn link_path(c: &Ctx, id: &str) -> String { match node(c, id) { Some(n) => format!("<a href=\"{}\"><code class=\"path\">{}#{}</code></a>", href_of(c, id), esc(&n.path), esc(&n.name)), None => esc(id) } }
fn notes_html(c: &Ctx, id: &str) -> String { match c.notes.get(id) { Some(n) => format!("<div class=\"notes{}\"><span class=\"eyebrow\">解説(Claude){}</span>{}</div>", if n.stale { " stale" } else { "" }, if n.stale { " — コードが変わっている(古い)" } else { "" }, esc(&n.body)), None => String::new() } }
fn stamp(c: &Ctx) -> &str { &c.stamp }
fn lang_of_node(n: &Node) -> &str { if n.lang.is_empty() { "ts" } else { &n.lang } }
fn auth_badge(a: &str) -> String { badge(a, match a { "middleware" => "ok", "none" => "warn", _ => "" }) }
fn unknown_box(c: &Ctx, words: &[&str]) -> String { let items: Vec<&Value> = c.meta.get("unknown").and_then(|u| u.as_array()).map(|a| a.iter().filter(|u| words.iter().any(|w| s(u, "what").contains(w))).collect()).unwrap_or_default(); if items.is_empty() { return String::new(); } format!("<div class=\"unknown\"><b>未確認</b>:{} <a href=\"{}\">→ Unknown</a></div>", items.iter().map(|u| format!("{}({})", esc(s(u, "what")), esc(s(u, "why")))).collect::<Vec<_>>().join(" / "), file_of("99")) }

/// 関数 1 件のブロック(呼ぶ / 呼ばれる / 表 / 外部 / notes / 抜粋)
fn fn_block(c: &Ctx, n: &Node, open: bool, max_lines: usize) -> String {
    let callers: Vec<String> = ins(c, &n.id, EdgeKind::Calls).iter().map(|e| link(c, &e.from)).take(12).collect();
    let callees: Vec<String> = outs(c, &n.id, EdgeKind::Calls).iter().map(|e| link(c, &e.to)).take(12).collect();
    let routes: Vec<String> = ins(c, &n.id, EdgeKind::Handles).iter().map(|e| link(c, &e.from)).collect();
    let reads: Vec<String> = outs(c, &n.id, EdgeKind::Reads).iter().map(|e| link(c, &e.to)).collect();
    let writes: Vec<String> = outs(c, &n.id, EdgeKind::Writes).iter().map(|e| link(c, &e.to)).collect();
    let exts: Vec<String> = outs(c, &n.id, EdgeKind::Uses).iter().filter(|e| e.to.starts_with("ext:")).map(|e| link(c, &e.to)).collect();
    let unres: Vec<String> = n.attrs.get("calls_unresolved").and_then(|v| v.as_array()).map(|a| a.iter().take(8).filter_map(|x| Some(format!("<code>{}</code>×{}", esc(x.get(0)?.as_str()?), x.get(1)?.as_u64()?))).collect()).unwrap_or_default();
    let mut meta: Vec<(String, String)> = Vec::new();
    if !routes.is_empty() { meta.push(("入口".into(), routes.join(" "))); }
    if !callers.is_empty() { meta.push(("Called by".into(), callers.join(", "))); }
    if !callees.is_empty() { meta.push(("Calls".into(), callees.join(", "))); }
    if !reads.is_empty() { meta.push(("Reads".into(), reads.join(", "))); }
    if !writes.is_empty() { meta.push(("Writes".into(), writes.join(", "))); }
    if !exts.is_empty() { meta.push(("External".into(), exts.join(", "))); }
    if !unres.is_empty() { meta.push(("未解決の呼び出し".into(), unres.join(" "))); }
    if n.attrs.get("tested").is_some() { meta.push(("Tests".into(), badge("tested", "ok"))); }
    let ex: String = n.excerpt.lines().take(max_lines).collect::<Vec<_>>().join("\n");
    let ex_html = if ex.is_empty() { String::new() } else { code_at(&format!("{}{}", ex, if n.excerpt.lines().count() > max_lines { "\n…" } else { "" }), lang_of_node(n), &n.path, n.line) };
    node_block(&anchor(&n.id), &badge(&n.subkind, ""), &n.name, &n.signature, &n.doc, &notes_html(c, &n.id), &ex_html, &meta, open)
}

fn type_block(c: &Ctx, n: &Node, open: bool) -> String {
    let ext: Vec<String> = outs(c, &n.id, EdgeKind::Extends).iter().map(|e| link(c, &e.to)).collect();
    let imp: Vec<String> = outs(c, &n.id, EdgeKind::Implements).iter().map(|e| link(c, &e.to)).collect();
    let refs: Vec<String> = outs(c, &n.id, EdgeKind::References).iter().map(|e| link(c, &e.to)).take(10).collect();
    let used: Vec<String> = ins(c, &n.id, EdgeKind::References).iter().map(|e| link(c, &e.from)).take(10).collect();
    let mut meta: Vec<(String, String)> = Vec::new();
    if !ext.is_empty() { meta.push(("Extends".into(), ext.join(", "))); } if !imp.is_empty() { meta.push(("Implements".into(), imp.join(", "))); }
    if !refs.is_empty() { meta.push(("References".into(), refs.join(", "))); } if !used.is_empty() { meta.push(("Referenced by".into(), used.join(", "))); }
    node_block(&anchor(&n.id), &badge(&n.subkind, ""), &n.name, "", &n.doc, &notes_html(c, &n.id), &code_at(&n.excerpt, lang_of_node(n), &n.path, n.line), &meta, open)
}

// ---------- index ----------
fn p_index(c: &Ctx) -> String {
    let m = &c.a.metrics;
    let nar0 = nar(c, "index");
    let mut body = summary(nar_list(c, "index", "summary").as_ref());
    let products: Vec<&Value> = c.meta.get("products").and_then(|p| p.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
    let center: Vec<svg::HubNode> = products.iter().take(3).enumerate().map(|(i, p)| svg::HubNode { title: s(p, "name").into(), sub: if i == 0 { format!("{} files · {} 行", n_u64(&c.meta, "src_files"), fmt_n(n_u64(&c.meta, "src_lines"))) } else { "製品境界(推定)".into() }, role: "entry".into(), href: file_of("01").into(), main: i == 0, ..Default::default() }).collect();
    let users: Vec<svg::HubNode> = nar0.get("users").and_then(|u| u.as_array()).map(|a| a.iter().map(|u| svg::HubNode { title: s(u, "name").into(), sub: s(u, "what").into(), role: "user".into(), ..Default::default() }).collect()).unwrap_or_else(|| c.a.roles.iter().take(4).map(|(r, n)| svg::HubNode { title: r.clone(), sub: format!("role({} 箇所)", n), role: "user".into(), href: file_of("13").into(), ..Default::default() }).collect());
    let users = if users.is_empty() { vec![svg::HubNode { title: "利用者(未確認)".into(), role: "user".into(), unknown: true, ..Default::default() }] } else { users };
    let mut exts: Vec<&Node> = nodes_of(c, NodeKind::External); exts.sort_by_key(|n| std::cmp::Reverse(n_u64(&Value::Object(n.attrs.clone().into_iter().collect()), "files")));
    let right: Vec<svg::HubNode> = exts.iter().take(8).map(|n| svg::HubNode { title: n.name.clone(), sub: format!("{} files", n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0)), count: n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0) as u32, role: if n.subkind == "llm" { "ai".into() } else { "external".into() }, href: href_of(c, &n.id), ..Default::default() }).collect();
    let stack: Vec<&Value> = c.cfg.get("stack").and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
    let mut bottom: Vec<svg::HubNode> = stack.iter().filter(|x| matches!(s(x, "category"), "db" | "cache" | "storage" | "queue")).take(4).map(|x| svg::HubNode { title: s(x, "name").into(), sub: if s(x, "category") == "db" { format!("{} 表", m.get("tables").and_then(|v| v.as_u64()).unwrap_or(0)) } else { s(x, "dep").into() }, role: "db".into(), href: if s(x, "category") == "db" { file_of("03").into() } else { file_of("14").into() }, ..Default::default() }).collect();
    if bottom.is_empty() && m.get("tables").and_then(|v| v.as_u64()).unwrap_or(0) > 0 { bottom.push(svg::HubNode { title: "DB".into(), sub: format!("{} 表", m["tables"]), role: "db".into(), href: file_of("03").into(), ..Default::default() }); }
    body += &fig(&svg::hub(&center, &users, &right, &bottom, stamp(c), "利用者", "外部", "データ"), "全体図:左 = 利用者、中央 = この repo(製品境界)、右 = 外部(ファイル数)、下 = データ。箱をクリックで章へ", "");
    body += &h2("読む順", "");
    let o1 = ["01", "02", "03", "04", "05", "06", "07", "11", "17"]; let o2 = ["16", "09", "10", "13", "14"];
    body += &format!("<div class=\"read-order\"><b>初めて入る:</b>{}</div>", o1.iter().map(|k| format!("<a href=\"{}\">{} {}</a>", file_of(k), k, title_of(k).0)).collect::<Vec<_>>().join("<span class=\"arrow\">→</span>"));
    body += &format!("<div class=\"read-order\"><b>久々に見る:</b>{}</div>", o2.iter().map(|k| format!("<a href=\"{}\">{} {}</a>", file_of(k), k, title_of(k).0)).collect::<Vec<_>>().join("<span class=\"arrow\">→</span>"));
    body += &h2("数字で見る", "");
    let auth_none = m.get("auth").and_then(|a| a.get("none")).and_then(|x| x.as_u64()).unwrap_or(0);
    let langs: Vec<String> = c.meta.get("langs").and_then(|l| l.as_array()).map(|a| a.iter().take(3).filter_map(|x| x.get(1).and_then(|y| y.as_str()).map(String::from)).collect()).unwrap_or_default();
    body += &tiles(&[("言語", langs.join(" / "), ""), ("ファイル", m["files"].to_string(), ""), ("行数", fmt_n(m["lines"].as_u64().unwrap_or(0)), ""), ("関数", fmt_n(m["functions"].as_u64().unwrap_or(0)), ""), ("型", fmt_n(m["types"].as_u64().unwrap_or(0)), ""), ("表", m["tables"].to_string(), ""),
        ("入口", m["routes"].to_string(), ""), ("認可なしの入口", auth_none.to_string(), if auth_none > 0 { "warn" } else { "ok" }), ("解決した呼び出し", fmt_n(m["calls_resolved"].as_u64().unwrap_or(0)), ""), ("外部", m["externals"].to_string(), ""), ("禁止辺の違反", m["rule_violations"].to_string(), if m["rule_violations"].as_u64().unwrap_or(0) > 0 { "warn" } else { "ok" }), ("90 日 commit", c.a.commits.to_string(), "")]);
    body += &h2("章", "");
    let one: BTreeMap<&str, &str> = [("01", "ディレクトリごとに何があるか"), ("02", "層・依存の向き・循環"), ("03", "ER 図・表・読み書きする関数"), ("04", "route / action / cron / webhook と認可"), ("05", "入口から辿った呼び出しとコード"), ("06", "ファイルごとの公開関数とコード"), ("07", "型の定義と関係"), ("08", "外部パッケージと版"), ("09", "90 日の churn"), ("10", "巨大×変更・認可なし・RLS 無し・循環"), ("11", "setup / test / CI / env"), ("12", "外部 API・webhook"), ("13", "role・RLS・middleware"), ("14", "cron・queue・副作用"), ("15", "model・prompt・呼び元"), ("16", "前回との差分"), ("17", "業務の語 ⇄ コード"), ("99", "読めなかったもの")].into_iter().collect();
    let cards: String = CHAPTERS.iter().filter(|ch| ch.0 != "index").map(|(k, f, en, ja)| { let sm = nar_list(c, k, "summary").and_then(|v| v.first().cloned()); let chg = if c.changed.iter().any(|x| super_changed(k, x)) { " changed" } else { "" }; format!("<a class=\"card{}\" href=\"{}\"><div class=\"n\">{} {}</div><div class=\"t\">{}</div><div class=\"d\">{}</div></a>", chg, f, k, en, ja, esc(&sm.unwrap_or(one.get(k).unwrap_or(&"").to_string()))) }).collect();
    body += &format!("<div class=\"cards\">{}</div>", cards);
    page("index", s(&c.meta, "name"), s(nar0, "lead"), &body, &html_meta(c), &c.changed)
}
fn super_changed(k: &str, section: &str) -> bool { matches!((k, section), ("03", "tables") | ("03", "table_access") | ("04", "routes") | ("05", "traces") | ("06", "functions") | ("06", "files") | ("07", "types") | ("02", "architecture") | ("08", "packages") | ("12", "externals") | ("14", "jobs") | ("11", "env")) }

// ---------- 01 File Tree ----------
fn p_tree(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "01", "summary").as_ref());
    let dirs_nar = nar(c, "01").get("dirs").cloned().unwrap_or(Value::Null);
    body += &note("ディレクトリごとのファイル数・行数・言語・役割(推定)。1 行の説明は index / mod / README の doc か、Claude が narrative.json の 01.dirs に書いたもの。クリックで Modules のそのディレクトリへ。");
    let mut tree = String::from("<div class=\"tree\">");
    let mut by_depth: BTreeMap<u32, Vec<&crate::analyze::TreeDir>> = BTreeMap::new();
    for d in &c.a.tree { by_depth.entry(d.depth).or_default().push(d); }
    fn render(c: &Ctx, prefix: &str, depth: u32, dirs_nar: &Value, out: &mut String) {
        let mut kids: Vec<&crate::analyze::TreeDir> = c.a.tree.iter().filter(|d| d.depth == depth && d.path.starts_with(prefix) && d.path[prefix.len()..].trim_end_matches('/').find('/').is_none()).collect();
        kids.sort_by(|a, b| b.lines.cmp(&a.lines));
        if kids.is_empty() { return; }
        out.push_str("<ul>");
        for d in kids.iter().take(40) {
            let nar1 = dirs_nar.get(&d.path).and_then(|x| x.as_str()).unwrap_or(&d.doc);
            out.push_str(&format!("<li><a class=\"dir\" href=\"{}#dir-{}\">{}</a>{}<span class=\"m\">{} files · {} 行 · {}</span>{}", file_of("06"), svg::id(&d.path), esc(d.path.rsplit('/').nth(1).map(|x| format!("{}/", x)).unwrap_or(d.path.clone()).as_str()), if d.role != "other" { role_badge(&d.role) } else { String::new() }, d.files, fmt_n(d.lines as u64), d.langs.join("/"), if nar1.is_empty() { String::new() } else { format!("<span class=\"d\">— {}</span>", esc(nar1)) }));
            if depth < 3 { render(c, &d.path, depth + 1, dirs_nar, out); }
            out.push_str("</li>");
        }
        if kids.len() > 40 { out.push_str(&format!("<li class=\"m\">ほか {} ディレクトリ</li>", kids.len() - 40)); }
        out.push_str("</ul>");
    }
    render(c, "", 1, &dirs_nar, &mut tree);
    tree.push_str("</div>");
    body += &tree;
    let roots: Vec<&Node> = c.g.nodes.iter().filter(|n| n.kind == NodeKind::File && !n.path.contains('/')).collect();
    body += &h2("root 直下のファイル", "");
    body += &table(&["file", "行", "doc"], &roots.iter().map(|n| vec![format!("<a href=\"{}\">{}</a>", href_of(c, &n.id), esc(&n.name)), n.end_line.to_string(), esc(n.doc.lines().next().unwrap_or(""))]).collect::<Vec<_>>(), &[1], &[], "無し");
    body += &explain(nar_list(c, "01", "notes").as_ref(), "解説(Claude)");
    page("01", "ファイルツリー", "", &body, &html_meta(c), &c.changed)
}

// ---------- 02 Architecture ----------
fn p_arch(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "02", "summary").as_ref());
    let layers: Vec<&Node> = nodes_of(c, NodeKind::Layer);
    let lb: Vec<svg::LayerBox> = layers.iter().map(|l| svg::LayerBox { path: l.name.clone(), role: l.subkind.clone(), files: l.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0) as u32, lines: l.attrs.get("lines").and_then(|x| x.as_u64()).unwrap_or(0) as u32, label: String::new(), warn: false }).collect();
    let le: Vec<svg::LEdge> = c.a.layer_edges.iter().map(|e| svg::LEdge { from: e.from.clone(), to: e.to.clone(), count: e.count }).collect();
    let viol: Vec<(String, String)> = c.a.rules.iter().filter(|r| r.kind.starts_with("禁止") && r.count > 0).map(|r| (r.from.clone(), r.to.clone())).collect();
    let cyc: Vec<(String, String)> = c.a.cycles.iter().map(|x| (x.a.clone(), x.b.clone())).collect();
    let viol_n: u32 = c.a.rules.iter().filter(|r| r.kind.starts_with("禁止")).map(|r| r.count).sum();
    body += &tiles(&[("層", layers.len().to_string(), ""), ("import の辺(層間)", c.a.layer_edges.iter().map(|e| e.count).sum::<u32>().to_string(), ""), ("禁止辺の違反", viol_n.to_string(), if viol_n > 0 { "warn" } else { "ok" }), ("層の循環", c.a.cycles.len().to_string(), if c.a.cycles.is_empty() { "ok" } else { "warn" })]);
    body += &fig(&svg::layered(&lb, &le, &viol, &cyc, stamp(c), "", ""), "層の図:箱 = ディレクトリ(src の下 1 段)、辺 = import の本数。赤 = 禁止辺の違反か循環", "");
    body += &h2("依存の規則", "rules");
    if c.a.rules_estimated { body += &note("推定:.roentgen.yml が無いので、ディレクトリ名から役割(entry / ui / application / core / adapter)を推定し、既定の規則を当てた。下の雛形を repo に置けば次回から確定になる。"); }
    body += &table(&["辺", "種類", "本数", "判定", "該当(例)"], &c.a.rules.iter().map(|r| vec![format!("{} → {}", esc(&r.from), esc(&r.to)), badge(r.kind.split('(').next().unwrap_or(""), if r.kind.starts_with("禁止") { "warn" } else { "" }), r.count.to_string(), if r.kind.starts_with("禁止") { if r.count == 0 { badge("守られている", "ok") } else { badge(&format!("違反 {}", r.count), "warn") } } else { String::new() }, r.files.iter().take(3).map(|f| path(f)).collect::<Vec<_>>().join(" ")]).collect::<Vec<_>>(), &[2], &[], "");
    if !c.a.yml_suggestion.is_empty() { body += &details(".roentgen.yml の雛形", &format!("<pre>{}</pre>", esc(&c.a.yml_suggestion))); }
    body += &h3("層の循環(両向きの import)");
    body += &table(&["A", "B", "A→B", "B→A"], &c.a.cycles.iter().map(|x| vec![esc(&x.a), esc(&x.b), x.ab.to_string(), x.ba.to_string()]).collect::<Vec<_>>(), &[2, 3], &[], "層の循環は無い");
    body += &h2("層の実寸", "layers");
    let mut ls: Vec<&Node> = layers.clone(); ls.sort_by_key(|l| std::cmp::Reverse(l.attrs.get("lines").and_then(|x| x.as_u64()).unwrap_or(0)));
    let tested = &c.a.tested_by_layer;
    body += &table(&["層", "役割(推定)", "ファイル", "行数", "関数", "テストから呼ばれる関数"], &ls.iter().map(|l| { let t = tested.get(&l.name).cloned().unwrap_or((0, 0)); vec![format!("<a href=\"{}#dir-{}\">{}</a>", file_of("06"), svg::id(&l.name), esc(&l.name)), role_badge(&l.subkind), l.attrs["files"].to_string(), fmt_n(l.attrs["lines"].as_u64().unwrap_or(0)), t.1.to_string(), if t.1 > 0 { format!("{} ({}%)", t.0, t.0 * 100 / t.1) } else { "-".into() }] }).collect::<Vec<_>>(), &[2, 3, 4], &ls.iter().map(|l| format!("layer-{}", svg::id(&l.name))).collect::<Vec<_>>(), "");
    body += &h3("import の辺(層間・本数順)");
    body += &table(&["from", "to", "本数", "例"], &c.a.layer_edges.iter().take(40).map(|e| vec![esc(&e.from), esc(&e.to), e.count.to_string(), e.files.iter().take(2).map(|f| path(f)).collect::<Vec<_>>().join(" ")]).collect::<Vec<_>>(), &[2], &[], "");
    body += &explain(nar_list(c, "02", "notes").as_ref(), "解説(Claude)");
    page("02", "層と依存の向き", "", &body, &html_meta(c), &c.changed)
}

// ---------- 03 Data Model ----------
fn p_data(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "03", "summary").as_ref());
    let mut tabs: Vec<&Node> = nodes_of(c, NodeKind::Table);
    let deg = |n: &Node| -> usize { outs(c, &n.id, EdgeKind::References).len() + ins(c, &n.id, EdgeKind::References).len() };
    tabs.sort_by(|a, b| deg(b).cmp(&deg(a)).then(a.name.cmp(&b.name)));
    let rw = |n: &Node| -> (usize, usize) { (ins(c, &n.id, EdgeKind::Reads).len(), ins(c, &n.id, EdgeKind::Writes).len()) };
    let er: Vec<svg::ErTable> = tabs.iter().map(|t| { let (r, w) = rw(t); svg::ErTable { name: t.name.clone(), columns: t.attrs.get("columns").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0), fks: outs(c, &t.id, EdgeKind::References).iter().map(|e| e.to.trim_start_matches("table:").to_string()).collect(), rls: t.attrs.get("rls").and_then(|x| x.as_bool()), rows: t.attrs.get("rows").and_then(|x| x.as_i64()), readers: r, writers: w } }).collect();
    let rls_off = tabs.iter().filter(|t| t.attrs.get("rls").and_then(|x| x.as_bool()) == Some(false)).count();
    let migs: Vec<&Value> = c.tables.get("migrations").and_then(|m| m.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
    body += &tiles(&[("表", tabs.len().to_string(), ""), ("出所", if s(&c.tables, "source") == "live" { "実物(--db)" } else { "migration" }.into(), ""), ("migration", migs.len().to_string(), ""), ("RLS 無し", rls_off.to_string(), if rls_off > 0 && rls_off < tabs.len() { "warn" } else { "" }), ("読み書きする関数", c.g.edges.iter().filter(|e| matches!(e.kind, EdgeKind::Reads | EdgeKind::Writes)).map(|e| e.from.as_str()).collect::<HashSet<_>>().len().to_string(), "")]);
    body += &fig(&svg::er(&er, stamp(c)), "ER 図:中核 15 表(FK の次数順)。全表は下の一覧", "");
    body += &fig(&svg::timeline(&migs.iter().map(|m| s(m, "date").to_string()).collect::<Vec<_>>(), migs.len(), stamp(c)), "migration の時間線", "");
    body += &h2("表の一覧", "tables");
    body += &table(&["表", "列", "FK", "RLS", "policy", "読む関数", "書く関数", "最初 〜 最後の migration"], &tabs.iter().map(|t| { let (r, w) = rw(t); vec![format!("<a href=\"#table-{}\"><b>{}</b></a>", svg::id(&t.name), esc(&t.name)), t.attrs.get("columns").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0).to_string(), outs(c, &t.id, EdgeKind::References).iter().map(|e| link(c, &e.to)).collect::<Vec<_>>().join(", "), match t.attrs.get("rls").and_then(|x| x.as_bool()) { Some(true) => badge("RLS", "ok"), Some(false) => if t.attrs.get("policies").and_then(|x| x.as_array()).map(|a| !a.is_empty()).unwrap_or(false) { badge("RLS 無効(policy はある)", "warn") } else { badge("RLS 無し", "warn") }, None => String::new() }, t.attrs.get("policies").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0).to_string(), r.to_string(), w.to_string(), format!("{} 〜 {}", s(&Value::Object(t.attrs.clone().into_iter().collect()), "first_migration"), s(&Value::Object(t.attrs.clone().into_iter().collect()), "last_migration"))] }).collect::<Vec<_>>(), &[1, 4, 5, 6], &[], "表が無い");
    body += &h2("表ごとの詳細(列・読む関数・書く関数)", "detail");
    for t in &tabs {
        let readers: Vec<String> = ins(c, &t.id, EdgeKind::Reads).iter().map(|e| link_path(c, &e.from)).take(30).collect();
        let writers: Vec<String> = ins(c, &t.id, EdgeKind::Writes).iter().map(|e| link_path(c, &e.from)).take(30).collect();
        let pol: Vec<Vec<String>> = t.attrs.get("policies").and_then(|x| x.as_array()).map(|a| a.iter().map(|p| vec![esc(s(p, "name")), esc(s(p, "cmd")), esc(s(p, "roles"))]).collect()).unwrap_or_default();
        let inner = format!("{}{}{}{}", code_at(&t.excerpt, "sql", &t.path, 0), notes_html(c, &t.id), format!("<h4>書く関数 {}</h4><div class=\"chips\">{}</div><h4>読む関数 {}</h4><div class=\"chips\">{}</div>", writers.len(), if writers.is_empty() { "<span class=\"note\">無し(migration だけ・または解決できない呼び出し)</span>".into() } else { writers.join(" ") }, readers.len(), if readers.is_empty() { "<span class=\"note\">無し</span>".into() } else { readers.join(" ") }), if pol.is_empty() { String::new() } else { format!("<h4>policy</h4>{}", table(&["policy", "cmd", "roles"], &pol, &[], &[], "")) });
        body += &format!("<details class=\"node\" id=\"table-{}\"><summary>{}<b>{}</b><span class=\"sig\">{} 列 · {}</span></summary>{}</details>", svg::id(&t.name), badge("table", ""), esc(&t.name), t.attrs.get("columns").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0), match t.attrs.get("rls").and_then(|x| x.as_bool()) { Some(true) => "RLS あり", Some(false) => if t.attrs.get("policies").and_then(|x| x.as_array()).map(|a| !a.is_empty()).unwrap_or(false) { "RLS 無効(policy はあるが効いていない)" } else { "RLS 無し" }, None => "" }, inner);
    }
    body += &h2("migration の置き場", "dirs");
    body += &table(&["ディレクトリ", "種類", "本数"], &c.tables.get("sources").and_then(|x| x.as_array()).map(|a| a.iter().map(|x| vec![path(x.get(0).and_then(|y| y.as_str()).unwrap_or("")), esc(x.get(1).and_then(|y| y.as_str()).unwrap_or("")), migs.iter().filter(|m| s(m, "file").starts_with(x.get(0).and_then(|y| y.as_str()).unwrap_or("\u{0}"))).count().to_string()]).collect::<Vec<Vec<String>>>()).unwrap_or_default(), &[2], &[], "無し");
    body += &explain(nar_list(c, "03", "notes").as_ref(), "解説(Claude)");
    body += &unknown_box(c, &["DB", "migration"]);
    page("03", "DB と表", "", &body, &html_meta(c), &c.changed)
}

// ---------- 04 Entry Points ----------
fn p_entry(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "04", "summary").as_ref());
    let mut routes: Vec<&Node> = nodes_of(c, NodeKind::Route);
    routes.sort_by(|a, b| a.subkind.cmp(&b.subkind).then(a.name.cmp(&b.name)));
    let kinds = ["http", "server_action", "webhook", "cron", "queue", "cli"];
    let counts: Vec<(String, Vec<(String, u32)>)> = kinds.iter().filter(|k| routes.iter().any(|r| r.subkind == **k)).map(|k| (k.to_string(), ["middleware", "inline", "none", "unknown"].iter().map(|a| (a.to_string(), routes.iter().filter(|r| r.subkind == *k && s(&Value::Object(r.attrs.clone().into_iter().collect()), "auth") == *a).count() as u32)).collect())).collect();
    body += &fig(&svg::entry_map(&counts, stamp(c)), "入口の地図:種類ごとの本数と認可の取り方", "");
    let mw = c.cfg.get("middleware");
    body += &note(&format!("middleware:{}。「handler で確認」= handler のファイルに認可の呼び出し(getUser / requireAuth / verify など)があるか、guard / auth 系のモジュールを import している。「認可なし」= どちらも見当たらない(公開 API・内部用・判定漏れのどれかで、AuthZ で個別に見る)。", mw.and_then(|m| m.get(0)).and_then(|x| x.as_str()).map(|f| format!("あり({})", f)).unwrap_or("無し、または認可を含まない".into())));
    body += &h2("入口の一覧(handler のコードつき)", "list");
    body += &note("種類 → path の順。開くと handler のコードと、そこから辿った関数・表・外部が出る(詳細は Call Graph)。");
    let traces: HashMap<&str, &crate::analyze::Trace> = c.a.traces.iter().map(|t| (t.route.as_str(), t)).collect();
    let mut cur_kind = String::new();
    for r in &routes {
        if r.subkind != cur_kind { cur_kind = r.subkind.clone(); body += &h3(&format!("{} ({})", cur_kind, routes.iter().filter(|x| x.subkind == cur_kind).count())); }
        let a = s(&Value::Object(r.attrs.clone().into_iter().collect()), "auth").to_string();
        let t = traces.get(r.id.as_str());
        let handler: Vec<String> = outs(c, &r.id, EdgeKind::Handles).iter().map(|e| link_path(c, &e.to)).collect();
        let mut meta: Vec<(String, String)> = vec![("認可".into(), format!("{}{}", auth_badge(&a), if r.attrs.get("auth_via").and_then(|x| x.as_str()) == Some("import") { badge("guard を import", "") } else { String::new() }))];
        if r.attrs.get("tested").is_some() { meta.push(("Tests".into(), badge("tested", "ok"))); }
        if !handler.is_empty() { meta.push(("Handler".into(), handler.join(" "))); }
        if let Some(t) = t { if !t.steps.is_empty() { meta.push(("Trace".into(), format!("{} 関数(深さ {})· <a href=\"{}#{}\">Call Graph</a>", t.fn_count, t.depth, file_of("05"), anchor(&r.id)))); } if !t.reads.is_empty() { meta.push(("Reads".into(), t.reads.iter().map(|x| format!("<a href=\"{}#table-{}\">{}</a>", file_of("03"), svg::id(x), esc(x))).collect::<Vec<_>>().join(", "))); } if !t.writes.is_empty() { meta.push(("Writes".into(), t.writes.iter().map(|x| format!("<a href=\"{}#table-{}\">{}</a>", file_of("03"), svg::id(x), esc(x))).collect::<Vec<_>>().join(", "))); } if !t.externals.is_empty() { meta.push(("External".into(), t.externals.iter().map(|x| esc(x)).collect::<Vec<_>>().join(", "))); } if t.llm { meta.push(("LLM".into(), badge("LLM", ""))); } }
        let ex = if r.excerpt.is_empty() { String::new() } else { code_at(&r.excerpt.lines().take(40).collect::<Vec<_>>().join("\n"), lang_of_node(r), &r.path, r.line) };
        body += &node_block(&anchor(&r.id), &badge(&r.subkind, ""), &r.name, "", "", &notes_html(c, &r.id), &ex, &meta, false);
    }
    body += &explain(nar_list(c, "04", "notes").as_ref(), "解説(Claude)");
    page("04", "入口", "", &body, &html_meta(c), &c.changed)
}

// ---------- 05 Call Graph ----------
fn p_callgraph(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "05", "summary").as_ref());
    let role_of_lane = |l: &str| -> String { if l == "DB" { "db".into() } else if l.starts_with("外部") || l == "LLM" { if l.contains("LLM") || l.contains("AI") { "ai".into() } else { "external".into() } } else if matches!(l, "利用者" | "ユーザー" | "ブラウザ" | "cron" | "Vercel Cron" | "webhook") { "user".into() } else { role_of(l).into() } };
    if let Some(flows) = nar(c, "05").get("flows").and_then(|f| f.as_array()) {
        body += &h2("代表フロー(Claude が選んで説明したもの)", "flows");
        for (i, f) in flows.iter().take(8).enumerate() {
            body += &h3(&format!("{}. {}", i + 1, s(f, "name")));
            if !s(f, "reason").is_empty() { body += &note(&format!("選んだ理由:{}", s(f, "reason"))); }
            let steps: Vec<svg::Step> = f.get("steps").and_then(|x| x.as_array()).map(|a| a.iter().map(|st| svg::Step { from: s(st, "from").into(), to: s(st, "to").into(), label: s(st, "label").into(), note: s(st, "note").into() }).collect()).unwrap_or_default();
            body += &fig(&svg::sequence(&steps, "", stamp(c), &role_of_lane), s(f, "caption"), "");
            if let Some(route) = f.get("route").and_then(|x| x.as_str()) { if let Some(r) = c.g.nodes.iter().find(|n| n.kind == NodeKind::Route && (n.id == route || n.name == route)) { body += &format!("<p class=\"note\">機械の trace:<a href=\"#{}\">{}</a></p>", anchor(&r.id), esc(&r.name)); } }
            body += &explain(f.get("notes").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect::<Vec<_>>()).as_ref(), "この流れの読み方");
        }
    }
    body += &h2("入口ごとの呼び出しグラフ(機械)", "traces");
    body += &note("入口の handler から解決できた呼び出しを深さ 8 まで辿ったもの。関数の箱をクリックで Modules のコードへ。右端は読み書きする表(赤 = write)と外部。表を書く入口・深い入口を先に並べる。");
    let mut traces: Vec<&crate::analyze::Trace> = c.a.traces.iter().filter(|t| !t.steps.is_empty()).collect();
    traces.sort_by(|a, b| (b.writes.len() * 3 + b.reads.len() + b.externals.len() * 2 + b.fn_count).cmp(&(a.writes.len() * 3 + a.reads.len() + a.externals.len() * 2 + a.fn_count)));
    let shown = traces.len().min(60);
    for t in traces.iter().take(shown) {
        let Some(r) = node(c, &t.route) else { continue };
        let step_ids: HashSet<&str> = t.steps.iter().map(|s| s.id.as_str()).collect();
        let nodes: Vec<svg::CgNode> = t.steps.iter().map(|st| svg::CgNode { id: st.id.clone(), name: st.name.clone(), layer: st.layer.clone(), role: role_of(&st.layer).into(), depth: st.depth, href: href_of(c, &st.id), kind: st.kind.clone() }).collect();
        let edges: Vec<(String, String)> = t.steps.iter().flat_map(|st| outs(c, &st.id, EdgeKind::Calls).into_iter().filter(|e| step_ids.contains(e.to.as_str())).map(|e| (e.from.clone(), e.to.clone())).collect::<Vec<_>>()).collect();
        let tabs: Vec<(String, String, bool)> = t.steps.iter().flat_map(|st| { let mut v: Vec<(String, String, bool)> = outs(c, &st.id, EdgeKind::Writes).iter().map(|e| (st.id.clone(), e.to.trim_start_matches("table:").to_string(), true)).collect(); v.extend(outs(c, &st.id, EdgeKind::Reads).iter().map(|e| (st.id.clone(), e.to.trim_start_matches("table:").to_string(), false))); v }).collect();
        let exts: Vec<(String, String)> = t.steps.iter().flat_map(|st| outs(c, &st.id, EdgeKind::Uses).iter().filter(|e| e.to.starts_with("ext:")).map(|e| (st.id.clone(), node(c, &e.to).map(|n| n.name.clone()).unwrap_or(e.to.clone()))).collect::<Vec<_>>()).collect();
        let a = s(&Value::Object(r.attrs.clone().into_iter().collect()), "auth").to_string();
        body += &format!("<h3 id=\"{}\">{} {} {}</h3>", anchor(&r.id), badge(&r.subkind, ""), esc(&r.name), auth_badge(&a));
        body += &fig(&svg::callgraph(&nodes, &edges, &tabs, &exts, stamp(c)), "", "");
        let mut steps_html = String::from("<div class=\"steps\">");
        for st in t.steps.iter().take(12) { if let Some(n) = node(c, &st.id) { steps_html += &format!("<div class=\"step\"><div>{}</div></div>", fn_block(c, n, false, 40)); } }
        steps_html += "</div>";
        body += &details(&format!("関数のコード({} 本)", t.steps.len().min(12)), &steps_html);
    }
    if traces.len() > shown { body += &note(&format!("ほか {} 本の入口の trace は Entry Points の各行から辿れる(query 'route <path>' でも引ける)。", traces.len() - shown)); }
    body += &explain(nar_list(c, "05", "notes").as_ref(), "解説(Claude)");
    page("05", "呼び出しの流れ", "", &body, &html_meta(c), &c.changed)
}

// ---------- 06 Modules ----------
fn p_modules(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "06", "summary").as_ref());
    body += &note("層 → ディレクトリ → ファイルの順。各ファイルは import / 被 import、公開している関数と型、関数ごとのコード抜粋。被 import が多いファイルほど上。巨大 repo では公開関数と呼ばれ方の多い関数を優先して載せ、残りは名前だけ。");
    let mut files: Vec<&Node> = nodes_of(c, NodeKind::File);
    let imported_by = |n: &Node| ins(c, &n.id, EdgeKind::Imports).len();
    files.sort_by(|a, b| a.layer.cmp(&b.layer).then(crate::repo::is_test(&a.path).cmp(&crate::repo::is_test(&b.path))).then(a.path.rsplit_once('/').map(|x| x.0).unwrap_or("").cmp(b.path.rsplit_once('/').map(|x| x.0).unwrap_or(""))).then(imported_by(b).cmp(&imported_by(a))).then(a.path.cmp(&b.path)));
    let in_deg: HashMap<&str, usize> = { let mut m = HashMap::new(); for e in &c.g.edges { if e.kind == EdgeKind::Calls { *m.entry(e.to.as_str()).or_insert(0) += 1; } } m };
    let mut budget: i64 = 2500;
    let mut cur_layer = String::new();
    let mut emitted: HashSet<String> = HashSet::new();
    for f in &files {
        if f.layer != cur_layer { cur_layer = f.layer.clone(); body += &h2_raw(&format!("{} {}", esc(&cur_layer), role_badge(role_of(&cur_layer))), &format!("dir-{}", svg::id(&cur_layer))); emitted.insert(cur_layer.clone()); }
        // ディレクトリの見出し(深さ 3 まで。File Tree からの anchor の受け口)
        let parts: Vec<&str> = f.path.split('/').collect();
        for depth in 1..parts.len().min(4) { let d = format!("{}/", parts[..depth].join("/")); if !emitted.contains(&d) && d != cur_layer && d.starts_with(&cur_layer) { emitted.insert(d.clone()); body += &format!("<h3 id=\"dir-{}\">{}</h3>", svg::id(&d), esc(&d)); } }
        let is_t = crate::repo::is_test(&f.path);
        let fns: Vec<&Node> = outs(c, &f.id, EdgeKind::Contains).iter().filter_map(|e| node(c, &e.to)).filter(|n| n.kind == NodeKind::Function).collect();
        let tys: Vec<&Node> = outs(c, &f.id, EdgeKind::Contains).iter().filter_map(|e| node(c, &e.to)).filter(|n| n.kind == NodeKind::Type).collect();
        let imports: Vec<String> = outs(c, &f.id, EdgeKind::Imports).iter().map(|e| node(c, &e.to).map(|n| if n.kind == NodeKind::Package { format!("<code>{}</code>", esc(&n.name)) } else { format!("<a href=\"#{}\">{}</a>", anchor(&e.to), esc(&n.path)) }).unwrap_or_default()).collect();
        let importers: Vec<String> = ins(c, &f.id, EdgeKind::Imports).iter().filter_map(|e| node(c, &e.from)).map(|n| format!("<a href=\"#{}\">{}</a>", anchor(&n.id), esc(&n.path))).take(20).collect();
        let mut shown: Vec<&Node> = if is_t { vec![] } else { fns.iter().copied().filter(|n| n.exported || *in_deg.get(n.id.as_str()).unwrap_or(&0) >= 2).collect() };
        shown.sort_by(|a, b| in_deg.get(b.id.as_str()).unwrap_or(&0).cmp(in_deg.get(a.id.as_str()).unwrap_or(&0)).then(a.line.cmp(&b.line)));
        let take = if budget > 0 { shown.len().min(25) } else { 0 };
        budget -= take as i64;
        let fn_html: String = shown.iter().take(take).map(|n| fn_block(c, n, false, 30)).collect();
        let rest: Vec<String> = fns.iter().filter(|n| !shown.iter().take(take).any(|x| x.id == n.id)).map(|n| format!("<code id=\"{}\">{}</code>", anchor(&n.id), esc(&n.name))).collect();
        let ty_html: String = if tys.is_empty() { String::new() } else { format!("<h4>型</h4><div class=\"chips\">{}</div>", tys.iter().map(|t| format!("<a href=\"{}#{}\">{}</a>", file_of("07"), anchor(&t.id), esc(&t.name))).collect::<Vec<_>>().join(" ")) };
        let tested_targets: Vec<String> = if is_t { fns.iter().flat_map(|n| outs(c, &n.id, EdgeKind::Calls).into_iter().filter(|e| !crate::repo::is_test(&e.to)).map(|e| link_path(c, &e.to)).collect::<Vec<_>>()).collect::<HashSet<_>>().into_iter().take(20).collect() } else { vec![] };
        let header_meta = format!("{} 行 · 関数 {} · 型 {} · import {} · 被 import {}{}{}{}", f.end_line, fns.len(), tys.len(), imports.len(), importers.len(), if f.attrs.get("directives").is_some() { format!(" · {}", badge(&f.attrs["directives"].to_string(), "")) } else { String::new() }, if f.attrs.get("parse_error").is_some() { badge("構文の一部を読めず", "warn") } else { String::new() }, if is_t { badge("test", "") } else { String::new() });
        let inner = format!("{}{}{}{}{}{}{}", if f.doc.is_empty() { String::new() } else { format!("<div class=\"doc\">{}</div>", esc(&f.doc)) }, notes_html(c, &f.id), if imports.is_empty() && importers.is_empty() { String::new() } else { format!("<div class=\"note\">import → {}<br>被 import ← {}</div>", imports.join(", "), if importers.is_empty() { "無し".into() } else { importers.join(", ") }) }, if tested_targets.is_empty() { String::new() } else { format!("<div class=\"note\">テストしている関数:{}</div>", tested_targets.join(" ")) }, ty_html, fn_html, if rest.is_empty() { String::new() } else { format!("<div class=\"note\">{}:{}</div>", if is_t { "テスト関数" } else { "ほかの関数" }, rest.join(" ")) });
        body += &format!("<details class=\"node\" id=\"{}\"><summary>{}<b>{}</b><span class=\"sig\">{}</span></summary>{}</details>", anchor(&f.id), badge(&f.lang, ""), esc(&f.path), header_meta, inner);
    }
    body += &explain(nar_list(c, "06", "notes").as_ref(), "解説(Claude)");
    page("06", "モジュール", "", &body, &html_meta(c), &c.changed)
}

// ---------- 07 Types ----------
fn p_types(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "07", "summary").as_ref());
    let mut tys: Vec<&Node> = nodes_of(c, NodeKind::Type).into_iter().filter(|n| !crate::repo::is_test(&n.path)).collect();
    let referenced = |n: &Node| ins(c, &n.id, EdgeKind::References).len() + ins(c, &n.id, EdgeKind::Extends).len() + ins(c, &n.id, EdgeKind::Implements).len();
    tys.sort_by(|a, b| referenced(b).cmp(&referenced(a)).then(a.name.cmp(&b.name)));
    let core: Vec<&Node> = tys.iter().copied().filter(|n| role_of(&n.layer) == "core").collect();
    let pick: Vec<&Node> = if core.len() >= 8 { core.iter().copied().take(30).collect() } else { tys.iter().copied().take(30).collect() };
    let names: HashSet<&str> = pick.iter().map(|n| n.name.as_str()).collect();
    let cn: Vec<svg::ClassNode> = pick.iter().map(|n| svg::ClassNode { name: n.name.clone(), kind: n.subkind.clone(), file: n.path.clone(), role: role_of(&n.layer).into(), extends: outs(c, &n.id, EdgeKind::Extends).iter().filter_map(|e| node(c, &e.to)).map(|x| x.name.clone()).filter(|x| names.contains(x.as_str())).collect(), implements: outs(c, &n.id, EdgeKind::Implements).iter().filter_map(|e| node(c, &e.to)).map(|x| x.name.clone()).filter(|x| names.contains(x.as_str())).collect(), refs: outs(c, &n.id, EdgeKind::References).iter().filter_map(|e| node(c, &e.to)).map(|x| x.name.clone()).filter(|x| names.contains(x.as_str())).collect(), referenced: referenced(n) as u32, href: format!("#{}", anchor(&n.id)) }).collect();
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new(); for t in &tys { *kinds.entry(t.subkind.as_str()).or_insert(0) += 1; }
    body += &tiles(&kinds.iter().map(|(k, v)| (*k, v.to_string(), "")).collect::<Vec<_>>());
    body += &fig(&svg::classdiag(&cn, stamp(c)), if core.len() >= 8 { "クラス図:core / domain 層の型(被参照の多い順に 30)。白抜き = 継承、破線 = 実装、細線 = 参照" } else { "クラス図:被参照の多い型 30" }, "");
    body += &h2("型の定義(被参照の多い順)", "list");
    body += &note("定義の本文そのもの。extends / implements / 参照する型 / 参照される型はリンク。");
    let mut cur = String::new();
    for t in tys.iter().take(400) { if t.layer != cur { cur = t.layer.clone(); body += &format!("<h3>{} {}</h3>", esc(&cur), role_badge(role_of(&cur))); } body += &type_block(c, t, false); }
    if tys.len() > 400 { body += &note(&format!("ほか {} 型(query 'type <name>' で引ける)", tys.len() - 400)); }
    body += &explain(nar_list(c, "07", "notes").as_ref(), "解説(Claude)");
    page("07", "型", "", &body, &html_meta(c), &c.changed)
}

// ---------- 08 Dependencies ----------
fn p_deps(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "08", "summary").as_ref());
    let mut pk: Vec<&Node> = nodes_of(c, NodeKind::Package); pk.sort_by_key(|n| std::cmp::Reverse(n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0)));
    let stack: Vec<&Value> = c.cfg.get("stack").and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
    let deps: Vec<&Value> = c.cfg.get("deps").and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
    body += &tiles(&[("import されるパッケージ", pk.len().to_string(), ""), ("マニフェストの依存", deps.len().to_string(), ""), ("runtime 依存", deps.iter().filter(|d| !d.get("dev").and_then(|x| x.as_bool()).unwrap_or(false)).count().to_string(), "")]);
    body += &h2("スタック(依存から判定)", "stack");
    body += &table(&["区分", "名前", "依存", "版"], &stack.iter().map(|x| vec![esc(s(x, "category")), esc(s(x, "name")), path(s(x, "dep")), esc(s(x, "version"))]).collect::<Vec<_>>(), &[], &[], "");
    body += &h2("import されるパッケージ(ファイル数順)", "packages");
    let ver: HashMap<&str, &str> = deps.iter().map(|d| (s(d, "name"), s(d, "version"))).collect();
    body += &table(&["パッケージ", "区分", "import するファイル", "層", "版"], &pk.iter().map(|n| { let layers: Vec<String> = ins(c, &n.id, EdgeKind::Imports).iter().filter_map(|e| node(c, &e.from)).map(|f| f.layer.clone()).collect::<HashSet<_>>().into_iter().collect(); vec![path(&n.name), esc(&n.subkind), n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0).to_string(), layers.iter().take(5).cloned().collect::<Vec<_>>().join(", "), esc(ver.get(n.name.as_str()).unwrap_or(&""))] }).collect::<Vec<_>>(), &[2], &[], "");
    body += &details(&format!("マニフェストの依存 {}", deps.len()), &table(&["名前", "版", "manifest", "dev"], &deps.iter().map(|d| vec![path(s(d, "name")), esc(s(d, "version")), esc(s(d, "manifest")), if d.get("dev").and_then(|x| x.as_bool()).unwrap_or(false) { "dev".into() } else { String::new() }]).collect::<Vec<_>>(), &[], &[], ""));
    body += &explain(nar_list(c, "08", "notes").as_ref(), "解説(Claude)");
    page("08", "外部パッケージ", "", &body, &html_meta(c), &c.changed)
}

// ---------- 09 Hotspots ----------
fn p_hotspots(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "09", "summary").as_ref());
    body += &tiles(&[("90 日の commit", c.a.commits.to_string(), ""), ("作者", c.a.authors.len().to_string(), ""), ("merge 済 PR(直近)", c.a.prs.len().to_string(), ""), ("1,000 行超", c.a.big.len().to_string(), if c.a.big.is_empty() { "ok" } else { "warn" })]);
    let layers: Vec<svg::TmLayer> = nodes_of(c, NodeKind::Layer).iter().map(|l| svg::TmLayer { path: l.name.clone(), lines: l.attrs.get("lines").and_then(|x| x.as_u64()).unwrap_or(0) as u32 }).collect();
    let files: Vec<svg::TmFile> = c.a.churn.iter().map(|f| svg::TmFile { path: f.path.clone(), layer: f.layer.clone(), count: f.count, lines: f.lines, href: format!("{}#{}", file_of("06"), anchor(&id_of(NodeKind::File, &f.path, ""))) }).collect();
    body += &fig(&svg::treemap(&layers, &files, stamp(c)), "ヒートマップ:面積 = 層の行数、濃さ = 変更の多さ、中の箱 = 変更の多いファイル(クリックで Modules へ)", "");
    body += &fig(&svg::weekly_bars(&c.a.weekly, stamp(c)), "13 週の活動", "");
    body += &h2("変更の集中(90 日・上位)", "churn");
    body += &table(&["path", "変更回数", "行数", "関数", "", "作者(上位 3)", "最後"], &c.a.churn.iter().take(60).map(|f| vec![format!("<a href=\"{}#{}\">{}</a>", file_of("06"), anchor(&id_of(NodeKind::File, &f.path, "")), esc(&f.path)), f.count.to_string(), f.lines.to_string(), f.functions.to_string(), if f.lines >= 1000 { badge("1,000 行超", "warn") } else { String::new() }, esc(&f.authors.join(", ")), esc(&f.last)]).collect::<Vec<_>>(), &[1, 2, 3], &[], "git の履歴が無い");
    body += &h2("直近の PR", "prs");
    body += &table(&["#", "タイトル", "作者", "merge", "触った層", "files", "+/-"], &c.a.prs.iter().map(|p| vec![p.number.to_string(), esc(&p.title), esc(&p.author), esc(&p.merged), p.layers.join(", "), p.files.to_string(), format!("+{} -{}", p.additions, p.deletions)]).collect::<Vec<_>>(), &[0, 5], &[], "PR を取れなかった(gh が無いか、remote が GitHub ではない)");
    body += &h2("誰が触っているか(ハンドル)", "authors");
    body += &table(&["作者", "commit"], &c.a.authors.iter().map(|(a, n)| vec![esc(a), n.to_string()]).collect::<Vec<_>>(), &[1], &[], "");
    body += &explain(nar_list(c, "09", "notes").as_ref(), "解説(Claude)");
    body += &unknown_box(c, &["PR"]);
    page("09", "変更の集中", "", &body, &html_meta(c), &c.changed)
}

// ---------- 10 Tech Debt ----------
fn p_debt(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "10", "summary").as_ref());
    let risks: Vec<Value> = nar(c, "10").get("risks").and_then(|r| r.as_array()).cloned().unwrap_or_else(|| auto_risks(c));
    if nar(c, "10").get("risks").is_none() { body += &note("順位は機械の既定(巨大 × churn → 認可なしの入口 → RLS 無し → 禁止辺 → 循環 → 解決できない呼び出しの多さ)。Claude が narrative.json の 10.risks に書けば置き換わる。"); }
    let mut per_layer: BTreeMap<String, u32> = BTreeMap::new();
    for r in &risks { let p = s(r, "path"); let l = if p.contains('/') { crate::repo::layer_of(p) } else { s(r, "layer").to_string() }; if !l.is_empty() { *per_layer.entry(l).or_insert(0) += 1; } }
    let lb: Vec<svg::LayerBox> = nodes_of(c, NodeKind::Layer).iter().map(|l| svg::LayerBox { path: l.name.clone(), role: l.subkind.clone(), files: l.attrs["files"].as_u64().unwrap_or(0) as u32, lines: l.attrs["lines"].as_u64().unwrap_or(0) as u32, label: match per_layer.get(&l.name) { Some(n) => format!("{}  ⚠{}", l.name, n), None => l.name.clone() }, warn: per_layer.contains_key(&l.name) }).collect();
    body += &fig(&svg::layered(&lb, &[], &[], &[], stamp(c), file_of("02"), "層ごとのリスク件数(⚠)。赤枠 = リスクがある層"), "リスクの地図", "");
    body += &h2("順位", "ranked");
    body += &table(&["#", "種類", "場所", "なぜ危ないか", "根拠"], &risks.iter().enumerate().map(|(i, r)| vec![(i + 1).to_string(), badge(s(r, "kind"), ""), if s(r, "link").is_empty() { esc(s(r, "path")) } else { format!("<a href=\"{}\">{}</a>", esc(s(r, "link")), esc(s(r, "path"))) }, esc(s(r, "why")), esc(s(r, "evidence"))]).collect::<Vec<_>>(), &[0], &[], "");
    body += &h2("根拠の表", "facts");
    body += &h3("巨大ファイル × 変更回数");
    body += &table(&["path", "行数", "90 日の変更"], &c.a.big.iter().map(|(p, l, ch)| vec![format!("<a href=\"{}#{}\">{}</a>", file_of("06"), anchor(&id_of(NodeKind::File, p, "")), esc(p)), l.to_string(), ch.to_string()]).collect::<Vec<_>>(), &[1, 2], &[], "無し");
    body += &h3("認可の無い入口(cli を除く)");
    let un: Vec<&Node> = nodes_of(c, NodeKind::Route).into_iter().filter(|r| s(&Value::Object(r.attrs.clone().into_iter().collect()), "auth") == "none" && r.subkind != "cli").collect();
    body += &table(&["種類", "入口", "file"], &un.iter().take(60).map(|r| vec![badge(&r.subkind, ""), format!("<a href=\"{}#{}\">{}</a>", file_of("04"), anchor(&r.id), esc(&r.name)), path(&r.path)]).collect::<Vec<_>>(), &[], &[], "無し");
    body += &h3("解決できない呼び出し(多い順。動的 import・外部 SDK・メソッド呼び出し)");
    body += &table(&["呼び出し", "回数"], &c.a.unresolved_top.iter().take(30).map(|(k, n)| vec![format!("<code>{}</code>", esc(k)), n.to_string()]).collect::<Vec<_>>(), &[1], &[], "");
    body += &h3("テストから呼ばれる関数の割合(層ごと)");
    body += &table(&["層", "関数", "テストから呼ばれる", "割合"], &c.a.tested_by_layer.iter().map(|(l, (t, n))| vec![esc(l), n.to_string(), t.to_string(), if *n > 0 { format!("{}%", t * 100 / n) } else { "-".into() }]).collect::<Vec<_>>(), &[1, 2, 3], &[], "");
    body += &explain(nar_list(c, "10", "notes").as_ref(), "解説(Claude)");
    page("10", "負債とリスク", "", &body, &html_meta(c), &c.changed)
}
fn auto_risks(c: &Ctx) -> Vec<Value> {
    let mut out = Vec::new();
    let mut big: Vec<&(String, u32, u32)> = c.a.big.iter().collect(); big.sort_by_key(|b| std::cmp::Reverse(b.1 as u64 * (1 + b.2 as u64)));
    for b in big.iter().take(5) { out.push(serde_json::json!({"kind": "巨大 × 変更", "path": b.0, "why": format!("{} 行で 90 日に {} 回変わっている。衝突と退行の起点", fmt_n(b.1 as u64), b.2), "link": format!("{}#churn", file_of("09")), "evidence": "Hotspots"})); }
    for r in nodes_of(c, NodeKind::Route).into_iter().filter(|r| s(&Value::Object(r.attrs.clone().into_iter().collect()), "auth") == "none" && matches!(r.subkind.as_str(), "http" | "server_action" | "webhook")).take(5) { out.push(serde_json::json!({"kind": "認可なし", "path": r.name, "why": "middleware も handler の確認も見当たらない(公開 API なら明記が要る)", "link": format!("{}#{}", file_of("04"), anchor(&r.id)), "evidence": "Entry Points"})); }
    let tabs = nodes_of(c, NodeKind::Table);
    if tabs.iter().any(|t| t.attrs.get("rls").and_then(|x| x.as_bool()) == Some(true)) { for t in tabs.iter().filter(|t| t.attrs.get("rls").and_then(|x| x.as_bool()) == Some(false)).take(4) { out.push(serde_json::json!({"kind": "RLS 無し", "path": t.name, "why": "他の表が RLS を使っているのにこの表は無い", "link": format!("{}#table-{}", file_of("03"), svg::id(&t.name)), "evidence": "Data Model"})); } }
    for r in c.a.rules.iter().filter(|r| r.kind.starts_with("禁止") && r.count > 0) { out.push(serde_json::json!({"kind": "禁止辺", "path": format!("{} → {}", r.from, r.to), "why": format!("層の規則に反する import が {} 本", r.count), "link": format!("{}#rules", file_of("02")), "evidence": "Architecture"})); }
    for x in c.a.cycles.iter().take(3) { out.push(serde_json::json!({"kind": "循環", "path": format!("{} ⇄ {}", x.a, x.b), "why": "層が両向きに依存していて、片方だけ変えられない", "link": format!("{}#rules", file_of("02")), "evidence": "Architecture"})); }
    out
}

// ---------- 11 Dev Setup & CI ----------
fn p_dev(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "11", "summary").as_ref());
    let sc: BTreeMap<String, Value> = c.cfg.get("scripts").and_then(|x| x.as_object()).map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    let ci: Vec<&Value> = c.cfg.get("ci").and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
    let conv = c.cfg.get("conventions").cloned().unwrap_or(Value::Null);
    let has = |k: &str| conv.get(k).and_then(|x| x.as_array()).map(|a| !a.is_empty()).unwrap_or(false);
    let cmd = |k: &str| sc.get(k).and_then(|v| v.get(0)).and_then(|x| x.as_str()).unwrap_or("").chars().take(28).collect::<String>();
    let stages: Vec<(String, String, Option<bool>)> = vec![("commit".into(), if has("hooks") { "pre-commit hook".into() } else { "hook 無し".into() }, if has("hooks") { Some(true) } else { None }), ("lint / typecheck".into(), cmd("lint"), if sc.contains_key("lint") || sc.contains_key("typecheck") { Some(true) } else { None }), ("test".into(), cmd("test"), if sc.contains_key("test") { Some(true) } else { None }), ("PR".into(), if has("pr_template") { "template あり".into() } else { String::new() }, if !ci.is_empty() { Some(true) } else { None }), ("CI".into(), ci.iter().map(|x| s(x, "name")).collect::<Vec<_>>().join(", ").chars().take(30).collect(), if ci.is_empty() { None } else { Some(true) }), ("deploy".into(), c.cfg.get("hosting").and_then(|h| h.as_array()).map(|a| a.iter().filter_map(|x| x.get(0).and_then(|y| y.as_str())).collect::<Vec<_>>().join(", ")).unwrap_or_default().chars().take(28).collect(), if ci.iter().any(|x| x.get("deploy").and_then(|d| d.as_bool()).unwrap_or(false)) || c.cfg.get("hosting").and_then(|h| h.as_array()).map(|a| !a.is_empty()).unwrap_or(false) { Some(true) } else { None })];
    body += &fig(&svg::pipeline(&stages, stamp(c)), "コードが本番に届く道。点線 = 見当たらない段", "");
    body += &h2("コマンド", "scripts");
    body += &table(&["名前", "コマンド", "出所"], &sc.iter().map(|(k, v)| vec![esc(k), format!("<code>{}</code>", esc(v.get(0).and_then(|x| x.as_str()).unwrap_or(""))), esc(v.get(1).and_then(|x| x.as_str()).unwrap_or(""))]).collect::<Vec<_>>(), &[], &[], "");
    body += &h2("ランタイム・ホスティング", "runtime");
    body += &table(&["種類", "名前", "版 / 根拠"], &c.cfg.get("runtime").and_then(|x| x.as_array()).map(|a| a.iter().map(|x| vec!["runtime".into(), esc(x.get(0).and_then(|y| y.as_str()).unwrap_or("")), esc(x.get(1).and_then(|y| y.as_str()).unwrap_or(""))]).collect::<Vec<Vec<String>>>()).unwrap_or_default().into_iter().chain(c.cfg.get("hosting").and_then(|x| x.as_array()).map(|a| a.iter().map(|x| vec!["hosting".into(), esc(x.get(0).and_then(|y| y.as_str()).unwrap_or("")), esc(x.get(1).and_then(|y| y.as_str()).unwrap_or(""))]).collect::<Vec<Vec<String>>>()).unwrap_or_default()).collect::<Vec<_>>(), &[], &[], "");
    body += &h2("CI", "ci");
    body += &table(&["名前", "トリガー", "ジョブ", "deploy", "file"], &ci.iter().map(|x| vec![esc(s(x, "name")), x.get("triggers").and_then(|t| t.as_array()).map(|a| a.iter().filter_map(|y| y.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(), x.get("jobs").and_then(|t| t.as_array()).map(|a| a.iter().filter_map(|y| y.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(), if x.get("deploy").and_then(|d| d.as_bool()).unwrap_or(false) { "あり".into() } else { String::new() }, path(s(x, "file"))]).collect::<Vec<_>>(), &[], &[], "CI の定義が見当たらない");
    body += &h2("規約・AI 向けの文書", "conventions");
    let row = |k: &str, label: &str| vec![label.to_string(), conv.get(k).and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|y| y.as_str()).map(|p| path(p)).collect::<Vec<_>>().join(" ")).filter(|s| !s.is_empty()).unwrap_or("無し".into())];
    body += &table(&["項目", "内容"], &[row("contributing", "CONTRIBUTING"), row("agent_docs", "CLAUDE.md / AGENTS.md"), row("skills", "skills(.claude/skills)"), row("hooks", "hooks"), row("pr_template", "PR テンプレ"), row("codeowners", "CODEOWNERS"), row("lint_config", "lint の設定"), row("adr", "ADR / decisions")], &[], &[], "");
    body += &h2("環境変数(キー名のみ・値は読まない)", "env");
    let mut envs: Vec<&Node> = nodes_of(c, NodeKind::Env); envs.sort_by_key(|n| std::cmp::Reverse(n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0)));
    let has_ex = c.cfg.get("has_env_example").and_then(|x| x.as_bool()).unwrap_or(false);
    let undeclared = envs.iter().filter(|n| n.subkind == "undeclared").count();
    body += &tiles(&[("キー", envs.len().to_string(), ""), ("example に無い", if has_ex { undeclared.to_string() } else { "-".into() }, if has_ex && undeclared > 0 { "warn" } else { "" })]);
    if !has_ex { body += &note(".env.example が無いので「宣言されているか」は判定できない。"); }
    body += &table(&["キー", "読むファイル", "例", ""], &envs.iter().take(120).map(|n| vec![path(&n.name), n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0).to_string(), ins(c, &n.id, EdgeKind::Uses).iter().take(3).filter_map(|e| node(c, &e.from)).map(|f| format!("<a href=\"{}#{}\">{}</a>", file_of("06"), anchor(&f.id), esc(&f.path))).collect::<Vec<_>>().join(" "), if has_ex && n.subkind == "undeclared" { badge("example に無し", "warn") } else { String::new() }]).collect::<Vec<_>>(), &[1], &[], "");
    body += &explain(nar_list(c, "11", "notes").as_ref(), "解説(Claude)");
    page("11", "開発と CI", "", &body, &html_meta(c), &c.changed)
}

// ---------- 12 Integrations ----------
fn p_integrations(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "12", "summary").as_ref());
    let mut exts: Vec<&Node> = nodes_of(c, NodeKind::External).into_iter().filter(|n| n.subkind != "llm").collect();
    exts.sort_by_key(|n| std::cmp::Reverse(n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0)));
    let hooks: Vec<&Node> = nodes_of(c, NodeKind::Route).into_iter().filter(|r| r.subkind == "webhook").collect();
    let saas: Vec<&Node> = nodes_of(c, NodeKind::Package).into_iter().filter(|p| matches!(p.subkind.as_str(), "saas" | "ai")).collect();
    body += &tiles(&[("外部 host", exts.len().to_string(), ""), ("webhook 受信", hooks.len().to_string(), ""), ("SaaS / AI の SDK", saas.len().to_string(), "")]);
    let center = vec![svg::HubNode { title: s(&c.meta, "name").into(), role: "entry".into(), main: true, ..Default::default() }];
    let left: Vec<svg::HubNode> = if hooks.is_empty() { vec![svg::HubNode { title: "webhook 受信 無し".into(), unknown: true, ..Default::default() }] } else { hooks.iter().take(7).map(|r| svg::HubNode { title: r.name.clone(), sub: r.path.rsplit('/').next().unwrap_or("").into(), href: format!("{}#{}", file_of("04"), anchor(&r.id)), ..Default::default() }).collect() };
    let right: Vec<svg::HubNode> = exts.iter().take(9).map(|n| svg::HubNode { title: n.name.clone(), sub: format!("{} files", n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0)), count: n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0) as u32, href: format!("#{}", anchor(&n.id)), ..Default::default() }).collect();
    let bottom: Vec<svg::HubNode> = saas.iter().take(4).map(|p| svg::HubNode { title: p.name.clone(), sub: format!("{} files", p.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0)), role: "db".into(), href: file_of("08").into(), ..Default::default() }).collect();
    body += &fig(&svg::hub(&center, &left, &right, &bottom, stamp(c), "受ける(webhook)", "呼ぶ(外部 API)", "SDK"), "外部連携図:左 = 受ける、右 = 呼ぶ(ファイル数)、下 = SDK", "");
    body += &h2("外部 API(host 単位・呼ぶ関数つき)", "hosts");
    for n in &exts {
        let callers: Vec<String> = ins(c, &n.id, EdgeKind::Uses).iter().map(|e| link_path(c, &e.from)).take(30).collect();
        let paths: Vec<String> = n.attrs.get("paths").and_then(|p| p.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).map(|p| format!("<code>{}</code>", esc(p))).collect()).unwrap_or_default();
        body += &format!("<details class=\"node\" id=\"{}\"><summary>{}<b>{}</b><span class=\"sig\">{} · {} files</span></summary>{}<div class=\"note\">path(例):{}</div><h4>呼ぶ関数</h4><div class=\"chips\">{}</div></details>", anchor(&n.id), badge(s(&Value::Object(n.attrs.clone().into_iter().collect()), "category"), ""), esc(&n.name), esc(&n.path), n.attrs.get("files").and_then(|x| x.as_u64()).unwrap_or(0), notes_html(c, &n.id), if paths.is_empty() { "-".into() } else { paths.join(" ") }, if callers.is_empty() { "<span class=\"note\">(ファイルのモジュールレベル)</span>".into() } else { callers.join(" ") });
    }
    body += &h2("webhook の受け口", "webhooks");
    body += &table(&["入口", "認可(署名検証など)", "file"], &hooks.iter().map(|r| vec![format!("<a href=\"{}#{}\">{}</a>", file_of("04"), anchor(&r.id), esc(&r.name)), auth_badge(s(&Value::Object(r.attrs.clone().into_iter().collect()), "auth")), path(&r.path)]).collect::<Vec<_>>(), &[], &[], "無し");
    body += &explain(nar_list(c, "12", "notes").as_ref(), "解説(Claude)");
    page("12", "外部連携", "", &body, &html_meta(c), &c.changed)
}

// ---------- 13 AuthZ ----------
fn p_authz(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "13", "summary").as_ref());
    let routes = nodes_of(c, NodeKind::Route);
    let kinds: Vec<String> = ["http", "server_action", "webhook", "cron", "queue", "cli"].iter().filter(|k| routes.iter().any(|r| r.subkind == **k)).map(|k| k.to_string()).collect();
    let cols: Vec<String> = ["middleware", "inline", "none"].iter().map(|x| x.to_string()).collect();
    let tabs = nodes_of(c, NodeKind::Table);
    let rls_on: Vec<&Node> = tabs.iter().copied().filter(|t| t.attrs.get("rls").and_then(|x| x.as_bool()) == Some(true)).collect();
    let rls_off: Vec<&Node> = tabs.iter().copied().filter(|t| t.attrs.get("rls").and_then(|x| x.as_bool()) == Some(false)).collect();
    let none_n = routes.iter().filter(|r| s(&Value::Object(r.attrs.clone().into_iter().collect()), "auth") == "none" && r.subkind != "cli").count();
    body += &tiles(&[("role", c.a.roles.len().to_string(), ""), ("認可なしの入口", none_n.to_string(), if none_n > 0 { "warn" } else { "ok" }), ("RLS あり / 無し", format!("{} / {}", rls_on.len(), rls_off.len()), ""), ("policy", tabs.iter().map(|t| t.attrs.get("policies").and_then(|x| x.as_array()).map(|a| a.len()).unwrap_or(0)).sum::<usize>().to_string(), "")]);
    let cell = |r: &str, col: &str| -> (String, String) { let n = routes.iter().filter(|x| x.subkind == r && s(&Value::Object(x.attrs.clone().into_iter().collect()), "auth") == col).count(); (if n > 0 { n.to_string() } else { String::new() }, if n == 0 { "var(--code)".into() } else if col == "none" { "var(--warn-soft)".into() } else if col == "middleware" { "var(--ok-soft)".into() } else { "var(--l-app)".into() }) };
    body += &fig(&svg::matrix(&kinds, &cols, &cell, "入口の種類 × 認可の取り方(本数)", stamp(c)), "権限の行列", "");
    if let Some(model) = nar(c, "13").get("model").and_then(|m| m.get("roles")).and_then(|r| r.as_array()) {
        let rows: Vec<String> = model.iter().map(|r| s(r, "name").to_string()).collect();
        let res: Vec<String> = { let mut v: Vec<String> = model.iter().flat_map(|r| r.get("can").and_then(|x| x.as_object()).map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()).collect(); v.sort(); v.dedup(); v };
        let cell2 = |r: &str, col: &str| -> (String, String) { let v = model.iter().find(|x| s(x, "name") == r).and_then(|x| x.get("can")).and_then(|x| x.get(col)).and_then(|x| x.as_str()).unwrap_or("").to_string(); (v.clone(), if v.is_empty() || v == "-" { "var(--code)".into() } else { "var(--ok-soft)".into() }) };
        body += &fig(&svg::matrix(&rows, &res, &cell2, "role × 資源(Claude が narrative に書いたもの・推定)", stamp(c)), "権限モデル(Claude)", "");
    }
    body += &h2("role(コード中の比較から)", "roles");
    body += &table(&["role", "出現"], &c.a.roles.iter().map(|(r, n)| vec![esc(r), n.to_string()]).collect::<Vec<_>>(), &[1], &[], "role の比較が見当たらない");
    body += &h2("テナント境界(どの列で区切っているか)", "tenant");
    body += &table(&["列", "出現"], &c.a.tenant.iter().map(|(r, n)| vec![path(r), n.to_string()]).collect::<Vec<_>>(), &[1], &[], "テナント列が見当たらない(単一テナントか、別の名前)");
    body += &h2("認証の仕組み", "libs");
    body += &table(&["仕組み", "files"], &c.a.auth_libs.iter().map(|(r, n)| vec![esc(r), n.to_string()]).collect::<Vec<_>>(), &[1], &[], "");
    if let Some(mw) = c.cfg.get("middleware") { body += &note(&format!("middleware:{} matcher: {}", mw.get(0).and_then(|x| x.as_str()).unwrap_or(""), mw.get(1).and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|y| y.as_str()).take(3).collect::<Vec<_>>().join(", ")).unwrap_or_default())); }
    body += &h2("RLS と policy", "rls");
    body += &table(&["状態", "表"], &[vec!["RLS あり".into(), rls_on.iter().map(|t| link(c, &t.id)).collect::<Vec<_>>().join(", ")], vec!["RLS 無し".into(), rls_off.iter().map(|t| link(c, &t.id)).collect::<Vec<_>>().join(", ")]], &[], &[], "");
    body += &h2("認可の無い入口", "unprotected");
    body += &note("公開 API・内部用・判定漏れのどれかで、1 本ずつ確かめる対象。cron は CRON_SECRET 等をヘッダで見ている場合、handler にその語があれば「handler で確認」に数えている。");
    body += &table(&["種類", "入口", "file"], &routes.iter().filter(|r| s(&Value::Object(r.attrs.clone().into_iter().collect()), "auth") == "none" && r.subkind != "cli").map(|r| vec![badge(&r.subkind, ""), format!("<a href=\"{}#{}\">{}</a>", file_of("04"), anchor(&r.id), esc(&r.name)), path(&r.path)]).collect::<Vec<_>>(), &[], &[], "無し");
    body += &explain(nar_list(c, "13", "notes").as_ref(), "解説(Claude)");
    page("13", "認証と認可", "", &body, &html_meta(c), &c.changed)
}

// ---------- 14 Jobs & Queues ----------
fn p_jobs(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "14", "summary").as_ref());
    let jobs = nodes_of(c, NodeKind::Job);
    let crons: Vec<(String, String, String)> = jobs.iter().map(|j| (j.name.clone(), s(&Value::Object(j.attrs.clone().into_iter().collect()), "schedule").into(), s(&Value::Object(j.attrs.clone().into_iter().collect()), "auth").into())).collect();
    let queues: Vec<&Node> = nodes_of(c, NodeKind::Route).into_iter().filter(|r| r.subkind == "queue").collect();
    let stack: Vec<&Value> = c.cfg.get("stack").and_then(|x| x.as_array()).map(|a| a.iter().filter(|x| matches!(s(x, "category"), "queue" | "cache" | "storage")).collect()).unwrap_or_default();
    body += &tiles(&[("cron", jobs.len().to_string(), ""), ("認可なしの cron", crons.iter().filter(|x| x.2 == "none").count().to_string(), if crons.iter().any(|x| x.2 == "none") { "warn" } else { "" }), ("queue consumer", queues.len().to_string(), ""), ("queue / cache / storage の SDK", stack.len().to_string(), "")]);
    body += &fig(&svg::cron_clock(&crons, stamp(c)), "cron の発火時刻(UTC)", "");
    body += &h2("cron", "crons");
    let traces: HashMap<&str, &crate::analyze::Trace> = c.a.traces.iter().map(|t| (t.route.as_str(), t)).collect();
    body += &table(&["いつ", "schedule", "入口", "認可", "辿った関数", "書く表", "外部"], &jobs.iter().map(|j| { let target = outs(c, &j.id, EdgeKind::Triggers).first().map(|e| e.to.clone()); let t = target.as_ref().and_then(|x| traces.get(x.as_str())); vec![esc(s(&Value::Object(j.attrs.clone().into_iter().collect()), "human")), esc(s(&Value::Object(j.attrs.clone().into_iter().collect()), "schedule")), target.as_ref().map(|x| link(c, x)).unwrap_or(path(&j.name)), auth_badge(s(&Value::Object(j.attrs.clone().into_iter().collect()), "auth")), t.map(|t| t.fn_count.to_string()).unwrap_or("-".into()), t.map(|t| t.writes.join(", ")).unwrap_or_default(), t.map(|t| t.externals.join(", ")).unwrap_or_default()] }).collect::<Vec<_>>(), &[4], &[], "無し");
    body += &h2("queue consumer", "queues");
    body += &table(&["名前", "file"], &queues.iter().map(|r| vec![format!("<a href=\"{}#{}\">{}</a>", file_of("04"), anchor(&r.id), esc(&r.name)), path(&r.path)]).collect::<Vec<_>>(), &[], &[], "無し");
    body += &h2("queue / cache / storage(依存から)", "sdk");
    body += &table(&["区分", "名前", "依存"], &stack.iter().map(|x| vec![esc(s(x, "category")), esc(s(x, "name")), path(s(x, "dep"))]).collect::<Vec<_>>(), &[], &[], "無し");
    body += &explain(nar_list(c, "14", "notes").as_ref(), "解説(Claude)");
    page("14", "ジョブと副作用", "", &body, &html_meta(c), &c.changed)
}

// ---------- 15 LLM Calls ----------
fn p_llm(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "15", "summary").as_ref());
    let Some(llm) = c.g.nodes.iter().find(|n| n.id == "ext:llm") else { body += &note("AI(LLM)の呼び出しは見当たらない。"); return page("15", "AI 呼び出し", "", &body, &html_meta(c), &c.changed); };
    let callers: Vec<&Node> = { let mut seen = HashSet::new(); ins(c, "ext:llm", EdgeKind::Uses).iter().filter_map(|e| node(c, &e.from)).filter(|n| seen.insert(n.id.clone())).collect() };
    let models: Vec<&Value> = llm.attrs.get("models").and_then(|m| m.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
    let mut by_layer: BTreeMap<String, u32> = BTreeMap::new(); for n in &callers { *by_layer.entry(n.layer.clone()).or_insert(0) += 1; }
    let mut bl: Vec<(String, u32)> = by_layer.into_iter().collect(); bl.sort_by(|a, b| b.1.cmp(&a.1));
    let providers: Vec<String> = { let mut v: Vec<String> = models.iter().map(|m| s(m, "provider").to_string()).collect(); v.sort(); v.dedup(); v };
    body += &tiles(&[("呼ぶ関数", callers.len().to_string(), ""), ("model", models.len().to_string(), ""), ("provider", providers.join(", "), "")]);
    let center = vec![svg::HubNode { title: "LLM".into(), sub: format!("{} 関数が呼ぶ", callers.len()), role: "ai".into(), main: true, ..Default::default() }];
    let left: Vec<svg::HubNode> = bl.iter().take(6).map(|(l, n)| svg::HubNode { title: l.clone(), sub: format!("{} 関数", n), role: role_of(l).into(), count: *n, href: format!("{}#dir-{}", file_of("06"), svg::id(l)), ..Default::default() }).collect();
    let mut ms: Vec<&Value> = models.clone(); ms.sort_by_key(|m| std::cmp::Reverse(n_u64(m, "files")));
    let right: Vec<svg::HubNode> = ms.iter().take(8).map(|m| svg::HubNode { title: s(m, "model").into(), sub: format!("{} · {} files", s(m, "provider"), n_u64(m, "files")), role: "ai".into(), count: n_u64(m, "files") as u32, ..Default::default() }).collect();
    body += &fig(&svg::hub(&center, &left, &right, &[], stamp(c), "呼び元(層)", "model", ""), "AI 呼び出しの地図:呼び元の層 → LLM → model(ファイル数)", "");
    body += &h2("model", "models");
    body += &table(&["model", "provider", "files"], &ms.iter().map(|m| vec![format!("<code>{}</code>", esc(s(m, "model"))), esc(s(m, "provider")), n_u64(m, "files").to_string()]).collect::<Vec<_>>(), &[2], &[], "");
    body += &h2("呼び出している関数(コードつき)", "callers");
    let mut cs: Vec<&Node> = callers.into_iter().filter(|n| n.kind == NodeKind::Function).collect(); cs.sort_by(|a, b| a.layer.cmp(&b.layer).then(a.path.cmp(&b.path)));
    for n in cs.iter().take(80) { body += &fn_block(c, n, false, 40); }
    if cs.len() > 80 { body += &note(&format!("ほか {} 関数(query 'external llm')", cs.len() - 80)); }
    body += &explain(nar_list(c, "15", "notes").as_ref(), "解説(Claude)");
    page("15", "AI 呼び出し", "", &body, &html_meta(c), &c.changed)
}

// ---------- 16 Diff ----------
fn p_diff(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "16", "summary").as_ref());
    let Some(d) = &c.diff else { body += &note("差分の基準が無い(`--since last` か `--since <sha>` で出す)。差分はグラフ同士の比較:表・入口(認可の変化)・関数(本文の hash)・型・外部・env・ジョブ・表の読み書き・入口ごとの trace・規則と循環。同じ形式で 2 回出した図を並べて見比べるのも基本の使い方。"); return page("16", "差分", "", &body, &html_meta(c), &c.changed); };
    let base = d.get("base").cloned().unwrap_or(Value::Null);
    body += &format!("<p class=\"lead\">基準:{}({})→ 今回 {}({})</p>", esc(s(&base, "sha")), esc(s(&base, "date")), esc(s(&c.meta, "sha")), esc(s(&c.meta, "date")));
    let order = ["routes", "table_access", "traces", "tables", "architecture", "functions", "types", "externals", "packages", "env", "jobs", "files"];
    let names: BTreeMap<&str, (&str, &str)> = [("routes", ("入口(追加・削除・認可の変化)", "04")), ("table_access", ("表の読み書き(関数 × 表)", "03")), ("traces", ("入口ごとの trace(呼ぶ関数・表・外部)", "05")), ("tables", ("表(列・FK・RLS)", "03")), ("architecture", ("規則の違反数・循環", "02")), ("functions", ("関数(追加・削除・本文の変化)", "06")), ("types", ("型", "07")), ("externals", ("外部 host", "12")), ("packages", ("パッケージ", "08")), ("env", ("env キー", "11")), ("jobs", ("cron", "14")), ("files", ("ファイル", "01"))].into_iter().collect();
    body += &tiles(&order.iter().map(|k| (*k, if c.changed.iter().any(|x| x == k) { "変化あり".to_string() } else { "変化なし".to_string() }, if c.changed.iter().any(|x| x == k) { "warn" } else { "ok" })).collect::<Vec<_>>());
    for k in order {
        let Some(sec) = d.get("sections").and_then(|x| x.get(k)) else { continue };
        let (title, ch) = names.get(k).copied().unwrap_or((k, ""));
        body += &h2(title, k);
        let added: Vec<&Value> = sec.get("added").and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
        let removed: Vec<&Value> = sec.get("removed").and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
        let changed: Vec<&Value> = sec.get("changed").and_then(|x| x.as_array()).map(|a| a.iter().collect()).unwrap_or_default();
        if added.is_empty() && removed.is_empty() && changed.is_empty() { body += &note("変化なし"); continue; }
        let mut rows: Vec<Vec<String>> = added.iter().map(|x| vec![badge("追加", "ok"), esc(x.as_str().unwrap_or("")), String::new()]).collect();
        rows.extend(removed.iter().map(|x| vec![badge("削除", "warn"), esc(x.as_str().unwrap_or("")), String::new()]));
        rows.extend(changed.iter().map(|x| vec![badge("変化", ""), esc(x.get(0).and_then(|y| y.as_str()).unwrap_or("")), esc(x.get(1).and_then(|y| y.as_str()).unwrap_or(""))]));
        body += &table(&["", "対象", "内容"], &rows, &[], &[], "");
        if !ch.is_empty() { body += &format!("<p class=\"note\"><a href=\"{}\">→ {} で確かめる</a></p>", file_of(ch), title_of(ch).0); }
    }
    body += &explain(nar_list(c, "16", "notes").as_ref(), "解説(Claude)");
    page("16", "差分", "", &body, &html_meta(c), &c.changed)
}

// ---------- 17 Glossary ----------
fn p_glossary(c: &Ctx) -> String {
    let mut body = summary(nar_list(c, "17", "summary").as_ref());
    if let Some(terms) = nar(c, "17").get("terms").and_then(|t| t.as_array()) {
        body += &table(&["ドメイン語", "コード上の名前", "出所", "意味"], &terms.iter().map(|t| vec![esc(s(t, "ja")), t.get("code").map(|x| match x { Value::Array(a) => a.iter().filter_map(|y| y.as_str()).map(|y| path(y)).collect::<Vec<_>>().join(" "), Value::String(y) => path(y), _ => String::new() }).unwrap_or_default(), path(s(t, "source")), esc(s(t, "note"))]).collect::<Vec<_>>(), &[], &[], "");
    } else {
        body += &note("LLM の段(narrative.json の 17.terms)が未記入。機械の材料として、表名と被参照の多い型を並べる(出所つき)。");
        let mut tys: Vec<&Node> = nodes_of(c, NodeKind::Type); tys.sort_by_key(|n| std::cmp::Reverse(ins(c, &n.id, EdgeKind::References).len()));
        let mut rows: Vec<Vec<String>> = nodes_of(c, NodeKind::Table).iter().take(40).map(|t| vec![badge("table", ""), link(c, &t.id), path(&t.path), String::new()]).collect();
        rows.extend(tys.iter().take(40).map(|t| vec![badge(&t.subkind, ""), link(c, &t.id), path(&t.path), esc(t.doc.lines().next().unwrap_or(""))]));
        body += &table(&["種類", "名前", "出所", "doc"], &rows, &[], &[], "");
    }
    body += &explain(nar_list(c, "17", "notes").as_ref(), "解説(Claude)");
    page("17", "用語集", "出所が無い語は載せない", &body, &html_meta(c), &c.changed)
}

// ---------- 99 Unknown ----------
fn p_unknown(c: &Ctx) -> String {
    let mut body = String::from("<p class=\"lead\">無いのではなく、読めなかったもの。読めば何が出るかを添える。</p>");
    let mut rows: Vec<Vec<String>> = c.meta.get("unknown").and_then(|u| u.as_array()).map(|a| a.iter().map(|u| vec![esc(s(u, "what")), esc(s(u, "why")), esc(s(u, "fix"))]).collect()).unwrap_or_default();
    if c.nar.is_null() { rows.push(vec!["要点・解説・代表フロー・リスクの順位・用語集".into(), "LLM の段(narrative.json)が未記入".into(), "Claude が data/ を読んで narrative.json を書き、`roentgen render` を回す".into()]); }
    let missing = c.g.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::Route | NodeKind::Table) && !c.notes.contains_key(&n.id)).count();
    let stale = c.notes.values().filter(|n| n.stale).count();
    rows.push(vec![format!("関数・入口・表の解説(notes)。未記入 {}・古い {}", missing, stale), "Claude がまだ書いていないか、コードが変わった".into(), "`roentgen notes <out> --repo <repo>` で未記入と古いものを重要度順に出す".into()]);
    rows.push(vec!["性能のホットパス(N+1 など)".into(), "静的には取れない".into(), "APM / DB のスロークエリログを別に見る".into()]);
    rows.push(vec!["解決できない呼び出し(メソッド・動的 import・外部 SDK の中)".into(), format!("{} 回(Tech Debt に上位)", c.a.metrics.get("calls_unresolved").and_then(|x| x.as_u64()).unwrap_or(0)), "型解決(tsc)を足せば減る".into()]);
    body += &table(&["何が", "なぜ読めなかったか", "読むには"], &rows, &[], &[], "");
    page("99", "読めなかったもの", "", &body, &html_meta(c), &c.changed)
}
