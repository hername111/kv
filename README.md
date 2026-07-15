# KV Database

一个使用 Rust 2024 实现的教学型关系数据库。项目从磁盘页面、B+Tree、缓冲池和事务管理开始，贯通 SQL 执行、MySQL Wire Protocol 和 React Web 工作台，形成一个可以启动、操作、测试和复盘的最小数据库系统。

> **项目定位**：本项目服务于 Rust 课程大作业和数据库原理学习，不宣称具备生产数据库的完整能力。当前版本没有 WAL、认证、权限控制、加密和完整的崩溃恢复机制，请仅在本机使用演示数据。

## 项目亮点

- **完整请求链路**：`SQL -> Lexer -> Parser -> Planner -> Executor -> Transaction -> Storage`。
- **可持久化存储**：4 KiB 页面、superblock、空闲页链表、catalog 和 B+Tree，服务重启后可恢复表结构。
- **事务与并发基础**：支持 `BEGIN`、`COMMIT`、`ROLLBACK`、事务内读己之写、MVCC 可见性和表级锁。
- **双入口访问**：兼容常用 MySQL 客户端的 TCP 接口，同时提供本地 HTTP API 和 React 工作台。
- **可观察的演示体验**：Web 界面展示 SQL、结果集、表结构、索引数量、记录数和后端实际执行耗时。
- **边界检查与自动化验证**：覆盖协议分片/截断、HTTP 请求大小、损坏页面、槽页边界、缓存失效和持久化恢复。

## 功能范围

| 模块 | 当前实现 |
| --- | --- |
| SQL | `CREATE TABLE`、`DROP TABLE`、`CREATE INDEX`、`SELECT`、`INSERT`、`UPDATE`、`DELETE` |
| 查询 | `WHERE`、投影、`ORDER BY`、等值 `JOIN`、比较与逻辑表达式 |
| 事务 | `BEGIN`、`COMMIT`、`ROLLBACK`、写缓冲、MVCC 可见性、表级锁 |
| 存储 | 4 KiB 页面、Pager、slotted page、缓冲池、B+Tree、catalog、空闲页复用 |
| 协议 | MySQL 握手、命令包、结果集、错误响应和 TCP 分片组包 |
| Web | SQL 编辑器、执行历史、结果表、schema 面板、索引状态、数据重置 |

## 快速开始

### 环境要求

- Rust `1.94.0` 或更新版本（`Cargo.toml` 声明最低版本）
- Node.js `20+` 与 npm `10+`（仅运行 Web 工作台需要）
- Python `3.10+`（运行端到端协议测试需要）
- 可选：MySQL CLI，用于直接连接协议服务

### 1. 启动数据库服务

在仓库根目录执行：

```powershell
cargo run -p kv-server
```

默认监听：

| 服务 | 地址 | 用途 |
| --- | --- | --- |
| MySQL Wire Protocol | `127.0.0.1:3307` | MySQL CLI 或协议客户端 |
| 演示 HTTP API | `127.0.0.1:8080` | React 工作台 |
| 数据文件 | `./kv_data/kv.db` | 页面、catalog 和索引持久化 |

可通过环境变量覆盖默认值：

```powershell
$env:KV_ADDR = "127.0.0.1:3307"
$env:KV_DEMO_ADDR = "127.0.0.1:8080"
$env:KV_DATA_DIR = "target/local-demo"
cargo run -p kv-server
```

### 2. 启动 Web 工作台

另开一个终端：

```powershell
cd demo-client
npm ci
npm run dev
```

打开 <http://127.0.0.1:5173>。Vite 会将 `/api` 请求代理到 `127.0.0.1:8080`。工作台适合录制演示视频，也可以直接观察建表、写入、查询、事务回滚和索引效果。

### 3. 使用 MySQL 客户端

```powershell
mysql -h 127.0.0.1 -P 3307 -u root
```

示例 SQL：

```sql
CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(100), age INT);
INSERT INTO users VALUES (1, 'Ada', 28), (2, 'Grace', 31);
SELECT id, name FROM users WHERE age > 28 ORDER BY id;
BEGIN;
UPDATE users SET age = 32 WHERE id = 2;
ROLLBACK;
```

## 验证与质量门禁

在提交前运行完整检查：

```powershell
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo doc --workspace --no-deps

cd demo-client
npm ci
npm run build
cd ..

python test_protocol.py
```

协议脚本会启动隔离的数据目录并验证 SQL、事务、索引、错误处理和重启持久化。当前基线包含 **74 个 Rust 测试**和 **87 项协议/持久化测试**；CI 还会在 GitHub Actions 中重复执行格式检查、Clippy、Rust 测试、前端构建和协议测试。

## 代码结构

```text
crates/
  kv-common/      跨模块共享类型、错误和 trait
  kv-storage/     页面、Pager、缓冲池、B+Tree 和存储引擎
  kv-txn/         事务生命周期、MVCC 和表级锁
  kv-sql/         Lexer、Parser、Planner 和 Executor
  kv-network/     MySQL Wire Protocol、TCP 连接和结果编码
  kv-server/      服务组装与本地演示 HTTP API
demo-client/      React + Vite 数据库工作台
docs/             架构、开发、开源参考和视频材料
test_protocol.py  端到端协议与持久化测试
```

依赖关系从上到下保持单向：网络和 HTTP 层调用 SQL 执行器，执行器依赖事务与存储；存储层不依赖前端或网络协议。模块设计和数据布局见[架构与设计依据](docs/architecture.md)。

## 开源参考与差异

本项目借鉴的是公开架构思想和接口组织方式，没有复制 SQLite、PostgreSQL、MySQL InnoDB 或 BusTub 的源文件。参考来源、许可证、固定版本、代码摘录、对应实现位置和改进点集中记录在：

- [开源参考与差异](docs/open-source-references.md)
- [源码对照录制卡](docs/source-code-comparison.md)

视频或答辩时，应同时说明“参考了什么”“本项目实现在哪里”“与原项目有什么取舍”，不要只列出项目名称。

## 文档导航

- [文档索引](docs/README.md)
- [架构与设计依据](docs/architecture.md)
- [本地 HTTP API](docs/api-reference.md)
- [开发与测试指南](docs/development.md)
- [开源参考与差异](docs/open-source-references.md)
- [源码对照录制卡](docs/source-code-comparison.md)
- [3 分钟视频录制指南](docs/video-recording-guide.md)
- [提交前检查清单](docs/submission-checklist.md)

## 当前边界

为了保持课程项目规模，当前版本明确不包含：WAL 和崩溃原子恢复、完整 B+Tree 删除再平衡、持久化 MVCC 版本链、行级锁和死锁图、完整 MySQL 兼容性、用户认证与权限控制。后续演进优先级见[架构文档](docs/architecture.md#后续优先级)。

## 许可证

本项目以 [MIT License](LICENSE) 发布。引用外部资料时，请同时保留其原始许可证和官方链接；本仓库不包含上游数据库源代码副本。
