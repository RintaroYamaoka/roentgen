//! analyze の本流:repo → IR → graph → analysis → data/*.json + graph.db。
use crate::extract;
use crate::model::Graph;
use crate::repo::Repo;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta { pub root: String, pub name: String, pub sha: String, pub date: String, pub branch: String, pub remote: String, pub analyzed: String, pub files: usize, pub src_files: usize, pub src_lines: u64, pub langs: Vec<(String, String, usize, u64)>, pub products: Vec<Product>, pub docs: Vec<(String, Vec<String>)>, pub version: String, pub elapsed_ms: u128, pub unknown: Vec<Unknown> }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Product { pub name: String, pub evidence: Vec<(String, Vec<String>)>, pub estimated: bool }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Unknown { pub what: String, pub why: String, pub fix: String }

pub struct Opts { pub db_url: Option<String>, pub use_gh: bool, pub rules: Option<Vec<(String, String, String)>> }

pub fn run(root: &Path, out: &Path, opts: &Opts, log: &dyn Fn(&str)) -> Result<(Meta, Graph, crate::analyze::Analysis)> {
    let t0 = Instant::now();
    let repo = Repo::open(root)?;
    log(&format!("  repo {}  files={} src={}", repo.root.display(), repo.files.len(), repo.src_files().len()));
    let irs = extract::extract_all(&repo);
    log(&format!("  extract   {:>6.1}s  ir={} parse_errors={}", t0.elapsed().as_secs_f32(), irs.len(), irs.iter().filter(|i| i.parse_error).count()));
    let cfg = extract::config::collect(&repo);
    let mut unknown = Vec::new();
    let (tables, migrations) = load_tables(&repo, &cfg, opts.db_url.as_deref(), &mut unknown);
    log(&format!("  schema    {:>6.1}s  tables={} migrations={}", t0.elapsed().as_secs_f32(), tables.len(), migrations.len()));
    let g = crate::graph::build(&crate::graph::Build { repo: &repo, irs: &irs, cfg: &cfg, tables: &tables });
    log(&format!("  graph     {:>6.1}s  nodes={} edges={}", t0.elapsed().as_secs_f32(), g.nodes.len(), g.edges.len()));
    let rules = opts.rules.clone().or_else(|| read_rules(&repo));
    let a = crate::analyze::run(&repo, &g, opts.use_gh, rules);
    log(&format!("  analyze   {:>6.1}s  traces={} churn={} commits={}", t0.elapsed().as_secs_f32(), a.traces.len(), a.churn.len(), a.commits));
    if a.prs.is_empty() && opts.use_gh && repo.is_git { unknown.push(Unknown { what: "PR の一覧".into(), why: "gh で取れなかった(権限・remote が GitHub でない・gh 未導入)".into(), fix: "gh auth login".into() }); }
    if a.rules_estimated { unknown.push(Unknown { what: "依存の規則(確定版)".into(), why: ".roentgen.yml が無い".into(), fix: "Dependencies の雛形を repo に置く".into() }); }
    if opts.db_url.is_none() { unknown.push(Unknown { what: "DB の実物(行数・index・実際の RLS)".into(), why: "--db を渡していない".into(), fix: "SELECT 専用ロールの接続文字列を --db に".into() }); }
    for i in irs.iter().filter(|i| i.parse_error).take(20) { unknown.push(Unknown { what: format!("構文木に誤りがある: {}", i.path), why: "tree-sitter が一部を読めなかった(新しい構文か壊れたコード)".into(), fix: "その部分の関数・呼び出しは欠ける".into() }); }
    let unsupported: Vec<String> = { let mut v: Vec<String> = irs.iter().filter(|i| !matches!(i.lang.as_str(), "ts" | "tsx" | "js" | "py" | "rs" | "go" | "sql")).map(|i| crate::repo::lang_name(&i.lang).to_string()).collect(); v.sort(); v.dedup(); v };
    if !unsupported.is_empty() { unknown.push(Unknown { what: format!("{} の関数・呼び出し・型", unsupported.join(" / ")), why: "言語アダプタが未対応(行数・churn・env は出る)".into(), fix: "extract/ にアダプタを足す".into() }); }
    let meta = meta_of(&repo, &irs, &cfg, unknown, t0.elapsed().as_millis());
    let data = out.join("data");
    std::fs::create_dir_all(&data)?;
    crate::store::write_json(&data.join("meta.json"), &meta)?;
    crate::store::write_json(&data.join("graph.json"), &g)?;
    crate::store::write_json(&data.join("analysis.json"), &a)?;
    crate::store::write_json(&data.join("config.json"), &cfg)?;
    crate::store::write_json(&data.join("tables.json"), &serde_json::json!({"tables": tables, "migrations": migrations, "sources": cfg.db_sources, "source": if opts.db_url.is_some() { "live" } else { "migration" }}))?;
    crate::store::write_db(&data.join("graph.db"), &g, &a)?;
    log(&format!("  saved     {:>6.1}s  → {}", t0.elapsed().as_secs_f32(), data.display()));
    Ok((meta, g, a))
}

