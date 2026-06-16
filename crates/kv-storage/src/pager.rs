use async_trait::async_trait;
use kv_common::error::{KvError, KvResult};
use kv_common::traits::Pager;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Mutex;

const PAGE_SIZE: u64 = 4096;
const SUPERBLOCK_PAGE: u64 = 0;

pub struct DiskPager {
    file: Mutex<std::fs::File>,
    free_pages: Mutex<Vec<u64>>,
    next_page_id: Mutex<u64>,
}

impl DiskPager {
    pub fn open(path: impl AsRef<Path>) -> KvResult<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path.as_ref())
            .map_err(|e| KvError::Io(e))?;

        let file_len = file.metadata().map_err(|e| KvError::Io(e))?.len();

        let (next_page_id, free_pages) = if file_len == 0 {
            file.set_len(PAGE_SIZE).map_err(|e| KvError::Io(e))?;
            let mut sb = vec![0u8; PAGE_SIZE as usize];
            sb[0..8].copy_from_slice(&1u64.to_le_bytes());
            sb[8..16].copy_from_slice(&0u64.to_le_bytes());
            file.seek(SeekFrom::Start(0)).map_err(|e| KvError::Io(e))?;
            file.write_all(&sb).map_err(|e| KvError::Io(e))?;
            file.flush().map_err(|e| KvError::Io(e))?;
            (1, Vec::new())
        } else {
            let mut sb = vec![0u8; PAGE_SIZE as usize];
            file.seek(SeekFrom::Start(0)).map_err(|e| KvError::Io(e))?;
            file.read_exact(&mut sb).map_err(|e| KvError::Io(e))?;
            let next_id = u64::from_le_bytes(sb[0..8].try_into().unwrap());
            let free_head = u64::from_le_bytes(sb[8..16].try_into().unwrap());

            let mut free_pages = Vec::new();
            let mut current = free_head;
            while current != 0 {
                free_pages.push(current);
                let mut page = vec![0u8; PAGE_SIZE as usize];
                let offset = current * PAGE_SIZE;
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| KvError::Io(e))?;
                file.read_exact(&mut page).map_err(|e| KvError::Io(e))?;
                current = u64::from_le_bytes(page[0..8].try_into().unwrap());
            }
            (next_id, free_pages)
        };

        Ok(DiskPager {
            file: Mutex::new(file),
            free_pages: Mutex::new(free_pages),
            next_page_id: Mutex::new(next_page_id),
        })
    }

    fn read_page_sync(&self, page_id: u64) -> KvResult<Vec<u8>> {
        let mut file = self.file.lock().unwrap();
        let offset = page_id * PAGE_SIZE;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| KvError::Io(e))?;
        let mut data = vec![0u8; PAGE_SIZE as usize];
        file.read_exact(&mut data).map_err(|e| KvError::Io(e))?;
        Ok(data)
    }

    fn write_page_sync(&self, page_id: u64, data: &[u8]) -> KvResult<()> {
        let mut file = self.file.lock().unwrap();
        let offset = page_id * PAGE_SIZE;
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| KvError::Io(e))?;
        let mut buf = vec![0u8; PAGE_SIZE as usize];
        let len = data.len().min(PAGE_SIZE as usize);
        buf[..len].copy_from_slice(&data[..len]);
        file.write_all(&buf).map_err(|e| KvError::Io(e))?;
        Ok(())
    }

    fn write_superblock(&self) -> KvResult<()> {
        let next_id = *self.next_page_id.lock().unwrap();
        let free_head = self
            .free_pages
            .lock()
            .unwrap()
            .first()
            .copied()
            .unwrap_or(0);
        let mut sb = vec![0u8; PAGE_SIZE as usize];
        sb[0..8].copy_from_slice(&next_id.to_le_bytes());
        sb[8..16].copy_from_slice(&free_head.to_le_bytes());
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
            self.write_superblock()?;
            return Ok(page_id);
        }
        drop(free_pages);

        let mut next_id = self.next_page_id.lock().unwrap();
        let page_id = *next_id;
        *next_id += 1;
        drop(next_id);

        let mut file = self.file.lock().unwrap();
        let required = (page_id + 1) * PAGE_SIZE;
        let current = file.metadata().map_err(|e| KvError::Io(e))?.len();
        if required > current {
            file.set_len(required).map_err(|e| KvError::Io(e))?;
        }
        drop(file);

        self.write_superblock()?;
        Ok(page_id)
    }

    async fn free_page(&self, page_id: u64) -> KvResult<()> {
        if page_id == SUPERBLOCK_PAGE {
            return Err(KvError::Internal("cannot free superblock".to_string()));
        }
        // Wipe the page
        self.write_page_sync(page_id, &vec![0u8; PAGE_SIZE as usize])?;
        self.free_pages.lock().unwrap().push(page_id);
        self.write_superblock()?;
        Ok(())
    }

    async fn flush(&self) -> KvResult<()> {
        self.file
            .lock()
            .unwrap()
            .flush()
            .map_err(|e| KvError::Io(e))?;
        Ok(())
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
}
