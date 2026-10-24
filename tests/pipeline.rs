//! 統合テスト:fixture(TS/Next + Python/FastAPI + Rust/axum + Go + SQL)を作って analyze → 期待するノード・辺・HTML・diff・query を確かめる。
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> PathBuf { PathBuf::from(env!("CARGO_BIN_EXE_roentgen")) }

fn write(root: &Path, rel: &str, body: &str) { let p = root.join(rel); std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, body).unwrap(); }

fn fixture(root: &Path) {
    write(root, "package.json", r#"{"name":"fx","private":true,"scripts":{"dev":"next dev","test":"vitest","lint":"eslint .","build":"next build"},"dependencies":{"next":"15.0.0","react":"19.0.0","@supabase/supabase-js":"2.0.0","zod":"3.0.0","ai":"5.0.0","stripe":"14.0.0"},"devDependencies":{"vitest":"2.0.0"}}"#);
    write(root, "tsconfig.json", r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["./src/*"]}}}"#);
    write(root, ".env.example", "SUPABASE_URL=\nSTRIPE_KEY=\n");
    write(root, "vercel.json", r#"{"crons":[{"path":"/api/cron/sweep","schedule":"0 3 * * *"}]}"#);
    write(root, "README.md", "# fx\n\n## 目的\n\nテスト用。customers 表は他システムと共有している。\n");
    write(root, ".github/workflows/ci.yml", "name: ci\non:\n  pull_request:\n  push:\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: npm test\n  deploy:\n    steps:\n      - run: vercel deploy\n");
    write(root, "src/middleware.ts", "import { NextResponse } from 'next/server';\nexport function middleware(req) { const session = req.cookies.get('session'); if (!session) return NextResponse.redirect('/login'); }\nexport const config = { matcher: ['/admin/:path*'] };\n");
    write(root, "src/app/api/customers/route.ts", "import { z } from 'zod';\nimport { listCustomers } from '@/application/customers';\nimport { getUser } from '@/adapter/auth';\nconst QuerySchema = z.object({ q: z.string() });\n/** 顧客の一覧 */\nexport async function GET(req: Request) { const u = await getUser(); if (!u) return new Response('', { status: 401 }); QuerySchema.parse({}); return Response.json(await listCustomers()); }\nexport async function POST(req: Request) { return Response.json({}, { status: 201 }); }\n");
    write(root, "src/app/api/cron/sweep/route.ts", "import { sweep } from '@/application/sweep';\nexport async function GET() { await sweep(); return Response.json({ ok: true }); }\n");
    write(root, "src/app/api/webhooks/stripe/route.ts", "import Stripe from 'stripe';\nexport async function POST(req: Request) { const stripe = new Stripe(process.env.STRIPE_KEY!); stripe.webhooks.constructEvent('', '', ''); return Response.json({}); }\n");
    write(root, "src/app/actions.ts", "'use server';\nimport { saveCustomer } from '@/adapter/supabase/customers';\nexport async function updateCustomer(id: string) { return saveCustomer(id); }\n");
    write(root, "src/application/customers.ts", "import { Customer } from '@/core/customer';\nimport { fetchCustomers } from '@/adapter/supabase/customers';\nexport async function listCustomers(): Promise<Customer[]> { return fetchCustomers(); }\n");
    write(root, "src/application/sweep.ts", "import { generateText } from 'ai';\nimport { fetchCustomers } from '@/adapter/supabase/customers';\nexport async function sweep() { const cs = await fetchCustomers(); await generateText({ model: 'anthropic/claude-sonnet-4-5', system: 'You are a sweeper.', prompt: 'go' }); }\n");
    write(root, "src/core/customer.ts", "export interface Entity { id: string }\nexport interface Customer extends Entity { name: string; plan: Plan }\nexport type Plan = 'free' | 'pro';\nexport enum Status { Active, Churned }\nexport class CustomerPolicy implements Rule { check(c: Customer) { return c.plan === 'pro'; } }\nexport interface Rule { check(c: Customer): boolean }\n");
    write(root, "src/core/bad.ts", "import { fetchCustomers } from '@/adapter/supabase/customers';\nexport const bad = fetchCustomers;\n");
    write(root, "src/adapter/supabase/customers.ts", "import { createClient } from '@supabase/supabase-js';\nimport type { Customer } from '@/core/customer';\nconst sb = createClient(process.env.SUPABASE_URL!, process.env.SUPABASE_SERVICE_ROLE_KEY!);\nexport async function fetchCustomers(): Promise<Customer[]> { const { data } = await sb.from('customers').select('*'); return data as Customer[]; }\nexport async function saveCustomer(id: string) { await fetch('https://api.stripe.com/v1/customers/' + id, { signal: AbortSignal.timeout(5000) }); return sb.from('customers').upsert({ id }); }\n");
    write(root, "src/adapter/auth.ts", "export async function getUser() { return { id: '1', role: 'staff' }; }\nexport function isAdmin(u) { return u.role === 'admin'; }\n");
    write(root, "src/ui/Button.tsx", "import React from 'react';\nexport default function Button() { return <button/>; }\n");
    write(root, "tests/customers.test.ts", "import { describe, it } from 'vitest';\nimport { listCustomers } from '../src/application/customers';\ndescribe('customers', () => { it('lists', async () => { await listCustomers(); }); });\n");
    write(root, "supabase/migrations/20240101000000_init.sql", "create table customers (\n  id uuid primary key,\n  name text not null,\n  plan text default 'free'\n);\ncreate table orders (\n  id uuid primary key,\n  customer_id uuid references customers(id),\n  amount integer not null\n);\nalter table customers enable row level security;\ncreate policy \"staff read\" on customers for select to authenticated using (true);\ncreate index orders_customer_idx on orders (customer_id);\n");
    write(root, "supabase/migrations/20240201000000_add.sql", "alter table orders add column note text;\nalter table customers add column tenant_id uuid;\n");
    write(root, "pyproject.toml", "[project]\nname = \"fxpy\"\nrequires-python = \">=3.11\"\ndependencies = [\"fastapi\", \"pydantic>=2\"]\n[tool.pytest.ini_options]\ntestpaths = [\"py/tests\"]\n");
    write(root, "py/app/main.py", "from fastapi import FastAPI, Depends\nfrom app.service import list_items\nfrom app.auth import get_current_user\napp = FastAPI()\n\n@app.get('/items')\ndef items(user=Depends(get_current_user)):\n    return list_items()\n\n@app.post('/webhooks/line')\ndef line_hook():\n    return {}\n\nif __name__ == '__main__':\n    pass\n");
    write(root, "py/app/service.py", "import os\nfrom app.models import Item\nfrom app.repo import load\n\ndef list_items() -> list[Item]:\n    \"\"\"一覧を返す\"\"\"\n    os.environ.get('ITEMS_BUCKET')\n    return load()\n");
    write(root, "py/app/models.py", "from dataclasses import dataclass\nfrom typing import Protocol\n\nclass Repo(Protocol):\n    def load(self) -> list: ...\n\n@dataclass\nclass Item:\n    id: int\n    name: str\n\nclass SpecialItem(Item):\n    pass\n");
    write(root, "py/app/repo.py", "import requests\n\ndef load():\n    requests.get('https://api.github.com/repos', timeout=3)\n    return []\n");
    write(root, "py/app/auth.py", "def get_current_user():\n    return {'role': 'admin'}\n");
    write(root, "py/tests/test_service.py", "from app.service import list_items\n\ndef test_list():\n    assert list_items() == []\n");
    write(root, "rs/Cargo.toml", "[package]\nname = \"fxrs\"\nversion = \"0.1.0\"\n[dependencies]\naxum = \"0.7\"\ntokio = { version = \"1\", features = [\"full\"] }\nsqlx = \"0.7\"\n");
    write(root, "rs/src/main.rs", "mod domain;\nmod handlers;\nuse crate::handlers::list;\n#[tokio::main]\nasync fn main() { let app = axum::Router::new().route(\"/items\", axum::routing::get(list)); }\n");
    write(root, "rs/src/domain.rs", "/// 値段を持つ\npub trait Priced { fn price(&self) -> u32; }\npub struct Item { pub id: u32, pub tag: Tag }\npub enum Tag { A, B }\nimpl Priced for Item { fn price(&self) -> u32 { 1 } }\n");
    write(root, "rs/src/handlers.rs", "use crate::domain::Item;\nuse std::env;\npub async fn list() -> String { let _ = env::var(\"DATABASE_URL\"); sqlx::query!(\"select id from items\"); String::new() }\n");
    write(root, "rs/migrations/20240301000000_items.sql", "create table items (id serial primary key, tag text);\n");
    write(root, "go/go.mod", "module example.com/fxgo\n\ngo 1.22\n");
    write(root, "go/cmd/server/main.go", "package main\n\nimport (\n\t\"net/http\"\n\t\"os\"\n\t\"example.com/fxgo/internal/store\"\n)\n\nfunc main() {\n\t_ = os.Getenv(\"PORT\")\n\thttp.HandleFunc(\"/health\", health)\n\t_ = store.New()\n}\n\nfunc health(w http.ResponseWriter, r *http.Request) {}\n");
    write(root, "go/internal/store/store.go", "package store\n\n// Store は保存先\ntype Store struct {\n\tName string\n}\n\ntype Reader interface {\n\tRead() string\n}\n\nfunc New() *Store { return &Store{} }\n");
    let git = |args: &[&str]| { Command::new("git").args(args).current_dir(root).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap(); };
    git(&["init", "-q"]); git(&["add", "."]); git(&["commit", "-q", "-m", "init"]);
}

