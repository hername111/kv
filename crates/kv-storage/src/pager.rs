//! 数据库文件、superblock 和持久化空闲页链表。

use async_trait::async_trait;
use kv_common::error::{KvError, KvResult};
use kv_common::traits::Pager;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

const PAGE_SIZE: u64 = 4096;
const SUPERBLOCK_PAGE: u64 = 0;
const FORMAT_MAGIC: &[u8; 8] = b"KVDBPAGE";
const FORMAT_VERSION: u32 = 1;
const MAGIC_OFFSET: usize = 24;
const VERSION_OFFSET: usize = 32;

/// 将页号映射到数据库文件中固定 4 KiB 区域的页面管理器。
pub struct DiskPager {
    file: Mutex<std::fs::File>,
    free_pages: Mutex<Vec<u64>>,
    next_page_id: Mutex<u64>,
    meta_root_page: Mutex<u64>,
}

impl DiskPager {
    /// 打开或初始化数据库文件，并验证 superblock 与空闲页链表。
    pub fn open(path: impl AsRef<Path>) -> KvResult<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.as_ref())
            .map_err(KvError::Io)?;

        let file_len = file.metadata().map_err(KvError::Io)?.len();

        let (next_page_id, free_pages, meta_root) = if file_len == 0 {
            file.set_len(PAGE_SIZE).map_err(KvError::Io)?;
            let mut sb = vec![0u8; PAGE_SIZE as usize];
            sb[0..8].copy_from_slice(&1u64.to_le_bytes());
            sb[8..16].copy_from_slice(&0u64.to_le_bytes());
            sb[16..24].copy_from_slice(&0u64.to_le_bytes());
            sb[MAGIC_OFFSET..MAGIC_OFFSET + FORMAT_MAGIC.len()].copy_from_slice(FORMAT_MAGIC);
            sb[VERSION_OFFSET..VERSION_OFFSET + 4].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
            file.seek(SeekFrom::Start(0)).map_err(KvError::Io)?;
            file.write_all(&sb).map_err(KvError::Io)?;
            file.sync_data().map_err(KvError::Io)?;
            (1, Vec::new(), 0)
        } else {
            if file_len < PAGE_SIZE || file_len % PAGE_SIZE != 0 {
                return Err(KvError::Internal(format!(
                    "invalid database file length: {file_len}"
                )));
            }
            let mut sb = vec![0u8; PAGE_SIZE as usize];
            file.seek(SeekFrom::Start(0)).map_err(KvError::Io)?;
            file.read_exact(&mut sb).map_err(KvError::Io)?;
            let magic = &sb[MAGIC_OFFSET..MAGIC_OFFSET + FORMAT_MAGIC.len()];
            let legacy_header = magic.iter().all(|byte| *byte == 0);
            if !legacy_header && magic != FORMAT_MAGIC {
                return Err(KvError::Internal(
                    "invalid database superblock magic".to_string(),
                ));
            }
            if !legacy_header {
                let version = u32::from_le_bytes(
                    sb[VERSION_OFFSET..VERSION_OFFSET + 4]
                        .try_into()
                        .expect("version field has a fixed width"),
                );
                if version != FORMAT_VERSION {
                    return Err(KvError::Internal(format!(
                        "unsupported database format version: {version}"
                    )));
                }
            }
            let next_id = u64::from_le_bytes(sb[0..8].try_into().unwrap());
            let free_head = u64::from_le_bytes(sb[8..16].try_into().unwrap());
            let meta_root = u64::from_le_bytes(sb[16..24].try_into().unwrap());
            let page_count = file_len / PAGE_SIZE;
            if next_id == 0 || next_id > page_count {
                return Err(KvError::Internal(format!(
                    "invalid next page id {next_id} for {page_count} pages"
                )));
            }
            if meta_root >= next_id && meta_root != 0 {
                return Err(KvError::Internal(format!(
                    "invalid metadata root page id: {meta_root}"
                )));
            }

            let mut free_pages = Vec::new();
            let mut visited = HashSet::new();
            let mut current = free_head;
            while current != 0 {
                if current >= next_id {
                    return Err(KvError::Internal(format!(
                        "free-list page id out of range: {current}"
                    )));
                }
                if !visited.insert(current) {
                    return Err(KvError::Internal(format!(
                        "cycle detected in free-list at page {current}"
                    )));
                }
                free_pages.push(current);
                let mut page = vec![0u8; PAGE_SIZE as usize];
                let offset = current * PAGE_SIZE;
                file.seek(SeekFrom::Start(offset)).map_err(KvError::Io)?;
                file.read_exact(&mut page).map_err(KvError::Io)?;
                current = u64::from_le_bytes(page[0..8].try_into().unwrap());
            }
            // Vec 尾部对应链表头，使页面分配可以 O(1) 弹出。
            free_pages.reverse();
            (next_id, free_pages, meta_root)
        };

        Ok(DiskPager {
            file: Mutex::new(file),
            free_pages: Mutex::new(free_pages),
            next_page_id: Mutex::new(next_page_id),
            meta_root_page: Mutex::new(meta_root),
        })
    }

    fn read_page_sync(&self, page_id: u64) -> KvResult<Vec<u8>> {
        let mut file = self.file.lock().unwrap();
        let offset = page_id * PAGE_SIZE;
        file.seek(SeekFrom::Start(offset)).map_err(KvError::Io)?;
        let mut data = vec![0u8; PAGE_SIZE as usize];
        file.read_exact(&mut data).map_err(KvError::Io)?;
        Ok(data)
    }

    fn write_page_sync(&self, page_id: u64, data: &[u8]) -> KvResult<()> {
        let mut file = self.file.lock().unwrap();
        let offset = page_id * PAGE_SIZE;
        file.seek(SeekFrom::Start(offset)).map_err(KvError::Io)?;
        let mut buf = vec![0u8; PAGE_SIZE as usize];
        let len = data.len().min(PAGE_SIZE as usize);
        buf[..len].copy_from_slice(&data[..len]);
        file.write_all(&buf).map_err(KvError::Io)?;
        Ok(())
    }

    fn write_superblock(&self) -> KvResult<()> {
        let next_id = *self.next_page_id.lock().unwrap();
        let free_head = self.free_pages.lock().unwrap().last().copied().unwrap_or(0);
        let meta_root = *self.meta_root_page.lock().unwrap();
        let mut sb = vec![0u8; PAGE_SIZE as usize];
        sb[0..8].copy_from_slice(&next_id.to_le_bytes());
        sb[8..16].copy_from_slice(&free_head.to_le_bytes());
        sb[16..24].copy_from_slice(&meta_root.to_le_bytes());
        sb[MAGIC_OFFSET..MAGIC_OFFSET + FORMAT_MAGIC.len()].copy_from_slice(FORMAT_MAGIC);
        sb[VERSION_OFFSET..VERSION_OFFSET + 4].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
        self.write_page_sync(SUPERBLOCK_PAGE, &sb)
    }
}

