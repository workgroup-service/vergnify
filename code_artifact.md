# ソフトウェアバージョン・脆弱性一括管理ツール 設計仕様書

JetBrains Toolboxライクな直感的UIを備え、ローカル環境のランタイム/CLIツールのバージョン追跡、アップデート、脆弱性検知、将来的にはAdmin一括管理を実現するデスクトップ & CLIアプリケーションの設計書です。

---

## 1. プロジェクト概要 & ビジョン

### 1.1 プロジェクト名（仮）
**`VerCheck` / `StackBox` (Rust + GPUI)**

### 1.2 目的
- 開発者PC内の言語ランタイム（Python, Node.js, PHP, Go, Rust等）やCLIツールのバージョンを一目で可視化。
- 最新安定バージョンとの比較、ワンクリックでのアップデート支援。
- 検出されたバージョンにおける既知の脆弱性（CVE）警告と通知。
- **CLI** によるターミナル完結操作およびCI/自動化スクリプト対応。
- 将来的な **中央管理サーバー（Admin Dashboard）** へのテレメトリ送信とフリート管理。

---

## 2. システムアーキテクチャ & 技術選定

### 2.1 アーキテクチャ構成図

```text
+-----------------------------------------------------------------------------+
|                                  ユーザー層                                  |
|     +----------------------------+         +--------------------------+     |
|     |  GPUI Native GUI App       |         |  CLI Application         |     |
|     |  (JetBrains Toolbox 風)    |         |  (clap-rs ベース)         |     |
|     +--------------+-------------+         +------------+-------------+     |
+--------------------|------------------------------------|-------------------+
                     |                                    |
                     +-----------------+------------------+
                                       |
+--------------------------------------v--------------------------------------+
|                     Core Engine (Rust Workspace / Crates)                  |
|                                                                             |
|  +---------------------+  +----------------------+  +--------------------+  |
|  |  Scanner Engine     |  |  Registry Client     |  |  Security / CVE    |  |
|  |  - Binary Execution |  |  - GitHub Releases    |  |  - OSV.dev API     |  |
|  |  - Path / Env Scan  |  |  - Version Parser   |  |  - NVD Database    |  |
|  +---------------------+  +----------------------+  +--------------------+  |
|                                                                             |
|  +---------------------+  +----------------------+  +--------------------+  |
|  |  Updater Module     |  |  Notification System |  |  Agent / Sync Client|  |
|  |  - Brew / Winget    |  |  - Native OS Alerts  |  |  - gRPC / HTTPS    |  |
|  |  - Direct Install   |  |  - In-app Badges     |  |  - Remote Admin API |  |
|  +---------------------+  +----------------------+  +--------------------+  |
+-----------------------------------------------------------------------------+
```

### 2.2 技術スタック

| 分類 | 選定技術 / ライブラリ | 選定理由・特徴 |
| :--- | :--- | :--- |
| **言語** | **Rust** (2021 Edition) | 高速、安全性、単一バイナリ配布、低リソース消費 |
| **GUI フレームワーク** | **GPUI** (Zedエディタ開発のUI基盤) | GPUアクセラレーション描画、超高速、宣言的UI、軽量 |
| **CLI パーサー** | **clap** (derive feature) | デファクトスタンダード、サブコマンド/引数補完対応 |
| **非同期ランタイム** | **tokio** | ネットワークリクエスト、非同期スキャン実行 |
| **バージョン比較** | **semver** / custom comparator | セマンティックバージョニングおよびPEP 440等のパース |
| **ローカルDB / キャッシュ** | **rusqlite** or **sled** | オフライン動作時のバージョンキャッシュ、履歴保持 |
| **脆弱性DB** | **OSV.dev API** | オープンソースエコシステム特化の高速脆弱性API |
| **OS通知** | **notify-rust** | macOS, Linux, WindowsでのOSネイティブ通知 |
| **パッケージマネージャ連携** | `homebrew`, `winget`, `scoop`, `apt` | 環境に合わせた最適なアップデートコマンドの実行 |

---

## 3. 主要機能要件

### 3.1 バージョン検出エンジン (Detector)
- **検出対象**:
  - 言語ランタイム: Python, Node.js, PHP, Go, Rust, Ruby, Java等
  - パッケージマネージャ / ツール: Docker, Git, Terraform, Kubectl, AWS CLI等
  - バージョン管理ツール対応: `pyenv`, `asdf`, `nvm`, `fnm`, `mise` 等が管理するローカルパスの考慮。

### 3.2 UI / UX (GPUI - JetBrains Toolbox ライク)
- **トレイ常駐 & クイックポップアップ**: メニューバー/タスクバーからワンクリックで展開。
- **一覧表示（リストビュー）**:
  - **アイコン**: ツール名とロゴアイコン
  - **現在バージョン** と **最新バージョン**（最新なら緑色バッジ、古い場合はオレンジ）
  - **アクション**:
    - `Update` ボタン（古い場合のみ表示、クリックでバックグラウンド実行）
  - **脆弱性バッジ**: CVE検知時に赤色警告アイコン＋深刻度（Critical, High）

### 3.3 脆弱性検知 & 通知 (Security & Notifications)
- **OSV.dev API 連携**: パッケージ名＋現在バージョンを照合し、既知の脆弱性（CVE/GHSA）をバックグラウンドで定時スキャン。
- **通知トリガー**: CVSS 7.0以上の脆弱性が検出された場合、OS通知を発報。

### 3.4 CLI機能
GUIと完全に共通のコアエンジンを利用。
```bash
# インストール済みツールのスキャンとバージョン一覧
$ stackbox list

# 最新バージョンとの差分・脆弱性ステータス表示
$ stackbox check

# 特定のツールまたは全ツールのアップデート
$ stackbox update node
```

### 3.5 管理者向けサーバー連携（Admin Fleet Management）
- **エージェント機能**: 定期的にインベントリ情報（ホスト名、OS、全ツールバージョン、CVE検出状況）をAdminサーバーへ安全に同期。
- **Admin Dashboard（Web）**: 社内PC全体のツールバージョン分布、サポート期限切れ（EOL）の可視化。

---

## 4. プロジェクト構成（Cargo Workspace）

```text
vercheck/
├── Cargo.toml                 # ワークスペース定義
├── crates/
│   ├── vercheck-core/         # コアロジック (検出, 比較, CVE, 設定)
│   ├── vercheck-cli/          # CLIフロントエンド
│   ├── vercheck-gui/          # GPUIネイティブGUIフロントエンド
│   └── vercheck-agent/        # 将来のAdmin通信用バックグラウンドデーモン
```

---

## 5. データモデル定義（抜粋）

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolItem {
    pub id: String,                 
    pub name: String,               
    pub current_version: String,    
    pub latest_version: String,     
    pub binary_path: String,        
    pub status: VersionStatus,
    pub vulnerabilities: Vec<Vulnerability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VersionStatus {
    UpToDate,
    Outdated,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub cve_id: String,             
    pub severity: Severity,         
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}
```

---

## 6. 実装ロードマップ

開発計画と進捗は [PLAN.md](PLAN.md) に移しました。