pub fn load_tables(repo: &Repo, cfg: &extract::config::Config, db_url: Option<&str>, unknown: &mut Vec<Unknown>) -> (BTreeMap<String, extract::sql::Table>, Vec<extract::sql::Migration>) {
    let mut tables = BTreeMap::new();
    let mut migs = Vec::new();
    // migration の本数が多いディレクトリを主にする(凍結した旧スキーマより新しい方を優先)
    let mut sources = cfg.db_sources.clone();
    sources.sort_by_key(|(d, k)| if k == "sql" { -(repo.files.iter().filter(|f| f.starts_with(&format!("{}/", d))).count() as i64) } else { 0 });
    for (dir, kind) in &sources {
        match kind.as_str() {
            "sql" => { let files: Vec<(String, String)> = repo.files.iter().filter(|f| f.starts_with(&format!("{}/", dir)) && f.ends_with(".sql")).map(|f| (f.clone(), repo.read(f))).collect(); let (t, m) = extract::sql::from_sql_files(&files); for (k, v) in t { tables.entry(k).or_insert(v); } migs.extend(m); }
            "prisma" => { for (k, v) in extract::sql::from_prisma(dir, &repo.read(dir)) { tables.entry(k).or_insert(v); } }
            "drizzle" => { for (k, v) in extract::sql::from_drizzle(dir, &repo.read(dir)) { tables.entry(k).or_insert(v); } }
            "diesel" => { for (k, v) in extract::sql::from_diesel(dir, &repo.read(dir)) { tables.entry(k).or_insert(v); } }
            _ => {}
        }
    }
    if let Some(url) = db_url { match crate::live::introspect(url) { Ok(live) => { let (t, drift) = crate::live::merge(&tables, live); tables = t; if !drift.is_empty() { unknown.push(Unknown { what: "migration と実物の差".into(), why: drift.join(" / "), fix: "Data Model の drift を見る".into() }); } } Err(e) => unknown.push(Unknown { what: "DB の実物".into(), why: format!("接続できなかった({})", e), fix: "psql を入れ、SELECT 専用ロールの接続文字列を --db に".into() }) } }
    if tables.is_empty() && sources.is_empty() { unknown.push(Unknown { what: "DB スキーマ".into(), why: "migration / Prisma / Drizzle / Diesel のどれも見つからなかった".into(), fix: "--db で実物に繋ぐ".into() }); }
    migs.sort_by(|a, b| a.file.cmp(&b.file));
    (tables, migs)
}

fn read_rules(repo: &Repo) -> Option<Vec<(String, String, String)>> {
    if !repo.exists(".roentgen.yml") { return None; }
    let txt = repo.read(".roentgen.yml");
    let rx = regex::Regex::new(r"from:\s*([^,}\n]+),\s*to:\s*([^,}\n]+)(?:,\s*kind:\s*([^}\n]+))?").unwrap();
    let v: Vec<(String, String, String)> = rx.captures_iter(&txt).map(|m| (m[1].trim().into(), m[2].trim().into(), m.get(3).map(|x| x.as_str().trim().to_string()).unwrap_or("禁止".into()))).collect();
    if v.is_empty() { None } else { Some(v) }
}