#[async_trait]
impl Pager for DiskPager {
    async fn read_page(&self, page_id: u64) -> KvResult<Vec<u8>> {
        self.read_page_sync(page_id)
    }

    async fn write_page(&self, page_id: u64, data: &[u8]) -> KvResult<()> {
        self.write_page_sync(page_id, data)
    }

    async fn allocate_page(&self) -> KvResult<u64> {
        let mut free_pages = self.free_pages.lock().unwrap();
        if let Some(page_id) = free_pages.pop() {
            drop(free_pages);
            self.write_page_sync(page_id, &[])?;
            self.write_superblock()?;
            return Ok(page_id);
        }
        drop(free_pages);

        let mut next_id = self.next_page_id.lock().unwrap();
        let page_id = *next_id;
        *next_id += 1;
        drop(next_id);

        let file = self.file.lock().unwrap();
        let required = (page_id + 1) * PAGE_SIZE;
        let current = file.metadata().map_err(KvError::Io)?.len();
        if required > current {
            file.set_len(required).map_err(KvError::Io)?;
        }
        drop(file);

        self.write_superblock()?;
        Ok(page_id)
    }

    async fn free_page(&self, page_id: u64) -> KvResult<()> {
        if page_id == SUPERBLOCK_PAGE {
            return Err(KvError::Internal("cannot free superblock".to_string()));
        }
        let next_page_id = *self.next_page_id.lock().unwrap();
        if page_id >= next_page_id {
            return Err(KvError::Internal(format!(
                "cannot free unallocated page {page_id}"
            )));
        }
        let mut free_pages = self.free_pages.lock().unwrap();
        if free_pages.contains(&page_id) {
            return Err(KvError::Internal(format!("page {page_id} is already free")));
        }
        let previous_head = free_pages.last().copied().unwrap_or(0);
        let mut free_page = vec![0u8; PAGE_SIZE as usize];
        free_page[0..8].copy_from_slice(&previous_head.to_le_bytes());
        self.write_page_sync(page_id, &free_page)?;
        free_pages.push(page_id);
        drop(free_pages);
        self.write_superblock()?;
        Ok(())
    }

