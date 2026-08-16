mod ata;

use ata::{AtaPio, SECTOR_SIZE};
use spin::Mutex;

pub const MAX_NODES: usize = 64;
pub const MAX_NAME_LEN: usize = 32;
pub const MAX_FILE_SIZE: usize = 460;
pub const MAX_PATH_LEN: usize = 128;

pub const FS_MAGIC: &[u8; 8] = b"NOVAFS01";
pub const FS_VERSION: u32 = 1;
pub const SUPERBLOCK_LBA: u32 = 4096;
pub const INODE_TABLE_START_LBA: u32 = 4097;
pub const TOTAL_FS_BLOCKS: u32 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    AlreadyExists,
    NotADirectory,
    IsADirectory,
    DirectoryNotEmpty,
    CannotRemoveRoot,
    DirectoryInUse,
    StorageFull,
    NameTooLong,
    FileTooLarge,
    InvalidPath,
    DiskUnavailable,
    IoError,
}

#[derive(Clone, Copy)]
pub struct Node {
    pub is_used: bool,
    pub kind: NodeKind,
    pub parent: usize,
    pub name: [u8; MAX_NAME_LEN],
    pub name_len: usize,
    pub content: [u8; MAX_FILE_SIZE],
    pub size: usize,
}

impl Node {
    pub const fn empty() -> Self {
        Self {
            is_used: false,
            kind: NodeKind::File,
            parent: 0,
            name: [0; MAX_NAME_LEN],
            name_len: 0,
            content: [0; MAX_FILE_SIZE],
            size: 0,
        }
    }

    pub fn name(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }

    pub fn set_name(&mut self, name: &str) -> Result<(), FsError> {
        let bytes = name.as_bytes();
        if bytes.is_empty() || bytes.len() > MAX_NAME_LEN {
            return Err(FsError::NameTooLong);
        }
        self.name[..bytes.len()].copy_from_slice(bytes);
        self.name_len = bytes.len();
        Ok(())
    }

    pub fn content_str(&self) -> &str {
        core::str::from_utf8(&self.content[..self.size]).unwrap_or("")
    }

    pub fn serialize(&self) -> [u8; SECTOR_SIZE] {
        let mut buf = [0u8; SECTOR_SIZE];
        buf[0] = if self.is_used { 1 } else { 0 };
        buf[1] = match self.kind {
            NodeKind::File => 0,
            NodeKind::Directory => 1,
        };

        let parent_bytes = (self.parent as u32).to_le_bytes();
        buf[2..6].copy_from_slice(&parent_bytes);

        let name_len_bytes = (self.name_len as u32).to_le_bytes();
        buf[6..10].copy_from_slice(&name_len_bytes);

        buf[10..10 + MAX_NAME_LEN].copy_from_slice(&self.name);

        let size_bytes = (self.size as u32).to_le_bytes();
        buf[42..46].copy_from_slice(&size_bytes);

        buf[46..46 + MAX_FILE_SIZE].copy_from_slice(&self.content);

        buf
    }

    pub fn deserialize(buf: &[u8; SECTOR_SIZE]) -> Option<Self> {
        let is_used = buf[0] == 1;
        let kind = match buf[1] {
            0 => NodeKind::File,
            1 => NodeKind::Directory,
            _ => return None,
        };

        let parent = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]) as usize;
        let name_len = u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]) as usize;

        if name_len > MAX_NAME_LEN {
            return None;
        }

        let mut name = [0u8; MAX_NAME_LEN];
        name.copy_from_slice(&buf[10..10 + MAX_NAME_LEN]);

        let size = u32::from_le_bytes([buf[42], buf[43], buf[44], buf[45]]) as usize;
        if size > MAX_FILE_SIZE {
            return None;
        }

        let mut content = [0u8; MAX_FILE_SIZE];
        content.copy_from_slice(&buf[46..46 + MAX_FILE_SIZE]);

        Some(Self {
            is_used,
            kind,
            parent,
            name,
            name_len,
            content,
            size,
        })
    }
}

#[derive(Clone, Copy)]
pub struct Superblock {
    pub magic: [u8; 8],
    pub version: u32,
    pub block_size: u32,
    pub total_blocks: u32,
    pub start_lba: u32,
    pub total_inodes: u32,
    pub used_inodes: u32,
    pub used_blocks: u32,
}

