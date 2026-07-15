# 开源源码对照录制卡

这份材料服务于视频和答辩，不替代实验报告。视频中只展示下面标注的少量代码行，画面角落同时
保留项目名、版本、许可证和 permalink。不要把上游文件复制到本仓库，也不要声称本项目复用了
它们的实现。

## 统一引用规则

1. 先展示上游文件名、固定版本和许可证，再展示不超过 8-12 行的原码片段。
2. 立即切到本项目对应文件，指出相同的设计问题和不同的实现取舍。
3. 口头使用“借鉴结构思想”“对照实现”，不要使用“照搬”“完全兼容”或“生产级”。
4. 视频最后一帧或仓库文档保留完整 URL；报告中按同样版本和许可证引用。

## 1. PostgreSQL：Slotted Page

**固定来源**：PostgreSQL `REL_18_0`，`src/include/storage/bufpage.h`，PostgreSQL License。

原码片段（`PageHeaderData`，第 159-172 行）：

```c
typedef struct PageHeaderData
{
    PageXLogRecPtr pd_lsn;
    uint16         pd_checksum;
    uint16         pd_flags;
    LocationIndex  pd_lower;
    LocationIndex  pd_upper;
    LocationIndex  pd_special;
    uint16         pd_pagesize_version;
    TransactionId  pd_prune_xid;
    ItemIdData     pd_linp[FLEXIBLE_ARRAY_MEMBER];
} PageHeaderData;
```

官方链接：
`https://github.com/postgres/postgres/blob/REL_18_0/src/include/storage/bufpage.h#L159-L174`

本项目对应：`crates/kv-storage/src/page.rs` 的 `PageHeader`、`SlotEntry` 和 `SlottedPage`。

```rust
pub struct PageHeader {
    pub page_id: u64,
    pub tuple_count: u16,
    pub free_start: u16,
    pub free_end: u16,
    pub flags: u8,
}
```

**差异说明**：两者都让槽目录向前增长、tuple 数据从页尾向前增长；PostgreSQL 额外保存 LSN、
checksum、special space、prune XID 和 line pointer。本项目只保留课程所需字段，并在读取时检查
槽目录、tuple 偏移和页面边界；没有 PostgreSQL 的 WAL 和 MVCC 页面头。

**视频台词**：
“PostgreSQL 的 `PageHeaderData` 把空闲区上下界和 line pointer 放在页头；我们的 `PageHeader`
用 `free_start/free_end` 表达同一布局，再通过 `SlottedPage::validate_layout` 拒绝越界数据。
我们借鉴的是页面组织原则，不是复制 PostgreSQL 的页面代码。”

## 2. SQLite：Free-list 与损坏检查

**固定来源**：SQLite `version-3.50.4`，`src/btree.c`，SQLite 公有领域声明。

原码片段（`freePage2`，第 6781-6795 行）：

```c
if( iPage<2 || iPage>pBt->nPage ){
  return SQLITE_CORRUPT_BKPT;
}
/* Increment the free page count on pPage1 */
nFree = get4byte(&pPage1->aData[36]);
put4byte(&pPage1->aData[36], nFree+1);
```

官方链接：
`https://github.com/sqlite/sqlite/blob/version-3.50.4/src/btree.c#L6769-L6795`

本项目对应：`crates/kv-storage/src/pager.rs` 的 `DiskPager::free_page`、`open` 和 superblock
字段 `free_list_head`。

```rust
if page_id >= next_page_id {
    return Err(KvError::Internal(format!(
        "cannot free unallocated page {page_id}"
    )));
}
```

**差异说明**：SQLite 使用 page 1 的 freelist trunk/leaf、计数和事务日志；本项目使用每个空闲页
前 8 字节串成单链表，并在打开文件时检查页号范围和链表环。SQLite 的完整 journal/WAL、指针
映射和 vacuum 没有被实现；本项目增加了适合教学文件格式的魔数、版本和截断文件拒绝。

**视频台词**：
“SQLite 的 free-list 代码先验证页号，再更新空闲页计数；我们保留了这个防损坏原则，但把数据
结构缩小为 superblock 加页内单链表，并用跨重启测试验证释放的多个页面可以复用。”

## 3. MySQL InnoDB：索引页面头

**固定来源**：MySQL `mysql-8.4.0`，`storage/innobase/include/page0types.h`，GPLv2。

