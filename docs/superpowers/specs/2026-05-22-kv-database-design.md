# KV Database Design Spec

> 学习型 Rust 数据库项目 — 实现 MySQL 核心功能子集
> 2026-05-22 | 预计工期 6-12 个月 | Rust 新手 | 双人协作

---

## 1. 项目目标

用 Rust 从零实现一个关系型数据库，具备 MySQL 的核心功能子集：

- SQL 解析与执行（SELECT/INSERT/UPDATE/DELETE，WHERE，ORDER BY，简单 JOIN）
- 磁盘存储引擎（B+Tree，页管理，缓冲池）
- 索引（主键索引 + 二级索引）
- ACID 事务（MVCC，锁管理）
- MySQL Wire Protocol 兼容（可用 `mysql` CLI 连接）

**目标代码量：** 1-3 万行  
**项目定位：** 深入理解数据库内部原理，非生产用途

---

## 2. 系统架构

### 2.1 五层架构

```
                  Client (mysql CLI)
                       │ TCP 3306
┌──────────────────────▼──────────────────────────────┐
│              ① Network Layer                        │
│  MySQL Wire Protocol Codec  │  TCP Server (tokio)   │
├─────────────────────────────────────────────────────┤
│              ② SQL Layer                            │
│  Lexer → Parser → Planner → Executor                │
├─────────────────────────────────────────────────────┤
│              ③ Transaction Layer                    │
│  MVCC Version Chain  │  Lock Manager  │  Txn Mgr    │
├─────────────────────────────────────────────────────┤
│              ④ Storage Layer                        │
│  B+Tree Index  │  Page Manager  │  Buffer Pool       │
├─────────────────────────────────────────────────────┤
│              ⑤ Catalog / Metadata                   │
│  databases → tables → columns → indexes → stats     │
└─────────────────────────────────────────────────────┘
```

### 2.2 层间接口（Trait 定义）

| 接口 | Trait | 职责 |
|------|-------|------|
| Network → SQL | `CommandHandler` | 接收解析后的 Command，返回 ResultSet |
| SQL → Transaction | `TxnContext` | 读写时携带 txn_id，获取可见版本 |
| Transaction → Storage | `StorageEngine` | get / put / delete / scan，携带 version 信息 |
| Storage → OS | `Pager` | 页粒度读写，封装 fsync 等系统调用 |

---

## 3. 项目结构（Cargo Workspace）

```
kv/
├── Cargo.toml                 # [workspace] 根配置
├── Cargo.lock
├── rust-toolchain.toml        # 固定 Rust 版本
├── .gitignore
├── README.md
│
├── crates/
│   ├── kv-common/             # ★ 共享类型 + 核心 trait（最重要）
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs       # Row, Value, Column, DataType, Schema
│   │       ├── error.rs       # KvError 统一错误枚举
│   │       └── traits.rs      # StorageEngine, CommandHandler, TxnContext, Pager
│   │
│   ├── kv-storage/            # 存储引擎 [B 主要负责]
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── page.rs        # Page 结构 (4KB)，页头/页体编解码
│   │       ├── btree.rs       # B+Tree：插入/删除/搜索/分裂/合并
│   │       ├── buffer.rs      # Buffer Pool (LRU-K 淘汰策略)
│   │       ├── codec.rs       # Row 的序列化/反序列化
│   │       └── engine.rs      # impl StorageEngine for KvStorage
│   │
│   ├── kv-sql/                # SQL 解析与执行 [A 主要负责]
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── lexer.rs       # 词法分析：SQL → Token 流
│   │       ├── ast.rs         # AST 节点定义
│   │       ├── parser.rs      # 语法分析：Token 流 → AST
│   │       ├── planner.rs     # 查询计划：AST → ExecutionPlan
│   │       └── executor.rs    # 执行器：ExecutionPlan → ResultSet
│   │
│   ├── kv-txn/                # 事务与并发控制 [共享/可并行]
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── mvcc.rs        # MVCC 版本链，可见性判断
│   │       ├── lock.rs        # 表锁 / 行锁
│   │       └── manager.rs     # 事务管理器 (begin/commit/rollback)
│   │
│   ├── kv-network/            # 网络协议层 [A 主要负责]
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs      # TCP Server (tokio async)
│   │       └── protocol.rs    # MySQL Wire Protocol 编解码
│   │
│   └── kv-server/             # 集成二进制 [两人协作]
│       ├── Cargo.toml
│       └── src/
│           └── main.rs        # 组装所有模块，启动服务
│
├── tests/                     # 集成测试
│   ├── integration.rs         # 端到端 SQL 测试
│   └── compatibility.rs       # MySQL 协议兼容性测试
│
├── docs/
│   └── superpowers/
│       └── specs/             # 设计文档
│
└── .github/
    └── workflows/
        └── ci.yml             # CI：cargo test + clippy + fmt
```