impl Superblock {
    pub const fn new() -> Self {
        Self {
            magic: *FS_MAGIC,
            version: FS_VERSION,
            block_size: SECTOR_SIZE as u32,
            total_blocks: TOTAL_FS_BLOCKS,
            start_lba: SUPERBLOCK_LBA,
            total_inodes: MAX_NODES as u32,
            used_inodes: 1, // Root node
            used_blocks: 2, // Superblock + Root inode
        }
    }

    pub fn serialize(&self) -> [u8; SECTOR_SIZE] {
        let mut buf = [0u8; SECTOR_SIZE];
        buf[0..8].copy_from_slice(&self.magic);
        buf[8..12].copy_from_slice(&self.version.to_le_bytes());
        buf[12..16].copy_from_slice(&self.block_size.to_le_bytes());
        buf[16..20].copy_from_slice(&self.total_blocks.to_le_bytes());
        buf[20..24].copy_from_slice(&self.start_lba.to_le_bytes());
        buf[24..28].copy_from_slice(&self.total_inodes.to_le_bytes());
        buf[28..32].copy_from_slice(&self.used_inodes.to_le_bytes());
        buf[32..36].copy_from_slice(&self.used_blocks.to_le_bytes());
        buf
    }

    pub fn deserialize(buf: &[u8; SECTOR_SIZE]) -> Option<Self> {
        if &buf[0..8] != FS_MAGIC {
            return None;
        }

        let version = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
        if version != FS_VERSION {
            return None;
        }

        let block_size = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let total_blocks = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
        let start_lba = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
        let total_inodes = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
        let used_inodes = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
        let used_blocks = u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);

        Some(Self {
            magic: *FS_MAGIC,
            version,
            block_size,
            total_blocks,
            start_lba,
            total_inodes,
            used_inodes,
            used_blocks,
        })
    }
}

pub struct FsInfo {
    pub fs_type: &'static str,
    pub version: u32,
    pub block_size: u32,
    pub total_blocks: u32,
    pub used_blocks: u32,
    pub free_blocks: u32,
    pub total_inodes: u32,
    pub used_inodes: u32,
    pub free_inodes: u32,
    pub is_persistent: bool,
}

pub struct FileSystem {
    nodes: [Node; MAX_NODES],
    cwd: usize,
    superblock: Superblock,
    is_disk_persistent: bool,
}

impl FileSystem {
    pub const fn new() -> Self {
        let mut nodes = [const { Node::empty() }; MAX_NODES];
        // Root directory '/' is node 0
        nodes[0].is_used = true;
        nodes[0].kind = NodeKind::Directory;
        nodes[0].parent = 0;
        nodes[0].name_len = 0;

        Self {
            nodes,
            cwd: 0,
            superblock: Superblock::new(),
            is_disk_persistent: false,
        }
    }

    pub fn init_storage(&mut self) -> bool {
        if !AtaPio::is_available() {
            self.is_disk_persistent = false;
            return false;
        }

        let mut sector_buf = [0u8; SECTOR_SIZE];
        if AtaPio::read_sector(SUPERBLOCK_LBA, &mut sector_buf).is_ok() {
            if let Some(sb) = Superblock::deserialize(&sector_buf) {
                // Detected existing NovaFS superblock, mount it!
                self.superblock = sb;
                let mut loaded_all = true;

                for i in 0..MAX_NODES {
                    let inode_lba = INODE_TABLE_START_LBA + i as u32;
                    let mut node_buf = [0u8; SECTOR_SIZE];
                    if AtaPio::read_sector(inode_lba, &mut node_buf).is_ok() {
                        if let Some(node) = Node::deserialize(&node_buf) {
                            self.nodes[i] = node;
                        } else {
                            loaded_all = false;
                            break;
                        }
                    } else {
                        loaded_all = false;
                        break;
                    }
                }

                if loaded_all && self.nodes[0].is_used && self.nodes[0].kind == NodeKind::Directory {
                    self.is_disk_persistent = true;
                    self.cwd = 0;
                    self.ensure_default_system_files();
                    return true;
                }
            }
        }

        // Format and create fresh NovaFS on disk
        let _ = self.format_disk();
        self.ensure_default_system_files();
        self.is_disk_persistent
    }

