# 开源参考说明

本项目参考公开数据库系统的设计思想和文档资料，项目代码采用独立实现。参考内容、固定版本和许可证如下。

| 项目 | 固定版本与许可证 | 参考内容 | 本项目对应实现 |
| --- | --- | --- | --- |
| SQLite | 3.50.4，Public Domain | 固定页、文件头、free-list | `crates/kv-storage/src/pager.rs` |
| PostgreSQL | `REL_18_0`，PostgreSQL License | slotted page 页面布局 | `crates/kv-storage/src/page.rs` |
| MySQL InnoDB | `mysql-8.4.0`，GPLv2 | B+Tree 索引页与叶节点组织 | `crates/kv-storage/src/btree.rs` |
| CMU BusTub | commit `f0d9e375...b48508`，MIT | 教学数据库分层与 BufferPoolManager 职责 | `crates/kv-storage/src/buffer.rs`、`kv-common` 的 `Pager` trait |

## 与参考项目的区别

- SQLite 使用成熟文件头和 page 1 free-list trunk/leaf 结构；本项目使用更小的 superblock 和空闲页单链表，并加入魔数、版本、页号范围和链表环检查。
- PostgreSQL 的页面头包含 LSN、checksum、special space、line pointer 等生产级字段；本项目保留课程项目所需的页头、槽目录和 tuple 数据区，重点校验页内边界。
- InnoDB 支持聚簇索引、二级索引、高并发访问和恢复机制；本项目实现固定阶 B+Tree、叶节点链表扫描和基本索引路径。
- BusTub 使用 page guard、pin count、锁和 replacer 管理页面生命周期；本项目用 Rust `Pager` trait 拆分读写、分配、释放和刷盘职责，并测试释放页后的缓存失效。

## 参考链接

- SQLite Database File Format: <https://www.sqlite.org/fileformat2.html>
- SQLite `src/btree.c`: <https://github.com/sqlite/sqlite/blob/version-3.50.4/src/btree.c>
- PostgreSQL Database Page Layout: <https://www.postgresql.org/docs/current/storage-page-layout.html>
- PostgreSQL `bufpage.h`: <https://github.com/postgres/postgres/blob/REL_18_0/src/include/storage/bufpage.h>
- MySQL InnoDB Index Types: <https://dev.mysql.com/doc/refman/8.4/en/innodb-index-types.html>
- MySQL `page0types.h`: <https://github.com/mysql/mysql-server/blob/mysql-8.4.0/storage/innobase/include/page0types.h>
- CMU BusTub: <https://github.com/cmu-db/bustub/tree/f0d9e3753482d45f2b5919da1873684600b48508>
