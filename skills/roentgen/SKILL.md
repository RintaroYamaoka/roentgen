---
name: roentgen
description: repo のコードを 1 回で読み解き、知識グラフ(関数・型・表・入口・呼び出し・読み書き・外部・env)と図主体の HTML 19 ページを出す。「このプロジェクトを把握したい」「構成を図にして」「roentgen」「レントゲン」「コードを読まずに理解したい」「どの関数がこの表を書いている」「この API を叩くと何が起きる」「久々に見るので何が変わったか」で使う。
---

# roentgen ― コードを読まずに読んだのと近い状態にする

北極星:**コードを読まずに読んだのと近い状態**。読み手はエンジニア。章名は開発用語。
事実(機械・決定的)と解釈(Claude)を分ける。機械の段はこのプラグインの Rust バイナリ(`bin/roentgen.sh`)。解釈の段は **このスキルを読んだ Claude が `narrative.json`(章の要点)と `notes`(関数・表・入口ごとの解説)を書く**。どちらも無くても全ページは出る。

## 手順

1. **解析と仮の描画**(87 万行で 7 秒)
   ```bash
   ROOT="$(cd "$(dirname "$(readlink -f ~/.claude/skills/roentgen/SKILL.md)")/../.." && pwd)"   # plugin root
   bash "$ROOT/bin/roentgen.sh" analyze <repo> --out <出力先>
   ```
   - 出力先を省くと `~/.roentgen/out/<repo>/<date-sha>/`。`<出力先>/data/` に graph.json・analysis.json・graph.db(SQLite)が出る
   - `--db postgres://…`:実物の DB(SELECT 専用)。無ければ migration から。両方あれば drift を Unknown に出す
   - `--since last`(前回のスナップショットと比較)/ `--since <sha>`(その sha を worktree で解析して比較)。差分は Diff の章
   - `--no-gh`(PR を取らない)/ `--no-snapshot`
2. **グラフに聞く。** コードを開く前に、query で確かめる(MCP でも同じ):
   ```bash
   bash "$ROOT/bin/roentgen.sh" query <出力先> routes            # 入口の一覧と認可
   bash "$ROOT/bin/roentgen.sh" query <出力先> route /api/x       # 入口の trace(関数 → 表 → 外部)と handler のコード
   bash "$ROOT/bin/roentgen.sh" query <出力先> writes customers   # 表を書く関数
   bash "$ROOT/bin/roentgen.sh" query <出力先> callers fetchX     # 呼び元(入口まで)
   bash "$ROOT/bin/roentgen.sh" query <出力先> node fetchX        # コード抜粋・入出の辺
   bash "$ROOT/bin/roentgen.sh" query <出力先> search 提案        # 名前・doc・path の部分一致
   bash "$ROOT/bin/roentgen.sh" query <出力先> help
   ```
3. **`<出力先>/data/narrative.json` を書く**(章の要点。型は下)。守ること:
   - 事実は data の数字・名前・path だけ。推定は「推定」と書く。人名は書かない(ハンドルのまま)。鍵の値は書かない
   - 各章 summary は 3 行。読み手は初見のエンジニア。業務の言葉とコードの名前を両方出す
   - `05.flows` は 3〜5 本。`analysis.json` の traces(入口 → 関数 → 表 → 外部)を材料に、入口の route id を `route` に入れる
   - `10.risks` は順位つき。根拠は Hotspots(巨大 × churn)・AuthZ(認可なし)・Data Model(RLS 無し)・Architecture(禁止辺・循環)。`link` は章の HTML#anchor
   - `17.terms` は出所(path)が無い語を載せない
4. **notes を書く**(関数・表・入口の解説。コードのハッシュに紐づくキャッシュ):
   ```bash
   bash "$ROOT/bin/roentgen.sh" notes <出力先> --repo <repo>     # 未記入・古いノードを重要度順に(入口 → 表 → 呼ばれる関数 → 型)
   ```
   出た `todo` の上位から、`<repo>/.roentgen/notes/<anchor>.md` を書く。形:
   ```
   ---
   id: fn:src/x.ts#foo
   hash: <todo に出た hash>
   name: foo
   path: src/x.ts
   ---
   何をする関数か 1〜3 行。なぜそうなっているか(分かれば)。触るときの注意。
   ```
   `anchor` は id から `query node foo` の結果か、`notes` の todo に出る id を使う(ファイル名は `roentgen notes` が出す `id` を渡して書く。無ければ id の英数字以外を - にして末尾に blake3 の先頭 8 桁)。書き方が分からなければ 1 件目は `roentgen notes <出力先> --repo <repo> --action status` の todo の `id` と `hash` をそのまま frontmatter に写す。
   notes は repo に commit してよい(チームの資産)。次回はコードが変わったノードだけ `stale` になる。
