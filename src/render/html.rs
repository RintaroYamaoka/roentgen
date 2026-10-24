//! HTML の共通部:ページの殻・ナビ・4 段(要点 / 図 / 事実 / 解説)・表・コード抜粋。外部 JS なし。
use std::fmt::Write;

pub const CHAPTERS: &[(&str, &str, &str, &str)] = &[
    ("index", "index.html", "Overview", "全体"), ("01", "01_tree.html", "File Tree", "ファイルツリー"), ("02", "02_architecture.html", "Architecture", "層と依存"), ("03", "03_data.html", "Data Model", "DB と表"),
    ("04", "04_entrypoints.html", "Entry Points", "入口"), ("05", "05_callgraph.html", "Call Graph", "呼び出しの流れ"), ("06", "06_modules.html", "Modules", "モジュール"), ("07", "07_types.html", "Types", "型"),
    ("08", "08_dependencies.html", "Dependencies", "外部パッケージ"), ("09", "09_hotspots.html", "Hotspots", "変更の集中"), ("10", "10_techdebt.html", "Tech Debt", "負債とリスク"), ("11", "11_devsetup.html", "Dev Setup & CI", "開発と CI"),
    ("12", "12_integrations.html", "Integrations", "外部連携"), ("13", "13_authz.html", "AuthZ", "認証と認可"), ("14", "14_jobs.html", "Jobs & Queues", "ジョブと副作用"), ("15", "15_llm.html", "LLM Calls", "AI 呼び出し"),
    ("16", "16_diff.html", "Diff", "差分"), ("17", "17_glossary.html", "Glossary", "用語集"), ("99", "99_unknown.html", "Unknown", "未確認"),
];
pub fn file_of(k: &str) -> &'static str { CHAPTERS.iter().find(|c| c.0 == k).map(|c| c.1).unwrap_or("index.html") }
pub fn title_of(k: &str) -> (&'static str, &'static str) { CHAPTERS.iter().find(|c| c.0 == k).map(|c| (c.2, c.3)).unwrap_or(("", "")) }

