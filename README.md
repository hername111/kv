# KV

> 用 Rust 从零实现 MySQL 风格的关系型数据库 — 学习项目

[![Rust](https://img.shields.io/badge/rust-1.94+-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

---

## 概述

一个学习型数据库项目，涵盖 SQL 解析、B+Tree 存储引擎、MVCC 事务、MySQL 网络协议。可用 `mysql` CLI 直接连接并执行 SQL。

### 目标功能

- SQL 解析与执行（SELECT / INSERT / UPDATE / DELETE / JOIN / ORDER BY）
- 磁盘存储引擎（B+Tree + 缓冲池）
- 主键索引 + 二级索引
- ACID 事务（MVCC + 锁管理）
- MySQL Wire Protocol 兼容（`mysql` CLI 可连接）

---

## 架构

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

各层通过 `trait` 解耦，可独立开发与测试。详见[设计文档](docs/superpowers/specs/2026-05-22-kv-database-design.md)。

---

## 开发路线

| 阶段 | 时间 | 内容 |
|------|------|------|
| ① 最小原型 | Day 1-3 | kv-common, SQL Parser, Page Manager + B+Tree |
| ② 内存数据库 | Day 4-6 | 查询执行, MySQL Wire Protocol, 持久化 |
| ③ 事务 | Day 7-9 | MVCC, 锁管理, ACID 保证 |
| ④ 查询增强 | Day 10-12 | 索引扫描, JOIN, 查询优化 |
| ⑤ 完善 | Day 13-14 | WAL, 完整 DDL, 性能测试 |

---

## 项目结构

```
kv/
├── crates/
│   ├── kv-common/      # 共享类型 + 核心 trait
│   ├── kv-storage/     # B+Tree / 页管理 / 缓冲池
│   ├── kv-sql/         # 词法分析 / 语法分析 / 查询执行
│   ├── kv-txn/         # MVCC / 锁 / 事务管理
│   ├── kv-network/     # TCP 服务器 / MySQL 协议
│   └── kv-server/      # 集成二进制
├── tests/              # 集成测试
└── docs/               # 设计文档
```

---

## 快速开始

### 环境要求

- Rust 1.94+
- MySQL 客户端（验证用）

### 构建与运行

```bash
# 构建所有 crate
cargo build --workspace

# 启动服务
cargo run -p kv-server

# 另开终端连接
mysql -h 127.0.0.1 -P 3306 -u root
```

### 测试

```bash
# 运行全部测试
cargo test --workspace

# Lint 检查
cargo clippy --workspace

# 格式检查
cargo fmt --check --all
```

---

## 协作指南（两人）

| 角色 | 负责 crate | 领域 |
|------|-----------|------|
| **A** | kv-sql, kv-network, kv-server | SQL 处理 / 网络 / 集成 |
| **B** | kv-storage, kv-txn | 存储 / 事务 |

### 开发规则

1. **`kv-common/src/traits.rs` 是接口合同** — 变更需两人 approve
2. **Mock 先行** — 不等对方，用 Mock 实现独立开发
3. **PR 合并前 CI 必过** — `cargo test --workspace` + `clippy` + `fmt`
4. **集成测试共同编写** — 接口对接时两人一起验证

详见[设计文档 §5](docs/superpowers/specs/2026-05-22-kv-database-design.md#5-双人协作方案)。

---

## 许可

MIT
