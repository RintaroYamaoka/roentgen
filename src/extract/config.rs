//! 設定ファイルからの事実:依存・スタック・ランタイム・ホスティング・cron・env の宣言・CI・scripts・規約。
use crate::repo::Repo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Dep { pub name: String, pub version: String, pub manifest: String, pub dev: bool }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StackItem { pub name: String, pub dep: String, pub version: String, pub category: String }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cron { pub path: String, pub schedule: String, pub source: String }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ci { pub file: String, pub name: String, pub triggers: Vec<String>, pub jobs: Vec<String>, pub deploy: bool }
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    pub deps: Vec<Dep>, pub stack: Vec<StackItem>, pub runtime: Vec<(String, String)>, pub hosting: Vec<(String, String)>, pub crons: Vec<Cron>,
    pub env_declared: Vec<String>, pub has_env_example: bool, pub ci: Vec<Ci>, pub scripts: BTreeMap<String, (String, String)>,
    pub conventions: BTreeMap<String, Vec<String>>, pub db_sources: Vec<(String, String)>, pub ts_aliases: Vec<(String, String)>, pub go_module: String, pub workspaces: Vec<String>, pub cargo_members: Vec<String>,
    pub middleware: Option<(String, Vec<String>)>,
}

const FW: &[(&str, &str, &str)] = &[
    (r"^next$", "Next.js", "framework"), (r"^nuxt$", "Nuxt", "framework"), (r"^@sveltejs/kit$", "SvelteKit", "framework"), (r"^astro$", "Astro", "framework"), (r"^@remix-run/", "Remix", "framework"),
    (r"^express$", "Express", "framework"), (r"^hono$", "Hono", "framework"), (r"^fastify$", "Fastify", "framework"), (r"^@nestjs/core$", "NestJS", "framework"), (r"^react$", "React", "ui"), (r"^vue$", "Vue", "ui"), (r"^svelte$", "Svelte", "ui"),
    (r"^fastapi$", "FastAPI", "framework"), (r"^django$", "Django", "framework"), (r"^flask$", "Flask", "framework"), (r"^axum$", "axum", "framework"), (r"^actix-web$", "actix-web", "framework"), (r"^rocket$", "Rocket", "framework"), (r"^tokio$", "tokio", "runtime"),
    (r"^github\.com/gin-gonic/gin", "Gin", "framework"), (r"^github\.com/labstack/echo", "Echo", "framework"), (r"^github\.com/go-chi/chi", "chi", "framework"),
    (r"^@supabase/", "Supabase", "db"), (r"^@prisma/client$|^prisma$", "Prisma", "db"), (r"^drizzle-orm$", "Drizzle", "db"), (r"^pg$|^postgres$|^psycopg2?(-binary)?$|^asyncpg$", "PostgreSQL", "db"), (r"^mysql2?$|^pymysql$", "MySQL", "db"),
    (r"^sqlx$", "sqlx", "db"), (r"^diesel$", "Diesel", "db"), (r"^sea-orm$", "SeaORM", "db"), (r"^sqlalchemy$", "SQLAlchemy", "db"), (r"^mongoose$|^mongodb$|^pymongo$", "MongoDB", "db"), (r"^firebase", "Firebase", "db"),
    (r"^@upstash/redis$|^ioredis$|^redis$", "Redis", "cache"), (r"^@vercel/kv$", "Vercel KV", "cache"), (r"^@vercel/blob$", "Vercel Blob", "storage"), (r"^@aws-sdk/client-s3$|^boto3$", "S3", "storage"),
    (r"^bullmq$|^bull$", "BullMQ", "queue"), (r"^@vercel/queue$", "Vercel Queues", "queue"), (r"^celery$", "Celery", "queue"), (r"^inngest$", "Inngest", "queue"), (r"^@trigger\.dev/", "Trigger.dev", "queue"), (r"^workflow$|^@workflow/", "Vercel Workflow", "queue"),
    (r"^stripe$", "Stripe", "saas"), (r"^@slack/", "Slack", "saas"), (r"^resend$", "Resend", "saas"), (r"^@sendgrid/", "SendGrid", "saas"), (r"^twilio$", "Twilio", "saas"), (r"^nodemailer$", "SMTP", "saas"), (r"^googleapis$|^google-api-python-client$", "Google APIs", "saas"), (r"^@notionhq/", "Notion", "saas"),
    (r"^openai$", "OpenAI", "ai"), (r"^@anthropic-ai/|^anthropic$", "Anthropic", "ai"), (r"^ai$", "AI SDK", "ai"), (r"^@ai-sdk/", "AI SDK provider", "ai"), (r"^@google/generative-ai$|^google-generativeai$|^@google/genai$", "Google Gemini", "ai"), (r"^langchain", "LangChain", "ai"),
    (r"^@sentry/|^sentry-sdk$", "Sentry", "monitoring"), (r"^dd-trace$|^datadog$", "Datadog", "monitoring"), (r"^@vercel/analytics$", "Vercel Analytics", "monitoring"), (r"^pino$|^winston$", "logger", "monitoring"),
    (r"^flags$|^@vercel/flags$", "Flags SDK", "flags"), (r"^launchdarkly", "LaunchDarkly", "flags"), (r"^posthog", "PostHog", "flags"), (r"^zod$", "zod", "validation"), (r"^pydantic$", "pydantic", "validation"), (r"^serde$", "serde", "validation"),
    (r"^vitest$|^jest$", "vitest/jest", "test"), (r"^pytest$", "pytest", "test"), (r"^playwright|^@playwright/", "Playwright", "test"), (r"^next-auth$|^@auth/", "NextAuth", "auth"), (r"^@clerk/", "Clerk", "auth"), (r"^jsonwebtoken$|^jose$|^PyJWT$", "JWT", "auth"),
];

