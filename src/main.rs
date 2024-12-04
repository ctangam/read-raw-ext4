use byteorder::{LittleEndian, ReadBytesExt};
use hex_slice::AsHex;
use positioned_io::{Cursor, ReadAt, Slice};
use std::{fs::OpenOptions, vec};

use color_eyre::Result;
use custom_debug::Debug as CustomDebug;

struct Reader<IO> {
    inner: IO,
}

impl<IO: ReadAt> Reader<IO> {
    fn new(inner: IO) -> Self {
        Self { inner }
    }

    fn u16(&self, offset: u64) -> Result<u16> {
        let mut cursor = Cursor::new_pos(&self.inner, offset);
        Ok(cursor.read_u16::<LittleEndian>()?)
    }

    fn u32(&self, offset: u64) -> Result<u32> {
        let mut cursor = Cursor::new_pos(&self.inner, offset);
        Ok(cursor.read_u32::<LittleEndian>()?)
    }

    fn u64_lohi(&self, lo: u64, hi: u64) -> Result<u64> {
        Ok(self.u32(lo)? as u64 + ((self.u32(hi)? as u64) << 32))
    }
}

#[derive(CustomDebug)]
struct Superblock {
    #[debug(format = "{:x}")]
    magic: u16,
    block_size: u64,
    blocks_per_group: u64,
    inodes_per_group: u64,
    inode_size: u64,
}

impl Superblock {
    fn new(dev: &dyn ReadAt) -> Result<Self> {
        let r = Reader::new(Slice::new(dev, 1024, None));
        Ok(Self {
            magic: r.u16(0x38)?,
            block_size: (2u32.pow(10 + r.u32(0x18)?)) as u64,
            blocks_per_group: r.u32(0x20)? as u64,
            inodes_per_group: r.u32(0x28)? as u64,
            inode_size: r.u16(0x58)? as u64,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct BlockGroupDescriptor {
    inode_table: u64,
}

impl BlockGroupDescriptor {
    const SIZE: u64 = 64;

    fn new(slice: &dyn ReadAt) -> Result<Self> {
        let r = Reader::new(slice);
        Ok(Self {
            inode_table: r.u64_lohi(0x8, 0x28)?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct BlockGroupNumber(u64);

impl BlockGroupNumber {
    fn desc_slice<T>(self, sb: &Superblock, dev: T) -> Slice<T>
    where
        T: ReadAt,
    {
        assert!(sb.block_size != 1024, "1024 block size not supported");
        let gdt_start = sb.block_size;
        let offset = gdt_start + self.0 * BlockGroupDescriptor::SIZE;
        Slice::new(dev, offset, None)
    }

    fn desc(self, sb: &Superblock, dev: &dyn ReadAt) -> Result<BlockGroupDescriptor> {
        let slice = self.desc_slice(sb, dev);
        BlockGroupDescriptor::new(&slice)
    }
}

#[derive(Debug, Clone, Copy)]
struct InodeNumber(u64);

impl InodeNumber {
    fn blockgroup_number(self, sb: &Superblock) -> BlockGroupNumber {
        let n = (self.0 - 1) / sb.inodes_per_group;
        BlockGroupNumber(n)
    }
}

fn main() -> Result<()> {
    let file = OpenOptions::new()
        .read(true)
        .open("/dev/mapper/ubuntu--vg-ubuntu--lv")?;

    let sb = Superblock::new(&file)?;
    println!("{sb:#?}");

    let bgd = InodeNumber(2).blockgroup_number(&sb).desc(&sb, &file)?;
    println!("{bgd:#?}");

    Ok(())
}
