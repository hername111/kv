//! 数据库各层之间的稳定接口。
use crate::error::KvResult;
use crate::types::{ColumnId, IndexId, IsolationLevel, ResultSet, Session, TableId, TableMeta};
use async_trait::async_trait;

/// SQL 执行器访问表、索引和目录的存储接口。
#[async_trait]
pub trait StorageEngine: Send + Sync {
    /// 创建表并返回全局唯一的表 ID。
    async fn create_table(&self, table_name: &str) -> KvResult<TableId>;

    /// 按主键插入或替换一行。
    async fn put(&self, table_id: TableId, key: &[u8], value: &[u8], txn_id: u64) -> KvResult<u64>;

    /// 读取事务可见的主键记录。
    async fn get(&self, table_id: TableId, key: &[u8], txn_id: u64) -> KvResult<Option<Vec<u8>>>;

    /// 扫描半开区间 `[start, end)`。
    async fn scan(
        &self,
        table_id: TableId,
        start: &[u8],
        end: &[u8],
        txn_id: u64,
    ) -> KvResult<Vec<(Vec<u8>, Vec<u8>)>>;

    /// 删除指定主键。
    async fn delete(&self, table_id: TableId, key: &[u8], txn_id: u64) -> KvResult<()>;

    /// 为指定表和列创建索引，并返回索引 ID。
    async fn create_index(&self, table_id: TableId, col_id: ColumnId) -> KvResult<IndexId>;

    /// 通过索引键查找主键列表。
    async fn index_lookup(
        &self,
        index_id: IndexId,
        key: &[u8],
        txn_id: u64,
    ) -> KvResult<Vec<Vec<u8>>>;

    /// 持久化表元数据。
    async fn save_table_meta(&self, _name: &str, _meta: &TableMeta) -> KvResult<()> {
        Ok(())
    }

    /// 加载所有持久化的表元数据。
    async fn load_all_table_meta(&self) -> KvResult<Vec<TableMeta>> {
        Ok(Vec::new())
    }

    /// 从 catalog 中删除表元数据。
    async fn delete_table_meta(&self, _name: &str) -> KvResult<()> {
        Ok(())
    }

    /// 获取表数据 B+Tree 的根页号。
    async fn get_table_root(&self, _table_id: TableId) -> KvResult<u64> {
        Ok(0)
    }

    /// 用已知根页号恢复表数据 B+Tree。
    async fn restore_table(&self, _table_id: TableId, _root_page_id: u64) -> KvResult<()> {
        Ok(())
    }

    /// 返回具体实现，供存储特有的维护操作使用。
    fn as_any(&self) -> &dyn std::any::Any;
}

/// 网络层调用的 SQL 命令接口。
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, sql: &str, session: &Session) -> KvResult<ResultSet>;
}

/// SQL 执行期间所需的事务快照信息。
pub trait TxnContext {
    fn txn_id(&self) -> u64;
    fn snapshot_version(&self) -> u64;
    fn isolation_level(&self) -> IsolationLevel;
}

/// 存储引擎使用的固定大小页面接口。
#[async_trait]
pub trait Pager: Send + Sync {
    /// 读取指定 4 KiB 页面。
    async fn read_page(&self, page_id: u64) -> KvResult<Vec<u8>>;

    /// 写入指定页面；不足一页时由实现决定是否补零。
    async fn write_page(&self, page_id: u64, data: &[u8]) -> KvResult<()>;

    /// 分配新页并返回页号。
    async fn allocate_page(&self) -> KvResult<u64>;

    /// 释放页，允许后续复用页号。
    async fn free_page(&self, page_id: u64) -> KvResult<()>;

    /// 将已写入数据刷新到底层介质。
    async fn flush(&self) -> KvResult<()>;

    /// 读取 superblock 中保存的目录树根页号。
    async fn get_meta_root(&self) -> KvResult<u64> {
        Ok(0)
    }

    /// 将目录树根页号写入 superblock。
    async fn set_meta_root(&self, _root: u64) -> KvResult<()> {
        Ok(())
    }
}