fn analyze(root: &Path, out: &Path) { let o = Command::new(bin()).args(["analyze", root.to_str().unwrap(), "--out", out.to_str().unwrap(), "--no-gh", "--no-snapshot"]).output().unwrap(); assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr)); }
fn graph(out: &Path) -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(out.join("data/graph.json")).unwrap()).unwrap() }
fn analysis(out: &Path) -> serde_json::Value { serde_json::from_str(&std::fs::read_to_string(out.join("data/analysis.json")).unwrap()).unwrap() }
fn edges<'a>(g: &'a serde_json::Value, kind: &str) -> Vec<(&'a str, &'a str)> { g["edges"].as_array().unwrap().iter().filter(|e| e["kind"] == kind).map(|e| (e["from"].as_str().unwrap(), e["to"].as_str().unwrap())).collect() }
fn nodes<'a>(g: &'a serde_json::Value, kind: &str) -> Vec<&'a serde_json::Value> { g["nodes"].as_array().unwrap().iter().filter(|n| n["kind"] == kind).collect() }

#[test]
fn end_to_end() {
    let tmp = std::env::temp_dir().join(format!("roentgen-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("fx"); let out = tmp.join("out");
    fixture(&root);
    analyze(&root, &out);
    let g = graph(&out); let a = analysis(&out);
    // ---- 入口
    let routes: Vec<String> = nodes(&g, "route").iter().map(|n| n["name"].as_str().unwrap().to_string()).collect();
    for r in ["GET /api/customers", "POST /api/customers", "GET /api/cron/sweep", "POST /api/webhooks/stripe", "updateCustomer", "GET /items", "POST /webhooks/line", "ANY /health"] { assert!(routes.contains(&r.to_string()), "route {} が無い: {:?}", r, routes); }
    let auth = |name: &str| nodes(&g, "route").iter().find(|n| n["name"] == name).map(|n| n["attrs"]["auth"].as_str().unwrap().to_string()).unwrap();
    assert_eq!(auth("GET /api/customers"), "inline"); assert_eq!(auth("GET /api/cron/sweep"), "none"); assert_eq!(auth("POST /api/webhooks/stripe"), "inline");
    assert!(nodes(&g, "route").iter().any(|n| n["name"] == "POST /api/webhooks/stripe" && n["subkind"] == "webhook"));
    // ---- 呼び出し(4 言語)
    let calls = edges(&g, "calls");
    for (f, t) in [("fn:src/app/api/customers/route.ts#GET", "fn:src/application/customers.ts#listCustomers"), ("fn:src/application/customers.ts#listCustomers", "fn:src/adapter/supabase/customers.ts#fetchCustomers"), ("fn:py/app/main.py#items", "fn:py/app/service.py#list_items"), ("fn:py/app/service.py#list_items", "fn:py/app/repo.py#load"), ("fn:go/cmd/server/main.go#main", "fn:go/internal/store/store.go#New")] { assert!(calls.contains(&(f, t)), "call {} → {} が無い", f, t); }
    // ---- 表の読み書き
    let reads = edges(&g, "reads"); let writes = edges(&g, "writes");
    assert!(reads.contains(&("fn:src/adapter/supabase/customers.ts#fetchCustomers", "table:customers")));
    assert!(writes.contains(&("fn:src/adapter/supabase/customers.ts#saveCustomer", "table:customers")));
    assert!(reads.contains(&("fn:rs/src/handlers.rs#list", "table:items")), "sqlx::query! の読みが無い: {:?}", reads);
    // ---- 表
    let t = nodes(&g, "table"); let names: Vec<&str> = t.iter().map(|n| n["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 3); assert!(names.contains(&"customers") && names.contains(&"orders") && names.contains(&"items"));
    let cust = t.iter().find(|n| n["name"] == "customers").unwrap();
    let cols: Vec<&str> = cust["attrs"]["columns"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(cols, vec!["id", "name", "plan", "tenant_id"]); assert_eq!(cust["attrs"]["rls"], true); assert_eq!(cust["attrs"]["policies"][0]["cmd"], "SELECT");
    assert!(edges(&g, "references").contains(&("table:orders", "table:customers")));
    // ---- 型
    let ty = edges(&g, "extends"); let im = edges(&g, "implements");
    assert!(ty.contains(&("type:src/core/customer.ts#Customer", "type:src/core/customer.ts#Entity")));
    assert!(im.contains(&("type:src/core/customer.ts#CustomerPolicy", "type:src/core/customer.ts#Rule")));
    assert!(im.contains(&("type:rs/src/domain.rs#Item", "type:rs/src/domain.rs#Priced")));
    assert!(ty.contains(&("type:py/app/models.py#SpecialItem", "type:py/app/models.py#Item")));
    assert!(nodes(&g, "type").iter().any(|n| n["name"] == "Repo" && n["subkind"] == "protocol"));
    assert!(nodes(&g, "type").iter().any(|n| n["name"] == "Store" && n["doc"].as_str().unwrap().contains("保存先")));
    // ---- 外部・env・LLM・cron
    assert!(nodes(&g, "external").iter().any(|n| n["path"] == "api.stripe.com"));
    assert!(nodes(&g, "external").iter().any(|n| n["path"] == "llm" && n["attrs"]["models"][0]["model"] == "anthropic/claude-sonnet-4-5"));
    let env: Vec<&str> = nodes(&g, "env").iter().map(|n| n["name"].as_str().unwrap()).collect();
    for k in ["SUPABASE_URL", "ITEMS_BUCKET", "DATABASE_URL", "PORT"] { assert!(env.contains(&k), "env {} が無い", k); }
    assert!(nodes(&g, "env").iter().any(|n| n["name"] == "SUPABASE_URL" && n["subkind"] == "declared"));
    assert!(nodes(&g, "env").iter().any(|n| n["name"] == "PORT" && n["subkind"] == "undeclared"));
    assert!(edges(&g, "triggers").iter().any(|(f, t)| f.starts_with("job:") && t.ends_with("GET /api/cron/sweep")));
    // ---- docs
    assert!(nodes(&g, "function").iter().any(|n| n["name"] == "GET" && n["doc"] == "顧客の一覧"));
    assert!(nodes(&g, "function").iter().any(|n| n["name"] == "list_items" && n["doc"] == "一覧を返す"));
    // ---- tested
    assert!(nodes(&g, "function").iter().any(|n| n["name"] == "listCustomers" && n["attrs"]["tested"] == true));
    // ---- analysis
    let rules = a["rules"].as_array().unwrap();
    assert_eq!(rules.iter().find(|r| r["from"] == "core" && r["to"] == "adapter").unwrap()["count"], 1);
    let tr = a["traces"].as_array().unwrap().iter().find(|t| t["route"].as_str().unwrap().ends_with("GET /api/customers")).unwrap();
    let steps: Vec<&str> = tr["steps"].as_array().unwrap().iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert!(steps.contains(&"listCustomers") && steps.contains(&"fetchCustomers")); assert_eq!(tr["reads"][0], "customers");
    assert!(a["roles"].as_array().unwrap().iter().any(|r| r[0] == "staff"));
    assert!(a["tenant"].as_array().unwrap().iter().any(|r| r[0] == "tenant_id"));
    // ---- HTML
    let pages: Vec<_> = std::fs::read_dir(&out).unwrap().flatten().filter(|e| e.path().extension().map(|x| x == "html").unwrap_or(false)).collect();
    assert_eq!(pages.len(), 19);
    let idx = std::fs::read_to_string(out.join("index.html")).unwrap(); assert!(idx.contains("<svg") && !idx.contains("<script src"));
    let cg = std::fs::read_to_string(out.join("05_callgraph.html")).unwrap(); assert!(cg.contains("fetchCustomers") && cg.contains("<pre>"));
    let dm = std::fs::read_to_string(out.join("03_data.html")).unwrap(); assert!(dm.contains("saveCustomer"));
    // ---- query
    let q = Command::new(bin()).args(["query", out.to_str().unwrap(), "writes", "customers"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&q.stdout).unwrap(); assert_eq!(v["writes"][0]["name"], "saveCustomer");
    // ---- diff(変更して比較)
    let root2 = tmp.join("fx2"); fixture(&root2);
    write(&root2, "supabase/migrations/20240301000000_more.sql", "create table invoices (id uuid primary key, customer_id uuid references customers(id));\nalter table orders drop column note;\n");
    write(&root2, "src/app/api/invoices/route.ts", "export async function GET() { return Response.json([]); }\n");
    std::fs::remove_file(root2.join("src/core/bad.ts")).unwrap();
    write(&root2, "src/app/api/customers/route.ts", "import { listCustomers } from '@/application/customers';\nexport async function GET(req: Request) { return Response.json(await listCustomers()); }\n");
    let out2 = tmp.join("out2"); analyze(&root2, &out2);
    let d = Command::new(bin()).args(["diff", out.join("data").to_str().unwrap(), out2.join("data").to_str().unwrap()]).output().unwrap(); assert!(d.status.success());
    let dj: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out2.join("data/diff.json")).unwrap()).unwrap();
    assert!(dj["sections"]["tables"]["added"].as_array().unwrap().iter().any(|x| x == "invoices"));
    assert!(dj["sections"]["routes"]["added"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().starts_with("GET /api/invoices")));
    assert!(dj["sections"]["routes"]["changed"].as_array().unwrap().iter().any(|x| x[1].as_str().unwrap().contains("inline → none")), "{}", dj["sections"]["routes"]);
    assert!(dj["sections"]["architecture"]["changed"].as_array().unwrap().iter().any(|x| x[0] == "core → adapter"));
    assert!(dj["sections"]["files"]["removed"].as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("bad.ts")));
    assert!(dj["sections"]["functions"]["changed"].as_array().unwrap().iter().any(|x| x[0].as_str().unwrap().starts_with("GET (src/app/api/customers")));
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn mcp_roundtrip() {
    let tmp = std::env::temp_dir().join(format!("roentgen-mcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("fx"); let out = tmp.join("out"); fixture(&root); analyze(&root, &out);
    let mut child = Command::new(bin()).args(["mcp", out.to_str().unwrap()]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    use std::io::Write;
    let req = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"query\",\"arguments\":{\"q\":\"callers fetchCustomers\"}}}\n";
    child.stdin.take().unwrap().write_all(req.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    let lines: Vec<&str> = std::str::from_utf8(&o.stdout).unwrap().lines().collect();
    assert_eq!(lines.len(), 2);
    let r2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let text = r2["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("listCustomers") && text.contains("sweep"));
    let _ = std::fs::remove_dir_all(&tmp);
}
