//! グラフから派生する解析:入口ごとの trace(関数 → 表 → 外部)、層の辺・規則・循環、churn と hotspot、数字。
use crate::model::*;
use crate::repo::{is_test, layer_of, role_of, Repo};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceStep { pub id: String, pub name: String, pub path: String, pub layer: String, pub depth: u32, pub kind: String }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Trace { pub route: String, pub steps: Vec<TraceStep>, pub reads: Vec<String>, pub writes: Vec<String>, pub externals: Vec<String>, pub llm: bool, pub depth: u32, pub fn_count: usize }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerEdge { pub from: String, pub to: String, pub count: u32, pub files: Vec<String> }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Rule { pub from: String, pub to: String, pub kind: String, pub count: u32, pub files: Vec<String>, pub estimated: bool }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cycle { pub a: String, pub b: String, pub ab: u32, pub ba: u32 }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Churn { pub path: String, pub count: u32, pub lines: u32, pub layer: String, pub authors: Vec<String>, pub last: String, pub functions: u32 }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Week { pub week: String, pub commits: u32, pub layers: BTreeMap<String, u32> }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pr { pub number: u64, pub title: String, pub author: String, pub merged: String, pub layers: Vec<String>, pub files: usize, pub additions: u64, pub deletions: u64 }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Analysis {
    pub traces: Vec<Trace>, pub layer_edges: Vec<LayerEdge>, pub rules: Vec<Rule>, pub rules_estimated: bool, pub cycles: Vec<Cycle>,
    pub churn: Vec<Churn>, pub weekly: Vec<Week>, pub prs: Vec<Pr>, pub authors: Vec<(String, u32)>, pub commits: u32, pub big: Vec<(String, u32, u32)>,
    pub table_access: BTreeMap<String, (Vec<String>, Vec<String>)>, pub metrics: BTreeMap<String, serde_json::Value>, pub tree: Vec<TreeDir>,
    pub unresolved_top: Vec<(String, u32)>, pub yml_suggestion: String,
    pub roles: Vec<(String, u32)>, pub tenant: Vec<(String, u32)>, pub auth_libs: Vec<(String, u32)>, pub tested_by_layer: BTreeMap<String, (u32, u32)>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TreeDir { pub path: String, pub depth: u32, pub files: u32, pub lines: u32, pub langs: Vec<String>, pub role: String, pub sample: Vec<String>, pub doc: String }

const DEFAULT_RULES: &[(&str, &str, &str)] = &[("core", "entry", "禁止"), ("core", "ui", "禁止"), ("core", "application", "禁止"), ("core", "adapter", "禁止"), ("application", "entry", "禁止"), ("application", "ui", "禁止"), ("adapter", "entry", "禁止"), ("adapter", "ui", "禁止"), ("adapter", "application", "禁止"), ("entry", "adapter", "事実(application を飛ばす)"), ("ui", "adapter", "事実(application を飛ばす)")];

pub fn run(repo: &Repo, g: &Graph, use_gh: bool, rules_cfg: Option<Vec<(String, String, String)>>) -> Analysis {
    let idx = g.index();
    let mut out = Analysis::default();
    // 隣接(calls / reads / writes / uses / handles)
    let mut adj: HashMap<&str, Vec<&Edge>> = HashMap::new();
    for e in &g.edges { adj.entry(e.from.as_str()).or_default().push(e); }
    // ---- traces
    for r in g.nodes.iter().filter(|n| n.kind == NodeKind::Route) {
        let mut t = Trace { route: r.id.clone(), ..Default::default() };
        let mut seen: HashSet<&str> = HashSet::new();
        let mut q: VecDeque<(&str, u32)> = VecDeque::new();
        for e in adj.get(r.id.as_str()).into_iter().flatten() { if e.kind == EdgeKind::Handles { q.push_back((e.to.as_str(), 0)); } }
        while let Some((id, d)) = q.pop_front() {
            if !seen.insert(id) || d > 8 { continue; }
            let Some(&i) = idx.get(id) else { continue };
            let n = &g.nodes[i];
            if n.kind == NodeKind::Function { t.steps.push(TraceStep { id: n.id.clone(), name: n.name.clone(), path: n.path.clone(), layer: n.layer.clone(), depth: d, kind: n.subkind.clone() }); t.depth = t.depth.max(d); }
            for e in adj.get(id).into_iter().flatten() {
                match e.kind {
                    EdgeKind::Calls if !e.unresolved => q.push_back((e.to.as_str(), d + 1)),
                    EdgeKind::Reads => t.reads.push(e.to.trim_start_matches("table:").into()),
                    EdgeKind::Writes => t.writes.push(e.to.trim_start_matches("table:").into()),
                    EdgeKind::Uses if e.to.starts_with("ext:") => { if e.to == "ext:llm" { t.llm = true; } else { t.externals.push(e.to.trim_start_matches("ext:").into()); } }
                    _ => {}
                }
            }
            if t.steps.len() >= 60 { break; }
        }
        for v in [&mut t.reads, &mut t.writes, &mut t.externals] { v.sort(); v.dedup(); }
        t.fn_count = t.steps.len();
        out.traces.push(t);
    }
    // ---- table access(表 → 読む関数 / 書く関数)
    for e in &g.edges { if matches!(e.kind, EdgeKind::Reads | EdgeKind::Writes) { let ent = out.table_access.entry(e.to.trim_start_matches("table:").into()).or_default(); if e.kind == EdgeKind::Reads { ent.0.push(e.from.clone()); } else { ent.1.push(e.from.clone()); } } }
    for (_, (r, w)) in out.table_access.iter_mut() { r.sort(); r.dedup(); w.sort(); w.dedup(); }
    // ---- 層の辺(file imports)
    let mut le: BTreeMap<(String, String), (u32, Vec<String>)> = BTreeMap::new();
    for e in &g.edges { if e.kind == EdgeKind::Imports && e.from.starts_with("file:") && e.to.starts_with("file:") { let (a, b) = (layer_of(&e.from[5..]), layer_of(&e.to[5..])); if a == b { continue; } if is_test(&e.from[5..]) { continue; } let ent = le.entry((a, b)).or_insert((0, vec![])); ent.0 += 1; if ent.1.len() < 5 && !ent.1.contains(&e.from[5..].to_string()) { ent.1.push(e.from[5..].into()); } } }
    out.layer_edges = le.iter().map(|((a, b), (c, f))| LayerEdge { from: a.clone(), to: b.clone(), count: *c, files: f.clone() }).collect();
    out.layer_edges.sort_by(|a, b| b.count.cmp(&a.count));
    let (rules, estimated): (Vec<(String, String, String)>, bool) = match rules_cfg { Some(r) if !r.is_empty() => (r, false), _ => (DEFAULT_RULES.iter().map(|(a, b, k)| (a.to_string(), b.to_string(), k.to_string())).collect(), true) };
    out.rules_estimated = estimated;
    for (from, to, kind) in &rules {
        let mut count = 0; let mut files = vec![];
        for e in &out.layer_edges { let ok_from = if from.ends_with('/') { e.from.starts_with(from.as_str()) } else { role_of(&e.from) == from }; let ok_to = if to.ends_with('/') { e.to.starts_with(to.as_str()) } else { role_of(&e.to) == to }; if ok_from && ok_to { count += e.count; files.extend(e.files.iter().cloned()); } }
        files.truncate(5);
        out.rules.push(Rule { from: from.clone(), to: to.clone(), kind: kind.clone(), count, files, estimated });
    }
    if estimated { out.yml_suggestion = format!("# roentgen の依存規則(推定から起こした雛形)。from/to は役割名(entry/ui/application/core/adapter)か path の prefix(末尾 /)\nrules:\n{}", out.rules.iter().map(|r| format!("  - {{ from: {}, to: {}, kind: {} }}", r.from, r.to, r.kind.split('(').next().unwrap_or(""))).collect::<Vec<_>>().join("\n")); }
    let mut seen = BTreeSet::new();
    for ((a, b), (n, _)) in &le { if let Some((m, _)) = le.get(&(b.clone(), a.clone())) { if !seen.contains(&(b.clone(), a.clone())) { seen.insert((a.clone(), b.clone())); out.cycles.push(Cycle { a: a.clone(), b: b.clone(), ab: *n, ba: *m }); } } }
    out.cycles.sort_by(|x, y| (y.ab + y.ba).cmp(&(x.ab + x.ba)));
    // ---- churn
    churn(repo, g, &mut out, use_gh);
    // ---- big
    for n in g.nodes.iter().filter(|n| n.kind == NodeKind::File && !is_test(&n.path)) { let lines = n.end_line; if lines >= 1000 { out.big.push((n.path.clone(), lines, out.churn.iter().find(|c| c.path == n.path).map(|c| c.count).unwrap_or(0))); } }
    out.big.sort_by(|a, b| b.1.cmp(&a.1)); out.big.truncate(30);
    // ---- tree
    out.tree = tree(g);
    // ---- unresolved の集計(どの外部呼び出しが多いか)
    let mut un: BTreeMap<String, u32> = BTreeMap::new();
    const NOISE: &[&str] = &["String", "Number", "Boolean", "new Error", "new Date", "new Map", "new Set", "new Promise", "Array", "Object", "parseInt", "parseFloat", "require", "setTimeout", "clearTimeout", "setInterval", "Symbol", "BigInt", "Error", "print", "len", "str", "int", "float", "list", "dict", "set", "range", "isinstance", "enumerate", "zip", "sorted", "format!", "println!", "vec!", "Some", "Ok", "Err", "Box::new", "panic!", "assert!", "assert_eq!", "make", "append", "len", "errors.New", "fmt.Sprintf", "fmt.Errorf"];
    for n in g.nodes.iter().filter(|n| !is_test(&n.path)) { if let Some(v) = n.attrs.get("calls_unresolved").and_then(|x| x.as_array()) { for it in v { if let (Some(k), Some(c)) = (it.get(0).and_then(|x| x.as_str()), it.get(1).and_then(|x| x.as_u64())) { let head = k.split('.').next().unwrap_or(""); if k.starts_with('.') || NOISE.contains(&k) || matches!(head, "console" | "JSON" | "Math" | "Object" | "Promise" | "Array" | "Number" | "String" | "Date" | "NextResponse" | "Response" | "process" | "window" | "document" | "React" | "Buffer" | "Reflect" | "Symbol" | "Intl" | "URL" | "crypto" | "fs" | "path" | "os" | "std" | "fmt" | "strings" | "strconv" | "json" | "re" | "sys" | "time" | "datetime" | "logging" | "logger" | "log") || k.starts_with("use") && k.len() < 24 && k.chars().nth(3).map(|c| c.is_uppercase()).unwrap_or(false) { continue; } *un.entry(k.into()).or_insert(0) += c as u32; } } } }
    let mut unv: Vec<(String, u32)> = un.into_iter().collect(); unv.sort_by(|a, b| b.1.cmp(&a.1)); unv.truncate(40); out.unresolved_top = unv;
    // ---- roles / tenant / auth libs(ソースを軽く走査)
    let role_rx = regex::Regex::new(r#"(?i)\brole\w*\s*(?:===?|!==?|==|!=|in|:|\.includes\(|\.has\()\s*\[?\(?\s*['"]([a-z_]+)['"]"#).unwrap();
    let tenant_rx = regex::Regex::new(r"\b(tenant_id|tenantId|org_id|orgId|organization_id|organizationId|workspace_id|workspaceId|company_id|companyId|account_id|accountId|customer_id|customerId|team_id|teamId)\b").unwrap();
    let libs: [(&str, &str); 12] = [("next-auth|@auth/", "NextAuth / Auth.js"), ("@clerk/", "Clerk"), ("@supabase/ssr|@supabase/auth|supabase.*auth", "Supabase Auth"), ("firebase/auth|firebase-admin", "Firebase Auth"), ("passport", "Passport"), ("jsonwebtoken|\\bjose\\b|PyJWT|python-jose|jwt\\.", "JWT"), ("auth0", "Auth0"), ("lucia", "Lucia"), ("django\\.contrib\\.auth", "Django auth"), ("fastapi\\.security|OAuth2PasswordBearer", "FastAPI security"), ("iron-session|express-session", "session"), ("liff", "LINE Login")];
    let lib_rx: Vec<(regex::Regex, &str)> = libs.iter().map(|(r, n)| (regex::Regex::new(r).unwrap(), *n)).collect();
    let noise = ["assistant", "system", "user", "tool", "function", "model", "button", "dialog", "alert", "status", "img", "presentation", "menu", "menuitem", "tab", "tablist", "tabpanel", "list", "listitem", "link", "navigation", "region", "group", "none", "heading", "cta", "headline", "developer", "info", "string", "sitelink", "header", "ai", "callout", "description", "disclaimer", "title", "body", "footer", "label", "primary", "secondary", "caption", "subtitle", "image", "video", "text", "name", "value", "type", "key", "id", "url", "link"];
    let known = ["admin", "staff", "owner", "member", "viewer", "editor", "manager", "operator", "customer", "superadmin", "super_admin", "service_role", "anon", "authenticated", "guest", "moderator"];
    let mut roles: BTreeMap<String, u32> = BTreeMap::new(); let mut tenant: BTreeMap<String, u32> = BTreeMap::new(); let mut auth_libs: BTreeMap<String, u32> = BTreeMap::new();
    for n in g.nodes.iter().filter(|n| n.kind == NodeKind::File && !is_test(&n.path)) {
        let src = repo.read(&n.path);
        if src.contains("role") || src.contains("Role") { for m in role_rx.captures_iter(&src) { let r = m[1].to_lowercase(); if !noise.contains(&r.as_str()) { *roles.entry(r).or_insert(0) += 1; } } }
        for m in tenant_rx.captures_iter(&src) { *tenant.entry(m[1].into()).or_insert(0) += 1; }
        let head: String = src.chars().take(4000).collect();
        for (rx, name) in &lib_rx { if rx.is_match(&head) { *auth_libs.entry((*name).into()).or_insert(0) += 1; } }
    }
    for n in g.nodes.iter().filter(|n| n.kind == NodeKind::Table) { if let Some(cols) = n.attrs.get("columns").and_then(|c| c.as_array()) { for c in cols { if let Some(name) = c.get("name").and_then(|x| x.as_str()) { if tenant_rx.is_match(name) { *tenant.entry(name.into()).or_insert(0) += 1; } } } } }
    let mut rv: Vec<(String, u32)> = roles.into_iter().filter(|(r, n)| *n >= 2 || known.contains(&r.as_str())).collect(); rv.sort_by(|a, b| b.1.cmp(&a.1)); rv.truncate(15); out.roles = rv;
    let mut tv: Vec<(String, u32)> = tenant.into_iter().collect(); tv.sort_by(|a, b| b.1.cmp(&a.1)); tv.truncate(6); out.tenant = tv;
    let mut lv: Vec<(String, u32)> = auth_libs.into_iter().collect(); lv.sort_by(|a, b| b.1.cmp(&a.1)); out.auth_libs = lv;
    for n in g.nodes.iter().filter(|n| n.kind == NodeKind::Function && !is_test(&n.path)) { let e = out.tested_by_layer.entry(n.layer.clone()).or_insert((0, 0)); e.1 += 1; if n.attrs.get("tested").is_some() { e.0 += 1; } }
    // ---- metrics
    let count = |k: NodeKind| g.nodes.iter().filter(|n| n.kind == k).count();
    let routes = g.nodes.iter().filter(|n| n.kind == NodeKind::Route);
    let mut auth: BTreeMap<String, u32> = BTreeMap::new();
    for r in routes { *auth.entry(r.attrs.get("auth").and_then(|a| a.as_str()).unwrap_or("unknown").into()).or_insert(0) += 1; }
    let resolved = g.edges.iter().filter(|e| e.kind == EdgeKind::Calls).count();
    let unresolved: u32 = out.unresolved_top.iter().map(|x| x.1).sum();
    out.metrics = BTreeMap::from([
        ("files".into(), json!(count(NodeKind::File))), ("functions".into(), json!(count(NodeKind::Function))), ("types".into(), json!(count(NodeKind::Type))), ("tables".into(), json!(count(NodeKind::Table))),
        ("routes".into(), json!(count(NodeKind::Route))), ("jobs".into(), json!(count(NodeKind::Job))), ("externals".into(), json!(count(NodeKind::External))), ("packages".into(), json!(count(NodeKind::Package))), ("env".into(), json!(count(NodeKind::Env))),
        ("auth".into(), json!(auth)), ("calls_resolved".into(), json!(resolved)), ("calls_unresolved".into(), json!(unresolved)), ("lines".into(), json!(g.nodes.iter().filter(|n| n.kind == NodeKind::File).map(|n| n.end_line as u64).sum::<u64>())),
        ("rls_off".into(), json!(g.nodes.iter().filter(|n| n.kind == NodeKind::Table && n.attrs.get("rls").and_then(|x| x.as_bool()) == Some(false)).count())),
        ("rule_violations".into(), json!(out.rules.iter().filter(|r| r.kind.starts_with("禁止")).map(|r| r.count).sum::<u32>())), ("cycles".into(), json!(out.cycles.len())),
    ]);
    out
}

fn churn(repo: &Repo, g: &Graph, out: &mut Analysis, use_gh: bool) {
    if !repo.is_git { return; }
    let log = repo.git(&["log", "--since=90 days ago", "--format=%x1e%h%x1f%an%x1f%cs", "--name-only", "--no-merges"]);
    let alive: HashSet<&str> = g.nodes.iter().filter(|n| n.kind == NodeKind::File).map(|n| n.path.as_str()).collect();
    let fn_count: HashMap<&str, u32> = { let mut m = HashMap::new(); for n in &g.nodes { if n.kind == NodeKind::Function { *m.entry(n.path.as_str()).or_insert(0) += 1; } } m };
    let mut files: BTreeMap<String, (u32, BTreeMap<String, u32>, String)> = BTreeMap::new();
    let mut weekly: BTreeMap<String, Week> = BTreeMap::new();
    let mut authors: BTreeMap<String, u32> = BTreeMap::new();
    let mut commits = 0;
    for rec in log.split('\x1e') {
        let rec = rec.trim_matches('\n');
        if rec.is_empty() { continue; }
        let (head, body) = rec.split_once('\n').unwrap_or((rec, ""));
        let p: Vec<&str> = head.split('\x1f').collect();
        if p.len() < 3 { continue; }
        let (author, date) = (p[1], p[2]);
        commits += 1;
        *authors.entry(author.into()).or_insert(0) += 1;
        let wk = week_of(date);
        let w = weekly.entry(wk.clone()).or_insert(Week { week: wk, commits: 0, layers: BTreeMap::new() });
        w.commits += 1;
        for f in body.lines() { let f = f.trim(); if f.is_empty() || !alive.contains(f) || is_test(f) { continue; } let e = files.entry(f.into()).or_insert((0, BTreeMap::new(), String::new())); e.0 += 1; *e.1.entry(author.into()).or_insert(0) += 1; if date > e.2.as_str() { e.2 = date.into(); } *w.layers.entry(layer_of(f)).or_insert(0) += 1; }
    }
    out.commits = commits;
    let lines_of: HashMap<&str, u32> = g.nodes.iter().filter(|n| n.kind == NodeKind::File).map(|n| (n.path.as_str(), n.end_line)).collect();
    let mut ch: Vec<Churn> = files.into_iter().map(|(p, (c, a, last))| { let mut av: Vec<(String, u32)> = a.into_iter().collect(); av.sort_by(|x, y| y.1.cmp(&x.1)); Churn { lines: *lines_of.get(p.as_str()).unwrap_or(&0), layer: layer_of(&p), authors: av.into_iter().take(3).map(|x| x.0).collect(), last, functions: *fn_count.get(p.as_str()).unwrap_or(&0), path: p, count: c } }).collect();
    ch.sort_by(|a, b| b.count.cmp(&a.count).then(a.path.cmp(&b.path)));
    ch.truncate(80);
    out.churn = ch;
    let today = chrono::Local::now().date_naive();
    let monday = today - chrono::Duration::days(today.format("%u").to_string().parse::<i64>().unwrap_or(1) - 1);
    out.weekly = (0..13).rev().map(|i| { let wk = (monday - chrono::Duration::weeks(i)).format("%Y-%m-%d").to_string(); weekly.get(&wk).cloned().map(|mut w| { let mut v: Vec<(String, u32)> = w.layers.into_iter().collect(); v.sort_by(|a, b| b.1.cmp(&a.1)); w.layers = v.into_iter().take(8).collect(); w }).unwrap_or(Week { week: wk, commits: 0, layers: BTreeMap::new() }) }).collect();
    let mut av: Vec<(String, u32)> = authors.into_iter().collect(); av.sort_by(|a, b| b.1.cmp(&a.1)); av.truncate(10); out.authors = av;
    if use_gh {
        let remote = repo.git(&["remote", "get-url", "origin"]);
        if let Some(m) = regex::Regex::new(r"github\.com[:/]([^/]+/[^/.\s]+)").unwrap().captures(remote.trim()) {
            let o = crate::repo::sh(&["gh", "pr", "list", "-R", &m[1], "--state", "merged", "--limit", "40", "--json", "number,title,author,mergedAt,files,additions,deletions"], None);
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&o) { for p in v.as_array().into_iter().flatten() { let files: Vec<String> = p.get("files").and_then(|f| f.as_array()).map(|a| a.iter().filter_map(|x| x.get("path").and_then(|s| s.as_str()).map(String::from)).collect()).unwrap_or_default(); let mut layers: Vec<String> = files.iter().map(|f| layer_of(f)).collect::<BTreeSet<_>>().into_iter().collect(); layers.truncate(6); out.prs.push(Pr { number: p.get("number").and_then(|x| x.as_u64()).unwrap_or(0), title: p.get("title").and_then(|x| x.as_str()).unwrap_or("").chars().take(80).collect(), author: p.get("author").and_then(|a| a.get("login")).and_then(|x| x.as_str()).unwrap_or("").into(), merged: p.get("mergedAt").and_then(|x| x.as_str()).unwrap_or("").chars().take(10).collect(), layers, files: files.len(), additions: p.get("additions").and_then(|x| x.as_u64()).unwrap_or(0), deletions: p.get("deletions").and_then(|x| x.as_u64()).unwrap_or(0) }); } }
        }
    }
}

fn week_of(date: &str) -> String { chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map(|d| { let wd = d.format("%u").to_string().parse::<i64>().unwrap_or(1); (d - chrono::Duration::days(wd - 1)).format("%Y-%m-%d").to_string() }).unwrap_or(date.into()) }

/// ディレクトリツリー(深さ 3 まで)。各ディレクトリの files / lines / 言語 / 役割 / 代表ファイル / 1 行の doc(README か index の doc)。
fn tree(g: &Graph) -> Vec<TreeDir> {
    let mut dirs: BTreeMap<String, TreeDir> = BTreeMap::new();
    let docs: HashMap<&str, &str> = g.nodes.iter().filter(|n| n.kind == NodeKind::File && !n.doc.is_empty()).map(|n| (n.path.as_str(), n.doc.as_str())).collect();
    for n in g.nodes.iter().filter(|n| n.kind == NodeKind::File) {
        let parts: Vec<&str> = n.path.split('/').collect();
        for depth in 1..parts.len().min(4) {
            let d = format!("{}/", parts[..depth].join("/"));
            let e = dirs.entry(d.clone()).or_insert_with(|| TreeDir { path: d.clone(), depth: depth as u32, role: role_of(&d).into(), ..Default::default() });
            e.files += 1; e.lines += n.end_line;
            if !e.langs.contains(&n.lang) { e.langs.push(n.lang.clone()); }
            if e.sample.len() < 4 && parts.len() == depth + 1 { e.sample.push(parts[depth].into()); }
            if e.doc.is_empty() { if let Some(doc) = docs.get(n.path.as_str()) { if parts.len() == depth + 1 && (n.name.starts_with("index.") || n.name.starts_with("mod.") || n.name.starts_with("__init__") || n.name.starts_with("lib.") || n.name.starts_with("main.")) { e.doc = doc.lines().next().unwrap_or("").chars().take(120).collect(); } } }
        }
    }
    dirs.into_values().collect()
}
