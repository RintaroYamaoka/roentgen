//! IR → 知識グラフ。import と呼び出しを解決し、入口・表の読み書き・外部・env を結ぶ。
use crate::extract::config::Config;
use crate::extract::ir::*;
use crate::extract::sql::Table;
use crate::model::*;
use crate::repo::{is_test, layer_of, role_of, Repo};
use regex::Regex;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub struct Build<'a> { pub repo: &'a Repo, pub irs: &'a [FileIR], pub cfg: &'a Config, pub tables: &'a BTreeMap<String, Table> }

static AUTH_RX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r"(?i)\b(getUser|getSession|auth\(\)|requireAuth|requireUser|requireAdmin|requireStaff|assertRole|verifyToken|verifyJwt|jwt\.verify|getServerSession|currentUser|get_current_user|login_required|permission_required|IsAuthenticated|authorize|checkAuth|checkPermission|withAuth|protect\(|AuthGuard|\.auth\.getUser|verify_signature|verifyWebhook|constructEvent|verifyRequest|validateSignature|CRON_SECRET|x-cron-secret|authorization)\b").unwrap());
static GUARD_RX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r"(?i)(guard|auth|session|permission|authori[sz]|require-?(user|staff|admin|login)|protect|verify|middleware)").unwrap());
static URL_RX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r"(?i)https?://([a-z0-9.-]+\.[a-z]{2,})(/[^\s'\x22`)]*)?").unwrap());
static SQL_READ: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r#"(?i)\b(?:from|join)\s+"?(?:public\.)?"?([a-z_][a-z0-9_]*)"?"#).unwrap());
static SQL_WRITE: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r#"(?i)\b(?:insert\s+into|update|delete\s+from|truncate(?:\s+table)?)\s+"?(?:public\.)?"?([a-z_][a-z0-9_]*)"?"#).unwrap());
static FROM_RX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r#"\.from\(\s*['"`]([A-Za-z_][\w.]*)['"`]"#).unwrap());
static MODEL_RX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r"(?i)^((?:claude|gpt|o[1-4](?:-mini)?|gemini|llama|mistral|mixtral|deepseek|grok|command|text-embedding|whisper|dall-e|sonar)[\w./:-]*|(?:anthropic|openai|google|xai|meta|mistral|amazon|cohere|perplexity|vercel)/[\w./:-]+)$").unwrap());
static WEBHOOK_RX: once_cell::sync::Lazy<Regex> = once_cell::sync::Lazy::new(|| Regex::new(r"(?i)webhook|callback|hook").unwrap());

const LLM_CALLS: &[&str] = &["generateText", "streamText", "generateObject", "streamObject", "streamUI", "generateUI", "embed", "embedMany", "ToolLoopAgent", "getGenerativeModel", "generateContent", "ChatOpenAI", "ChatAnthropic", "invoke_model", "converse"];
const SKIP_HOST: &[&str] = &["localhost", "example.com", "example.", ".test", ".local", ".invalid", ".example", "schema.org", "w3.org", "npmjs", "unpkg", "jsdelivr", "fonts.g", "vercel.app", "nextjs.org", "reactjs", "developer.mozilla", "wikipedia", "twitter.com", "x.com", "youtube", "linkedin", "apple.com", "microsoft.com", "json-schema", "xmlns", "purl.org", "opengraph", "mozilla.org", "creativecommons", "github.com", "127.0.0.1", "docs.", "www.google.com", "google.com", "facebook.com", "instagram.com"];

struct Sym { file: String, name: String }