fn meta_of(repo: &Repo, irs: &[extract::ir::FileIR], cfg: &extract::config::Config, unknown: Vec<Unknown>, elapsed_ms: u128) -> Meta {
    let sha = repo.git(&["rev-parse", "--short", "HEAD"]).trim().to_string();
    let date = repo.git(&["log", "-1", "--format=%cs"]).trim().to_string();
    let mut langs: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for i in irs { let e = langs.entry(i.lang.clone()).or_insert((0, 0)); e.0 += 1; e.1 += i.lines as u64; }
    let mut lv: Vec<(String, String, usize, u64)> = langs.into_iter().map(|(l, (f, n))| (l.clone(), crate::repo::lang_name(&l).to_string(), f, n)).collect();
    lv.sort_by(|a, b| b.3.cmp(&a.3));
    let docs: Vec<(String, Vec<String>)> = repo.files.iter().filter(|f| { let b = f.rsplit('/').next().unwrap_or(""); (b.to_uppercase().starts_with("README") || b == "CLAUDE.md" || b == "AGENTS.md" || b.to_uppercase().starts_with("CONTRIBUTING") || b.to_uppercase().starts_with("ARCHITECTURE")) && b.ends_with(".md") || (f.starts_with("docs/") && f.matches('/').count() == 1 && f.ends_with(".md")) }).take(40)
        .map(|f| { let heads: Vec<String> = regex::Regex::new(r"(?m)^#{1,2}\s+(.+)$").unwrap().captures_iter(&repo.read(f)).map(|m| m[1].trim().to_string()).take(12).collect(); (f.clone(), heads) }).collect();
    Meta { root: repo.root.to_string_lossy().to_string(), name: repo.root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), sha: if sha.is_empty() { "nogit".into() } else { sha }, date: if date.is_empty() { crate::repo::today() } else { date }, branch: repo.git(&["rev-parse", "--abbrev-ref", "HEAD"]).trim().into(), remote: repo.git(&["remote", "get-url", "origin"]).trim().into(), analyzed: crate::repo::today(), files: repo.files.len(), src_files: irs.len(), src_lines: lv.iter().map(|l| l.3).sum(), langs: lv, products: products(repo, cfg), docs, version: env!("CARGO_PKG_VERSION").into(), elapsed_ms, unknown }
}

fn products(repo: &Repo, cfg: &extract::config::Config) -> Vec<Product> {
    let mut ev: Vec<(String, Vec<String>)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let rx = regex::Regex::new(r"(?:^|/)app/\(([^)]+)\)/").unwrap();
    let mut groups: Vec<String> = repo.files.iter().filter_map(|f| rx.captures(f).map(|m| m[1].to_string())).collect(); groups.sort(); groups.dedup();
    if groups.len() >= 2 { ev.push(("route_group".into(), groups.clone())); for g in &groups { if !matches!(g.to_lowercase().as_str(), "auth" | "public" | "private" | "protected" | "dashboard" | "marketing" | "main" | "app" | "site" | "admin" | "root" | "default") { names.push(g.clone()); } } }
    let mut roots: Vec<String> = cfg.db_sources.iter().filter(|(_, k)| k == "sql").map(|(d, _)| d.split('/').next().unwrap_or("").to_string()).collect(); roots.sort(); roots.dedup();
    if roots.len() >= 2 { ev.push(("migration_dirs".into(), roots.clone())); for r in &roots { let n = regex::Regex::new(r"^(supabase|prisma|drizzle|migrations?)[-_]?").unwrap().replace(r, "").to_string(); if !n.is_empty() && !names.contains(&n) { names.push(n); } } }
    if !cfg.workspaces.is_empty() { ev.push(("workspaces".into(), cfg.workspaces.clone())); for w in &cfg.workspaces { if !w.contains('*') { names.push(w.rsplit('/').next().unwrap_or(w).into()); } } }
    if !cfg.cargo_members.is_empty() { ev.push(("cargo_members".into(), cfg.cargo_members.clone())); for w in &cfg.cargo_members { if !w.contains('*') { names.push(w.rsplit('/').next().unwrap_or(w).into()); } } }
    let readme = if repo.exists("README.md") { repo.read("README.md") } else { String::new() };
    if regex::Regex::new(r"(\d)\s*製品|two products|(\d) products|同居").unwrap().is_match(&readme) { ev.push(("readme".into(), vec!["README に複数製品の記述".into()])); }
    let main = repo.root.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if ev.is_empty() { return vec![Product { name: main, evidence: vec![], estimated: false }]; }
    let mut out = vec![Product { name: format!("{}(主)", main), evidence: ev.clone(), estimated: true }];
    names.sort(); names.dedup();
    for n in names.into_iter().take(6) { out.push(Product { name: n, evidence: ev.clone(), estimated: true }); }
    out
}

pub fn default_out(root: &Path, meta: &Meta) -> PathBuf { crate::snapshot::home().join("out").join(crate::snapshot::repo_key(root)).join(format!("{}-{}", meta.analyzed, meta.sha)) }