pub fn collect(repo: &Repo) -> Config {
    let mut c = Config::default();
    // 依存
    for f in repo.files.iter().filter(|f| f.ends_with("package.json") && !crate::repo::is_skipped_dir(f)).take(30) {
        if let Ok(v) = serde_json::from_str::<Value>(&repo.read(f)) {
            for (sec, dev) in [("dependencies", false), ("devDependencies", true)] { if let Some(o) = v.get(sec).and_then(|x| x.as_object()) { for (k, ver) in o { c.deps.push(Dep { name: k.clone(), version: ver.as_str().unwrap_or("").into(), manifest: f.clone(), dev }); } } }
            if f == "package.json" {
                if let Some(s) = v.get("scripts").and_then(|x| x.as_object()) { for (k, val) in s { c.scripts.insert(k.clone(), (val.as_str().unwrap_or("").chars().take(160).collect(), "package.json".into())); } }
                if let Some(w) = v.get("workspaces").and_then(|x| x.as_array()) { c.workspaces = w.iter().filter_map(|x| x.as_str().map(String::from)).collect(); }
                if let Some(n) = v.get("engines").and_then(|e| e.get("node")).and_then(|x| x.as_str()) { c.runtime.push(("Node.js".into(), n.into())); } else if repo.exists(".nvmrc") { c.runtime.push(("Node.js".into(), repo.read(".nvmrc").trim().into())); } else { c.runtime.push(("Node.js".into(), String::new())); }
                for (lock, pm) in [("pnpm-lock.yaml", "pnpm"), ("yarn.lock", "yarn"), ("bun.lockb", "bun"), ("bun.lock", "bun"), ("package-lock.json", "npm")] { if repo.exists(lock) { c.runtime.push(("package manager".into(), pm.into())); break; } }
            }
        }
    }
    for f in repo.files.iter().filter(|f| f.ends_with("pyproject.toml") || f.ends_with("requirements.txt") || f.ends_with("Pipfile")).take(10) {
        let txt = repo.read(f);
        if f.ends_with(".toml") {
            if let Ok(v) = toml::from_str::<toml::Value>(&txt) {
                if let Some(arr) = v.get("project").and_then(|p| p.get("dependencies")).and_then(|d| d.as_array()) { for d in arr { if let Some(s) = d.as_str() { let name: String = s.chars().take_while(|ch| ch.is_alphanumeric() || *ch == '-' || *ch == '_' || *ch == '.').collect(); c.deps.push(Dep { name: name.to_lowercase(), version: s[name.len()..].trim().into(), manifest: f.clone(), dev: false }); } } }
                if let Some(t) = v.get("tool").and_then(|t| t.get("poetry")).and_then(|p| p.get("dependencies")).and_then(|d| d.as_table()) { for (k, ver) in t { if k != "python" { c.deps.push(Dep { name: k.to_lowercase(), version: ver.as_str().unwrap_or("").into(), manifest: f.clone(), dev: false }); } } }
                if let Some(py) = v.get("project").and_then(|p| p.get("requires-python")).and_then(|x| x.as_str()) { c.runtime.push(("Python".into(), py.into())); }
                if txt.contains("[tool.pytest") { c.scripts.entry("test".into()).or_insert(("pytest".into(), f.clone())); }
                if txt.contains("[tool.ruff") { c.scripts.entry("lint".into()).or_insert(("ruff check .".into(), f.clone())); }
            }
        } else { for line in txt.lines() { let l = line.trim(); if l.is_empty() || l.starts_with('#') || l.starts_with('-') { continue; } let name: String = l.chars().take_while(|ch| ch.is_alphanumeric() || *ch == '-' || *ch == '_' || *ch == '.').collect(); if !name.is_empty() { c.deps.push(Dep { name: name.to_lowercase(), version: l[name.len()..].trim().into(), manifest: f.clone(), dev: f.contains("dev") }); } } }
    }
    for f in repo.files.iter().filter(|f| f.ends_with("Cargo.toml")).take(20) {
        if let Ok(v) = toml::from_str::<toml::Value>(&repo.read(f)) {
            for (sec, dev) in [("dependencies", false), ("dev-dependencies", true), ("build-dependencies", true)] { if let Some(t) = v.get(sec).and_then(|d| d.as_table()) { for (k, ver) in t { let version = ver.as_str().map(String::from).or_else(|| ver.get("version").and_then(|x| x.as_str()).map(String::from)).unwrap_or_default(); c.deps.push(Dep { name: k.clone(), version, manifest: f.clone(), dev }); } } }
            if f == "Cargo.toml" { if let Some(m) = v.get("workspace").and_then(|w| w.get("members")).and_then(|x| x.as_array()) { c.cargo_members = m.iter().filter_map(|x| x.as_str().map(String::from)).collect(); } c.runtime.push(("Rust".into(), v.get("package").and_then(|p| p.get("rust-version")).and_then(|x| x.as_str()).unwrap_or("").into())); c.scripts.entry("build".into()).or_insert(("cargo build".into(), f.clone())); c.scripts.entry("test".into()).or_insert(("cargo test".into(), f.clone())); c.scripts.entry("lint".into()).or_insert(("cargo clippy".into(), f.clone())); }
        }
    }
    if let Some(gomod) = repo.files.iter().find(|f| *f == "go.mod").or_else(|| repo.files.iter().find(|f| f.ends_with("/go.mod"))).cloned() {
        let txt = repo.read(&gomod);
        for line in txt.lines() { let l = line.trim(); if let Some(m) = l.strip_prefix("module ") { c.go_module = m.trim().into(); } else if let Some(g) = l.strip_prefix("go ") { c.runtime.push(("Go".into(), g.trim().into())); } else { let p: Vec<&str> = l.split_whitespace().collect(); if p.len() >= 2 && p[1].starts_with('v') && p[0].contains('.') { c.deps.push(Dep { name: p[0].into(), version: p[1].into(), manifest: "go.mod".into(), dev: false }); } } }
        c.scripts.entry("build".into()).or_insert(("go build ./...".into(), "go.mod".into())); c.scripts.entry("test".into()).or_insert(("go test ./...".into(), "go.mod".into()));
    }
    for (f, kind) in [("Makefile", "make"), ("justfile", "just"), ("Justfile", "just"), ("Taskfile.yml", "task")] { if repo.exists(f) { let rx = regex::Regex::new(r"(?m)^([A-Za-z][\w-]*)\s*:(?:[^=]|$)").unwrap(); for m in rx.captures_iter(&repo.read(f)) { c.scripts.entry(m[1].into()).or_insert((format!("{} {}", kind, &m[1]), f.into())); } } }
    // スタック
    for d in &c.deps { for (rx, name, cat) in FW { if regex::Regex::new(rx).unwrap().is_match(&d.name) && !c.stack.iter().any(|s| s.name == *name) { c.stack.push(StackItem { name: name.to_string(), dep: d.name.clone(), version: d.version.clone(), category: cat.to_string() }); } } }
    // ホスティング
    for (f, name) in [("vercel.json", "Vercel"), ("vercel.ts", "Vercel"), (".vercel/project.json", "Vercel"), ("fly.toml", "Fly.io"), ("render.yaml", "Render"), ("netlify.toml", "Netlify"), ("serverless.yml", "Serverless"), ("app.yaml", "Google App Engine"), ("Procfile", "Heroku 系"), ("wrangler.toml", "Cloudflare Workers"), ("railway.json", "Railway"), ("supabase/config.toml", "Supabase")] { if repo.exists(f) && !c.hosting.iter().any(|h| h.0 == name) { c.hosting.push((name.into(), f.into())); } }
    let dockers: Vec<&String> = repo.files.iter().filter(|f| f.rsplit('/').next().map(|b| b.starts_with("Dockerfile") || b.starts_with("docker-compose")).unwrap_or(false)).collect();
    if !dockers.is_empty() { c.hosting.push(("Docker".into(), dockers.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(", "))); }
    if repo.files.iter().any(|f| f.starts_with("k8s/") || f.contains("/k8s/") || f.starts_with("helm/") || f.contains("kubernetes/")) { c.hosting.push(("Kubernetes".into(), "k8s/".into())); }
    if repo.files.iter().any(|f| f.ends_with(".tf")) { c.hosting.push(("Terraform".into(), "*.tf".into())); }
    // cron
    if repo.exists("vercel.json") { let raw = repo.read("vercel.json"); let v: Option<Value> = serde_json::from_str(&raw).ok().or_else(|| serde_json::from_str(&regex::Regex::new(r"(?m)^\s*//.*$").unwrap().replace_all(&raw, "")).ok()); if let Some(v) = v { if let Some(cr) = v.get("crons").and_then(|x| x.as_array()) { for x in cr { c.crons.push(Cron { path: x.get("path").and_then(|p| p.as_str()).unwrap_or("").into(), schedule: x.get("schedule").and_then(|p| p.as_str()).unwrap_or("").into(), source: "vercel.json".into() }); } } } }
    let cron_rx = regex::Regex::new(r#"cron:\s*['"]([^'"]+)['"]"#).unwrap();
    for f in repo.files.iter().filter(|f| f.starts_with(".github/workflows/")) { for m in cron_rx.captures_iter(&repo.read(f)) { c.crons.push(Cron { path: f.clone(), schedule: m[1].into(), source: "github-actions".into() }); } }
    // env の宣言
    for f in repo.files.iter().filter(|f| { let b = f.rsplit('/').next().unwrap_or(""); b == ".env.example" || b == ".env.sample" || b == ".env.template" || b == ".env.local.example" }) { c.has_env_example = true; let rx = regex::Regex::new(r"(?m)^\s*(?:export\s+)?([A-Z][A-Z0-9_]+)\s*=").unwrap(); for m in rx.captures_iter(&repo.read(f)) { c.env_declared.push(m[1].into()); } }
    c.env_declared.sort(); c.env_declared.dedup();
    // CI
    for f in repo.files.iter().filter(|f| f.starts_with(".github/workflows/") && (f.ends_with(".yml") || f.ends_with(".yaml"))) {
        let txt = repo.read(f);
        let name = regex::Regex::new(r"(?m)^name:\s*(.+)$").unwrap().captures(&txt).map(|m| m[1].trim().to_string()).unwrap_or(f.clone());
        let on_block = { let mut out = String::new(); let mut inside = false; for l in txt.lines() { if l.starts_with("on:") { inside = true; out.push_str(l.trim_start_matches("on:")); out.push('\n'); continue; } if inside { if !l.is_empty() && !l.starts_with(' ') && !l.starts_with('\t') { break; } out.push_str(l); out.push('\n'); } } out };
        let mut triggers: Vec<String> = regex::Regex::new(r"(?m)^\s*(push|pull_request|schedule|workflow_dispatch|release|workflow_call|issue_comment):").unwrap().captures_iter(&on_block).map(|m| m[1].to_string()).collect();
        if triggers.is_empty() { if let Some(m) = regex::Regex::new(r"(?m)^on:\s*\[?([^\]\n]+)").unwrap().captures(&txt) { triggers = m[1].split(',').map(|s| s.trim().to_string()).collect(); } }
        triggers.sort(); triggers.dedup();
        let jobs_part = txt.split("\njobs:").nth(1).unwrap_or("");
        let jobs: Vec<String> = regex::Regex::new(r"(?m)^  ([A-Za-z_][\w-]*):\s*$").unwrap().captures_iter(jobs_part).map(|m| m[1].to_string()).take(10).collect();
        let deploy = regex::Regex::new(r"(?i)vercel|deploy|flyctl|kubectl|helm|aws |gcloud|wrangler").unwrap().is_match(&txt);
        c.ci.push(Ci { file: f.clone(), name, triggers, jobs, deploy });
    }
    for (f, n) in [("Jenkinsfile", "Jenkins"), (".gitlab-ci.yml", "GitLab CI"), (".circleci/config.yml", "CircleCI")] { if repo.exists(f) { c.ci.push(Ci { file: f.into(), name: n.into(), triggers: vec![], jobs: vec![], deploy: true }); } }
    // 規約
    let mut conv: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in &repo.files {
        let b = f.rsplit('/').next().unwrap_or("");
        if b.eq_ignore_ascii_case("CONTRIBUTING.md") { conv.entry("contributing".into()).or_default().push(f.clone()); }
        if b == "CLAUDE.md" || b == "AGENTS.md" || b == "CLAUDE.local.md" { conv.entry("agent_docs".into()).or_default().push(f.clone()); }
        if f.starts_with(".claude/skills/") { let s = f.split('/').nth(2).unwrap_or("").to_string(); let e = conv.entry("skills".into()).or_default(); if !e.contains(&s) { e.push(s); } }
        if f.starts_with(".githooks/") || f.starts_with(".husky/") || b == ".pre-commit-config.yaml" || b == "lefthook.yml" { conv.entry("hooks".into()).or_default().push(f.clone()); }
        if b.to_lowercase().contains("pull_request_template") { conv.entry("pr_template".into()).or_default().push(f.clone()); }
        if b == "CODEOWNERS" { conv.entry("codeowners".into()).or_default().push(f.clone()); }
        if regex::Regex::new(r"^(\.eslintrc[^/]*|eslint\.config\.[mc]?js|biome\.json|\.prettierrc[^/]*|ruff\.toml|rustfmt\.toml|\.golangci\.ya?ml)$").unwrap().is_match(b) { conv.entry("lint_config".into()).or_default().push(f.clone()); }
        if regex::Regex::new(r"^(ADR|adr|decisions)").unwrap().is_match(f) || f.contains("/decisions/") || f.contains("/adr/") { if f.ends_with(".md") { conv.entry("adr".into()).or_default().push(f.clone()); } }
        // DB の置き場
        if b.ends_with(".sql") && (f.contains("/migrations/") || f.contains("/migration/")) { let d = f.rsplit_once('/').map(|x| x.0).unwrap_or("").to_string(); if !c.db_sources.iter().any(|x| x.0 == d) { c.db_sources.push((d, "sql".into())); } }
        else if b == "schema.prisma" { c.db_sources.push((f.clone(), "prisma".into())); }
        else if b == "schema.rs" && repo.read(f).contains("table!") { c.db_sources.push((f.clone(), "diesel".into())); }
        else if (f.contains("drizzle") || b == "schema.ts") && (b.ends_with(".ts") || b.ends_with(".js")) && repo.read(f).contains("pgTable") { c.db_sources.push((f.clone(), "drizzle".into())); }
    }
    c.conventions = conv;
    // tsconfig の paths
    for cfg in ["tsconfig.json", "jsconfig.json"] { if repo.exists(cfg) { let txt = regex::Regex::new(r"//[^\n]*|/\*.*?\*/").unwrap().replace_all(&repo.read(cfg), "").to_string(); let base = regex::Regex::new(r#""baseUrl"\s*:\s*"([^"]*)""#).unwrap().captures(&txt).map(|m| m[1].trim_start_matches("./").trim_end_matches('/').to_string()).unwrap_or_default(); for m in regex::Regex::new(r#""([^"]+)/\*"\s*:\s*\[\s*"([^"]+)/\*""#).unwrap().captures_iter(&txt) { let tgt = m[2].trim_start_matches("./").trim_end_matches('/'); let tgt = if base.is_empty() || base == "." { tgt.to_string() } else { format!("{}/{}", base, tgt) }; c.ts_aliases.push((format!("{}/", &m[1]), if tgt == "." || tgt.is_empty() { String::new() } else { format!("{}/", tgt) })); } } }
    if !c.ts_aliases.iter().any(|a| a.0 == "@/") { c.ts_aliases.push(("@/".into(), if repo.exists("src") { "src/".into() } else { String::new() })); }
    // middleware(認可を含むか)
    for f in ["middleware.ts", "src/middleware.ts", "proxy.ts", "src/proxy.ts", "middleware.js", "src/middleware.js"] { if repo.exists(f) { let txt = repo.read(f); let has_auth = regex::Regex::new(r"(?i)getUser|getSession|auth|session|cookies\(\)|redirect\([^)]*login|jwt|token").unwrap().is_match(&txt); let matcher: Vec<String> = regex::Regex::new(r#"(?s)matcher\s*:\s*(\[[^\]]*\]|['"][^'"]+['"])"#).unwrap().captures(&txt).map(|m| regex::Regex::new(r#"['"]([^'"]+)['"]"#).unwrap().captures_iter(&m[1]).map(|x| x[1].to_string()).collect()).unwrap_or_default(); if has_auth { c.middleware = Some((f.into(), matcher)); } break; } }
    c
}
