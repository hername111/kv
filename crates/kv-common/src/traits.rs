// 核心 trait 定义：StorageEngine, CommandHandler, TxnContext, Pager
use crate::error::KvResult;
use crate::types::{ColumnId, IndexId, IsolationLevel, ResultSet, Session, TableId, TableMeta};
use async_trait::async_trait;

/// 存储引擎抽象 — SQL 层和事务层依赖此 trait 访问数据
#[async_trait]
pub trait StorageEngine: Send + Sync {
    /// 创建表，返回 table_id
    async fn create_table(&self, table_name: &str) -> KvResult<TableId>;

    /// 插入或更新一行
    async fn put(&self, table_id: TableId, key: &[u8], value: &[u8], txn_id: u64) -> KvResult<u64>;

    /// 读取指定版本的可见数据
    async fn get(&self, table_id: TableId, key: &[u8], txn_id: u64) -> KvResult<Option<Vec<u8>>>;

    /// 范围扫描
    async fn scan(
        &self,
        table_id: TableId,
        start: &[u8],
        end: &[u8],
        txn_id: u64,
    ) -> KvResult<Vec<(Vec<u8>, Vec<u8>)>>;

    /// 标记删除
    async fn delete(&self, table_id: TableId, key: &[u8], txn_id: u64) -> KvResult<()>;

    /// 创建索引
    async fn create_index(&self, table_id: TableId, col_id: ColumnId) -> KvResult<IndexId>;

    /// 通过索引查找
    async fn index_lookup(
        &self,
        index_id: IndexId,
        key: &[u8],
        txn_id: u64,
    ) -> KvResult<Vec<Vec<u8>>>;

    /// 持久化表元数据
    async fn save_table_meta(&self, _name: &str, _meta: &TableMeta) -> KvResult<()> {
        Ok(())
    }
    /// 加载所有持久化的表元数据
    async fn load_all_table_meta(&self) -> KvResult<Vec<TableMeta>> {
        Ok(Vec::new())
    }
    /// 从 catalog 中删除表元数据
    async fn delete_table_meta(&self, _name: &str) -> KvResult<()> {
        Ok(())
    }

    /// 获取表的 B+Tree root page ID（用于持久化）
    async fn get_table_root(&self, _table_id: TableId) -> KvResult<u64> {
        Ok(0)
    }

    /// 用已知 root page ID 恢复表的 B+Tree
    async fn restore_table(&self, _table_id: TableId, _root_page_id: u64) -> KvResult<()> {
        Ok(())
    }

    /// 用于向下转型到具体实现
    fn as_any(&self) -> &dyn std::any::Any;
}

/// SQL 命令处理接口 — Network 层调用此 trait 执行 SQL
#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn execute(&self, sql: &str, session: &Session) -> KvResult<ResultSet>;
}

/// 事务上下文 — 携带当前事务的快照信息
pub trait TxnContext {
    fn txn_id(&self) -> u64;
    fn snapshot_version(&self) -> u64;
    fn isolation_level(&self) -> IsolationLevel;
}

/// 页管理器接口 — 存储引擎通过此 trait 读写磁盘页
#[async_trait]
pub trait Pager: Send + Sync {
    /// 读取指定页（4KB）
    async fn read_page(&self, page_id: u64) -> KvResult<Vec<u8>>;

    /// 写入指定页
    async fn write_page(&self, page_id: u64, data: &[u8]) -> KvResult<()>;

    /// 分配新页，返回页号
    async fn allocate_page(&self) -> KvResult<u64>;

    /// 释放页
    async fn free_page(&self, page_id: u64) -> KvResult<()>;

    /// 刷盘
    async fn flush(&self) -> KvResult<()>;

    /// 读取 superblock 中保存的 meta tree root page ID
    async fn get_meta_root(&self) -> KvResult<u64> {
        Ok(0)
    }

    /// 写入 meta tree root page ID 到 superblock
    async fn set_meta_root(&self, _root: u64) -> KvResult<()> {
        Ok(())
    }
}
