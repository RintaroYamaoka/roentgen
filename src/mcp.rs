//! MCP サーバー(stdio・JSON-RPC 2.0)。tools: query / trace / node / search。Claude Code からコードの地図を引けるようにする。
use crate::query::{ask, load, Db};
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

pub fn serve(out: &Path) -> Result<()> {
    // 出力先が無ければ、cwd の repo の最新のスナップショット(~/.roentgen/out/<repo>/<date-sha>)を使う
    let out_path = if out.join("data/graph.json").exists() || out.join("graph.json").exists() { out.to_path_buf() } else {
        let cwd = std::env::current_dir().unwrap_or_default();
        let d = crate::snapshot::home().join("out").join(crate::snapshot::repo_key(&cwd));
        let mut names: Vec<String> = std::fs::read_dir(&d).ok().into_iter().flatten().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        match names.last() { Some(n) => d.join(n), None => out.to_path_buf() }
    };
    let db: Db = load(&out_path)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let req: Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));
        let result = match method {
            "initialize" => json!({"protocolVersion": "2024-11-05", "capabilities": {"tools": {}}, "serverInfo": {"name": "roentgen", "version": env!("CARGO_PKG_VERSION")}}),
            "notifications/initialized" | "initialized" => continue,
            "tools/list" => json!({"tools": [
                {"name": "query", "description": "コードの知識グラフに質問する。例: 'writes customers'(表を書く関数) / 'callers listCustomers' / 'calls sweep' / 'route /api/customers'(入口の trace とハンドラのコード) / 'table customers' / 'imports src/x.ts' / 'env SUPABASE_URL' / 'external stripe' / 'routes' / 'tables' / 'metrics' / 'help'", "inputSchema": {"type": "object", "properties": {"q": {"type": "string"}}, "required": ["q"]}},
                {"name": "node", "description": "関数・型・表・入口の詳細(コード抜粋・入出の辺)", "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}},
                {"name": "search", "description": "名前・doc・path を部分一致で探す", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}}
            ]}),
            "tools/call" => {
                let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let q = match name { "query" => args.get("q").and_then(|x| x.as_str()).unwrap_or("help").to_string(), "node" => format!("node {}", args.get("name").and_then(|x| x.as_str()).unwrap_or("")), "search" => format!("search {}", args.get("text").and_then(|x| x.as_str()).unwrap_or("")), _ => "help".into() };
                let res = ask(&db, &q);
                json!({"content": [{"type": "text", "text": serde_json::to_string_pretty(&res).unwrap_or_default()}]})
            }
            "ping" => json!({}),
            _ => { let resp = json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("method not found: {}", method)}}); writeln!(stdout, "{}", resp)?; stdout.flush()?; continue; }
        };
        if id.is_null() { continue; }
        writeln!(stdout, "{}", json!({"jsonrpc": "2.0", "id": id, "result": result}))?;
        stdout.flush()?;
    }
    Ok(())
}