### 3.1 Crate 依赖图

```
kv-server ──→ kv-network ──→ kv-common
           ──→ kv-sql ──────→ kv-common
           ──→ kv-txn ──────→ kv-common
           ──→ kv-storage ──→ kv-common

kv-sql ──→ kv-txn    (通过 trait，编译时依赖)
kv-txn ──→ kv-storage (通过 trait，编译时依赖)
```

---

## 4. 核心 Trait 定义

### 4.1 StorageEngine

```rust
/// 存储引擎抽象 — SQL 层和事务层依赖此 trait 访问数据
#[async_trait]
pub trait StorageEngine: Send + Sync {
    /// 插入或更新一行，返回旧版本号
    async fn put(&self, table_id: TableId, key: &[u8], value: &[u8], txn_id: u64) -> KvResult<u64>;

    /// 读取指定版本的可见数据
    async fn get(&self, table_id: TableId, key: &[u8], txn_id: u64) -> KvResult<Option<Vec<u8>>>;

    /// 范围扫描
    async fn scan(&self, table_id: TableId, start: &[u8], end: &[u8], txn_id: u64)
        -> KvResult<Vec<(Vec<u8>, Vec<u8>)>>;

    /// 标记删除
    async fn delete(&self, table_id: TableId, key: &[u8], txn_id: u64) -> KvResult<()>;

    /// 创建索引
    async fn create_index(&self, table_id: TableId, col_id: ColumnId) -> KvResult<IndexId>;

    /// 通过索引查找
    async fn index_lookup(&self, index_id: IndexId, key: &[u8], txn_id: u64)
        -> KvResult<Vec<Vec<u8>>>;
}
```

### 4.2 CommandHandler

```rust
/// SQL 命令处理接口 — Network 层调用此 trait 执行 SQL
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, sql: &str, session: &Session) -> KvResult<ResultSet>;
}
```

### 4.3 TxnContext

```rust
/// 事务上下文 — 携带当前事务的快照信息
pub trait TxnContext {
    fn txn_id(&self) -> u64;
    fn snapshot_version(&self) -> u64;
    fn isolation_level(&self) -> IsolationLevel;
}
```

---

## 5. 双人协作方案

### 5.1 分工

| 角色 | 负责 crate | 关注领域 | 依赖 |
|------|-----------|----------|------|
| **A** | kv-sql, kv-network, kv-server | SQL 解析、查询执行、网络协议、二进制集成 | kv-common (traits) |
| **B** | kv-storage, kv-txn | B+Tree 存储、缓冲池、MVCC、锁管理 | kv-common (traits) |

### 5.2 协作规则

1. **trait 先行，共同 review**
   - `kv-common/src/traits.rs` 是两人的"接口合同"
   - 任何 trait 变更的 PR 必须两人都 approve
   - 第一批合入 main 的代码必须是 kv-common

2. **各自分支，PR 合入**
   - A：`feat/sql-parser` → `feat/network` → `feat/executor`
   - B：`feat/page-btree` → `feat/buffer-pool` → `feat/mvcc`
   - 禁止直接 push 到 main

3. **Mock 先行**
   - A 用 `MockStorageEngine` 开发 SQL 层，不等 B
   - B 用单元测试验证存储引擎，不依赖 SQL 层
   - Mock 实现在各自 crate 的 `tests/` 目录下

4. **CI 门禁**
   - PR 合并前必须通过：`cargo test --workspace`、`cargo clippy`、`cargo fmt --check`
   - 配置文件：`.github/workflows/ci.yml`

5. **集成测试共同维护**
   - `tests/integration.rs` 是验证接口对接正确性的唯一手段
   - 当 A 和 B 的代码需要对接时，两人一起编写集成测试

6. **trait 冻结机制**
   - 每完成一个阶段后，trait 进入冻结期，禁止 break change
   - 如需修改，先讨论、更新 Mock、再改 trait

### 5.3 分支策略

```
main
 ├── feat/kv-common          ← 第 1 步：两人共同 review，快速合入
 ├── feat/page-manager       ← B：存储引擎底层
 ├── feat/btree              ← B：B+Tree（依赖 page-manager）
 ├── feat/buffer-pool        ← B：缓冲池（依赖 page-manager）
 ├── feat/sql-parser         ← A：词法+语法分析
 ├── feat/query-executor     ← A：查询执行（依赖 sql-parser）
 ├── feat/network-server     ← A：TCP + Wire Protocol
 ├── feat/mvcc               ← B：事务 MVCC
 └── feat/integration        ← 两人：集成所有模块
```

---

## 6. 各层设计要点

### 6.1 存储引擎（kv-storage）

**页格式 (Page Layout)**