原码片段（第 53-105 行中的页面字段）：

```cpp
constexpr uint32_t PAGE_HEADER = FSEG_PAGE_DATA;
constexpr uint32_t PAGE_N_DIR_SLOTS = 0;
constexpr uint32_t PAGE_N_RECS = 16;
constexpr uint32_t PAGE_LEVEL = 26;
constexpr uint32_t PAGE_INDEX_ID = 28;
constexpr uint32_t PAGE_DATA = PAGE_HEADER + 36 + 2 * FSEG_HEADER_SIZE;
```

官方链接：
`https://github.com/mysql/mysql-server/blob/mysql-8.4.0/storage/innobase/include/page0types.h#L53-L105`

本项目对应：`crates/kv-storage/src/btree.rs` 的节点标记、键数量、键值数组和叶节点 `next`。

```rust
const FLAG_INTERNAL: u8 = 0;
const FLAG_LEAF: u8 = 1;
```

**差异说明**：InnoDB 页头包含目录槽数、记录数、层级和索引 ID，并围绕聚簇索引、压缩页、锁和
恢复提供完整实现。本项目只实现固定阶教学 B+Tree：内部节点保存 child page，叶节点按键排序并
链接范围扫描；超过 4KB 或损坏字段直接返回错误。

**视频台词**：
“InnoDB 的页面头把记录目录、层级和索引身份作为持久化字段；我们的节点格式更小，只有内部/叶
节点标记和 B+Tree 必需内容，重点是展示 Rust 所有权、页面编码和分裂逻辑，而不是复刻 InnoDB。”

## 4. BusTub：Buffer Pool 接口

**固定来源**：BusTub commit `f0d9e3753482d45f2b5919da1873684600b48508`，MIT License。

原码片段（`BufferPoolManager`，第 110-127 行）：

```cpp
class BufferPoolManager {
 public:
  auto NewPage() -> page_id_t;
  auto DeletePage(page_id_t page_id) -> bool;
  auto CheckedWritePage(page_id_t page_id, AccessType access_type)
      -> std::optional<WritePageGuard>;
  auto CheckedReadPage(page_id_t page_id, AccessType access_type)
      -> std::optional<ReadPageGuard>;
  auto FlushPage(page_id_t page_id) -> bool;
};
```

官方链接：
`https://github.com/cmu-db/bustub/blob/f0d9e3753482d45f2b5919da1873684600b48508/src/include/buffer/buffer_pool_manager.h#L110-L127`

本项目对应：`crates/kv-storage/src/buffer.rs` 的 `BufferPool`、`BufferedPager` 和 `Pager` trait。

**差异说明**：BusTub 用 page guard、pin count、读写锁和 replacer 支撑并发 buffer manager；本项目
使用 Mutex 保护的 LRU 页面副本和写穿 Pager，释放页时显式失效缓存。两者都把页面缓存从 B+Tree
中隔离，但本项目不声称具有 BusTub 的并发保护和淘汰策略完整度。

**视频台词**：
“BusTub 把 New/Delete/Read/Write/Flush 组织为 BufferPoolManager 接口；我们用 Rust trait `Pager`
把同样的职责拆成读写页、分配页、释放页和刷盘，并额外测试页号复用不会读到旧缓存。”

## 5. 40 秒源码对比镜头

按这个顺序录制，不要现场搜索：

1. 浏览器标签 1：PostgreSQL 链接，放大 `PageHeaderData` 8 行；标签 2：本项目 `page.rs` 的 `PageHeader`。
2. 浏览器标签 3：SQLite `freePage2` 的页号检查；标签 4：本项目 `pager.rs` 的 `free_page`。
3. 浏览器标签 5：InnoDB `PAGE_LEVEL/PAGE_INDEX_ID`；标签 6：本项目 `btree.rs` 的节点编码。
4. 浏览器标签 7：BusTub BufferPoolManager 方法；标签 8：本项目 `buffer.rs` 的 Pager 实现。
5. 每组只说“共同问题、不同实现、我们增加的测试/限制”，每组不超过 8 秒；最后停在本文件的许可证表。

四个来源的许可证必须在画面或报告中同时出现：SQLite 公有领域声明、PostgreSQL License、
MySQL GPLv2、BusTub MIT。引用原码只用于课程批评性对比，不把任何上游文件加入本项目分发物。