pub fn build(b: &Build) -> Graph {
    let repo = b.repo;
    let files: HashSet<&str> = repo.files.iter().map(|s| s.as_str()).collect();
    let ir_by_path: HashMap<&str, &FileIR> = b.irs.iter().map(|i| (i.path.as_str(), i)).collect();
    let mut g = Graph::default();
    let mut edges: BTreeSet<Edge> = BTreeSet::new();

    // ---- layers
    let mut layer_lines: BTreeMap<String, (u32, u32)> = BTreeMap::new();
    for ir in b.irs { let e = layer_lines.entry(layer_of(&ir.path)).or_insert((0, 0)); e.0 += 1; e.1 += ir.lines; }
    for (l, (files_n, lines)) in &layer_lines { g.nodes.push(Node { id: id_of(NodeKind::Layer, l, ""), kind: NodeKind::Layer, name: l.clone(), path: l.clone(), layer: l.clone(), subkind: role_of(l).into(), attrs: BTreeMap::from([("files".into(), json!(files_n)), ("lines".into(), json!(lines))]), ..Default::default() }); }

    // ---- files, functions, types
    let mut fn_by_file: HashMap<&str, HashMap<String, String>> = HashMap::new(); // file → (name → fn id)
    let mut type_by_file: HashMap<&str, HashMap<String, String>> = HashMap::new();
    let mut type_global: HashMap<String, Vec<String>> = HashMap::new();
    for ir in b.irs {
        let src = repo.read(&ir.path);
        let layer = layer_of(&ir.path);
        let fid = id_of(NodeKind::File, &ir.path, "");
        let mut fattrs = BTreeMap::new();
        fattrs.insert("lines".into(), json!(ir.lines)); fattrs.insert("test".into(), json!(is_test(&ir.path))); fattrs.insert("role".into(), json!(role_of(&layer)));
        if !ir.directives.is_empty() { fattrs.insert("directives".into(), json!(ir.directives)); }
        if ir.parse_error { fattrs.insert("parse_error".into(), json!(true)); }
        g.nodes.push(Node { id: fid.clone(), kind: NodeKind::File, name: ir.path.rsplit('/').next().unwrap_or(&ir.path).into(), path: ir.path.clone(), line: 1, end_line: ir.lines, layer: layer.clone(), lang: ir.lang.clone(), hash: crate::extract::ts_util::hash(&src), doc: ir.doc.clone(), attrs: fattrs, ..Default::default() });
        edges.insert(Edge { from: id_of(NodeKind::Layer, &layer, ""), to: fid.clone(), kind: EdgeKind::Contains, line: 0, unresolved: false });
        let fmap = fn_by_file.entry(ir.path.as_str()).or_default();
        for f in &ir.functions {
            let qname = match &f.class { Some(c) => format!("{}.{}", c, f.name), None => f.name.clone() };
            let id = id_of(NodeKind::Function, &ir.path, &qname);
            fmap.insert(qname.clone(), id.clone());
            if f.class.is_some() { fmap.entry(f.name.clone()).or_insert(id.clone()); }
            let mut attrs = BTreeMap::new();
            if !f.decorators.is_empty() { attrs.insert("decorators".into(), json!(f.decorators)); }
            if f.is_async { attrs.insert("async".into(), json!(true)); }
            attrs.insert("calls".into(), json!(f.calls.len()));
            g.nodes.push(Node { id: id.clone(), kind: NodeKind::Function, name: qname, path: ir.path.clone(), line: f.line, end_line: f.end_line, layer: layer.clone(), lang: ir.lang.clone(), hash: crate::extract::ts_util::hash(&f.body), exported: f.exported, signature: f.signature.clone(), doc: f.doc.clone(), subkind: f.kind.clone(), attrs, excerpt: f.body.clone() });
            edges.insert(Edge { from: fid.clone(), to: id.clone(), kind: EdgeKind::Contains, line: f.line, unresolved: false });
            if f.exported { edges.insert(Edge { from: fid.clone(), to: id, kind: EdgeKind::Exports, line: f.line, unresolved: false }); }
        }
        for _ in 0..2 { for (a, t) in &ir.aliases { if let Some(id) = fmap.get(t).cloned() { fmap.entry(a.clone()).or_insert(id); } } }
        let tmap = type_by_file.entry(ir.path.as_str()).or_default();
        for t in &ir.types {
            let id = id_of(NodeKind::Type, &ir.path, &t.name);
            tmap.insert(t.name.clone(), id.clone());
            type_global.entry(t.name.clone()).or_default().push(id.clone());
            let mut attrs = BTreeMap::new();
            attrs.insert("methods".into(), json!(t.methods)); attrs.insert("extends".into(), json!(t.extends)); attrs.insert("implements".into(), json!(t.implements)); attrs.insert("refs".into(), json!(t.refs));
            g.nodes.push(Node { id: id.clone(), kind: NodeKind::Type, name: t.name.clone(), path: ir.path.clone(), line: t.line, end_line: t.end_line, layer: layer.clone(), lang: ir.lang.clone(), hash: crate::extract::ts_util::hash(&t.text), exported: t.exported, doc: t.doc.clone(), subkind: t.kind.clone(), attrs, excerpt: t.text.clone(), ..Default::default() });
            edges.insert(Edge { from: fid.clone(), to: id.clone(), kind: EdgeKind::Contains, line: t.line, unresolved: false });
            if t.exported { edges.insert(Edge { from: fid.clone(), to: id, kind: EdgeKind::Exports, line: t.line, unresolved: false }); }
        }
    }

    // ---- imports(file → file / package)
    let mut import_map: HashMap<&str, HashMap<String, Sym>> = HashMap::new(); // file → local name → (file, exported name)
    let mut ns_map: HashMap<&str, HashMap<String, String>> = HashMap::new();   // file → namespace local → file(or dir for Go)
    let mut packages: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ir in b.irs {
        let fid = id_of(NodeKind::File, &ir.path, "");
        let imap = import_map.entry(ir.path.as_str()).or_default();
        let nmap = ns_map.entry(ir.path.as_str()).or_default();
        for im in &ir.imports {
            match resolve_import(&ir.lang, &im.spec, &ir.path, &files, b.cfg) {
                Resolved::File(target) => {
                    edges.insert(Edge { from: fid.clone(), to: id_of(NodeKind::File, &target, ""), kind: EdgeKind::Imports, line: im.line, unresolved: false });
                    for (name, local) in &im.names { imap.insert(local.clone(), Sym { file: target.clone(), name: name.clone() }); }
                    if let Some(d) = &im.default { imap.insert(d.clone(), Sym { file: target.clone(), name: "default".into() }); }
                    if let Some(ns) = &im.namespace { nmap.insert(ns.clone(), target.clone()); }
                    if im.names.is_empty() && im.default.is_none() && im.namespace.is_none() { if let Some(n) = im.spec.rsplit("::").next() { imap.insert(n.to_string(), Sym { file: target.clone(), name: n.to_string() }); } }
                }
                Resolved::Package(pkg) => {
                    packages.entry(pkg.clone()).or_default().insert(ir.path.clone());
                    edges.insert(Edge { from: fid.clone(), to: id_of(NodeKind::Package, &pkg, ""), kind: EdgeKind::Imports, line: im.line, unresolved: false });
                    for (_, local) in &im.names { nmap.insert(local.clone(), format!("pkg:{}", pkg)); }
                    if let Some(d) = &im.default { nmap.insert(d.clone(), format!("pkg:{}", pkg)); }
                    if let Some(ns) = &im.namespace { nmap.insert(ns.clone(), format!("pkg:{}", pkg)); }
                }
                Resolved::None => {}
            }
        }
    }
    for (pkg, users) in &packages {
        let cat = pkg_category(pkg);
        g.nodes.push(Node { id: id_of(NodeKind::Package, pkg, ""), kind: NodeKind::Package, name: pkg.clone(), path: pkg.clone(), subkind: cat.into(), attrs: BTreeMap::from([("files".into(), json!(users.len()))]), ..Default::default() });
    }

    // ---- tables
    let table_names: HashSet<&str> = b.tables.keys().map(|s| s.as_str()).collect();
    for (name, t) in b.tables {
        let mut attrs = BTreeMap::new();
        attrs.insert("columns".into(), json!(t.columns)); attrs.insert("fks".into(), json!(t.fks)); attrs.insert("rls".into(), json!(t.rls)); attrs.insert("policies".into(), json!(t.policies)); attrs.insert("indexes".into(), json!(t.indexes));
        attrs.insert("first_migration".into(), json!(t.first_migration)); attrs.insert("last_migration".into(), json!(t.last_migration)); attrs.insert("source".into(), json!(t.source));
        if let Some(r) = t.rows { attrs.insert("rows".into(), json!(r)); }
        let text = format!("{}(\n{}\n)", name, t.columns.iter().map(|c| format!("  {} {}{}", c.name, c.ty, if c.nullable { "" } else { " not null" })).collect::<Vec<_>>().join(",\n"));
        g.nodes.push(Node { id: id_of(NodeKind::Table, name, ""), kind: NodeKind::Table, name: name.clone(), path: t.source.clone(), hash: crate::extract::ts_util::hash(&text), subkind: "table".into(), attrs, excerpt: text, ..Default::default() });
        for fk in &t.fks { if b.tables.contains_key(&fk.ref_table) { edges.insert(Edge { from: id_of(NodeKind::Table, name, ""), to: id_of(NodeKind::Table, &fk.ref_table, ""), kind: EdgeKind::References, line: 0, unresolved: false }); } }
    }

    // ---- calls / table access / externals / env / llm(関数ごと)
    let mut env_nodes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut ext_nodes: BTreeMap<String, (String, BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new(); // host → (category, files, paths)
    let mut llm_models: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut unresolved_calls: HashMap<String, BTreeMap<String, u32>> = HashMap::new();
    for ir in b.irs {
        let fid = id_of(NodeKind::File, &ir.path, "");
        for k in &ir.env_keys { env_nodes.entry(k.clone()).or_default().insert(ir.path.clone()); edges.insert(Edge { from: fid.clone(), to: id_of(NodeKind::Env, k, ""), kind: EdgeKind::Uses, line: 0, unresolved: false }); }
        let local = fn_by_file.get(ir.path.as_str());
        let imap = import_map.get(ir.path.as_str());
        let nmap = ns_map.get(ir.path.as_str());
        let mut units: Vec<(String, &[Call], &[String])> = ir.functions.iter().map(|f| { let q = match &f.class { Some(c) => format!("{}.{}", c, f.name), None => f.name.clone() }; (id_of(NodeKind::Function, &ir.path, &q), f.calls.as_slice(), f.strings.as_slice()) }).collect();
        units.push((fid.clone(), ir.top_calls.as_slice(), ir.top_strings.as_slice()));
        for (from, calls, strings) in units {
            let mut unres: BTreeMap<String, u32> = BTreeMap::new();
            for c in calls {
                // 解決
                let target: Option<String> = match &c.receiver {
                    None => local.and_then(|m| m.get(&c.name).cloned()).or_else(|| imap.and_then(|m| m.get(&c.name)).and_then(|s| fn_by_file.get(s.file.as_str()).and_then(|m| m.get(&s.name).cloned().or_else(|| if s.name == "default" { m.values().next().cloned() } else { None })))),
                    Some(r) if r == "this" || r == "self" || r == "cls" => local.and_then(|m| m.get(&c.name).cloned()),
                    Some(r) if r == "new" => local.and_then(|m| m.get(&format!("{}.constructor", c.name)).or(m.get(&format!("{}.__init__", c.name))).or(m.get(&format!("{}.new", c.name))).cloned()).or_else(|| imap.and_then(|m| m.get(&c.name)).and_then(|s| fn_by_file.get(s.file.as_str()).and_then(|m| m.get(&format!("{}.constructor", s.name)).or(m.get(&format!("{}.__init__", s.name))).cloned()))),
                    Some(r) => {
                        let head = r.split(|ch: char| ch == '.' || ch == '(' || ch == ':').next().unwrap_or("").trim();
                        if let Some(nm) = nmap.and_then(|m| m.get(head)) {
                            if nm.starts_with("pkg:") { None } else if ir.lang == "go" { go_pkg_fn(nm, &c.name, &fn_by_file) } else { fn_by_file.get(nm.as_str()).and_then(|m| m.get(&c.name).cloned()) }
                        } else if ir.lang == "rs" { let last = r.rsplit("::").next().unwrap_or(r); local.and_then(|m| m.get(&format!("{}.{}", last, c.name)).cloned()).or_else(|| imap.and_then(|m| m.get(last)).and_then(|s| fn_by_file.get(s.file.as_str()).and_then(|m| m.get(&format!("{}.{}", s.name, c.name)).or(m.get(&c.name)).cloned()))) }
                        else { local.and_then(|m| m.get(&format!("{}.{}", head, c.name)).cloned()).or_else(|| imap.and_then(|m| m.get(head)).and_then(|s| fn_by_file.get(s.file.as_str()).and_then(|m| m.get(&format!("{}.{}", s.name, c.name)).cloned()))) }
                    }
                };
                match target {
                    Some(t) if t != from => { edges.insert(Edge { from: from.clone(), to: t, kind: EdgeKind::Calls, line: c.line, unresolved: false }); }
                    Some(_) => {}
                    None => { let key = match &c.receiver { Some(r) if !r.is_empty() && r != "new" => format!("{}.{}", r.rsplit(|ch: char| ch == '.' || ch == ')').next().unwrap_or(r).chars().take(30).collect::<String>(), c.name), Some(_) => format!("new {}", c.name), None => c.name.clone() }; *unres.entry(key).or_insert(0) += 1; }
                }
                // 表の読み書き
                let mut access: Vec<(String, EdgeKind)> = Vec::new();
                if let Some(r) = &c.receiver { if let Some(m) = FROM_RX.captures(r) { let t = m[1].to_string(); let kind = match c.name.as_str() { "insert" | "upsert" | "update" | "delete" | "rpc" => EdgeKind::Writes, _ => EdgeKind::Reads }; access.push((t, kind)); } }
                if let Some(a) = &c.arg { if matches!(c.name.as_str(), "from" | "table" | "collection" | "model") && table_names.contains(a.as_str()) { /* chained の方で判定するので無視 */ } else if matches!(c.name.as_str(), "query" | "query!" | "query_as!" | "execute" | "raw" | "sql" | "unsafe" | "run" | "all" | "get" | "exec" | "query_raw" | "prepare" | "fetch_all" | "fetch_one" | "Query" | "QueryRow" | "Exec") || a.to_lowercase().trim_start().starts_with("select") || a.to_lowercase().trim_start().starts_with("insert") || a.to_lowercase().trim_start().starts_with("update") || a.to_lowercase().trim_start().starts_with("delete") { for m in SQL_WRITE.captures_iter(a) { access.push((m[1].to_string(), EdgeKind::Writes)); } for m in SQL_READ.captures_iter(a) { access.push((m[1].to_string(), EdgeKind::Reads)); } } }
                // prisma.customer.findMany / drizzle db.select().from(customers)
                if let Some(r) = &c.receiver { if let Some(rest) = r.strip_prefix("prisma.").or(r.strip_prefix("db.")).or(r.strip_prefix("this.prisma.")) { let model = rest.split('.').next().unwrap_or("").to_string(); let snake = to_snake(&model); let cand = [model.clone(), snake.clone(), format!("{}s", snake)]; if let Some(t) = cand.iter().find(|x| table_names.contains(x.as_str())) { let kind = if matches!(c.name.as_str(), "create" | "createMany" | "update" | "updateMany" | "upsert" | "delete" | "deleteMany") { EdgeKind::Writes } else if c.name.starts_with("find") || c.name == "count" || c.name == "aggregate" || c.name == "groupBy" { EdgeKind::Reads } else { continue }; access.push((t.clone(), kind)); } } }
                for (t, kind) in access { if table_names.contains(t.as_str()) { edges.insert(Edge { from: from.clone(), to: id_of(NodeKind::Table, &t, ""), kind, line: c.line, unresolved: false }); } }
                // LLM
                if LLM_CALLS.contains(&c.name.as_str()) || (c.name == "create" && c.receiver.as_deref().map(|r| r.contains("messages") || r.contains("completions") || r.contains("responses")).unwrap_or(false)) { edges.insert(Edge { from: from.clone(), to: id_of(NodeKind::External, "llm", ""), kind: EdgeKind::Uses, line: c.line, unresolved: false }); }
            }
            // SQL 文字列(呼び出しの引数以外):テンプレートの中など
            for s in strings {
                let low = s.to_lowercase(); let lt = low.trim_start();
                if lt.starts_with("select ") || lt.starts_with("insert ") || lt.starts_with("update ") || lt.starts_with("delete ") || lt.starts_with("with ") { for m in SQL_WRITE.captures_iter(s) { if table_names.contains(&m[1]) { edges.insert(Edge { from: from.clone(), to: id_of(NodeKind::Table, &m[1], ""), kind: EdgeKind::Writes, line: 0, unresolved: false }); } } for m in SQL_READ.captures_iter(s) { if table_names.contains(&m[1]) { edges.insert(Edge { from: from.clone(), to: id_of(NodeKind::Table, &m[1], ""), kind: EdgeKind::Reads, line: 0, unresolved: false }); } } }
                if let Some(m) = URL_RX.captures(s) { let host = m[1].to_lowercase(); let path = m.get(2).map(|p| p.as_str()).unwrap_or(""); if !SKIP_HOST.iter().any(|k| host.contains(k) || path.contains(k)) && (host.contains("api") || path.contains("api") || host.split('.').count() >= 3 || path.contains("/v") || path.contains("hook") || host.contains("googleapis") || host.contains("graph")) { let e = ext_nodes.entry(host.clone()).or_insert_with(|| (host_category(&host), BTreeSet::new(), BTreeSet::new())); e.1.insert(ir.path.clone()); if !path.is_empty() { e.2.insert(regex::Regex::new(r"\$\{[^}]*\}|\{[^}]*\}|\d{4,}").unwrap().replace_all(path, "{}").chars().take(60).collect()); } edges.insert(Edge { from: from.clone(), to: id_of(NodeKind::External, &host, ""), kind: EdgeKind::Uses, line: 0, unresolved: false }); } }
                if MODEL_RX.is_match(s) && s.len() >= 4 && s.chars().any(|ch| ch.is_ascii_digit() || ch == '/') { llm_models.entry(s.clone()).or_default().insert(ir.path.clone()); edges.insert(Edge { from: from.clone(), to: id_of(NodeKind::External, "llm", ""), kind: EdgeKind::Uses, line: 0, unresolved: false }); }
            }
            if !unres.is_empty() { unresolved_calls.insert(from.clone(), unres); }
        }
    }
    for (k, files_) in &env_nodes { g.nodes.push(Node { id: id_of(NodeKind::Env, k, ""), kind: NodeKind::Env, name: k.clone(), subkind: if b.cfg.env_declared.contains(k) { "declared" } else { "undeclared" }.into(), attrs: BTreeMap::from([("files".into(), json!(files_.len())), ("declared".into(), json!(b.cfg.env_declared.contains(k)))]), ..Default::default() }); }
    for (host, (cat, files_, paths)) in &ext_nodes { g.nodes.push(Node { id: id_of(NodeKind::External, host, ""), kind: NodeKind::External, name: host_name(host), path: host.clone(), subkind: "host".into(), attrs: BTreeMap::from([("category".into(), json!(cat)), ("files".into(), json!(files_.len())), ("paths".into(), json!(paths.iter().take(12).collect::<Vec<_>>()))]), ..Default::default() }); }
    if !llm_models.is_empty() || edges.iter().any(|e| e.to == id_of(NodeKind::External, "llm", "")) {
        let models: Vec<serde_json::Value> = llm_models.iter().map(|(m, f)| json!({"model": m, "provider": provider_of(m), "files": f.len()})).collect();
        g.nodes.push(Node { id: id_of(NodeKind::External, "llm", ""), kind: NodeKind::External, name: "LLM".into(), path: "llm".into(), subkind: "llm".into(), attrs: BTreeMap::from([("models".into(), json!(models))]), ..Default::default() });
    }
    let node_index: HashMap<String, usize> = g.nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
    for (fid, m) in unresolved_calls { if let Some(n) = node_index.get(&fid).map(|&i| &mut g.nodes[i]) { let mut v: Vec<(String, u32)> = m.into_iter().collect(); v.sort_by(|a, b2| b2.1.cmp(&a.1)); n.attrs.insert("calls_unresolved".into(), json!(v.iter().take(12).map(|(k, c)| json!([k, c])).collect::<Vec<_>>())); } }

    // ---- types: extends / implements / references
    for ir in b.irs {
        let tmap = type_by_file.get(ir.path.as_str());
        let imap = import_map.get(ir.path.as_str());
        let resolve_t = |name: &str| -> Option<String> { tmap.and_then(|m| m.get(name).cloned()).or_else(|| imap.and_then(|m| m.get(name)).and_then(|s| type_by_file.get(s.file.as_str()).and_then(|m| m.get(&s.name).cloned()))).or_else(|| type_global.get(name).filter(|v| v.len() == 1).map(|v| v[0].clone())) };
        for t in &ir.types {
            let from = id_of(NodeKind::Type, &ir.path, &t.name);
            for e in &t.extends { if let Some(to) = resolve_t(e) { if to != from { edges.insert(Edge { from: from.clone(), to, kind: EdgeKind::Extends, line: t.line, unresolved: false }); } } }
            for e in &t.implements { if let Some(to) = resolve_t(e) { if to != from { edges.insert(Edge { from: from.clone(), to, kind: EdgeKind::Implements, line: t.line, unresolved: false }); } } }
            for e in &t.refs { if let Some(to) = resolve_t(e) { if to != from && !t.extends.contains(e) && !t.implements.contains(e) { edges.insert(Edge { from: from.clone(), to, kind: EdgeKind::References, line: t.line, unresolved: false }); } } }
        }
    }

    // ---- routes(入口)
    let fn_index: HashMap<String, usize> = g.nodes.iter().enumerate().filter(|(_, n)| n.kind == NodeKind::Function).map(|(i, n)| (n.id.clone(), i)).collect();
    let mw = b.cfg.middleware.as_ref();
    let mut route_count = 0;
    for ir in b.irs {
        if is_test(&ir.path) { continue; }
        let src = repo.read(&ir.path);
        let fid = id_of(NodeKind::File, &ir.path, "");
        let imports_guard = import_map.get(ir.path.as_str()).map(|m| m.values().any(|s| GUARD_RX.is_match(s.file.rsplit('/').next().unwrap_or("")))).unwrap_or(false) || edges.iter().any(|e| e.from == fid && e.kind == EdgeKind::Imports && GUARD_RX.is_match(e.to.rsplit('/').next().unwrap_or("")));
        let file_auth = AUTH_RX.is_match(&src) || imports_guard;
        let mut add = |g: &mut Graph, edges: &mut BTreeSet<Edge>, kind: &str, route: String, method: &str, handler: Option<String>, line: u32, extra: BTreeMap<String, serde_json::Value>| {
            let rkind = if kind == "http" && WEBHOOK_RX.is_match(&route) { "webhook" } else { kind };
            let auth = if file_auth { "inline" } else if let Some((_, matcher)) = mw { if rkind == "http" && matcher_hits(&route, matcher) { "middleware" } else { "none" } } else { "none" };
            let name = if method.is_empty() { route.clone() } else { format!("{} {}", method, route) };
            let id = id_of(NodeKind::Route, &ir.path, &name);
            let mut attrs = extra; attrs.insert("method".into(), json!(method)); attrs.insert("auth".into(), json!(auth)); attrs.insert("auth_via".into(), json!(if imports_guard { "import" } else if file_auth { "body" } else { "" }));
            let (excerpt, hash) = handler.as_ref().and_then(|h| fn_index.get(h)).map(|&i| (g.nodes[i].excerpt.clone(), g.nodes[i].hash.clone())).unwrap_or_default();
            g.nodes.push(Node { id: id.clone(), kind: NodeKind::Route, name, path: ir.path.clone(), line, layer: layer_of(&ir.path), lang: ir.lang.clone(), subkind: rkind.into(), attrs, excerpt, hash, ..Default::default() });
            edges.insert(Edge { from: fid.clone(), to: id.clone(), kind: EdgeKind::Contains, line, unresolved: false });
            if let Some(h) = handler { edges.insert(Edge { from: id, to: h, kind: EdgeKind::Handles, line, unresolved: false }); }
            route_count += 1;
        };
        let fmap = fn_by_file.get(ir.path.as_str());
        match ir.lang.as_str() {
            "ts" | "tsx" | "js" => {
                let is_route = regex::Regex::new(r"/route\.[tj]sx?$").unwrap().is_match(&ir.path) || ir.path.contains("pages/api/");
                if is_route {
                    let route = next_route_path(&ir.path);
                    let methods: Vec<&Func> = ir.functions.iter().filter(|f| f.exported && matches!(f.name.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS")).collect();
                    let aliased: Vec<(String, String)> = ir.aliases.iter().filter(|(a, _)| matches!(a.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS") && ir.extra_exports.contains(a)).cloned().collect();
                    for (m, local) in &aliased { if !methods.iter().any(|f| &f.name == m) { add(&mut g, &mut edges, "http", route.clone(), m, fmap.and_then(|fm| fm.get(local).cloned()), 1, BTreeMap::new()); } }
                    if methods.is_empty() && aliased.is_empty() { let h = ir.functions.iter().find(|f| f.exported).map(|f| id_of(NodeKind::Function, &ir.path, &f.name)); add(&mut g, &mut edges, "http", route.clone(), "ANY", h, 1, BTreeMap::new()); }
                    for f in methods { add(&mut g, &mut edges, "http", route.clone(), &f.name, Some(id_of(NodeKind::Function, &ir.path, &f.name)), f.line, BTreeMap::new()); }
                }
                if ir.directives.iter().any(|d| d == "use server") { for f in ir.functions.iter().filter(|f| f.exported && f.class.is_none()) { add(&mut g, &mut edges, "server_action", f.name.clone(), "", Some(id_of(NodeKind::Function, &ir.path, &f.name)), f.line, BTreeMap::new()); } }
                for c in ir.all_calls() { if let (Some(r), Some(a)) = (&c.receiver, &c.arg) { let rh = r.rsplit('.').next().unwrap_or(r); if matches!(c.name.as_str(), "get" | "post" | "put" | "patch" | "delete" | "all") && matches!(rh, "app" | "router" | "server" | "api" | "hono" | "fastify" | "r") && a.starts_with('/') { let h = fmap.and_then(|m| m.iter().find(|(_, _)| false).map(|(_, v)| v.clone())); add(&mut g, &mut edges, "http", a.clone(), &c.name.to_uppercase(), h.or_else(|| enclosing_fn(ir, c.line)), c.line, BTreeMap::new()); } } }
                if regex::Regex::new(r"(?:^|/)(?:functions|supabase/functions)/[^/]+/index\.[tj]s$").unwrap().is_match(&ir.path) { let n = ir.path.split('/').rev().nth(1).unwrap_or("").to_string(); add(&mut g, &mut edges, "http", format!("/functions/{}", n), "ANY", None, 1, BTreeMap::new()); }
            }
            "py" => {
                for f in &ir.functions { for d in &f.decorators { if let Some(m) = regex::Regex::new(r#"^\w+\.(get|post|put|patch|delete|route|api_route|websocket)\(\s*['"]([^'"]+)['"]"#).unwrap().captures(d) { add(&mut g, &mut edges, "http", m[2].into(), &m[1].to_uppercase(), Some(id_of(NodeKind::Function, &ir.path, &f.name)), f.line, BTreeMap::new()); } if regex::Regex::new(r"(?:^|\.)(task|shared_task|actor)\b").unwrap().is_match(d) { add(&mut g, &mut edges, "queue", f.name.clone(), "", Some(id_of(NodeKind::Function, &ir.path, &f.name)), f.line, BTreeMap::new()); } } }
                if ir.path.ends_with("urls.py") { for c in ir.all_calls() { if (c.name == "path" || c.name == "re_path") { if let Some(a) = &c.arg { add(&mut g, &mut edges, "http", format!("/{}", a.trim_start_matches('^').trim_start_matches('/')), "ANY", None, c.line, BTreeMap::new()); } } } }
                if src.contains("__name__") && src.contains("__main__") { add(&mut g, &mut edges, "cli", ir.path.clone(), "", fmap.and_then(|m| m.get("main").cloned()), 1, BTreeMap::new()); }
            }
            "rs" => {
                for c in ir.all_calls() { if c.name == "route" { if let Some(a) = &c.arg { let line_txt = src.lines().nth(c.line as usize - 1).unwrap_or(""); let m = regex::Regex::new(r"(get|post|put|patch|delete|any)\s*\(\s*([A-Za-z_][\w:]*)").unwrap().captures(line_txt); let method = m.as_ref().map(|x| x[1].to_uppercase()).unwrap_or("ANY".into()); let h = m.as_ref().and_then(|x| { let n = x[2].rsplit("::").next().unwrap_or("").to_string(); fmap.and_then(|fm| fm.get(&n).cloned()).or_else(|| import_map.get(ir.path.as_str()).and_then(|im| im.get(&n)).and_then(|s| fn_by_file.get(s.file.as_str()).and_then(|fm| fm.get(&s.name).cloned()))) }); add(&mut g, &mut edges, "http", a.clone(), &method, h, c.line, BTreeMap::new()); } } }
                for f in &ir.functions { for d in &f.decorators { if let Some(m) = regex::Regex::new(r#"#\[(get|post|put|patch|delete)\(\s*"([^"]+)""#).unwrap().captures(d) { add(&mut g, &mut edges, "http", m[2].into(), &m[1].to_uppercase(), Some(id_of(NodeKind::Function, &ir.path, &f.name)), f.line, BTreeMap::new()); } } if f.name == "main" && f.class.is_none() { add(&mut g, &mut edges, "cli", ir.path.clone(), "", Some(id_of(NodeKind::Function, &ir.path, "main")), f.line, BTreeMap::new()); } }
            }
            "go" => {
                for c in ir.all_calls() { if matches!(c.name.as_str(), "HandleFunc" | "Handle" | "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "Get" | "Post" | "Put" | "Patch" | "Delete") { if let Some(a) = &c.arg { if a.starts_with('/') { let line_txt = src.lines().nth(c.line as usize - 1).unwrap_or(""); let h = regex::Regex::new(r#"",\s*([A-Za-z_][\w.]*)\s*\)"#).unwrap().captures(line_txt).and_then(|m| { let n = m[1].rsplit('.').next().unwrap_or("").to_string(); fmap.and_then(|fm| fm.get(&n).cloned()) }); add(&mut g, &mut edges, "http", a.clone(), &(if c.name.len() <= 6 && c.name.chars().all(|ch| ch.is_uppercase()) { c.name.clone() } else { "ANY".into() }), h.or_else(|| enclosing_fn(ir, c.line)), c.line, BTreeMap::new()); } } } }
                for f in &ir.functions { if f.name == "main" && f.class.is_none() { add(&mut g, &mut edges, "cli", ir.path.clone(), "", Some(id_of(NodeKind::Function, &ir.path, "main")), f.line, BTreeMap::new()); } }
            }
            _ => {}
        }
    }
    let _ = route_count;

    // ---- jobs(cron)
    for c in &b.cfg.crons {
        let id = id_of(NodeKind::Job, &c.source, &format!("{} {}", c.schedule, c.path));
        let target = if c.source == "vercel.json" { let p = c.path.split('?').next().unwrap_or("").to_string(); g.nodes.iter().find(|n| n.kind == NodeKind::Route && n.subkind == "http" && n.name.ends_with(&format!(" {}", p))).map(|n| n.id.clone()) } else { None };
        let auth = target.as_ref().and_then(|t| g.nodes.iter().find(|n| &n.id == t)).and_then(|n| n.attrs.get("auth")).and_then(|a| a.as_str()).unwrap_or("unknown").to_string();
        g.nodes.push(Node { id: id.clone(), kind: NodeKind::Job, name: c.path.clone(), path: c.source.clone(), subkind: "cron".into(), attrs: BTreeMap::from([("schedule".into(), json!(c.schedule)), ("human".into(), json!(cron_human(&c.schedule))), ("auth".into(), json!(auth))]), ..Default::default() });
        if let Some(t) = target { edges.insert(Edge { from: id, to: t, kind: EdgeKind::Triggers, line: 0, unresolved: false }); }
    }
    // ---- tested(テストファイルの関数から呼ばれている)
    let test_fns: HashSet<&str> = g.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Function | NodeKind::File) && is_test(&n.path)).map(|n| n.id.as_str()).collect();
    let tested: HashSet<String> = edges.iter().filter(|e| e.kind == EdgeKind::Calls && test_fns.contains(e.from.as_str())).map(|e| e.to.clone()).collect();
    let idx2: HashMap<String, usize> = g.nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
    for id in &tested { if let Some(&i) = idx2.get(id) { g.nodes[i].attrs.insert("tested".into(), json!(true)); } }
    // route の tested = handler が tested か、同じファイルの名前がテストに出る
    let handled: Vec<(String, String)> = edges.iter().filter(|e| e.kind == EdgeKind::Handles).map(|e| (e.from.clone(), e.to.clone())).collect();
    for (r, h) in handled { if tested.contains(&h) { if let Some(&i) = idx2.get(&r) { g.nodes[i].attrs.insert("tested".into(), json!(true)); } } }
    g.edges = edges.into_iter().collect();
    g
}

fn enclosing_fn(ir: &FileIR, line: u32) -> Option<String> { ir.functions.iter().filter(|f| f.line <= line && line <= f.end_line).min_by_key(|f| f.end_line - f.line).map(|f| id_of(NodeKind::Function, &ir.path, &match &f.class { Some(c) => format!("{}.{}", c, f.name), None => f.name.clone() })) }

fn go_pkg_fn(target_file: &str, name: &str, fn_by_file: &HashMap<&str, HashMap<String, String>>) -> Option<String> {
    let d = target_file.rsplit_once('/').map(|x| x.0).unwrap_or(target_file);
    for (f, m) in fn_by_file { if f.rsplit_once('/').map(|x| x.0) == Some(d) { if let Some(id) = m.get(name) { return Some(id.clone()); } } }
    None
}

enum Resolved { File(String), Package(String), None }

fn resolve_import(lang: &str, spec: &str, from: &str, files: &HashSet<&str>, cfg: &Config) -> Resolved {
    let dir = from.rsplit_once('/').map(|x| x.0).unwrap_or("");
    let try_files = |base: &str, exts: &[&str]| -> Option<String> {
        let base = normalize(base);
        if files.contains(base.as_str()) && !base.ends_with('/') { return Some(base); }
        for (from_ext, to_exts) in [(".js", [".ts", ".tsx"]), (".mjs", [".mts", ".ts"]), (".cjs", [".cts", ".ts"]), (".jsx", [".tsx", ".ts"])] { if let Some(stem) = base.strip_suffix(from_ext) { for e in to_exts { let c = format!("{}{}", stem, e); if files.contains(c.as_str()) { return Some(c); } } } }
        for e in exts { let c = format!("{}{}", base, e); if files.contains(c.as_str()) { return Some(c); } }
        for e in exts { let c = format!("{}/index{}", base, e); if files.contains(c.as_str()) { return Some(c); } }
        None
    };
    match lang {
        "ts" | "tsx" | "js" => {
            let exts = [".ts", ".tsx", ".js", ".jsx", ".mjs", ".mts", ".vue", ".svelte"];
            if spec.starts_with('.') { return try_files(&format!("{}/{}", dir, spec), &exts).map(Resolved::File).unwrap_or(Resolved::None); }
            for (alias, target) in &cfg.ts_aliases { if spec.starts_with(alias) { return try_files(&format!("{}{}", target, &spec[alias.len()..]), &exts).map(Resolved::File).unwrap_or(Resolved::None); } if spec == alias.trim_end_matches('/') { return try_files(target.trim_end_matches('/'), &exts).map(Resolved::File).unwrap_or(Resolved::None); } }
            if spec.starts_with("node:") { return Resolved::None; }
            let pkg = if spec.starts_with('@') { spec.splitn(3, '/').take(2).collect::<Vec<_>>().join("/") } else { spec.split('/').next().unwrap_or(spec).to_string() };
            Resolved::Package(pkg)
        }
        "py" => {
            if spec.starts_with('.') {
                let dots = spec.chars().take_while(|c| *c == '.').count();
                let mut d = dir.to_string();
                for _ in 1..dots { d = d.rsplit_once('/').map(|x| x.0).unwrap_or("").to_string(); }
                let rel = spec.trim_start_matches('.').replace('.', "/");
                let base = if rel.is_empty() { d.clone() } else if d.is_empty() { rel } else { format!("{}/{}", d, rel) };
                return try_files(&base, &[".py", "/__init__.py"]).map(Resolved::File).unwrap_or(Resolved::None);
            }
            let base = spec.replace('.', "/");
            for pre in ["", "src/", "py/", "app/", "backend/"] { if let Some(f) = try_files(&format!("{}{}", pre, base), &[".py", "/__init__.py"]) { return Resolved::File(f); } }
            // from の呼び元と同じ親ディレクトリに置かれたパッケージ(例: py/app/service.py から `from app.x`)
            let mut d = dir.to_string();
            while !d.is_empty() { if let Some(f) = try_files(&format!("{}/{}", d, base), &[".py", "/__init__.py"]) { return Resolved::File(f); } d = d.rsplit_once('/').map(|x| x.0).unwrap_or("").to_string(); }
            if spec.starts_with("typing") || spec == "os" || spec == "sys" || spec == "json" || spec == "re" { return Resolved::None; }
            Resolved::Package(spec.split('.').next().unwrap_or(spec).to_string())
        }
        "rs" => {
            if let Some(m) = spec.strip_prefix("mod ") { let stem = from.rsplit('/').next().unwrap_or("").trim_end_matches(".rs"); let base = if matches!(stem, "mod" | "lib" | "main") { dir.to_string() } else { format!("{}/{}", dir, stem) }; return try_files(&format!("{}/{}", base, m), &[".rs", "/mod.rs"]).map(Resolved::File).unwrap_or(Resolved::None); }
            let parts: Vec<&str> = spec.split("::").collect();
            if parts.is_empty() { return Resolved::None; }
            let (root, segs): (String, &[&str]) = match parts[0] { "crate" => { let r = from.find("src/").map(|i| &from[..i + 3]).unwrap_or("src").to_string(); (r, &parts[1..]) } "super" => (dir.rsplit_once('/').map(|x| x.0).unwrap_or("").to_string(), &parts[1..]), "self" => (dir.to_string(), &parts[1..]), "std" | "core" | "alloc" => return Resolved::None, other => return Resolved::Package(other.to_string()) };
            for n in (1..=segs.len()).rev() { let base = format!("{}/{}", root, segs[..n].join("/")); if let Some(f) = try_files(&base, &[".rs", "/mod.rs"]) { return Resolved::File(f); } }
            try_files(&root, &["/mod.rs", "/lib.rs", "/main.rs"]).map(Resolved::File).unwrap_or(Resolved::None)
        }
        "go" => {
            if !cfg.go_module.is_empty() && spec.starts_with(&cfg.go_module) { let d = spec[cfg.go_module.len()..].trim_start_matches('/'); let gd = from.find("go/").map(|i| format!("{}{}", &from[..i + 3], d)).unwrap_or(d.to_string()); let cand = files.iter().find(|f| f.rsplit_once('/').map(|x| x.0) == Some(gd.as_str()) && f.ends_with(".go")); return cand.map(|f| Resolved::File(f.to_string())).unwrap_or(Resolved::None); }
            if spec.split('/').next().map(|s| s.contains('.')).unwrap_or(false) { return Resolved::Package(spec.split('/').take(3).collect::<Vec<_>>().join("/")); }
            Resolved::None
        }
        _ => Resolved::None,
    }
}

fn normalize(p: &str) -> String { let mut out: Vec<&str> = Vec::new(); for seg in p.split('/') { match seg { "" | "." => {} ".." => { out.pop(); } s => out.push(s) } } out.join("/") }

fn next_route_path(path: &str) -> String {
    if let Some(m) = regex::Regex::new(r"(?:^|/)(?:app|src/app)/(.+)/route\.[tj]sx?$").unwrap().captures(path) { let segs: Vec<&str> = m[1].split('/').filter(|s| !(s.starts_with('(') && s.ends_with(')'))).collect(); return format!("/{}", segs.join("/")); }
    if let Some(m) = regex::Regex::new(r"(?:^|/)pages/api/(.+)\.[tj]sx?$").unwrap().captures(path) { return format!("/api/{}", m[1].trim_end_matches("/index")); }
    "/".into()
}

fn matcher_hits(route: &str, matcher: &[String]) -> bool {
    if matcher.is_empty() { return true; }
    matcher.iter().any(|pat| {
        // Next.js の matcher は否定先読みを使うことが多い(regex crate 非対応)。先読みを含む場合は「除外対象の語を含まなければ一致」とみなす
        if let Some(i) = pat.find("(?!") { let end = pat[i..].find(')').map(|e| i + e).unwrap_or(pat.len()); let excl: Vec<&str> = pat[i + 3..end].split('|').collect(); return !excl.iter().any(|x| !x.is_empty() && route.contains(x.trim_end_matches("$").trim_end_matches(".*"))); }
        let rx = format!("^{}$", regex::Regex::new(r":\w+\*?").unwrap().replace_all(&pat.replace('*', ".*"), "[^/]+"));
        regex::Regex::new(&rx).map(|r| r.is_match(route)).unwrap_or(true)
    })
}

pub fn cron_human(s: &str) -> String {
    let p: Vec<&str> = s.split_whitespace().collect();
    if p.len() != 5 { return s.into(); }
    let (mi, h, dom, _mon, dow) = (p[0], p[1], p[2], p[3], p[4]);
    if let Some(n) = h.strip_prefix("*/") { return format!("{} 時間ごと", n); }
    if let Some(n) = mi.strip_prefix("*/") { if h == "*" { return format!("{} 分ごと", n); } }
    if h == "*" && mi == "*" { return "毎分".into(); }
    if h == "*" { return format!("毎時 {} 分", mi); }
    let when = if h.chars().all(|c| c.is_ascii_digit()) { format!("{}:{:0>2} UTC", h, mi) } else { s.into() };
    let days = |d: &str| d.split(',').map(|x| match x { "0" | "7" => "日", "1" => "月", "2" => "火", "3" => "水", "4" => "木", "5" => "金", "6" => "土", o => o }).collect::<Vec<_>>().join("・");
    if dow != "*" { return format!("毎週 {} {}", days(dow), when); }
    if dom != "*" { return format!("毎月 {} 日 {}", dom, when); }
    format!("毎日 {}", when)
}

fn to_snake(s: &str) -> String { let mut out = String::new(); for (i, c) in s.chars().enumerate() { if c.is_uppercase() { if i > 0 { out.push('_'); } out.push(c.to_ascii_lowercase()); } else { out.push(c); } } out }

fn pkg_category(p: &str) -> &'static str {
    let l = p.to_lowercase();
    if l == "ai" || l.starts_with("@ai-sdk") || l == "openai" || l.contains("anthropic") || l.contains("genai") || l.contains("generative") || l.contains("langchain") { "ai" }
    else if l.contains("supabase") || l == "prisma" || l.contains("prisma") || l.contains("drizzle") || l == "pg" || l == "sqlx" || l == "diesel" || l.contains("sqlalchemy") || l == "mongoose" { "db" }
    else if l == "stripe" || l.starts_with("@slack") || l == "resend" || l.contains("sendgrid") || l == "twilio" || l.contains("googleapis") || l.contains("notion") || l.contains("@line/") || l.contains("octokit") { "saas" }
    else if l == "react" || l == "next" || l == "vue" || l == "antd" || l.starts_with("@radix") || l.contains("tailwind") { "framework" }
    else if l.contains("sentry") || l.contains("datadog") { "monitoring" }
    else { "lib" }
}

fn host_category(h: &str) -> String { if h.contains("openai") || h.contains("anthropic") || h.contains("generativelanguage") { "ai".into() } else if h.contains("slack") || h.contains("discord") || h.contains("line.me") { "chat".into() } else if h.contains("stripe") { "payment".into() } else if h.contains("supabase") || h.contains("vercel") { "infra".into() } else { "api".into() } }

pub fn host_name(h: &str) -> String {
    for (rx, n) in [("api.openai.com", "OpenAI"), ("api.anthropic.com", "Anthropic"), ("generativelanguage.googleapis.com", "Gemini"), ("googleads.googleapis.com", "Google Ads"), ("sheets.googleapis.com", "Google Sheets"), ("drive.googleapis.com", "Google Drive"), ("oauth2.googleapis.com", "Google OAuth"), ("www.googleapis.com", "Google APIs"), ("googleapis.com", "Google APIs"), ("hooks.slack.com", "Slack Webhook"), ("slack.com", "Slack"), ("api.stripe.com", "Stripe"), ("graph.facebook.com", "Meta Graph"), ("api.line.me", "LINE"), ("api.notion.com", "Notion"), ("api.github.com", "GitHub"), ("api.resend.com", "Resend"), ("api.twilio.com", "Twilio"), ("supabase.co", "Supabase"), ("api.supabase.com", "Supabase Management"), ("vercel.com", "Vercel"), ("api.hubapi.com", "HubSpot"), ("kintone.com", "kintone"), ("tiktok.com", "TikTok"), ("discord.com", "Discord"), ("sentry.io", "Sentry"), ("drive.google.com", "Google Drive")] { if h.contains(rx) { return n.into(); } }
    h.into()
}

pub fn provider_of(m: &str) -> &'static str {
    let l = m.to_lowercase();
    if let Some(p) = l.split('/').next() { if l.contains('/') { return match p { "anthropic" => "anthropic", "openai" => "openai", "google" => "google", "xai" => "xai", "meta" => "meta", "mistral" => "mistral", "amazon" => "amazon", "cohere" => "cohere", "perplexity" => "perplexity", _ => "other" }; } }
    for (k, p) in [("claude", "anthropic"), ("gpt", "openai"), ("o1", "openai"), ("o3", "openai"), ("o4", "openai"), ("text-embedding", "openai"), ("whisper", "openai"), ("dall-e", "openai"), ("gemini", "google"), ("llama", "meta"), ("mistral", "mistral"), ("mixtral", "mistral"), ("deepseek", "deepseek"), ("grok", "xai"), ("command", "cohere"), ("sonar", "perplexity")] { if l.starts_with(k) { return p; } }
    "unknown"
}
