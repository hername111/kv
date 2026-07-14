# 架构与设计依据

本文描述当前代码，而不是早期开发计划。各层通过 `kv-common` 中的 trait 和共享类型连接。

## 请求路径

```text
MySQL CLI ──> kv-network ─┐
                         ├─> kv-sql ─> kv-txn ─> kv-storage ─> kv.db
Web UI ──> demo HTTP API ┘
```

1. `kv-network` 处理握手、命令包和结果集编码。
2. `kv-sql` 将 SQL 依次转换为 Token、AST 和执行计划，再执行计划节点。
3. `kv-txn` 管理事务状态、读写集、版本可见性和表级锁。
4. `kv-storage` 把表映射到 B+Tree，并通过缓冲池和 Pager 访问 4KB 磁盘页。
5. catalog 使用独立 B+Tree 保存表元数据，根页号记录在 superblock 中。

## 磁盘布局

数据库文件由固定 4096 字节页面组成。

```text
page 0: superblock
  0..8    next_page_id
  8..16   free_list_head
  16..24  catalog_root
  24..32  format magic (KVDBPAGE)
  32..36  format version

page N: B+Tree node or free-list node
```

空闲页采用页内单链表。释放页的前 8 字节保存前一个链表头，superblock 指向新头；重新分配
时从头部弹出。打开文件时会校验文件长度、格式版本、页号范围和链表环，避免把损坏数据继续
解释成页面。

旧版文件的 24..36 字节为零时按 legacy superblock 读取，下一次写 superblock 时升级格式标识。

## 设计参考

实现保持教学项目的规模，但采用成熟数据库中可验证的结构原则：

| 参考系统 | 借鉴点 | 本项目对应实现 |
| --- | --- | --- |
| SQLite | 固定页、文件头、freelist trunk 思路 | 固定 4KB 页、带魔数/版本的 superblock、页内空闲链 |
| PostgreSQL | slotted page、buffer manager 分层 | `page.rs` 的槽页和 `buffer.rs` 的缓存层 |
| InnoDB | 聚簇主键 B+Tree、叶节点链式范围扫描 | 每表 B+Tree、叶节点 `next` 指针 |
| BusTub | 清晰的存储/事务/执行器教学分层 | workspace crate 边界和 `kv-common` trait |

这里的“借鉴”是结构思想，不复制上述项目代码。实现和测试均在本仓库维护。

## 一致性与持久化

- `Pager::flush` 使用 `sync_data`，要求操作系统把数据同步到稳定存储。
- catalog 保存表 ID、根页和列定义，服务启动时恢复表 B+Tree。
- 事务执行器维护会话内事务 ID；MySQL 连接和本地 Web 演示会话均可跨语句保持事务。
- 缓冲池只在存储引擎入口创建一次，避免重复缓存同一 Pager。

## 当前边界

- 没有 WAL，因此进程在多页更新中途崩溃时不保证原子恢复。
- B+Tree 使用固定阶数，未实现按页面剩余空间动态分裂和完整删除再平衡。
- 二级索引为教学实现，非唯一键、增量维护和重启恢复仍需完善。
- MVCC 版本主要保存在内存中，尚未形成完整的磁盘版本链与垃圾回收。
- 锁粒度为表级，未实现等待队列和完整死锁图检测。
- MySQL Wire Protocol 只实现项目 SQL 子集所需的命令与类型。
- 演示 HTTP API 面向单用户本地展示，共享一个服务端会话，不应暴露到公网。

## 后续优先级

1. 引入带 LSN 的 WAL、checkpoint 和崩溃恢复测试。
2. 让 B+Tree 按字节占用分裂，所有解码路径返回可诊断的损坏错误。
3. 持久化二级索引根页，并在 `INSERT/UPDATE/DELETE` 中原子维护索引。
4. 将 MVCC 版本信息落盘，补充 vacuum/版本回收。
5. 用行锁与等待图替代超时式表锁。