    pub fn format_disk(&mut self) -> Result<(), FsError> {
        if !AtaPio::is_available() {
            return Err(FsError::DiskUnavailable);
        }

        self.nodes = [const { Node::empty() }; MAX_NODES];
        self.nodes[0].is_used = true;
        self.nodes[0].kind = NodeKind::Directory;
        self.nodes[0].parent = 0;
        self.nodes[0].name_len = 0;
        self.cwd = 0;

        self.superblock = Superblock::new();
        self.update_usage_counts();

        // Write superblock
        let sb_buf = self.superblock.serialize();
        if AtaPio::write_sector(SUPERBLOCK_LBA, &sb_buf).is_err() {
            return Err(FsError::IoError);
        }

        // Write root inode
        let root_buf = self.nodes[0].serialize();
        if AtaPio::write_sector(INODE_TABLE_START_LBA, &root_buf).is_err() {
            return Err(FsError::IoError);
        }

        // Clear remaining inode sectors
        let empty_buf = [0u8; SECTOR_SIZE];
        for i in 1..MAX_NODES {
            let lba = INODE_TABLE_START_LBA + i as u32;
            if AtaPio::write_sector(lba, &empty_buf).is_err() {
                return Err(FsError::IoError);
            }
        }

        self.is_disk_persistent = true;
        Ok(())
    }

    fn update_usage_counts(&mut self) {
        let mut used_inodes = 0;
        for node in &self.nodes {
            if node.is_used {
                used_inodes += 1;
            }
        }
        self.superblock.used_inodes = used_inodes;
        self.superblock.used_blocks = 1 + used_inodes; // Superblock + 1 block per inode
    }

    fn sync_node(&mut self, idx: usize) -> Result<(), FsError> {
        if !self.is_disk_persistent {
            return Ok(());
        }

        let lba = INODE_TABLE_START_LBA + idx as u32;
        let buf = self.nodes[idx].serialize();
        if AtaPio::write_sector(lba, &buf).is_err() {
            return Err(FsError::IoError);
        }

        self.update_usage_counts();
        let sb_buf = self.superblock.serialize();
        if AtaPio::write_sector(SUPERBLOCK_LBA, &sb_buf).is_err() {
            return Err(FsError::IoError);
        }

        Ok(())
    }

    pub fn get_fs_info(&self) -> FsInfo {
        let total_blocks = self.superblock.total_blocks;
        let used_blocks = self.superblock.used_blocks;
        let free_blocks = if total_blocks >= used_blocks {
            total_blocks - used_blocks
        } else {
            0
        };

        let total_inodes = self.superblock.total_inodes;
        let used_inodes = self.superblock.used_inodes;
        let free_inodes = if total_inodes >= used_inodes {
            total_inodes - used_inodes
        } else {
            0
        };

        FsInfo {
            fs_type: "NovaFS",
            version: self.superblock.version,
            block_size: self.superblock.block_size,
            total_blocks,
            used_blocks,
            free_blocks,
            total_inodes,
            used_inodes,
            free_inodes,
            is_persistent: self.is_disk_persistent,
        }
    }

    fn alloc_node(&mut self) -> Result<usize, FsError> {
        for i in 1..MAX_NODES {
            if !self.nodes[i].is_used {
                return Ok(i);
            }
        }
        Err(FsError::StorageFull)
    }

    pub fn get_pwd(&self, out: &mut [u8; MAX_PATH_LEN]) -> usize {
        if self.cwd == 0 {
            out[0] = b'/';
            return 1;
        }

        let mut ancestors = [0usize; 16];
        let mut count = 0;
        let mut curr = self.cwd;

        while curr != 0 && count < ancestors.len() {
            ancestors[count] = curr;
            count += 1;
            curr = self.nodes[curr].parent;
        }

        let mut offset = 0;
        for i in (0..count).rev() {
            let node_idx = ancestors[i];
            let name = self.nodes[node_idx].name();
            if offset + 1 + name.len() <= MAX_PATH_LEN {
                out[offset] = b'/';
                offset += 1;
                out[offset..offset + name.len()].copy_from_slice(name.as_bytes());
                offset += name.len();
            }
        }

        offset
    }

