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

打开 <http://127.0.0.1:5173>。Vite 会将 `/api` 请求代理到 `127.0.0.1:8080`。工作台可以直接观察建表、写入、查询、事务回滚和索引效果。

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

提交前也可以使用原生 PowerShell 脚本一次执行并汇总上述检查：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-and-show-tests.ps1
```

其中协议/持久化测试会自动选择临时本地端口，不会占用或依赖演示服务默认使用的 `3307` 端口。

如果要从干净数据目录启动演示环境，先清空专用目录，再用同一个目录启动服务：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\show-kv-db.ps1 -ResetDemoData
$env:KV_DATA_DIR = "target/demo"
cargo run -p kv-server
```

数据库文件的只读页信息和 Web 工作台当前表数据可用以下命令查看：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\show-kv-db.ps1 -DbPath target/demo/kv.db
```

`show-kv-db.ps1` 默认还会读取 `http://127.0.0.1:8080/api/state`，所以在 Web 执行 SQL 后可以看到表名、字段和行数据。它不会写入数据库；如果启动前显示已有页面或表数据，说明
`target/demo/kv.db` 是之前演示留下的文件，先运行 `show-kv-db.ps1 -ResetDemoData` 后再启动服务。

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
docs/             使用、架构、测试和开源参考文档
report/           实验报告 LaTeX 源文件和最终 PDF
scripts/          验证汇总和数据库文件检查辅助脚本
test_protocol.py  端到端协议与持久化测试
```

## 架构与接口

```text
MySQL CLI --> kv-network --+
                           +--> kv-sql --> kv-txn --> kv-storage --> kv.db
Web UI ----> demo HTTP API-+
```

依赖关系从上到下保持单向：网络和 HTTP 层调用 SQL 执行器，执行器依赖事务与存储；存储层不依赖前端或网络协议。数据库文件由固定 4096 字节页面组成，page 0 的 superblock 保存 `next_page_id`、`free_list_head`、`catalog_root`、`KVDBPAGE` 魔数和格式版本。打开文件时会检查长度、版本、页号范围和空闲链表环。

本地工作台使用以下接口，均由 `kv-server` 提供：

| 接口 | 作用 | 主要限制 |
| --- | --- | --- |
| `GET /api/state` | 返回表结构、索引数量和记录快照 | 只用于本地演示 |
| `POST /api/query` | 执行一条 SQL，返回结果、状态和 `durationMicros` | JSON 请求体最大 64 KiB |
| `POST /api/reset` | 回滚活动事务并清空演示表 | 共享单个演示会话 |

对应实现位于 `crates/kv-server/src/demo_http.rs`。API 没有认证、权限控制或多用户隔离，不应暴露到公网。

## 开源参考与差异

本项目借鉴公开架构思想和接口组织方式，没有复制上游源码文件：

| 项目 | 固定版本与许可证 | 借鉴内容 | 本项目差异 |
| --- | --- | --- | --- |
| [SQLite](https://www.sqlite.org/fileformat2.html) | 3.50.4，Public Domain | 固定页、文件头、free-list | 使用更小的 superblock 和页内单链表，增加魔数、版本、截断与链表环检查 |
| [PostgreSQL](https://www.postgresql.org/docs/current/storage-page-layout.html) | `REL_18_0`，PostgreSQL License | slotted page | 仅保留教学字段，集中校验槽目录和 tuple 边界，不实现 WAL 页头 |
| [MySQL InnoDB](https://dev.mysql.com/doc/refman/8.4/en/innodb-index-types.html) | `mysql-8.4.0`，GPLv2 | B+Tree 索引页与叶节点组织 | 固定阶教学 B+Tree，叶链支持扫描，超页或损坏字段直接报错 |
| [CMU BusTub](https://github.com/cmu-db/bustub/tree/f0d9e3753482d45f2b5919da1873684600b48508) | commit `f0d9e375...b48508`，MIT | 教学数据库分层与 BufferPoolManager 职责 | 使用 Rust `Pager` trait、Mutex 和 LRU，并测试释放页后的缓存失效 |

## 必要文档

- [文档索引](docs/README.md)：使用、架构、测试和开源参考文档入口。
- [快速上手](docs/getting-started.md)：启动服务、运行工作台、执行 SQL。
- [系统架构](docs/architecture.md)：模块划分、请求链路、数据库文件和 HTTP API。
- [测试与验证](docs/testing.md)：质量门禁、协议测试和数据库文件检查。
- [开源参考说明](docs/open-source-references.md)：参考来源、许可证、差异和边界。
- [实验报告 PDF](report/main.pdf)：课程提交版报告。
- [实验报告 LaTeX 源文件](report/main.tex)：报告的可维护源文件。

## 开发约定

- 页面、文件格式、B+Tree 与缓存放在 `kv-storage`；事务语义放在 `kv-txn`。
- SQL 语法变更应同时覆盖 lexer、parser、planner 和 executor。
- `demo-client` 只消费后端 API，不复制 SQL 或事务规则。
- 文件格式变更必须补充兼容或明确迁移错误；持久化测试必须包含关闭和重新打开。
- 网络和磁盘输入必须有边界检查，并测试分片、截断、超限或损坏路径。
- `target/`、`node_modules/`、`dist/`、`kv_data/` 和 LaTeX 中间文件不进入版本控制。

## 当前边界

为了保持课程项目规模，当前版本明确不包含：WAL 和崩溃原子恢复、完整 B+Tree 删除再平衡、持久化 MVCC 版本链、行级锁和死锁图、完整 MySQL 兼容性、用户认证与权限控制。

## 许可证

本项目以 [MIT License](LICENSE) 发布。引用外部资料时，请同时保留其原始许可证和官方链接；本仓库不包含上游数据库源代码副本。