```
┌─────────────────────────────────────┐
│ Page Header (64 bytes)              │
│  - page_id: u32                     │
│  - page_type: PageType (1 byte)     │
│  - free_space_offset: u16           │
│  - cell_count: u16                  │
│  - checksum: u32                    │
│  - lsn: u64 (Log Sequence Number)   │
├─────────────────────────────────────┤
│ Cell Pointers (从页尾向前增长)       │
│  [offset_u16, offset_u16, ...]      │
├─────────────────────────────────────┤
│ Free Space                          │
├─────────────────────────────────────┤
│ Cells (从页头向后增长)               │
│  [key | value | version_chain_ptr]  │
└─────────────────────────────────────┘
```

**页大小：** 4KB（简化实现，MySQL InnoDB 默认 16KB）

**B+Tree 实现要点：**
- 叶节点存储实际数据，内部节点仅存 key + child_page_id
- 支持插入时分裂（split）和删除时合并（merge）
- 使用 Slotted Page 布局管理变长记录
- 根节点固定在 page_id=1

**Buffer Pool：**
- 固定大小的页缓存（可配置，默认 1000 页 = 4MB）
- 淘汰策略：LRU-2（记录访问次数，优先淘汰访问 0 次的页）
- 脏页跟踪 + 定期刷盘

### 6.2 SQL 层（kv-sql）

**支持的 SQL 语法子集：**

```sql
-- DDL
CREATE TABLE t (id INT PRIMARY KEY, name VARCHAR(100), age INT);
CREATE INDEX idx_name ON t (name);
DROP TABLE t;

-- DML
INSERT INTO t VALUES (1, 'Alice', 25);
INSERT INTO t (id, name) VALUES (2, 'Bob');
SELECT * FROM t WHERE age > 20 ORDER BY name;
SELECT t1.name, t2.value FROM t1 JOIN t2 ON t1.id = t2.id;
UPDATE t SET age = 26 WHERE id = 1;
DELETE FROM t WHERE age < 18;
```

**AST 节点类型：**

```rust
pub enum Statement {
    Select { columns: Vec<ColumnRef>, from: TableRef, where_clause: Option<Expr>,
             order_by: Vec<OrderBy>, join: Option<Join> },
    Insert { table: TableRef, columns: Option<Vec<String>>, values: Vec<Vec<Expr>> },
    Update { table: TableRef, set: Vec<(String, Expr)>, where_clause: Option<Expr> },
    Delete { table: TableRef, where_clause: Option<Expr> },
    CreateTable { name: String, columns: Vec<ColumnDef>, primary_key: String },
    CreateIndex { name: String, table: String, column: String },
    DropTable { name: String },
}
```

**执行计划（ExecutionPlan）：**
- `SeqScan` → 全表扫描
- `IndexScan` → 索引扫描
- `Filter` → 条件过滤
- `Sort` → 排序
- `NestedLoopJoin` → 嵌套循环连接
- `Projection` → 列投影

### 6.3 事务层（kv-txn）

**MVCC 实现方案：**

```
Row 版本链 (单向链表，新→旧):
┌──────────┐    ┌──────────┐    ┌──────────┐
│ v=100    │───→│ v=50     │───→│ v=10     │───→ NULL
│ txn_id=5 │    │ txn_id=3 │    │ txn_id=1 │
│ data={..}│    │ data={..}│    │ data={..}│
│ next_ptr │    │ next_ptr │    │ next_ptr │
└──────────┘    └──────────┘    └──────────┘
```

**可见性判断：**
- 每个事务在开始时获取一个快照版本 `snapshot_version`
- 读取时从版本链头部遍历，返回第一个 `version <= snapshot_version` 且事务已提交的版本
- 写入时在版本链头部插入新版本（还未提交时标记为 pending）

**隔离级别：**
- 阶段 1：仅支持 Read Committed
- 阶段 2：支持 Repeatable Read（Snapshot Isolation）

**锁管理：**
- 表级读写锁（简化实现）
- 死锁检测：简单的超时机制

### 6.4 网络层（kv-network）

**MySQL Wire Protocol 核心实现：**

```
握手阶段:
  Client → Server:  (TCP connect)
  Server → Client:  Initial Handshake Packet (server version, connection id, auth plugin)
  Client → Server:  Handshake Response (username, auth response)
  Server → Client:  OK Packet 或 ERR Packet

查询阶段:
  Client → Server:  COM_QUERY (command byte + SQL text)
  Server → Client:  Column Count + Column Definitions + Row Data + OK/ERR

结果集格式:
  [Column Count]
  [Column Definition × N]
  [Row Data × M]   ← 每行以 0x00 开头，各字段为 length-encoded string
  [OK/EOF/ERR Packet]
```

**实现范围：**
- 支持 `mysql` CLI 连接（`mysql -h 127.0.0.1 -P 3306 -u root`）
- 支持 `COM_QUERY`（查询命令）
- 支持 `COM_QUIT`（断开连接）
- 加密认证可跳过（仅支持 `mysql_native_password` 的简化版）

