# Crossport

**「8080番、誰が使ってる？」をもう推測しない。** 取得できるGit/Docker/K8s情報とともに、ローカルのTCP待受ポートを表示・管理するツール。

[🇺🇸 English](./README.md)

[![CI](https://github.com/mt4110/crossport/workflows/CI/badge.svg)](https://github.com/mt4110/crossport/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)

<p align="center">
  <img src="./assets/demo.gif" width="650" alt="Crossport demo" />
</p>

## なぜ Crossport?

単なるプロセス終了ツールではなく、`crossport` は開発者のために作られました：

- **プロジェクト認識** – `node` や `python` だけでなく、どの**プロジェクト**がポートを使っているかを表示
- **Docker / K8s 統合** – コンテナ名やKubernetesサービスのターゲットを自動解決して表示
- **インタラクティブ TUI** – キーボードで操作できるリアルタイムプロセスマネージャー (`crossport ui`)
- **優しい終了戦略** – `SIGINT` → `SIGTERM` → `SIGKILL` の順に試行（Unix）
- **スマートポート検索** – 空きポートを探して `.env` ファイルを自動更新
- **Unixパイプライン** – スクリプト用にJSON出力が可能 (`crossport scan --json | jq ...`)

## インストール

### GitHub Releases (推奨)

[Releases](https://github.com/mt4110/crossport/releases) ページからお使いのOS用のバイナリをダウンロードしてください。

### ソースからビルド

```bash
git clone https://github.com/mt4110/crossport.git
cd crossport
cargo install --path .
```

### Nix を使用する場合

```bash
nix develop
cargo run -- ui
# Build without entering the shell:
nix build
./result/bin/crossport ui
# Or run directly:
nix run . -- ui
```

Rust 1.98.1と依存元を`flake.lock`で固定しています。macOS/Linux向けです。
初回はツールチェーンと依存のダウンロードが必要です。Docker/Kubernetesの識別には、
ホスト側のDocker/kubectlとアクセス権が別途必要です。

## クイックスタート

### インタラクティブモード (TUI)

推奨される使用方法です：

```bash
crossport ui
```

**操作:**

- `j` / `k` または ↑/↓ – 移動
- `Enter` – 詳細を表示
- `/` – フィルター、`s` / `S` – ソート項目・方向を変更
- `x` – プロセス全体の終了確認を表示
- 詳細・終了確認では `j` / `k`・矢印・PageUp/PageDownでスクロール
- `Esc` – 詳細を閉じる。終了確認では `y` で実行、`n` / `Esc` で取り消し
- `q` – 終了

詳細には実行ファイル、macOSのアプリバンドル、親プロセス、選択時点の起動経過時間、
作業ディレクトリ、引数、同じPIDの全TCP待受ポートを表示します。長い情報は折り返します。
終了確認はフィルターで隠れたポートも含み、プロセス全体への影響を示します。
終了直前にPID・起動時刻・実行ファイルを再確認し、不一致や起動時刻の取得不足では中止します。
ただし、この確認は終了処理全体でのPID再利用を完全に防ぐものではありません。

待受中は利用中を意味しません。最終利用日時・自動再起動の有無は監視していないため不明です。
終了操作は未保存の作業に影響し、強制終了へ進む場合があります。

### コマンドライン

#### ポートスキャン

```bash
# デフォルト範囲をスキャン (3000-9999)
crossport scan

# 特定の範囲をスキャン
crossport scan --from 8000 --to 9000

# JSON出力（スクリプト用）
crossport scan --json | jq '.[] | select(.kind == "docker")'
```

**出力例:**
```
PORT   PID      USER     CMD      KIND     PROJ
3000   39338    user     node     dev      my-frontend
5432   12456    user     postgres docker   my-db-container
8080   67890    user     kubectl  k8s      svc/backend-api
```

#### プロセス終了

```bash
# 対話的な確認付きで終了（デフォルト）
crossport kill 3000

# 確認付きで強制終了（確認設定が有効な場合）
crossport kill 3000 --force

# 何が終了されるかを確認（ドライラン）
crossport kill 3000 --dry-run
```

#### 空きポート提案

```bash
# 3000番から順に空きポートを探す
crossport suggest

# .env ファイルを自動更新
crossport suggest --env .env.local --key PORT
```

## 設定

既存の設定ファイルを次の優先順で1つ選択します（ファイル間のマージはしません）。

1. `--config PATH` で指定した既存ファイル
2. `./crossport.toml`
3. `~/.config/crossport/config.toml`

対応するCLIオプションは設定値より優先されます。`kill` の設定はCLIの終了操作用です。
TUIの終了操作は確認付きの段階的終了を使います。`ui.color` は現状、表示へ反映されません。

**`crossport.toml` の例:**

```toml
[scan]
default_range = "3000-9000"

[kill]
confirm = true
default_signal = "TERM"
```

## 機能

### プロジェクト認識

各プロセスの git ルートを自動検出し、コマンド名だけでなく **プロジェクト名** を表示します。これにより、どのアプリがどのポートを使用しているかが一目でわかります。

### Docker & Kubernetes 統合

Docker コンテナによってポートが公開されている場合、`PROJ` 列に **コンテナ名** を表示します。また、`kubectl port-forward` も検知し、ターゲットとなるサービス名を表示するため、コンテナ化されたサービスを容易に識別できます。

### JSON エクスポート

Unixパイプラインや自動化に最適です：

```bash
# 開発用ポートをJSONで取得
crossport scan --json | jq '.[] | select(.kind == "dev") | .port'
```

### 優しい終了 (Gentle Kill)

いきなり `SIGKILL` を送るのではなく、以下の順序で試行します：
1. `SIGINT` (Ctrl+C と同じ) – クリーンアップ処理を実行させる
2. `SIGTERM` (約1秒後) – 標準的な終了シグナル
3. `SIGKILL` (さらに約1秒後) – 最終手段としての強制終了

これはUnixでシグナルを明示指定しない場合の動作です。処理完了や状態保存は保証しません。
`--signal` / `kill.default_signal` には `INT`・`TERM`・`KILL` を指定でき、指定時はそのシグナルだけを送ります。

## 検証

```bash
nix develop -c cargo test --locked
nix develop -c cargo clippy --locked --all-targets -- -D warnings
```

`nix build` もテストを実行します。バックエンドE2Eはテスト自身が確保したTCPポートを使い、
CLIのスキャン、プロセス情報、終了ドライラン、ソケット解放後の検出結果を確認します。
実プロセスへの終了シグナル送信やDocker/Kubernetesの操作は、このE2Eの対象外です。

## 実装状況と制約

- 実装済み: TUIのソート・フィルター・スクロール可能な詳細・終了確認、JSON出力、設定ファイル、Watchモード。
- Docker: 公開ポートとコンテナ名の対応取得、再起動操作。
- Kubernetes: `port-forward` の引数と `k8s_` コンテナ名からの推定。一般的なPod列挙や完全なPod識別は行いません。
- Windows: スキャンは `netstat`、終了は `taskkill` を使用。Unixの段階的シグナル終了とは異なります。
- Nix: 開発環境・ビルド・実行を提供。macOSで動作確認済み、Linuxでの実行は未検証です。
- 未実装: 終了履歴・ログ保存、UID比較によるアプリ側の所有者確認、TUIのコンテナ再起動エラー表示。

`kind` は推定分類で、終了して安全かを保証しません。CLIの `--all-users` は
`system` 分類の除外を解除するもので、権限を付与するものではありません。
シグナル送信の許可はOS側でも判定されます。TUIにはCLIと同じ分類除外はありません。
上記の未実装項目は将来の検討事項で、今回の変更には含みません。

## プライバシーとセキュリティ

**Crossport は完全にローカル環境で動作します。**

- **データ収集なし**: PC内のコードや分析データを収集・保存・送信することは一切ありません。
- **ローカル処理**: すべてのポートスキャンやプロセス管理は、標準的なシステムAPI (`lsof`, `netstat`, Docker API) を使用してローカルで完結しています。
- **オープンソース**: コードは100%オープンソースです。ソースコードを確認することで、ツールが何を行っているか完全に検証可能です。

## ライセンス

MIT License. 詳細は [LICENSE](./LICENSE) をご覧ください。
