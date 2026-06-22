// Slotted Page 布局 (4KB)：页头 + 槽目录 + 元组数据区
use std::io::{self, Error, ErrorKind};

pub const PAGE_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageHeader {
    pub page_id: u64,
    pub tuple_count: u16,
    pub free_start: u16,
    pub free_end: u16,
    pub flags: u8,
}

impl PageHeader {
    pub const SIZE: usize = 16;

    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..8].copy_from_slice(&self.page_id.to_le_bytes());
        buf[8..10].copy_from_slice(&self.tuple_count.to_le_bytes());
        buf[10..12].copy_from_slice(&self.free_start.to_le_bytes());
        buf[12..14].copy_from_slice(&self.free_end.to_le_bytes());
        buf[14] = self.flags;
        buf
    }

    pub fn decode(data: &[u8]) -> Self {
        let page_id = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let tuple_count = u16::from_le_bytes(data[8..10].try_into().unwrap());
        let free_start = u16::from_le_bytes(data[10..12].try_into().unwrap());
        let free_end = u16::from_le_bytes(data[12..14].try_into().unwrap());
        let flags = data[14];
        PageHeader {
            page_id,
            tuple_count,
            free_start,
            free_end,
            flags,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlotEntry {
    pub offset: u16,
    pub length: u16,
    pub flags: u8,
}

impl SlotEntry {
    pub const SIZE: usize = 5;

    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..2].copy_from_slice(&self.offset.to_le_bytes());
        buf[2..4].copy_from_slice(&self.length.to_le_bytes());
        buf[4] = self.flags;
        buf
    }

    pub fn decode(data: &[u8]) -> Self {
        let offset = u16::from_le_bytes(data[0..2].try_into().unwrap());
        let length = u16::from_le_bytes(data[2..4].try_into().unwrap());
        let flags = data[4];
        SlotEntry {
            offset,
            length,
            flags,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SlottedPage {
    pub data: [u8; PAGE_SIZE],
}

impl Default for SlottedPage {
    fn default() -> Self {
        Self::new(0)
    }
}

impl SlottedPage {
    pub fn new(page_id: u64) -> Self {
        let mut page = SlottedPage {
            data: [0u8; PAGE_SIZE],
        };
        let header = PageHeader {
            page_id,
            tuple_count: 0,
            free_start: PageHeader::SIZE as u16,
            free_end: PAGE_SIZE as u16,
            flags: 0,
        };
        let encoded = header.encode();
        page.data[..PageHeader::SIZE].copy_from_slice(&encoded);
        page
    }

    pub fn from_bytes(data: &[u8]) -> io::Result<Self> {
        if data.len() != PAGE_SIZE {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "page must be 4096 bytes",
            ));
        }
        let mut arr = [0u8; PAGE_SIZE];
        arr.copy_from_slice(data);
        Ok(SlottedPage { data: arr })
    }

    pub fn header(&self) -> PageHeader {
        PageHeader::decode(&self.data[..PageHeader::SIZE])
    }

    fn set_header(&mut self, header: &PageHeader) {
        let encoded = header.encode();
        self.data[..PageHeader::SIZE].copy_from_slice(&encoded);
    }

    pub fn free_space(&self) -> u16 {
        let h = self.header();
        h.free_end - h.free_start - h.tuple_count * SlotEntry::SIZE as u16
    }

    pub fn insert(&mut self, tuple: &[u8]) -> io::Result<u16> {
        let mut h = self.header();
        let required = tuple.len() as u16 + SlotEntry::SIZE as u16;
        if self.free_space() < required {
            return Err(Error::new(ErrorKind::OutOfMemory, "page full"));
        }

        let new_end = h.free_end - tuple.len() as u16;
        self.data[new_end as usize..h.free_end as usize].copy_from_slice(tuple);

        let slot = SlotEntry {
            offset: new_end,
            length: tuple.len() as u16,
            flags: 0,
        };
        let slot_offset = h.free_start as usize + h.tuple_count as usize * SlotEntry::SIZE;
        let slot_enc = slot.encode();
        self.data[slot_offset..slot_offset + SlotEntry::SIZE].copy_from_slice(&slot_enc);

        h.tuple_count += 1;
        h.free_end = new_end;
        self.set_header(&h);
        Ok(h.tuple_count - 1)
    }

    pub fn get(&self, slot_idx: u16) -> io::Result<&[u8]> {
        let h = self.header();
        if slot_idx >= h.tuple_count {
            return Err(Error::new(ErrorKind::NotFound, "slot index out of range"));
        }
        let slot_off = h.free_start as usize + slot_idx as usize * SlotEntry::SIZE;
        let slot = SlotEntry::decode(&self.data[slot_off..slot_off + SlotEntry::SIZE]);
        let start = slot.offset as usize;
        let end = start + slot.length as usize;
        Ok(&self.data[start..end])
    }

    pub fn iter_tuples(&self) -> SlotIter<'_> {
        SlotIter {
            page: self,
            current: 0,
        }
    }
}

pub struct SlotIter<'a> {
    page: &'a SlottedPage,
    current: u16,
}

impl<'a> Iterator for SlotIter<'a> {
    type Item = (u16, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let h = self.page.header();
        if self.current >= h.tuple_count {
            return None;
        }
        let idx = self.current;
        self.current += 1;
        let result = self.page.get(idx).ok()?;
        Some((idx, result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_page_free_space() {
        let page = SlottedPage::new(42);
        assert_eq!(page.header().page_id, 42);
        assert!(page.free_space() > 4000);
    }

    #[test]
    fn test_insert_and_get() {
        let mut page = SlottedPage::new(1);
        let tuple = b"hello world";
        let slot_id = page.insert(tuple).unwrap();
        assert_eq!(slot_id, 0);
        let read = page.get(0).unwrap();
        assert_eq!(read, tuple);
    }

    #[test]
    fn test_multiple_inserts() {
        let mut page = SlottedPage::new(1);
        for i in 0u8..10u8 {
            let data = vec![i; 10];
            page.insert(&data).unwrap();
        }
        assert_eq!(page.header().tuple_count, 10);
        let read = page.get(5).unwrap();
        assert_eq!(read, &[5u8; 10]);
    }

    #[test]
    fn test_iter() {
        let mut page = SlottedPage::new(1);
        page.insert(b"aaa").unwrap();
        page.insert(b"bbb").unwrap();
        page.insert(b"ccc").unwrap();
        let items: Vec<_> = page.iter_tuples().map(|(_, d)| d.to_vec()).collect();
        assert_eq!(
            items,
            vec![b"aaa".to_vec(), b"bbb".to_vec(), b"ccc".to_vec()]
        );
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let mut page = SlottedPage::new(99);
        page.insert(b"test data").unwrap();
        let bytes = page.data.to_vec();
        let restored = SlottedPage::from_bytes(&bytes).unwrap();
        assert_eq!(restored.header().page_id, 99);
        assert_eq!(restored.header().tuple_count, 1);
        assert_eq!(restored.get(0).unwrap(), b"test data");
    }
}
