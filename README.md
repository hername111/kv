# KV Database

KV Database 是一个使用 Rust 实现的教学型关系数据库。项目包含 SQL 前端、事务管理、B+Tree
存储、缓冲池、磁盘页管理和 MySQL Wire Protocol，并提供 React 数据库工作台用于演示。

> 项目用于数据库原理学习与课程展示，不适合生产环境。

## 已实现能力

- SQL：`CREATE/DROP TABLE`、`CREATE INDEX`、`SELECT`、`INSERT`、`UPDATE`、`DELETE`
- 查询：`WHERE`、投影、`ORDER BY`、等值 `JOIN`
- 事务：`BEGIN`、`COMMIT`、`ROLLBACK`，包含写集、MVCC 可见性和表级锁
- 存储：4KB 页面、B+Tree、缓冲池、持久化 catalog、可恢复空闲页链表
- 接入：MySQL Wire Protocol TCP 服务和本地 HTTP 演示接口
- Web：SQL 编辑、执行链路、真实后端耗时、表结构、索引状态、记录与可重置演示环境

当前限制请查看[架构文档](docs/architecture.md#当前边界)。

## 快速开始

### 环境

- Rust 1.94.0（`rust-toolchain.toml` 会自动选择）
- Node.js 20+
- npm 10+
- 可选：MySQL CLI 或 Python 3

### 启动数据库

```powershell
cargo run -p kv-server
```

默认地址：

- MySQL 协议：`127.0.0.1:3307`
- 演示 API：`127.0.0.1:8080`
- 数据目录：`./kv_data`

可通过 `KV_ADDR`、`KV_DEMO_ADDR` 和 `KV_DATA_DIR` 环境变量覆盖。

### 启动 Web 工作台

```powershell
cd demo-client
npm ci
npm run dev
```

访问 `http://127.0.0.1:5173`。Vite 会把 `/api` 代理到演示 API。

### 使用 MySQL CLI

```powershell
mysql -h 127.0.0.1 -P 3307 -u root
```

## 验证

```powershell
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cd demo-client
npm ci
npm run build
```

完整协议测试会启动本地服务并使用独立测试数据目录：

```powershell
python test_protocol.py
```

## 仓库结构

```text
crates/
  kv-common/    共享类型、错误和跨层 trait
  kv-storage/   页面、Pager、缓冲池、B+Tree 和存储引擎
  kv-txn/       事务、MVCC 和锁管理
  kv-sql/       Lexer、Parser、Planner 和 Executor
  kv-network/   MySQL Wire Protocol 和 TCP 服务
  kv-server/    服务组装与演示 HTTP API
demo-client/    React + Vite 数据库工作台
docs/           架构与开发资料
test_protocol.py 端到端协议和持久化验证
```

更多资料：

- [文档索引](docs/README.md)
- [架构与设计依据](docs/architecture.md)
- [开发与测试指南](docs/development.md)
- [开源参考与差异](docs/open-source-references.md)
- [视频录制指南](docs/video-recording-guide.md)

## License

[MIT](LICENSE)
