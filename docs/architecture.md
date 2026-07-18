# 系统架构

KV Database 采用 Rust workspace 组织后端模块，前端工作台通过 HTTP API 访问同一个 SQL 执行器。

```text
MySQL CLI --> kv-network --+
                           +--> kv-sql --> kv-txn --> kv-storage --> kv.db
Web UI ----> HTTP API -----+
```

## 模块划分

| 模块 | 职责 |
| --- | --- |
| `kv-common` | 共享类型、错误定义和 `Pager` trait |
| `kv-storage` | 4 KiB 页面、磁盘 Pager、slotted page、缓冲池、B+Tree、catalog |
| `kv-txn` | 事务生命周期、写缓冲、MVCC 可见性、表级锁 |
| `kv-sql` | Lexer、Parser、Planner、Executor |
| `kv-network` | MySQL Wire Protocol、TCP 连接处理、结果集编码 |
| `kv-server` | 服务组装、本地 HTTP API、演示状态查询 |
| `demo-client` | React + Vite Web 工作台 |

## 请求链路

SQL 文本进入系统后依次经过：

1. Lexer 生成 token。
2. Parser 构建 AST。
3. Planner 生成执行计划。
4. Executor 调用事务层。
5. 事务层处理写缓冲、提交、回滚和可见性。
6. 存储层通过 B+Tree、catalog 和 Pager 访问磁盘页。

## 数据库文件

数据库文件由固定 4096 字节页面组成。page 0 是 superblock，保存：

- `next_page_id`
- `free_list_head`
- `catalog_root`
- `KVDBPAGE` 魔数
- 文件格式版本

页面内部使用 slotted page 管理变长 tuple。B+Tree 节点基于页面存储，叶节点通过 `next` 指针支持顺序扫描。

## HTTP API

| 接口 | 作用 |
| --- | --- |
| `GET /api/state` | 返回表结构、索引数量、记录快照和执行状态 |
| `POST /api/query` | 执行 SQL 并返回结果、错误或执行耗时 |
| `POST /api/reset` | 回滚活动事务并清空演示表 |

HTTP API 面向本地开发和教学展示，没有认证和多用户隔离，不应暴露到公网。
