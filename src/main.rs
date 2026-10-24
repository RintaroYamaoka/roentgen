mod analyze;
mod diff;
mod extract;
mod graph;
mod live;
mod mcp;
mod model;
mod notes;
mod pipeline;
mod query;
mod render;
mod repo;
mod snapshot;
mod store;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "roentgen", version, about = "repo を 1 回で読み解き、コードの知識グラフと図主体の HTML を出す")]
struct Cli { #[command(subcommand)] cmd: Cmd }

#[derive(Subcommand)]
enum Cmd {
    /// 解析して data/ と HTML を出す(既定の命令)
    Analyze {
        repo: PathBuf,
        /// 出力先(省略時 ~/.roentgen/out/<repo>/<date-sha>)
        #[arg(long)] out: Option<PathBuf>,
        /// PostgreSQL の接続文字列(SELECT 専用)。無ければ migration から
        #[arg(long)] db: Option<String>,
        /// 差分の基準:last(前回のスナップショット)か sha / ref
        #[arg(long)] since: Option<String>,
        #[arg(long)] no_gh: bool,
        #[arg(long)] no_snapshot: bool,
        #[arg(long)] no_render: bool,
    },
    /// data/ から HTML だけ作り直す(narrative.json と notes を入れた後)
    Render { out: PathBuf },
    /// 2 つの data/ を比べる
    Diff { prev: PathBuf, cur: PathBuf },
    /// グラフに質問する(例: "writes customers" / "callers listCustomers" / "route /api/x" / "search foo")
    Query { out: PathBuf, q: Vec<String> },
    /// MCP サーバー(stdio)。Claude Code などから query / trace / node を引く
    Mcp { out: PathBuf },
    /// notes(解説キャッシュ)の状態:未記入・古くなったノード
    Notes { out: PathBuf, #[arg(long)] repo: Option<PathBuf>, #[arg(long, default_value = "status")] action: String },
    /// 抽出だけ試す(デバッグ)
    Ir { repo: PathBuf, file: String },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Analyze { repo, out, db, since, no_gh, no_snapshot, no_render } => {
            let root = repo.canonicalize()?;
            let log = |s: &str| println!("{}", s);
            println!("roentgen {}  {}", env!("CARGO_PKG_VERSION"), root.display());
            let tmp_out = out.clone().unwrap_or_else(|| snapshot::home().join("tmp"));
            let opts = pipeline::Opts { db_url: db.or_else(|| std::env::var("ROENTGEN_DB_URL").ok()), use_gh: !no_gh, rules: None };
            let (meta, _g, _a) = pipeline::run(&root, &tmp_out, &opts, &log)?;
            let out_dir = match out { Some(o) => o, None => { let o = pipeline::default_out(&root, &meta); if o.exists() { std::fs::remove_dir_all(&o)?; } snapshot::copy_dir(&tmp_out.join("data"), &o.join("data"))?; let _ = std::fs::remove_dir_all(&tmp_out); o } };
            let mut base: Option<PathBuf> = None;
            if let Some(s) = since {
                if s == "last" { base = snapshot::previous(&root, &format!("{}-{}", meta.analyzed, meta.sha)).map(|p| p.join("data")); if base.is_none() { println!("  前回のスナップショットが無いので差分は出さない(今回を保存する)"); } }
                else if let Some((tmp, wt)) = snapshot::worktree_at(&root, &s) { println!("  基準 {} を一時 worktree で解析", s); let b = tmp.join("out"); let r = pipeline::run(&wt, &b, &pipeline::Opts { db_url: None, use_gh: false, rules: None }, &|_| {}); snapshot::worktree_remove(&root, &tmp, &wt); if r.is_ok() { base = Some(b.join("data")); } else { println!("  基準の解析に失敗"); } }
                else { println!("  {} を worktree に出せなかった(差分なし)", s); }
            }
            if let Some(b) = &base { let d = diff::compare(b, &out_dir.join("data")); store::write_json(&out_dir.join("data/diff.json"), &d)?; println!("  diff      changed = {}", if d.changed.is_empty() { "なし".into() } else { d.changed.join(", ") }); }
            if !no_snapshot && root.join(".git").exists() { let s = snapshot::save(&root, &out_dir.join("data"), &meta.analyzed, &meta.sha)?; println!("  snapshot  {}", s.display()); }
            if !no_render { render::build(&out_dir)?; println!("  html      {}", out_dir.join("index.html").display()); }
            println!("  次: Claude が {}/data を読んで narrative.json と notes を書き、`roentgen render {}` で解説入りにする", out_dir.display(), out_dir.display());
        }
        Cmd::Render { out } => { render::build(&out)?; println!("rendered → {}", out.join("index.html").display()); }
        Cmd::Diff { prev, cur } => { let d = diff::compare(&prev, &cur); store::write_json(&cur.join("diff.json"), &d)?; println!("changed: {:?}", d.changed); }
        Cmd::Query { out, q } => { let res = query::run(&out, &q.join(" "))?; println!("{}", serde_json::to_string_pretty(&res)?); }
        Cmd::Mcp { out } => mcp::serve(&out)?,
        Cmd::Notes { out, repo, action } => notes::cli(&out, repo.as_deref(), &action)?,
        Cmd::Ir { repo, file } => { let r = repo::Repo::open(&repo)?; let src = r.read(&file); let lang = repo::lang_of(&file).unwrap_or("other"); let ir = match lang { "ts" | "tsx" | "js" => extract::ts::extract(&file, lang, &src), "py" => extract::python::extract(&file, &src), "rs" => extract::rust::extract(&file, &src), "go" => extract::go::extract(&file, &src), _ => Default::default() }; println!("{}", serde_json::to_string_pretty(&ir)?); }
    }
    Ok(())
}