    async fn flush(&self) -> KvResult<()> {
        self.file.lock().unwrap().sync_data().map_err(KvError::Io)?;
        Ok(())
    }

    async fn get_meta_root(&self) -> KvResult<u64> {
        Ok(*self.meta_root_page.lock().unwrap())
    }

    async fn set_meta_root(&self, root: u64) -> KvResult<()> {
        let next_page_id = *self.next_page_id.lock().unwrap();
        if root >= next_page_id && root != 0 {
            return Err(KvError::Internal(format!(
                "metadata root page {root} is not allocated"
            )));
        }
        *self.meta_root_page.lock().unwrap() = root;
        self.write_superblock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disk_pager_allocate_read_write() {
        let dir = std::env::temp_dir().join("kv_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_disk_pager.db");
        let _ = std::fs::remove_file(&path);
        let pager = DiskPager::open(&path).unwrap();
        let pid = pager.allocate_page().await.unwrap();
        assert!(pid >= 1);
        let test_data = b"hello world!";
        pager.write_page(pid, test_data).await.unwrap();
        let data = pager.read_page(pid).await.unwrap();
        assert_eq!(&data[..12], test_data);
        pager.flush().await.unwrap();
        drop(pager);

        let pager2 = DiskPager::open(&path).unwrap();
        let data2 = pager2.read_page(pid).await.unwrap();
        assert_eq!(&data2[..12], test_data);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn test_disk_pager_free_and_reuse() {
        let dir = std::env::temp_dir().join("kv_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_disk_pager2.db");
        let _ = std::fs::remove_file(&path);
        let pager = DiskPager::open(&path).unwrap();
        let pid1 = pager.allocate_page().await.unwrap();
        let _pid2 = pager.allocate_page().await.unwrap();
        pager.free_page(pid1).await.unwrap();
        let pid3 = pager.allocate_page().await.unwrap();
        assert_eq!(pid3, pid1);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn test_free_list_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("free-list.db");
        let pager = DiskPager::open(&path).unwrap();
        let pages = [
            pager.allocate_page().await.unwrap(),
            pager.allocate_page().await.unwrap(),
            pager.allocate_page().await.unwrap(),
        ];
        for page_id in pages {
            pager.free_page(page_id).await.unwrap();
        }
        pager.flush().await.unwrap();
        drop(pager);

        let reopened = DiskPager::open(&path).unwrap();
        let reused = [
            reopened.allocate_page().await.unwrap(),
            reopened.allocate_page().await.unwrap(),
            reopened.allocate_page().await.unwrap(),
        ];
        assert_eq!(reused, [pages[2], pages[1], pages[0]]);
        assert!(reopened.free_page(reused[0]).await.is_ok());
        assert!(reopened.free_page(reused[0]).await.is_err());
    }

    #[test]
    fn test_rejects_truncated_database_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("truncated.db");
        std::fs::write(&path, b"not a database").unwrap();
        assert!(DiskPager::open(&path).is_err());
    }
}
