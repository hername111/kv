# KV

> 用 Rust 从零实现 MySQL 风格的关系型数据库 — 学习项目
> 工期 2 周 | Rust 1.94+ | 双人协作

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

### 开发者

| 代号 | 定位 | 负责领域 |
|------|------|---------|
| **hhy** | 核心模块 | 复杂数据结构、并发控制、协议解析 |
| **zwd** | 基础模块 | 类型定义、简单算法、集成组装 |

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

## 完整项目文件树与分工

```
kv/
├── Cargo.toml                          # [hhy] workspace 根配置
├── Cargo.lock
├── rust-toolchain.toml                 # [zwd] 固定 Rust 版本
├── .gitignore
├── README.md
│
├── crates/
│   ├── kv-common/                      # —— 共享层 (Day 1 完成) ——
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # [zwd] crate 入口
│   │       ├── types.rs                # [zwd] Row / Value / Column / DataType / Schema
│   │       ├── error.rs                # [zwd] KvError 统一错误枚举
│   │       └── traits.rs               # [hhy] ★ StorageEngine / CommandHandler / TxnContext / Pager
│   │
│   ├── kv-storage/                     # —— 存储引擎层 ——
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # [zwd] crate 入口
│   │       ├── page.rs                 # [hhy] 页结构 (4KB) / 页头编解码 / Slotted Page
│   │       ├── btree.rs                # [hhy] B+Tree 插入/删除/搜索/分裂/合并
│   │       ├── buffer.rs               # [zwd] Buffer Pool (LRU-2 淘汰策略)
│   │       ├── codec.rs                # [zwd] Row 序列化/反序列化
│   │       └── engine.rs               # [hhy] impl StorageEngine for KvStorage
│   │
│   ├── kv-sql/                         # —— SQL 解析与执行层 ——
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # [zwd] crate 入口
│   │       ├── lexer.rs                # [zwd] 词法分析：SQL 文本 → Token 流
│   │       ├── ast.rs                  # [zwd] AST 节点定义 (Statement / Expr / Operator)
│   │       ├── parser.rs               # [hhy] 语法分析：Token 流 → AST (递归下降)
│   │       ├── planner.rs              # [hhy] 查询计划：AST → ExecutionPlan
│   │       └── executor.rs             # [zwd] 执行器：ExecutionPlan → ResultSet
│   │
│   ├── kv-txn/                         # —— 事务与并发控制层 ——
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # [zwd] crate 入口
│   │       ├── mvcc.rs                 # [hhy] MVCC 版本链 / 可见性判断
│   │       ├── lock.rs                 # [hhy] 表级读写锁 / 死锁检测
│   │       └── manager.rs              # [zwd] 事务 begin/commit/rollback 生命周期
│   │
│   ├── kv-network/                     # —— 网络协议层 ——
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # [zwd] crate 入口
│   │       ├── server.rs               # [zwd] TCP Server (tokio) / 连接管理
│   │       └── protocol.rs             # [hhy] MySQL Wire Protocol 握手/查询/结果集
│   │
│   └── kv-server/                      # —— 集成二进制 ——
│       ├── Cargo.toml
│       └── src/
│           └── main.rs                 # [zwd] 组装所有模块，启动服务
│
├── tests/                              # —— 集成测试 ——
│   ├── integration.rs                  # [hhy + zwd] 端到端 SQL 测试
│   └── compatibility.rs               # [zwd] MySQL 协议兼容性测试
│
├── docs/
│   └── superpowers/specs/
│       └── 2026-05-22-kv-database-design.md
│
└── .github/
    └── workflows/
        └── ci.yml                       # [zwd] CI：cargo test + clippy + fmt
```

### 分工统计

| 开发者 | 文件数 | 核心职责 |
|--------|--------|---------|
| **hhy** | 9 | traits 接口、B+Tree、页管理、SQL 解析/计划、MVCC、锁、MySQL 协议 |
| **zwd** | 16 | 类型/错误定义、词法分析、AST、执行器、缓冲池、编解码、TCP Server、事务管理、集成、CI |
| **共同** | 1 | 集成测试 |

---

## 阶段计划与每日进度

### 阶段 ①：最小原型（Day 1-3）

