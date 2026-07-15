# 开源参考与差异

本项目代码由小组自行实现，没有复制下列项目源码。参考内容限于数据库教材和官方资料中公开的
架构思想、磁盘布局原则与接口分层；在报告、视频或答辩中应按本文说明引用，避免笼统表述为
“参考了某数据库”。

## 参考来源

| 项目 | 来源 | 本项目借鉴内容 |
| --- | --- | --- |
| SQLite | [Database File Format](https://www.sqlite.org/fileformat2.html) | 固定大小页面、文件头、空闲页链表及格式兼容意识 |
| PostgreSQL | [Database Page Layout](https://www.postgresql.org/docs/current/storage-page-layout.html) | 页头、槽目录和 tuple 数据分离的 slotted page 思路 |
| MySQL InnoDB | [Clustered and Secondary Indexes](https://dev.mysql.com/doc/refman/8.4/en/innodb-index-types.html) | 主键 B+Tree、叶节点有序存储和二级索引概念 |
| CMU BusTub | [cmu-db/bustub](https://github.com/cmu-db/bustub) | 教学数据库的模块边界：存储、事务、执行器与公共接口分离 |

上述链接用于概念引用。项目依赖中不包含这些数据库的代码包，提交历史也不包含其源文件。

## 差异与改进

| 对比维度 | 参考项目 | 本项目实现 | 为课程项目做的改进或取舍 |
| --- | --- | --- | --- |
| 文件头 | SQLite 有完整 header、版本和 freelist 信息 | page 0 保存页号、catalog root、`KVDBPAGE` 魔数和版本 | 兼容旧版零魔数文件；打开时检查截断、越界与 freelist 环 |
| 空闲页 | SQLite 使用 freelist trunk/leaf | 空闲页前 8 字节组成单链表 | 实现跨重启复用并用测试覆盖多个页的 LIFO 恢复 |
| 数据页 | PostgreSQL 页面含 item identifier | `SlottedPage` 分离 slot 与 tuple | 保留核心布局，省略 MVCC tuple header 等生产字段 |
| 索引 | InnoDB 支持高并发、页分裂与完整恢复 | 教学型固定阶 B+Tree，叶节点链接支持范围扫描 | 同一接口承载表主键树、catalog 树和二级索引树 |
| 系统分层 | BusTub 是完整教学 DBMS | 六个 Rust crate 分离 common/storage/txn/sql/network/server | 使用 Rust trait 约束层间依赖，并提供 MySQL 协议与 Web 双入口 |
| 事务 | 成熟数据库具有 WAL、锁表和磁盘 MVCC | 写缓冲、事务管理、表锁和内存版本链 | 同时覆盖 commit、rollback 和事务内读己之写，明确不宣称完整 ACID 恢复 |
| 可展示性 | 原项目主要面向 CLI 或测试 | React 工作台显示 SQL、结果、schema、索引数和执行链路 | API 返回后端真实微秒耗时，支持一键清理演示数据 |

## 可复核代码位置

- 文件格式与 freelist：`crates/kv-storage/src/pager.rs`
- Slotted Page：`crates/kv-storage/src/page.rs`
- B+Tree 插入、查找、范围扫描：`crates/kv-storage/src/btree.rs`
- 缓冲池：`crates/kv-storage/src/buffer.rs`
- MVCC、锁与事务管理：`crates/kv-txn/src/`
- SQL 全链路：`crates/kv-sql/src/`
- MySQL 协议：`crates/kv-network/src/protocol.rs`
- 端到端对比验证：`test_protocol.py`

## 对比分析结论

本项目没有追求与 SQLite、PostgreSQL 或 InnoDB 的性能对等。生产数据库在 WAL、并发控制、
变长页面分裂、统计信息和崩溃恢复方面远强于本项目。本项目的区别在于用约 6 个边界清晰的
Rust crate 独立实现最小闭环，并通过 MySQL 客户端、Web 工作台和 87 项协议测试让每一层都可
运行、可观察、可验证。该取舍更符合 Rust 课程对所有权、trait、错误处理、并发和工程规范的
考查范围。