### 6.5 元数据管理（Catalog）

**内存数据结构：**

```rust
pub struct Catalog {
    databases: HashMap<String, Database>,
}

pub struct Database {
    tables: HashMap<String, TableMeta>,
    next_table_id: TableId,
}

pub struct TableMeta {
    pub table_id: TableId,
    pub columns: Vec<ColumnDef>,
    pub primary_key: usize,     // 列索引
    pub indexes: Vec<IndexMeta>,
}
```

元数据本身存储在特殊的系统表中（Bootstrap 时加载到内存缓存）。

---

## 7. 统一错误处理

```rust
#[derive(Debug, thiserror::Error)]
pub enum KvError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error at position {pos}: {message}")]
    ParseError { pos: usize, message: String },

    #[error("Table '{0}' not found")]
    TableNotFound(String),

    #[error("Column '{0}' not found")]
    ColumnNotFound(String),

    #[error("Type mismatch: expected {expected:?}, got {actual:?}")]
    TypeMismatch { expected: DataType, actual: DataType },

    #[error("Duplicate key: {0:?}")]
    DuplicateKey(Vec<u8>),

    #[error("Transaction conflict: txn {txn_id}")]
    TxnConflict { txn_id: u64 },

    #[error("Internal: {0}")]
    Internal(String),
}
```

---

## 8. 测试策略

| 层级 | 工具 | 覆盖范围 |
|------|------|---------|
| 单元测试 | `#[test]` + `cargo test` | 每个 crate 内部逻辑 |
| Mock 测试 | 手动 Mock trait 实现 | 跨层接口验证 |
| 集成测试 | `tests/` 目录，真实存储 | SQL → Storage 端到端 |
| 兼容性测试 | `mysql` CLI + shell 脚本 | Wire Protocol 正确性 |
| 模糊测试 | 可选，后期引入 `proptest` | 边界条件和 crash 检测 |

**每个 crate 的测试策略：**
- `kv-storage`：B+Tree 插入/删除/搜索的正确性，页分裂/合并，缓冲池淘汰逻辑
- `kv-sql`：SQL 解析正确性（合法/非法输入），查询计划生成，执行结果验证
- `kv-txn`：并发事务测试，MVCC 可见性，锁竞争场景
- `kv-network`：协议编解码往返测试，多连接并发
- `kv-server`：docker compose 启动，mysql CLI 连接测试

---

## 9. 构建阶段（垂直切片）

### 阶段 1：最小可用原型（Month 1-2）

```
[两人协作] kv-common (types + traits)
[A] SQL Lexer + Parser → AST（纯解析，不执行）
[B] Page Manager + 简单 B+Tree（无事务）
[集成] 命令行 REPL：输入 SQL → 打印 AST
```

### 阶段 2：内存数据库（Month 3-4）

```
[A] 查询执行器：SeqScan + Filter + Sort
[A] Network Server + MySQL Wire Protocol (COM_QUERY)
[B] Buffer Pool + B+Tree 持久化
[集成] 可以用 mysql CLI 连接，执行 CRUD
```

### 阶段 3：事务（Month 5-7）

```
[B] MVCC 版本链 + 事务管理器
[B] 锁管理 + 死锁检测
[A] 查询执行器支持事务上下文
[集成] ACID 保证 + 并发测试
```

### 阶段 4：查询增强（Month 8-10）

```
[A] 索引扫描 + 简单 JOIN
[A] 查询计划优化（谓词下推，索引选择）
[B] 二级索引 + 索引维护
[集成] 性能基准测试
```

### 阶段 5：完善（Month 10-12）

```
[A] 完整 DDL 支持
[B] 崩溃恢复（WAL 简化版）
[两人] 文档、测试、性能优化
```

---

## 10. 关键依赖（Cargo）

```toml
# 核心依赖
tokio = { version = "1", features = ["full"] }     # 异步运行时
thiserror = "2"                                      # 错误派生
bytes = "1"                                          # 字节缓冲

# SQL 解析辅助
nom = "8"                                            # 解析器组合子

# 序列化（可选，后期按需）
serde = { version = "1", features = ["derive"] }

# 开发依赖
criterion = "0.5"                                    # 性能基准
tempfile = "3"                                       # 临时文件（测试用）
proptest = "1"                                       # 属性测试（后期）
```

---

## 11. 未决问题（后续细化）

- [ ] 列存 vs 行存的选择（阶段 1 默认行存）
- [ ] B+Tree 的并发控制（latch coupling vs 更简单的页锁）
- [ ] WAL 格式和恢复策略
- [ ] 字符集支持范围（仅 UTF-8 还是多字符集）
- [ ] 权限系统（阶段 5 再考虑）
