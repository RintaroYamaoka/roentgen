//! repo の走査(gitignore を尊重)・言語判定・層の判定・git の呼び出し。
use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Repo {
    pub root: PathBuf,
    pub files: Vec<String>,
    pub is_git: bool,
    contents: BTreeMap<String, String>,
}

const MAX_READ: u64 = 2_000_000;

pub fn lang_of(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?;
    Some(match ext {
        "ts" | "mts" | "cts" => "ts", "tsx" => "tsx", "js" | "mjs" | "cjs" | "jsx" => "js", "py" => "py", "rs" => "rs", "go" => "go", "sql" => "sql",
        "java" => "java", "kt" => "kotlin", "rb" => "ruby", "php" => "php", "cs" => "csharp", "swift" => "swift", "scala" => "scala", "dart" => "dart", "ex" | "exs" => "elixir", "vue" | "svelte" => "js",
        _ => return None,
    })
}

pub fn lang_name(l: &str) -> &'static str {
    match l { "ts" | "tsx" => "TypeScript", "js" => "JavaScript", "py" => "Python", "rs" => "Rust", "go" => "Go", "sql" => "SQL", "java" => "Java", "kotlin" => "Kotlin", "ruby" => "Ruby", "php" => "PHP", "csharp" => "C#", "swift" => "Swift", "scala" => "Scala", "dart" => "Dart", "elixir" => "Elixir", _ => "other" }
}

pub fn is_generated(p: &str) -> bool {
    p.ends_with(".d.ts") || p.ends_with(".min.js") || p.ends_with("database.types.ts") || p.contains(".generated.") || p.ends_with("_pb2.py") || p.ends_with(".pb.go") || p.ends_with("lock.json") || p.ends_with(".lock")
}

pub fn is_skipped_dir(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    parts.iter().take(parts.len().saturating_sub(1)).any(|d| matches!(*d, "node_modules" | "dist" | "build" | "out" | ".next" | ".nuxt" | ".svelte-kit" | "target" | "vendor" | ".git" | "__pycache__" | ".venv" | "venv" | ".mypy_cache" | ".ruff_cache" | "coverage" | ".turbo" | ".cache" | "public"))
}

pub fn is_test(p: &str) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    let base = *parts.last().unwrap_or(&"");
    parts.iter().any(|d| matches!(*d, "tests" | "test" | "__tests__" | "spec" | "e2e") && *d != base)
        || base.contains(".test.") || base.contains(".spec.") || base.ends_with("_test.py") || base.ends_with("_test.go") || base.ends_with("_test.rs") || base.starts_with("test_") && base.ends_with(".py")
        || p.starts_with("tests-") || p.contains("/tests-")
}

/// 層 = src/ の下 1 段(src/adapter/)。root 直下は 1 段。app/ pages/ api/ は深く切らない。
pub fn layer_of(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 1 { return "(root)".into(); }
    let dirs = &parts[..parts.len() - 1];
    let mut i = 0;
    if matches!(dirs[0], "src" | "lib" | "packages" | "apps") && dirs.len() > 1 { i = 1; }
    let picked = &dirs[..(i + 1).min(dirs.len())];
    format!("{}/", picked.join("/"))
}

pub fn role_of(layer: &str) -> &'static str {
    let l = layer.trim_end_matches('/');
    let last = l.rsplit('/').next().unwrap_or(l);
    match last {
        "app" | "pages" | "routes" | "api" | "entry" | "entries" | "cmd" | "handlers" | "handler" | "controllers" | "controller" | "presentation" | "web" | "http" | "bin" | "mcp" => "entry",
        "ui" | "components" | "views" | "screens" | "templates" | "frontend" | "client" => "ui",
        "application" | "usecases" | "usecase" | "use_cases" | "services" | "service" | "workflows" | "features" | "commands" | "queries" => "application",
        "core" | "domain" | "entities" | "models" | "model" | "types" | "schemas" | "ports" | "interfaces" => "core",
        "adapters" | "adapter" | "infra" | "infrastructure" | "repositories" | "repository" | "db" | "database" | "persistence" | "gateways" | "clients" | "external" | "integrations" | "providers" | "supabase" | "prisma" | "lib" | "utils" | "util" | "shared" | "common" | "pkg" | "internal" | "store" => "adapter",
        _ => "other",
    }
}

impl Repo {
    pub fn open(root: &Path) -> Result<Repo> {
        let root = root.canonicalize()?;
        let is_git = root.join(".git").exists();
        let mut files = Vec::new();
        let root_c = root.clone();
        let walker = ignore::WalkBuilder::new(&root).hidden(false).git_ignore(true).git_exclude(true).follow_links(false)
            .filter_entry(move |e| { let p = e.path(); if p == root_c { return true; } if e.file_type().map(|t| t.is_dir()).unwrap_or(false) { if p.join(".git").exists() { return false; } let n = e.file_name().to_string_lossy(); if matches!(n.as_ref(), "node_modules" | "dist" | "build" | ".next" | "target" | "vendor" | "__pycache__" | ".venv" | "venv" | "coverage" | ".turbo" | ".cache" | ".mypy_cache" | ".ruff_cache") { return false; } } true })
            .build();
        for e in walker.flatten() {
            if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let rel = pathdiff::diff_paths(e.path(), &root).unwrap_or_default().to_string_lossy().replace('\\', "/");
                if rel.starts_with(".git/") || rel.contains("/.git/") || is_skipped_dir(&rel) { continue; }
                files.push(rel);
            }
        }
        files.sort();
        Ok(Repo { root, files, is_git, contents: BTreeMap::new() })
    }

    pub fn src_files(&self) -> Vec<&String> {
        self.files.iter().filter(|f| lang_of(f).is_some() && !is_generated(f) && !is_skipped_dir(f)).collect()
    }

    pub fn exists(&self, rel: &str) -> bool { self.root.join(rel).exists() }

    pub fn read(&self, rel: &str) -> String {
        let p = self.root.join(rel);
        match std::fs::metadata(&p) { Ok(m) if m.len() <= MAX_READ => std::fs::read_to_string(&p).unwrap_or_default(), _ => String::new() }
    }

    pub fn git(&self, args: &[&str]) -> String {
        if !self.is_git { return String::new(); }
        Command::new("git").args(args).current_dir(&self.root).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn cache(&mut self, rel: &str) -> &str {
        if !self.contents.contains_key(rel) { let t = self.read(rel); self.contents.insert(rel.to_string(), t); }
        self.contents.get(rel).map(|s| s.as_str()).unwrap_or("")
    }
}

pub fn sh(args: &[&str], cwd: Option<&Path>) -> String {
    let mut c = Command::new(args[0]);
    c.args(&args[1..]);
    if let Some(d) = cwd { c.current_dir(d); }
    c.output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
}

pub fn today() -> String { chrono::Local::now().format("%Y-%m-%d").to_string() }