| 日期 | hhy | zwd | 产出 |
|------|-----|-----|------|
| **Day 1** | `traits.rs` — 定义 4 个核心 trait 接口 | `types.rs` + `error.rs` — 全部基础类型和错误枚举 | kv-common 合入，双方对齐接口 |
| **Day 2** | `page.rs` — 页结构、页头编解码、Slotted Page 布局 | `lexer.rs` + `ast.rs` — Token 定义、全部 AST 节点 | page 和 lexer 可单测 |
| **Day 3** | `btree.rs` — 叶节点插入/搜索、page_id=1 根节点 | `parser.rs` 框架 — 递归下降骨架，SELECT/INSERT 解析 | REPL 输入 SQL → 打印 AST |

> 阶段 ① 验收：命令行输入 `SELECT * FROM t;` 能输出 AST 树

### 阶段 ②：内存数据库（Day 4-6）

| 日期 | hhy | zwd | 产出 |
|------|-----|-----|------|
| **Day 4** | `btree.rs` — 分裂/合并、内部节点、范围扫描 | `buffer.rs` + `codec.rs` — Buffer Pool + Row 编解码 | B+Tree 完整可用 |
| **Day 5** | `engine.rs` — impl StorageEngine，对接 page/btree/buffer | `executor.rs` — SeqScan + Filter + Sort | 存储引擎可读写 |
| **Day 6** | `protocol.rs` — MySQL 握手 + COM_QUERY 编解码 | `server.rs` — tokio TCP Server + 连接管理 + `main.rs` | mysql CLI 连接成功 |

> 阶段 ② 验收：mysql CLI 连接 → `CREATE TABLE` → `INSERT` → `SELECT` 全链路

### 阶段 ③：事务（Day 7-9）

| 日期 | hhy | zwd | 产出 |
|------|-----|-----|------|
| **Day 7** | `mvcc.rs` — 版本链结构、快照读、可见性判断 | `manager.rs` — 事务 begin/commit/rollback 生命周期 | MVCC 核心就绪 |
| **Day 8** | `lock.rs` — 表级读写锁、超时死锁检测 | 改造 `executor.rs` 集成 TxnContext | 事务上下文贯穿 SQL 执行 |
| **Day 9** | MVCC + 存储引擎集成（version chain 嵌入 page cell） | `integration.rs` — 并发事务测试用例 | ACID 验证通过 |

> 阶段 ③ 验收：两个并发 mysql 连接，一个未提交的修改对另一个不可见

### 阶段 ④：查询增强（Day 10-12）

| 日期 | hhy | zwd | 产出 |
|------|-----|-----|------|
| **Day 10** | `planner.rs` — 索引扫描计划、简单 JOIN 计划 | `btree.rs` 二级索引支持 | 索引扫描可用 |
| **Day 11** | planner 优化 — 谓词下推、索引选择 | executor JOIN 实现 (NestedLoopJoin) | JOIN 查询通过 |
| **Day 12** | protocol.rs 完善 — 完整结果集格式、错误包 | `compatibility.rs` + 性能基准 | 协议兼容性测试通过 |

> 阶段 ④ 验收：`SELECT ... JOIN ... WHERE ... ORDER BY` 正确执行

### 阶段 ⑤：完善（Day 13-14）

| 日期 | hhy | zwd | 产出 |
|------|-----|-----|------|
| **Day 13** | WAL 简化版 — 先写日志再写页 | 完整 DDL — CREATE/DROP TABLE/INDEX + Catalog 持久化 | 崩溃恢复基本可用 |
| **Day 14** | Bug 修复 + 边界条件 | 文档补全 + `cargo clippy` + `cargo fmt` | 项目交付 |

> 阶段 ⑤ 验收：全量测试通过 + mysql CLI 完整操作演示

---

## 快速开始

### 环境要求

- Rust 1.94+
- MySQL 客户端（验证用）

### 构建与运行

```bash
cargo build --workspace
cargo run -p kv-server
mysql -h 127.0.0.1 -P 3306 -u root
```

### 测试

```bash
cargo test --workspace
cargo clippy --workspace
cargo fmt --check --all
```

---

## 协作规则

1. **`kv-common/src/traits.rs` 是接口合同** — 变更需 hhy + zwd 两人 approve
2. **Mock 先行** — 不等对方，用 Mock 实现独立开发
3. **每日合并** — 每天收工前合入 main，避免长时间分叉
4. **集成测试共同编写** — 接口对接时两人一起验证

详见[设计文档 §5](docs/superpowers/specs/2026-05-22-kv-database-design.md#5-双人协作方案)。

---

## 许可

MIT
