# roentgen

**コードを読まずに読んだのと近い状態にする。** repo を 1 回で解析して、コードの知識グラフ(ファイル・関数・型・表・入口・ジョブ・外部・env と、import / calls / reads / writes / extends / implements / handles / triggers の辺)を作り、そこから図主体の HTML 19 ページ・差分・query・MCP を出す。Rust の単一バイナリ。

```bash
roentgen analyze ~/dev/some-repo --out ./out      # 解析 + HTML
open ./out/index.html
roentgen query ./out "writes customers"          # 表を書く関数
roentgen query ./out "route /api/customers"      # 入口の trace と handler のコード
roentgen analyze ~/dev/some-repo --since last    # 前回との差分(意味の diff)
roentgen mcp ./out                               # MCP サーバー(stdio)
```

| 章 | 問い | 主役 |
|---|---|---|
| Overview | 何で、何と繋がっているか | 全体図 |
| File Tree | どこに何があるか | ツリー(役割・行数・1 行) |
| Architecture | 層・依存の向き・循環 | 層の図・規則 |
| Data Model | 表と関係、誰が読み書きするか | ER 図・表ごとの読む関数 / 書く関数 |
| Entry Points | 入口と認可 | 入口の地図・handler のコード |
| Call Graph | 入口から何が呼ばれ、どの表に書くか | 関数単位の呼び出しグラフ・コード |
| Modules | ファイルごとの公開関数 | シグネチャ・doc・呼ぶ / 呼ばれる・コード |
| Types | 型の定義と関係 | クラス図・定義本文 |
| Dependencies / Hotspots / Tech Debt / Dev Setup & CI / Integrations / AuthZ / Jobs & Queues / LLM Calls / Diff / Glossary / Unknown | | |

対応言語:TypeScript / TSX / JavaScript・Python・Rust・Go(tree-sitter)。SQL migration(sqlparser)・Prisma・Drizzle・Diesel。`--db` で PostgreSQL の実物(psql)。他の言語は行数・churn・env まで。

## 2 段の構造

1. **機械(このバイナリ)**:事実。決定的。`data/graph.json` `analysis.json` `graph.db` と HTML
2. **Claude(プラグインのスキル)**:解釈。`narrative.json`(章の要点・代表フロー・リスクの順位・用語集)と `.roentgen/notes/`(関数・表・入口ごとの解説。コードのハッシュに紐づくので、変わった所だけ書き直す)

## 配布

- Claude Code プラグイン:このリポジトリを `claude plugin add`。`bin/roentgen.sh` がバイナリを探す(`~/.roentgen/bin` → `target/release` → cargo でビルド → GitHub Releases から取得)
- バイナリだけ:GitHub Releases(Linux x86_64 / aarch64・macOS)。タグ `v*` で CI がビルドして添付
- ソースから:`cargo build --release`

テスト:`cargo test --release`(TS / Python / Rust / Go / SQL の fixture で、入口・呼び出し・読み書き・型・外部・env・cron・docs・tested・規則・trace・HTML・query・diff・MCP を確かめる)
設計:`docs/設計.md`(北極星と決定)・`docs/ワイヤー.md`・`docs/図.md`