5. **描画し直す**
   ```bash
   bash "$ROOT/bin/roentgen.sh" render <出力先>
   ```
6. 本人には `index.html` の絶対 path、要点 3 行、未確認の件数、差分があれば変わった章を短く伝える。

## narrative.json の型

```json
{
  "index": {"lead": "1 行の説明", "summary": ["…", "…", "…"], "users": [{"name": "staff", "what": "何をする人か"}]},
  "01": {"summary": ["…"], "notes": ["…"], "dirs": {"src/adapter/": "1 行の説明"}},
  "02": {"summary": ["…"], "notes": ["…"]}, "03": {"summary": ["…"], "notes": ["中心の表・状態を持つ表(推定)"]}, "04": {"summary": ["…"], "notes": ["…"]},
  "05": {"summary": ["…"], "notes": ["…"], "flows": [{"name": "流れの名前", "reason": "選んだ理由", "route": "route:src/app/api/x/route.ts#POST /api/x", "caption": "図の下の 1 行",
          "steps": [{"from": "利用者", "to": "src/app/", "label": "POST /api/x"}, {"from": "src/app/", "to": "src/adapter/", "label": "fetchX()"}, {"from": "src/adapter/", "to": "DB", "label": "customers"}], "notes": ["この流れの読み方"]}]},
  "06": {"summary": ["…"]}, "07": {"summary": ["…"]}, "08": {"summary": ["…"]}, "09": {"summary": ["…"], "notes": ["今 熱い場所・誰に聞くか(ハンドル)"]},
  "10": {"summary": ["…"], "risks": [{"rank": 1, "kind": "巨大 × 変更", "path": "src/x.ts", "why": "なぜ危ないか 1 文", "link": "09_hotspots.html#churn", "evidence": "Hotspots"}]},
  "11": {"summary": ["…"]}, "12": {"summary": ["…"]}, "13": {"summary": ["…"], "model": {"roles": [{"name": "admin", "can": {"顧客": "全部", "設定": "書く"}}]}}, "14": {"summary": ["…"]}, "15": {"summary": ["…"]},
  "16": {"summary": ["差分を 5 行で"]}, "17": {"summary": ["…"], "terms": [{"ja": "提案", "code": ["ad_change_proposals", "Proposal"], "source": "src/core/x.ts", "note": "意味"}]}
}
```
`from` / `to` は層の path(`src/app/`)か `利用者` `cron` `DB` `外部 <名前>` `LLM`。

## 出力の構成

```
<出力先>/
  index.html                      Overview:全体図・読む順・数字・章
  01_tree … 17_glossary.html      File Tree / Architecture / Data Model / Entry Points / Call Graph / Modules / Types / Dependencies / Hotspots / Tech Debt / Dev Setup & CI / Integrations / AuthZ / Jobs & Queues / LLM Calls / Diff / Glossary
  99_unknown.html                 読めなかったもの
  data/graph.json analysis.json   事実の正本(知識グラフ)。graph.db は SQLite
  data/narrative.json             Claude の章の解説
<repo>/.roentgen/notes/*.md        Claude のノード単位の解説(コードのハッシュつき)
```

## MCP

このプラグインは MCP サーバー `roentgen` を登録する(`bin/roentgen.sh mcp <出力先>`)。tools は `query` / `node` / `search`。コーディングやレビューの最中に「この表を書く関数は」「この入口は何を呼ぶ」をグラフから引く。出力先は環境変数 `ROENTGEN_OUT`(既定 `.roentgen/out`)。

## 規則

- 自己完結 HTML(外部 JS 無し・インライン SVG・ダーク対応)。社外 repo でもそのまま渡せる
- レイアウトは決定的。同じ構造なら同じ位置に同じ箱。2 回出した図を並べれば変化が見える
- 依存の規則は repo の `.roentgen.yml`(`rules: [{from, to, kind}]`)。無ければ推定し「推定」と出す。Architecture に雛形
- `.env` の値は読まない(キー名だけ)。`--db` の接続文字列は出力に出さない
- 解析される repo には `.roentgen/notes/` 以外何も置かない
