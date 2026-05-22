# KV

> Rust で実装する MySQL 風リレーショナルデータベース — 学習用

[![Rust](https://img.shields.io/badge/rust-1.94+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## 概要

SQL 解析、B+Tree ストレージエンジン、MVCC トランザクション、MySQL ワイヤープロトコルを Rust でゼロから実装する学習プロジェクト。`mysql` CLI から接続して SQL を実行できます。

### 目標機能

- SQL の解析と実行（SELECT / INSERT / UPDATE / DELETE / JOIN / ORDER BY）
- ディスクベースのストレージエンジン（B+Tree + バッファプール）
- プライマリキーインデックス + セカンダリインデックス
- ACID トランザクション（MVCC + ロックマネージャ）
- MySQL ワイヤープロトコル互換（`mysql` CLI 接続可能）

---

## アーキテクチャ

```
                 Client (mysql CLI)
                      │ TCP 3306
┌─────────────────────▼──────────────────────────────┐
│             ① Network Layer                        │
│ MySQL Wire Protocol Codec  │  TCP Server (tokio)   │
├────────────────────────────────────────────────────┤
│             ② SQL Layer                            │
│ Lexer → Parser → Planner → Executor                │
├────────────────────────────────────────────────────┤
│             ③ Transaction Layer                    │
│ MVCC Version Chain  │  Lock Manager  │  Txn Mgr    │
├────────────────────────────────────────────────────┤
│             ④ Storage Layer                        │
│ B+Tree Index  │  Page Manager  │  Buffer Pool       │
├────────────────────────────────────────────────────┤
│             ⑤ Catalog / Metadata                   │
└────────────────────────────────────────────────────┘
```

各層は `trait` で分離されており、独立して開発・テストが可能です。詳細は [設計ドキュメント](docs/superpowers/specs/2026-05-22-kv-database-design.md) を参照してください。

---

## 開発ロードマップ

| フェーズ | 期間 | 内容 |
|---------|------|------|
| ① 最小プロトタイプ | Month 1-2 | kv-common, SQL Parser, Page Manager + B+Tree |
| ② インメモリ DB | Month 3-4 | クエリ実行, MySQL Wire Protocol, 永続化 |
| ③ トランザクション | Month 5-7 | MVCC, ロック管理, ACID 保証 |
| ④ クエリ強化 | Month 8-10 | インデックススキャン, JOIN, クエリ最適化 |
| ⑤ 完成度向上 | Month 10-12 | WAL, DDL 完全対応, パフォーマンステスト |

---

## プロジェクト構成

```
kv/
├── crates/
│   ├── kv-common/      # 共有型定義 + コア trait
│   ├── kv-storage/     # B+Tree / ページ管理 / バッファプール
│   ├── kv-sql/         # 字句解析 / 構文解析 / クエリ実行
│   ├── kv-txn/         # MVCC / ロック / トランザクション管理
│   ├── kv-network/     # TCP サーバー / MySQL プロトコル
│   └── kv-server/      # 全モジュール統合バイナリ
├── tests/              # 統合テスト
└── docs/               # 設計ドキュメント
```

---

## クイックスタート

### 必要環境

- Rust 1.94+
- MySQL クライアント（動作確認用）

### ビルドと実行

```bash
# 全 crate ビルド
cargo build --workspace

# サーバー起動
cargo run -p kv-server

# 別ターミナルから接続
mysql -h 127.0.0.1 -P 3306 -u root
```

### テスト

```bash
# 全テスト実行
cargo test --workspace

# リントチェック
cargo clippy --workspace

# フォーマットチェック
cargo fmt --check --all
```

---

## 協業ガイド（2 名）

| 担当 | クレート | 担当領域 |
|------|---------|---------|
| **A** | kv-sql, kv-network, kv-server | SQL 処理系 / ネットワーク / 統合 |
| **B** | kv-storage, kv-txn | ストレージ / トランザクション |

### 開発ルール

1. **`kv-common/src/traits.rs` が契約** — 変更は両者 approve 必須
2. **Mock 先行** — 相手の実装を待たず、Mock で開発を進める
3. **PR マージ前に CI 通過必須** — `cargo test --workspace` + `clippy` + `fmt`
4. **統合テストは両者で書く** — インターフェースの結合確認は二人作業

詳細は [設計ドキュメント §5](docs/superpowers/specs/2026-05-22-kv-database-design.md#5-双人协作方案) を参照。

---

## ライセンス

MIT
