# 开发与测试指南

## 模块边界

- 公共数据结构或跨层接口放在 `kv-common`，修改 trait 时同步检查所有实现和测试替身。
- 页面、文件格式、B+Tree 与缓存只放在 `kv-storage`。
- SQL 语法变化应依次覆盖 lexer、parser、planner、executor。
- 事务语义放在 `kv-txn`，连接或协议状态放在 `kv-network`/`kv-server`。
- `demo-client` 只调用 `/api/state` 和 `/api/query`，不复制数据库规则。

## 常用命令

```powershell
# Rust 快速反馈
cargo test -p kv-storage
cargo test -p kv-sql

# 提交前完整检查
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets

# 前端
cd demo-client
npm ci
npm run build
```

`test_protocol.py` 覆盖真实 TCP 协议、DDL/DML/查询、事务、索引和服务重启后的持久化。
它使用临时数据目录，不应读取或修改 `./kv_data` 中的本地演示数据。

## 测试原则

- 文件格式变更必须包含旧格式兼容或明确迁移错误。
- 持久化测试必须执行“写入、关闭、重新打开、读取”的完整循环。
- SQL 功能至少包含 parser 单测和 executor/集成测试。
- 修复事务问题时同时验证事务内可见性、提交后可见性和回滚不可见性。
- 前端改动至少通过生产构建，并在桌面和移动视口检查溢出与交互。

## 生成文件

以下内容不进入版本控制：

- Rust 构建目录 `target/`
- npm 依赖 `demo-client/node_modules/`
- Vite 产物 `demo-client/dist/`
- 本地数据库 `kv_data/`
- 编辑器或本地代理配置 `.claude/`

依赖版本分别由 `Cargo.lock` 和 `demo-client/package-lock.json` 固定。