    pub fn resolve_path(&self, path: &str) -> Result<usize, FsError> {
        let trimmed = path.trim();
        if trimmed.is_empty() || trimmed == "." {
            return Ok(self.cwd);
        }
        if trimmed == "/" {
            return Ok(0);
        }

        let mut curr = if trimmed.starts_with('/') {
            0
        } else {
            self.cwd
        };

        for component in trimmed.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            if component == ".." {
                curr = self.nodes[curr].parent;
                continue;
            }

            if self.nodes[curr].kind != NodeKind::Directory {
                return Err(FsError::NotADirectory);
            }

            let mut found = None;
            for i in 1..MAX_NODES {
                if self.nodes[i].is_used
                    && self.nodes[i].parent == curr
                    && self.nodes[i].name() == component
                {
                    found = Some(i);
                    break;
                }
            }

            match found {
                Some(idx) => curr = idx,
                None => return Err(FsError::NotFound),
            }
        }

        Ok(curr)
    }

    pub fn list_dir<F>(&self, path: &str, mut callback: F) -> Result<(), FsError>
    where
        F: FnMut(&str, NodeKind),
    {
        let dir_idx = self.resolve_path(path)?;
        if self.nodes[dir_idx].kind != NodeKind::Directory {
            return Err(FsError::NotADirectory);
        }

        for i in 1..MAX_NODES {
            if self.nodes[i].is_used && self.nodes[i].parent == dir_idx {
                callback(self.nodes[i].name(), self.nodes[i].kind);
            }
        }

        Ok(())
    }

    pub fn change_dir(&mut self, path: &str) -> Result<(), FsError> {
        let idx = self.resolve_path(path)?;
        if self.nodes[idx].kind != NodeKind::Directory {
            return Err(FsError::NotADirectory);
        }
        self.cwd = idx;
        Ok(())
    }

    pub fn create_dir(&mut self, path: &str) -> Result<usize, FsError> {
        let (parent_path, name) = split_parent_and_name(path)?;
        if name.is_empty() || name == "." || name == ".." {
            return Err(FsError::InvalidPath);
        }

        let parent_idx = self.resolve_path(parent_path)?;
        if self.nodes[parent_idx].kind != NodeKind::Directory {
            return Err(FsError::NotADirectory);
        }

        for i in 1..MAX_NODES {
            if self.nodes[i].is_used
                && self.nodes[i].parent == parent_idx
                && self.nodes[i].name() == name
            {
                return Err(FsError::AlreadyExists);
            }
        }

        let free_idx = self.alloc_node()?;
        self.nodes[free_idx].is_used = true;
        self.nodes[free_idx].kind = NodeKind::Directory;
        self.nodes[free_idx].parent = parent_idx;
        self.nodes[free_idx].size = 0;
        self.nodes[free_idx].set_name(name)?;

        self.sync_node(free_idx)?;

        Ok(free_idx)
    }

    pub fn create_file(&mut self, path: &str) -> Result<usize, FsError> {
        let (parent_path, name) = split_parent_and_name(path)?;
        if name.is_empty() || name == "." || name == ".." {
            return Err(FsError::InvalidPath);
        }

        let parent_idx = self.resolve_path(parent_path)?;
        if self.nodes[parent_idx].kind != NodeKind::Directory {
            return Err(FsError::NotADirectory);
        }

        for i in 1..MAX_NODES {
            if self.nodes[i].is_used
                && self.nodes[i].parent == parent_idx
                && self.nodes[i].name() == name
            {
                if self.nodes[i].kind == NodeKind::File {
                    return Ok(i);
                } else {
                    return Err(FsError::AlreadyExists);
                }
            }
        }

        let free_idx = self.alloc_node()?;
        self.nodes[free_idx].is_used = true;
        self.nodes[free_idx].kind = NodeKind::File;
        self.nodes[free_idx].parent = parent_idx;
        self.nodes[free_idx].size = 0;
        self.nodes[free_idx].set_name(name)?;

        self.sync_node(free_idx)?;

        Ok(free_idx)
    }

    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<(), FsError> {
        if data.len() > MAX_FILE_SIZE {
            return Err(FsError::FileTooLarge);
        }

        let file_idx = match self.resolve_path(path) {
            Ok(idx) => {
                if self.nodes[idx].kind != NodeKind::File {
                    return Err(FsError::IsADirectory);
                }
                idx
            }
            Err(FsError::NotFound) => self.create_file(path)?,
            Err(e) => return Err(e),
        };

        self.nodes[file_idx].content[..data.len()].copy_from_slice(data);
        self.nodes[file_idx].size = data.len();

        self.sync_node(file_idx)?;

        Ok(())
    }

    pub fn read_file(&self, path: &str) -> Result<&str, FsError> {
        let idx = self.resolve_path(path)?;
        if self.nodes[idx].kind != NodeKind::File {
            return Err(FsError::IsADirectory);
        }
        Ok(self.nodes[idx].content_str())
    }

    pub fn read_file_bytes(&self, path: &str) -> Result<&[u8], FsError> {
        let idx = self.resolve_path(path)?;
        if self.nodes[idx].kind != NodeKind::File {
            return Err(FsError::IsADirectory);
        }
        Ok(&self.nodes[idx].content[..self.nodes[idx].size])
    }

    pub fn ensure_default_system_files(&mut self) {
        if self.resolve_path("/bin").is_err() {
            let _ = self.create_dir("/bin");
        }
        let _ = self.write_file("/bin/hello", crate::elf::ELF_HELLO_BIN);
        let _ = self.write_file("/bin/child", crate::elf::ELF_CHILD_BIN);
        let _ = self.write_file("/bin/exec-test", crate::elf::ELF_EXEC_TEST_BIN);
        let _ = self.write_file("/bin/pipe-test", crate::elf::ELF_PIPE_TEST_BIN);
        let _ = self.write_file("/bin/fork-test", crate::elf::ELF_FORK_TEST_BIN);
        let _ = self.write_file("/bin/cow-test", crate::elf::ELF_COW_TEST_BIN);
    }

    pub fn remove_file(&mut self, path: &str) -> Result<(), FsError> {
        let node_idx = self.resolve_path(path)?;
        if node_idx == 0 {
            return Err(FsError::CannotRemoveRoot);
        }
        if self.nodes[node_idx].kind != NodeKind::File {
            return Err(FsError::IsADirectory);
        }

        self.nodes[node_idx] = Node::empty();
        self.sync_node(node_idx)?;

        Ok(())
    }

    pub fn remove_dir(&mut self, path: &str) -> Result<(), FsError> {
        let node_idx = self.resolve_path(path)?;
        if node_idx == 0 {
            return Err(FsError::CannotRemoveRoot);
        }
        if self.nodes[node_idx].kind != NodeKind::Directory {
            return Err(FsError::NotADirectory);
        }

        // Check if directory is current working directory or an ancestor of cwd
        let mut check = self.cwd;
        while check != 0 {
            if check == node_idx {
                return Err(FsError::DirectoryInUse);
            }
            check = self.nodes[check].parent;
        }

        // Check if directory contains any child entries
        for i in 1..MAX_NODES {
            if self.nodes[i].is_used && self.nodes[i].parent == node_idx {
                return Err(FsError::DirectoryNotEmpty);
            }
        }

        self.nodes[node_idx] = Node::empty();
        self.sync_node(node_idx)?;

        Ok(())
    }
}

pub static FILESYSTEM: Mutex<FileSystem> = Mutex::new(FileSystem::new());

pub fn init() -> bool {
    let mut fs = FILESYSTEM.lock();
    fs.init_storage()
}

fn split_parent_and_name(path: &str) -> Result<(&str, &str), FsError> {
    let mut trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(FsError::InvalidPath);
    }

    // Strip trailing slashes (unless path is just "/")
    while trimmed.len() > 1 && trimmed.ends_with('/') {
        trimmed = &trimmed[..trimmed.len() - 1];
    }

    if trimmed == "/" || trimmed == "." || trimmed == ".." {
        return Err(FsError::InvalidPath);
    }

    if let Some(pos) = trimmed.rfind('/') {
        let parent = if pos == 0 { "/" } else { &trimmed[..pos] };
        let name = &trimmed[pos + 1..];
        if name.is_empty() {
            Err(FsError::InvalidPath)
        } else {
            Ok((parent, name))
        }
    } else {
        Ok((".", trimmed))
    }
}