pub const CSS: &str = r#"
:root{--bg:#FFFFFF;--surface:#F7F8FA;--surface2:#EEF0F4;--ink:#111827;--ink2:#374151;--muted:#6B7280;--line:#E5E7EB;--line2:#D1D5DB;--code:#F3F4F6;--edge:#9CA3AF;--heat:#2563EB;
--accent:#2563EB;--accent-soft:#DBEAFE;--warn:#B91C1C;--warn-soft:#FEE2E2;--ok:#15803D;--ok-soft:#DCFCE7;--focus:#2563EB;
--l-entry:#FDE7C7;--l-ui:#CFE8F7;--l-app:#D6E0FB;--l-core:#D4F0DC;--l-adapter:#E5DBF7;--l-other:#E7E9EE;--l-ext:#E3E5EA;--l-db:#FAEFC4;--l-ai:#FBD8E8;--l-user:#EDE9E0;
--kw:#7C3AED;--str:#15803D;--cm:#9CA3AF;--fnc:#2563EB;--radius:8px;--mono:ui-monospace,"SF Mono","JetBrains Mono",Menlo,Consolas,monospace;--sans:Inter,"Noto Sans JP","Hiragino Sans","Yu Gothic UI",system-ui,-apple-system,sans-serif}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#0B0F17;--surface:#111827;--surface2:#1F2937;--ink:#E5E7EB;--ink2:#D1D5DB;--muted:#9CA3AF;--line:#1F2937;--line2:#374151;--code:#161E2E;--edge:#6B7280;--heat:#60A5FA;
--accent:#60A5FA;--accent-soft:#1E3A5F;--warn:#F87171;--warn-soft:#3F1D1D;--ok:#4ADE80;--ok-soft:#14321F;--focus:#60A5FA;
--l-entry:#4A3A22;--l-ui:#1F3A4A;--l-app:#243352;--l-core:#1F3D2A;--l-adapter:#352B4F;--l-other:#232936;--l-ext:#262B35;--l-db:#4A4320;--l-ai:#4A2538;--l-user:#38332B;--kw:#C4B5FD;--str:#86EFAC;--cm:#6B7280;--fnc:#93C5FD}}
:root[data-theme="dark"]{--bg:#0B0F17;--surface:#111827;--surface2:#1F2937;--ink:#E5E7EB;--ink2:#D1D5DB;--muted:#9CA3AF;--line:#1F2937;--line2:#374151;--code:#161E2E;--edge:#6B7280;--heat:#60A5FA;
--accent:#60A5FA;--accent-soft:#1E3A5F;--warn:#F87171;--warn-soft:#3F1D1D;--ok:#4ADE80;--ok-soft:#14321F;--focus:#60A5FA;
--l-entry:#4A3A22;--l-ui:#1F3A4A;--l-app:#243352;--l-core:#1F3D2A;--l-adapter:#352B4F;--l-other:#232936;--l-ext:#262B35;--l-db:#4A4320;--l-ai:#4A2538;--l-user:#38332B;--kw:#C4B5FD;--str:#86EFAC;--cm:#6B7280;--fnc:#93C5FD}
*{box-sizing:border-box} html{scroll-padding-top:70px}
body{margin:0;background:var(--bg);color:var(--ink);font-family:var(--sans);font-size:14.5px;line-height:1.65;-webkit-font-smoothing:antialiased;font-feature-settings:"palt"}
a{color:var(--accent);text-decoration:none} a:hover{text-decoration:underline} a.nodelink{text-decoration:none}
code,.path{font-family:var(--mono);font-size:.85em;background:var(--code);padding:.1em .4em;border-radius:4px;word-break:break-all;color:var(--ink2)}
h1,h2,h3,h4{font-family:var(--sans);margin:0;letter-spacing:-.01em;text-wrap:balance}
h1{font-size:26px;font-weight:700;line-height:1.3} h1 .en{font-size:13px;color:var(--muted);font-family:var(--mono);font-weight:400;margin-left:10px;letter-spacing:0}
h2{font-size:18px;font-weight:650;margin:44px 0 12px;padding-bottom:8px;border-bottom:1px solid var(--line)} h3{font-size:15px;font-weight:650;margin:26px 0 8px} h4{font-size:13px;font-weight:650;margin:16px 0 6px;color:var(--muted);text-transform:uppercase;letter-spacing:.06em}
.eyebrow{font-size:11px;letter-spacing:.1em;text-transform:uppercase;color:var(--muted);font-weight:600}
.topbar{position:sticky;top:0;z-index:20;background:var(--bg);border-bottom:1px solid var(--line);height:52px;display:flex;align-items:center;gap:16px;padding:0 20px}
.topbar .brand{font-weight:700;font-size:14px;color:var(--ink)} .topbar .brand span{color:var(--muted);font-weight:400;margin-left:8px;font-family:var(--mono);font-size:12px}
.topbar .sp{flex:1} .topbar .meta{color:var(--muted);font-size:12px;font-family:var(--mono)} .topbar button{background:var(--surface);border:1px solid var(--line);border-radius:6px;padding:4px 10px;font:inherit;font-size:12px;color:var(--ink2);cursor:pointer}
.wrap{display:grid;grid-template-columns:240px minmax(0,1fr) 220px;gap:0;max-width:1480px;margin:0 auto}
@media (max-width:1100px){.wrap{grid-template-columns:220px minmax(0,1fr)}.toc{display:none}} @media (max-width:800px){.wrap{grid-template-columns:minmax(0,1fr)}.side{display:none}}
.side{position:sticky;top:52px;align-self:start;height:calc(100vh - 52px);overflow:auto;padding:20px 12px 40px 20px;border-right:1px solid var(--line);font-size:13px}
.side .group{margin:10px 0 4px;padding:0 10px} .side nav a{display:flex;align-items:baseline;gap:8px;color:var(--ink2);text-decoration:none;padding:5px 10px;border-radius:6px;line-height:1.35}
.side nav a:hover{background:var(--surface);text-decoration:none} .side nav a.cur{background:var(--accent-soft);color:var(--ink);font-weight:600}
.side nav a .n{color:var(--muted);font-size:10.5px;font-family:var(--mono);min-width:16px} .side nav a .en{color:var(--muted);font-size:11px;margin-left:auto;font-family:var(--mono)}
.side input{width:100%;border:1px solid var(--line);border-radius:6px;padding:6px 10px;font:inherit;font-size:12.5px;background:var(--surface);color:var(--ink);margin:0 0 8px}
main{padding:36px 48px 120px;min-width:0} .content{max-width:900px} .content .fig,.content table{max-width:none}
.toc{position:sticky;top:52px;align-self:start;height:calc(100vh - 52px);overflow:auto;padding:28px 20px 40px 16px;font-size:12.5px;border-left:1px solid var(--line)}
.toc .eyebrow{margin-bottom:8px} .toc a{display:block;color:var(--muted);padding:3px 0 3px 10px;border-left:2px solid transparent;line-height:1.4} .toc a.h3{padding-left:22px;font-size:12px} .toc a.on{color:var(--ink);border-left-color:var(--accent)} .toc a:hover{text-decoration:none;color:var(--ink)}
.crumb{font-size:12px;color:var(--muted);margin-bottom:10px} .crumb a{color:var(--muted)}
.lead{font-size:15.5px;color:var(--ink2);margin:12px 0 0;max-width:820px}
.summary{background:var(--surface);border:1px solid var(--line);border-radius:var(--radius);padding:14px 18px;margin:22px 0} .summary ol{margin:4px 0 0;padding-left:1.2em} .summary li{margin:3px 0} .summary.empty{color:var(--muted)}
.fig{background:var(--bg);border:1px solid var(--line);border-radius:var(--radius);padding:14px;margin:16px 0;position:relative} .fig .vp{overflow:hidden;cursor:grab;touch-action:none} .fig .vp.drag{cursor:grabbing} .fig svg{display:block;max-width:100%;height:auto;transform-origin:0 0;user-select:none}
.fig .tools{position:absolute;top:8px;right:8px;display:flex;gap:4px;opacity:.35;transition:opacity .15s} .fig:hover .tools,.fig.active .tools{opacity:1} .fig.active{border-color:var(--accent);box-shadow:0 0 0 3px var(--accent-soft)} .fig .tools button{background:var(--surface);border:1px solid var(--line);border-radius:6px;width:28px;height:26px;font:inherit;font-size:13px;color:var(--ink2);cursor:pointer;line-height:1} .fig .tools button:hover{border-color:var(--accent)} .fig:fullscreen{background:var(--bg);padding:24px;display:flex;flex-direction:column} .fig:fullscreen .vp{flex:1;display:flex;align-items:center;justify-content:center} .fig:fullscreen svg{max-height:100%} .fig .hint{position:absolute;left:12px;top:8px;font-size:11px;color:var(--muted);opacity:0;transition:opacity .15s} .fig:hover .hint{opacity:1} .fig .cap{font-size:12px;color:var(--muted);margin-top:8px}
svg .node-title{fill:var(--ink)} svg .node-sub{fill:var(--muted)} svg .col-title{fill:var(--muted);font-weight:600;font-size:11.5px} svg .note{fill:var(--muted)} svg .edge-label{fill:var(--ink)} svg .edge-label-bg{fill:var(--bg);stroke:var(--line2)} svg .legend{fill:var(--muted)} svg .stamp{fill:var(--muted)}
svg .axis{fill:var(--muted)} svg .cell{fill:var(--ink)} svg .row-title{fill:var(--ink)} svg .tm-title,svg .tm-file{fill:var(--ink)} svg .seq-label{fill:var(--ink)} svg .node:hover{filter:brightness(.95)} svg rect.node{stroke:var(--line2);stroke-width:.6}
table{border-collapse:separate;border-spacing:0;width:100%;font-size:13px;margin:10px 0 18px;border:1px solid var(--line);border-radius:var(--radius);overflow:hidden}
th,td{padding:7px 10px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top} th{background:var(--surface);font-weight:600;color:var(--muted);font-size:11.5px;letter-spacing:.04em;text-transform:uppercase;white-space:nowrap} tr:last-child td{border-bottom:none} td.num,th.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap} tbody tr:hover td{background:var(--surface)}
.badge{display:inline-block;font-size:11px;padding:1px 7px;border-radius:999px;background:var(--surface2);color:var(--ink2);margin-right:4px;white-space:nowrap;font-family:var(--mono);line-height:1.6} .badge.warn{background:var(--warn-soft);color:var(--warn)} .badge.ok{background:var(--ok-soft);color:var(--ok)} .badge.est{background:var(--accent-soft);color:var(--accent)}
.badge.l-entry{background:var(--l-entry)} .badge.l-ui{background:var(--l-ui)} .badge.l-application{background:var(--l-app)} .badge.l-core{background:var(--l-core)} .badge.l-adapter{background:var(--l-adapter)} .badge.l-other{background:var(--l-other)}
.cards{display:grid;grid-template-columns:repeat(auto-fill,minmax(240px,1fr));gap:12px;margin:16px 0} .card{display:block;background:var(--bg);border:1px solid var(--line);border-radius:var(--radius);padding:14px 16px;color:var(--ink);text-decoration:none;transition:border-color .15s} .card:hover{border-color:var(--accent);text-decoration:none} .card .t{font-weight:600;margin-bottom:4px} .card .d{font-size:12.5px;color:var(--muted);line-height:1.5} .card .n{font-family:var(--mono);font-size:11px;color:var(--muted)} .card.changed{border-left:3px solid var(--accent)}
.tiles{display:grid;grid-template-columns:repeat(auto-fill,minmax(150px,1fr));gap:10px;margin:16px 0} .tile{background:var(--surface);border:1px solid var(--line);border-radius:var(--radius);padding:10px 12px} .tile .v{font-size:20px;font-weight:650;font-variant-numeric:tabular-nums;letter-spacing:-.01em} .tile .k{font-size:11.5px;color:var(--muted)} .tile.warn .v{color:var(--warn)} .tile.ok .v{color:var(--ok)}
.note{color:var(--muted);font-size:12.5px;line-height:1.6}
.explain{background:var(--surface);border-left:3px solid var(--accent);border-radius:0 var(--radius) var(--radius) 0;padding:12px 16px;margin:16px 0} .explain ul{margin:4px 0;padding-left:1.2em}
.unknown{background:var(--surface);border:1px dashed var(--line2);border-radius:var(--radius);padding:10px 14px;margin:16px 0;font-size:13px;color:var(--muted)}
.read-order{display:flex;flex-wrap:wrap;gap:6px;align-items:center;margin:8px 0 14px} .read-order a{background:var(--surface);border:1px solid var(--line);border-radius:6px;padding:3px 10px;color:var(--ink2);font-size:12.5px} .read-order .arrow{color:var(--line2)}
details{margin:8px 0} summary{cursor:pointer;color:var(--accent);font-size:13px}
details.node{background:var(--bg);border:1px solid var(--line);border-radius:var(--radius);padding:8px 14px;margin:8px 0} details.node summary{color:var(--ink);display:flex;gap:10px;align-items:baseline;flex-wrap:wrap;font-size:14px;list-style:none} details.node summary::-webkit-details-marker{display:none} details.node summary::before{content:"▸";color:var(--muted);font-size:11px} details.node[open] summary::before{content:"▾"} details.node summary .sig{font-family:var(--mono);font-size:12px;color:var(--muted)} details.node[open]{padding-bottom:12px;border-color:var(--line2)} details.node:target{border-color:var(--accent);box-shadow:0 0 0 3px var(--accent-soft)}
pre{background:var(--code);border:1px solid var(--line);padding:10px 14px;border-radius:var(--radius);overflow-x:auto;font-size:12.5px;line-height:1.55;margin:10px 0;font-family:var(--mono)} pre code{background:none;padding:0;font-size:inherit;color:var(--ink)}
.codebox{margin:10px 0;border:1px solid var(--line);border-radius:var(--radius);overflow:hidden} .codebox .hd{display:flex;gap:10px;align-items:center;background:var(--surface);border-bottom:1px solid var(--line);padding:5px 12px;font-family:var(--mono);font-size:11.5px;color:var(--muted)} .codebox .hd b{color:var(--ink2);font-weight:500} .codebox pre{border:none;border-radius:0;margin:0;counter-reset:ln} .codebox pre code .ln{display:inline-block;width:2.6em;color:var(--cm);user-select:none;text-align:right;padding-right:1em}
.kw{color:var(--kw)} .str{color:var(--str)} .cm{color:var(--cm)} .fnc{color:var(--fnc)}
.two{display:grid;grid-template-columns:1fr 1fr;gap:16px} @media (max-width:900px){.two{grid-template-columns:1fr}}
.est{color:var(--accent)} .doc{color:var(--ink2);font-size:13px;white-space:pre-line;margin:6px 0}
dl.meta{display:grid;grid-template-columns:max-content 1fr;gap:4px 14px;margin:8px 0;font-size:12.5px} dl.meta dt{color:var(--muted);font-weight:500;white-space:nowrap} dl.meta dd{margin:0;color:var(--ink2);word-break:break-word}
.tree ul{list-style:none;padding-left:18px;margin:2px 0;border-left:1px solid var(--line)} .tree>ul{border-left:none;padding-left:0} .tree li{margin:2px 0} .tree .dir{font-weight:600;font-family:var(--mono);font-size:13px} .tree .m{color:var(--muted);font-size:12px;margin-left:6px} .tree .d{color:var(--ink2);font-size:12.5px;margin-left:8px}
.notes{border-left:3px solid var(--ok);padding:8px 14px;margin:8px 0;background:var(--surface);font-size:13px;white-space:pre-line;border-radius:0 var(--radius) var(--radius) 0} .notes.stale{border-left-color:var(--warn)} .notes .eyebrow{display:block;margin-bottom:2px}
.steps{counter-reset:s} .step{display:grid;grid-template-columns:28px 1fr;gap:8px;margin:6px 0;align-items:start} .step::before{counter-increment:s;content:counter(s);display:inline-block;width:22px;height:22px;border-radius:11px;background:var(--surface2);text-align:center;font-size:11.5px;line-height:22px;color:var(--muted)}
.chips{display:flex;flex-wrap:wrap;gap:6px;margin:4px 0} .chips a{font-family:var(--mono);font-size:12px;background:var(--surface);border:1px solid var(--line);border-radius:5px;padding:1px 7px;color:var(--ink2)}
.pager{display:flex;justify-content:space-between;margin-top:60px;padding-top:20px;border-top:1px solid var(--line);font-size:13px} .pager a{color:var(--ink2);background:var(--surface);border:1px solid var(--line);border-radius:6px;padding:8px 14px} .pager a span{display:block;color:var(--muted);font-size:11px}
.hidden{display:none !important}
"#;

pub fn esc(s: &str) -> String { let mut o = String::with_capacity(s.len()); for c in s.chars() { match c { '&' => o.push_str("&amp;"), '<' => o.push_str("&lt;"), '>' => o.push_str("&gt;"), '"' => o.push_str("&quot;"), '\'' => o.push_str("&#39;"), _ => o.push(c) } } o }

pub struct Meta<'a> { pub name: &'a str, pub sha: &'a str, pub date: &'a str, pub analyzed: &'a str, pub src_files: usize, pub src_lines: u64 }

pub fn page(key: &str, title: &str, lead: &str, body: &str, m: &Meta, changed: &[String]) -> String {
    let mut nav = String::new();
    let groups: &[(&str, &[&str])] = &[("", &["index"]), ("構造", &["01", "02", "03", "04", "05", "06", "07", "08"]), ("健康", &["09", "10", "11"]), ("境界", &["12", "13", "14", "15"]), ("変化と語彙", &["16", "17", "99"])];
    for (g, keys) in groups {
        if !g.is_empty() { let _ = write!(nav, "<div class=\"group eyebrow\">{}</div>", g); }
        for k in *keys { if let Some((_, f, en, ja)) = CHAPTERS.iter().find(|c| c.0 == *k) { let cur = if *k == key { " class=\"cur\"" } else { "" }; let mark = if changed.iter().any(|c| diff_key(k).contains(&c.as_str())) { " ●" } else { "" }; let _ = write!(nav, "<a href=\"{f}\"{cur}><span class=\"n\">{}</span>{ja}{mark}<span class=\"en\">{en}</span></a>", if *k == "index" { "" } else { k }); } }
    }
    let (en, _) = title_of(key);
    // 本文の h2 / h3 に id を振り、右の目次を作る
    let mut body2 = String::with_capacity(body.len() + 2000);
    let mut toc = String::new();
    let mut n = 0;
    let rx = regex::Regex::new(r#"<h([23])( id="([^"]*)")?>(.*?)</h[23]>"#).unwrap();
    let mut last = 0;
    for cap in rx.captures_iter(body) {
        let mm = cap.get(0).unwrap();
        body2.push_str(&body[last..mm.start()]);
        let lvl = &cap[1]; let text = strip_tags(&cap[4]);
        let id = match cap.get(3) { Some(x) if !x.as_str().is_empty() => x.as_str().to_string(), _ => { n += 1; format!("s-{}", n) } };
        let _ = write!(body2, "<h{lvl} id=\"{}\">{}</h{lvl}>", esc(&id), &cap[4]);
        if toc.matches("<a ").count() < 60 { let _ = write!(toc, "<a href=\"#{}\" class=\"{}\">{}</a>", esc(&id), if lvl == "3" { "h3" } else { "" }, esc(&text.chars().take(48).collect::<String>())); }
        last = mm.end();
    }
    body2.push_str(&body[last..]);
    let idx = CHAPTERS.iter().position(|c| c.0 == key).unwrap_or(0);
    let prev = if idx > 0 { Some(CHAPTERS[idx - 1]) } else { None }; let next = CHAPTERS.get(idx + 1).copied();
    let pager = format!("<div class=\"pager\">{}{}</div>", prev.map(|p| format!("<a href=\"{}\"><span>← 前</span>{} {}</a>", p.1, p.3, p.2)).unwrap_or("<span></span>".into()), next.map(|p| format!("<a href=\"{}\" style=\"text-align:right\"><span>次 →</span>{} {}</a>", p.1, p.3, p.2)).unwrap_or_default());
    format!("<!doctype html><html lang=\"ja\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{t} — {n_} · roentgen</title><style>{css}</style></head><body>\
<div class=\"topbar\"><div class=\"brand\">{n_}<span>roentgen</span></div><div class=\"sp\"></div><div class=\"meta\">{sha} · {date} · 解析 {an}</div><button id=\"theme\" type=\"button\" title=\"ダーク / ライト\">◐</button></div>\
<div class=\"wrap\"><aside class=\"side\"><input id=\"filter\" type=\"search\" placeholder=\"このページの項目を絞る…\" autocomplete=\"off\"><nav>{nav}</nav></aside>\
<main><div class=\"content\"><div class=\"crumb\"><a href=\"index.html\">{n_}</a> / {k} {en}</div><h1>{t}<span class=\"en\">{en}</span></h1>{lead}\n{body}\n{pager}</div></main>\
<aside class=\"toc\"><div class=\"eyebrow\">On this page</div>{toc}</aside></div>\
<script>{js}</script></body></html>",
        t = esc(title), n_ = esc(m.name), css = CSS, sha = esc(m.sha), date = esc(m.date), an = esc(m.analyzed), k = if key == "index" { "" } else { key }, en = en, nav = nav, body = body2, toc = toc, pager = pager, js = JS,
        lead = if lead.is_empty() { String::new() } else { format!("<p class=\"lead\">{}</p>", esc(lead)) })
}

fn strip_tags(s: &str) -> String { let rx = regex::Regex::new(r"<[^>]+>").unwrap(); rx.replace_all(s, "").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").trim().to_string() }

/// 小さい JS:テーマ切替・hash で details を開く・目次のスクロール追従・項目の絞り込み。外部依存なし。
pub const JS: &str = r#"(function(){var d=document,r=d.documentElement;try{var t=localStorage.getItem('roentgen-theme');if(t)r.setAttribute('data-theme',t);}catch(e){}
var b=d.getElementById('theme');if(b)b.onclick=function(){var cur=r.getAttribute('data-theme');var dark=cur?cur==='dark':matchMedia('(prefers-color-scheme: dark)').matches;var n=dark?'light':'dark';r.setAttribute('data-theme',n);try{localStorage.setItem('roentgen-theme',n);}catch(e){}};
function openHash(){var h=location.hash;if(!h)return;var el=d.getElementById(decodeURIComponent(h.slice(1)));if(!el)return;var p=el;while(p){if(p.tagName==='DETAILS')p.open=true;p=p.parentElement;}el.scrollIntoView({block:'start'});}
openHash();addEventListener('hashchange',openHash);
var links=[].slice.call(d.querySelectorAll('.toc a'));var hs=links.map(function(a){return d.getElementById(a.getAttribute('href').slice(1));});
function spy(){var y=scrollY+80,i=0;for(var j=0;j<hs.length;j++){if(hs[j]&&hs[j].offsetTop<=y)i=j;}links.forEach(function(a,j){a.classList.toggle('on',j===i);});}
if(links.length){addEventListener('scroll',spy,{passive:true});spy();}
var f=d.getElementById('filter');if(f){var items=[].slice.call(d.querySelectorAll('details.node, table tbody tr'));f.oninput=function(){var q=f.value.trim().toLowerCase();items.forEach(function(el){var tx=(el.tagName==='TR'?el.textContent:(el.querySelector('summary')||el).textContent).toLowerCase();el.classList.toggle('hidden',q&&tx.indexOf(q)<0);});};}
[].slice.call(d.querySelectorAll('.fig')).forEach(function(fig){var vp=fig.querySelector('.vp'),svg=vp&&vp.querySelector('svg');if(!svg)return;var sc=1,tx=0,ty=0,drag=null,active=false;
function size(){var r=vp.getBoundingClientRect();var w=svg.getBoundingClientRect().width/sc,h=svg.getBoundingClientRect().height/sc;return {W:r.width,H:r.height,w:w,h:h,r:r};}
function clamp(){var z=size(),sw=z.w*sc,sh=z.h*sc;if(sw<=z.W){tx=Math.max(0,Math.min(z.W-sw,tx));}else{tx=Math.min(0,Math.max(z.W-sw,tx));}if(sh<=z.H){ty=Math.max(0,Math.min(z.H-sh,ty));}else{ty=Math.min(0,Math.max(z.H-sh,ty));}}
function apply(){clamp();svg.style.transform='translate('+tx+'px,'+ty+'px) scale('+sc+')';vp.style.height=sc>1?(size().h)+'px':'';}
function zoomAt(f,cx,cy){var r=vp.getBoundingClientRect();var x=cx-r.left,y=cy-r.top;var ns=Math.min(8,Math.max(.5,sc*f));tx=x-(x-tx)*(ns/sc);ty=y-(y-ty)*(ns/sc);sc=ns;apply();}
function setActive(v){active=v;fig.classList.toggle('active',v);}
vp.addEventListener('wheel',function(e){if(!active&&!e.ctrlKey&&!e.metaKey)return;e.preventDefault();zoomAt(e.deltaY<0?1.12:1/1.12,e.clientX,e.clientY);},{passive:false});
vp.addEventListener('pointerdown',function(e){if(e.button!==0)return;setActive(true);drag={x:e.clientX-tx,y:e.clientY-ty,moved:false,t:e.target};vp.setPointerCapture(e.pointerId);vp.classList.add('drag');});
vp.addEventListener('pointermove',function(e){if(!drag)return;var nx=e.clientX-drag.x,ny=e.clientY-drag.y;if(Math.abs(e.movementX)+Math.abs(e.movementY)>0&&(Math.abs(nx-tx)+Math.abs(ny-ty))>2)drag.moved=true;tx=nx;ty=ny;apply();});
vp.addEventListener('pointerup',function(){drag=null;vp.classList.remove('drag');});
vp.addEventListener('click',function(e){if(drag===null&&fig.dataset.moved==='1'){e.preventDefault();e.stopPropagation();}},true);
vp.addEventListener('dblclick',function(){sc=1;tx=0;ty=0;apply();});
d.addEventListener('pointerdown',function(e){if(!fig.contains(e.target))setActive(false);});
d.addEventListener('keydown',function(e){if(e.key==='Escape')setActive(false);});
fig.querySelectorAll('.tools button').forEach(function(b){b.onclick=function(){var r=vp.getBoundingClientRect();var k=b.getAttribute('data-z');setActive(true);if(k==='in')zoomAt(1.25,r.left+r.width/2,r.top+r.height/2);else if(k==='out')zoomAt(1/1.25,r.left+r.width/2,r.top+r.height/2);else if(k==='reset'){sc=1;tx=0;ty=0;apply();}else if(k==='full'){if(d.fullscreenElement)d.exitFullscreen();else fig.requestFullscreen&&fig.requestFullscreen();}};});
d.addEventListener('fullscreenchange',function(){sc=1;tx=0;ty=0;apply();});});
})();"#;

fn diff_key(k: &str) -> Vec<&'static str> { match k { "03" => vec!["tables", "table_access"], "04" => vec!["routes"], "05" => vec!["traces", "functions"], "06" => vec!["functions", "files"], "07" => vec!["types"], "02" => vec!["architecture"], "08" => vec!["packages"], "12" => vec!["externals"], "14" => vec!["jobs"], "11" => vec!["env"], _ => vec![] } }

pub fn fmt_n(n: u64) -> String { let s = n.to_string(); let mut o = String::new(); for (i, c) in s.chars().enumerate() { if i > 0 && (s.len() - i) % 3 == 0 { o.push(','); } o.push(c); } o }

pub fn summary(lines: Option<&Vec<String>>) -> String {
    match lines { Some(v) if !v.is_empty() => format!("<div class=\"summary\"><div class=\"eyebrow\">要点</div><ol>{}</ol></div>", v.iter().take(5).map(|l| format!("<li>{}</li>", esc(l))).collect::<String>()),
        _ => "<div class=\"summary empty\"><div class=\"eyebrow\">要点(LLM の段・未記入)</div><div>機械の段だけ回した状態。Claude が data/ を読んで narrative.json を書くとここに要点が入る。下の図と表は事実。</div></div>".into() }
}
pub fn explain(notes: Option<&Vec<String>>, title: &str) -> String { match notes { Some(v) if !v.is_empty() => format!("<div class=\"explain\"><div class=\"eyebrow\">{}</div><ul>{}</ul></div>", esc(title), v.iter().map(|n| format!("<li>{}</li>", esc(n))).collect::<String>()), _ => String::new() } }
pub fn fig(svg: &str, caption: &str, anchor: &str) -> String { if svg.is_empty() { return String::new(); } format!("<div class=\"fig\"{}><div class=\"tools\"><button type=\"button\" data-z=\"in\" title=\"拡大\">+</button><button type=\"button\" data-z=\"out\" title=\"縮小\">−</button><button type=\"button\" data-z=\"reset\" title=\"元に戻す\">⟲</button><button type=\"button\" data-z=\"full\" title=\"全画面\">⛶</button></div><div class=\"vp\">{}</div><div class=\"hint\">クリックで操作開始 → ホイールで拡大縮小 · ドラッグで移動 · ダブルクリックで戻す · Esc で解除</div>{}</div>", if anchor.is_empty() { String::new() } else { format!(" id=\"{}\"", esc(anchor)) }, svg, if caption.is_empty() { String::new() } else { format!("<div class=\"cap\">{}</div>", esc(caption)) }) }
pub fn h2(t: &str, anchor: &str) -> String { if anchor.is_empty() { format!("<h2>{}</h2>", esc(t)) } else { format!("<h2 id=\"{}\">{}</h2>", esc(anchor), esc(t)) } }
pub fn h2_raw(html: &str, anchor: &str) -> String { format!("<h2 id=\"{}\">{}</h2>", esc(anchor), html) }
pub fn h3(t: &str) -> String { format!("<h3>{}</h3>", esc(t)) }
pub fn badge(t: &str, cls: &str) -> String { format!("<span class=\"badge {}\">{}</span>", cls, esc(t)) }
pub fn role_badge(role: &str) -> String { badge(role, &format!("l-{}", role)) }
pub fn path(p: &str) -> String { format!("<code class=\"path\">{}</code>", esc(p)) }
pub fn details(summary: &str, inner: &str) -> String { format!("<details><summary>{}</summary>{}</details>", esc(summary), inner) }
pub fn note(t: &str) -> String { format!("<p class=\"note\">{}</p>", esc(t)) }
pub fn tiles(items: &[(&str, String, &str)]) -> String { format!("<div class=\"tiles\">{}</div>", items.iter().map(|(k, v, c)| format!("<div class=\"tile {}\"><div class=\"v\">{}</div><div class=\"k\">{}</div></div>", c, esc(v), esc(k))).collect::<String>()) }

/// 表。cells は既に HTML(esc 済みか path()/badge() の出力)。
pub fn table(headers: &[&str], rows: &[Vec<String>], num_cols: &[usize], ids: &[String], empty: &str) -> String {
    if rows.is_empty() { return note(empty); }
    let th: String = headers.iter().enumerate().map(|(i, h)| format!("<th class=\"{}\">{}</th>", if num_cols.contains(&i) { "num" } else { "" }, esc(h))).collect();
    let trs: String = rows.iter().enumerate().map(|(ri, r)| { let id = ids.get(ri).filter(|s| !s.is_empty()).map(|s| format!(" id=\"{}\"", esc(s))).unwrap_or_default(); format!("<tr{}>{}</tr>", id, r.iter().enumerate().map(|(i, c)| format!("<td class=\"{}\">{}</td>", if num_cols.contains(&i) { "num" } else { "" }, c)).collect::<String>()) }).collect();
    format!("<table><thead><tr>{}</tr></thead><tbody>{}</tbody></table>", th, trs)
}

/// コード抜粋(軽い色付け:キーワード・文字列・コメント)。
pub fn code(src: &str, lang: &str) -> String { code_at(src, lang, "", 0) }

/// ファイル名と開始行つきのコード箱。
pub fn code_at(src: &str, lang: &str, path: &str, start_line: u32) -> String {
    let kw: &[&str] = match lang { "py" => &["def", "class", "return", "if", "else", "elif", "for", "while", "import", "from", "async", "await", "with", "try", "except", "raise", "yield", "lambda", "None", "True", "False", "in", "not", "and", "or", "pass"],
        "rs" => &["fn", "pub", "struct", "enum", "impl", "trait", "let", "mut", "return", "if", "else", "match", "for", "while", "loop", "use", "mod", "async", "await", "where", "self", "Self", "type", "const", "static", "move", "ref", "as", "in"],
        "go" => &["func", "return", "if", "else", "for", "range", "var", "const", "type", "struct", "interface", "package", "import", "go", "defer", "select", "switch", "case", "map", "chan", "nil", "err"],
        _ => &["function", "const", "let", "var", "return", "if", "else", "for", "while", "import", "export", "from", "async", "await", "class", "interface", "type", "enum", "new", "try", "catch", "throw", "default", "extends", "implements", "switch", "case", "typeof", "null", "undefined", "true", "false", "this", "of", "in"] };
    let mut out = String::new();
    for line in src.lines() {
        let e = esc(line);
        let (code_part, comment) = { let t = line.trim_start(); if t.starts_with("//") || t.starts_with("#") && lang == "py" || t.starts_with("*") || t.starts_with("/*") { (String::new(), e.clone()) } else if let Some(i) = line.find("// ") { (esc(&line[..i]), esc(&line[i..])) } else { (e.clone(), String::new()) } };
        let mut colored = String::new();
        let mut word = String::new();
        let mut in_str: Option<char> = None;
        let mut chars = code_part.chars().peekable();
        while let Some(c) = chars.next() {
            if let Some(q) = in_str { colored.push(c); if c == q { in_str = None; colored.push_str("</span>"); } continue; }
            if c == '"' || c == '\'' || c == '`' { flush(&mut word, &mut colored, kw); colored.push_str("<span class=\"str\">"); colored.push(c); in_str = Some(c); continue; }
            if c.is_alphanumeric() || c == '_' || c == '$' { word.push(c); } else { flush(&mut word, &mut colored, kw); if c == '(' { if let Some(i) = colored.rfind("</span>") { let _ = i; } } colored.push(c); }
        }
        flush(&mut word, &mut colored, kw);
        if in_str.is_some() { colored.push_str("</span>"); }
        out.push_str(&colored);
        if !comment.is_empty() { out.push_str("<span class=\"cm\">"); out.push_str(&comment); out.push_str("</span>"); }
        out.push('\n');
    }
    let body = out.trim_end_matches('\n');
    if path.is_empty() && start_line == 0 { return format!("<pre><code>{}</code></pre>", body); }
    let numbered: String = body.lines().enumerate().map(|(i, l)| if l.starts_with("… (") { format!("<span class=\"ln\"></span>{}", l) } else { format!("<span class=\"ln\">{}</span>{}", start_line as usize + i, l) }).collect::<Vec<_>>().join("\n");
    format!("<div class=\"codebox\"><div class=\"hd\"><b>{}</b>{}</div><pre><code>{}</code></pre></div>", esc(path), if start_line > 0 { format!("<span>L{}</span>", start_line) } else { String::new() }, numbered)
}
fn flush(word: &mut String, out: &mut String, kw: &[&str]) { if word.is_empty() { return; } if kw.contains(&word.as_str()) { out.push_str(&format!("<span class=\"kw\">{}</span>", word)); } else { out.push_str(word); } word.clear(); }

/// 関数・型・入口 1 件の折りたたみ(API リファレンス風)。meta は (label, html) の定義リスト。
pub fn node_block(anchor: &str, kind_badge: &str, name: &str, sig: &str, doc: &str, notes_html: &str, excerpt_html: &str, meta: &[(String, String)], open: bool) -> String {
    let dl = if meta.is_empty() { String::new() } else { format!("<dl class=\"meta\">{}</dl>", meta.iter().map(|(k, v)| format!("<dt>{}</dt><dd>{}</dd>", esc(k), v)).collect::<String>()) };
    format!("<details class=\"node\" id=\"{id}\"{op}><summary>{kb}<b>{n}</b><span class=\"sig\">{s}</span></summary>{doc}{notes}{dl}{ex}</details>",
        id = esc(anchor), op = if open { " open" } else { "" }, kb = kind_badge, n = esc(name), s = esc(sig),
        doc = if doc.is_empty() { String::new() } else { format!("<div class=\"doc\">{}</div>", esc(doc)) }, notes = notes_html, ex = excerpt_html)
}
