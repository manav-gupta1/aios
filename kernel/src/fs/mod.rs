use spin::Mutex;

pub const MAX_NODES: usize = 64;
pub const MAX_NAME_LEN: usize = 32;
pub const MAX_FILE_SIZE: usize = 512;
pub const MAX_PATH_LEN: usize = 128;

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
    StorageFull,
    NameTooLong,
    FileTooLarge,
    InvalidPath,
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
}

pub struct FileSystem {
    nodes: [Node; MAX_NODES],
    cwd: usize,
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
                    && self.nodes[i].name().eq_ignore_ascii_case(component)
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
        let (parent_path, name) = split_parent_and_name(path);
        if name.is_empty() {
            return Err(FsError::InvalidPath);
        }

        let parent_idx = self.resolve_path(parent_path)?;
        if self.nodes[parent_idx].kind != NodeKind::Directory {
            return Err(FsError::NotADirectory);
        }

        for i in 1..MAX_NODES {
            if self.nodes[i].is_used
                && self.nodes[i].parent == parent_idx
                && self.nodes[i].name().eq_ignore_ascii_case(name)
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

        Ok(free_idx)
    }

    pub fn create_file(&mut self, path: &str) -> Result<usize, FsError> {
        let (parent_path, name) = split_parent_and_name(path);
        if name.is_empty() {
            return Err(FsError::InvalidPath);
        }

        let parent_idx = self.resolve_path(parent_path)?;
        if self.nodes[parent_idx].kind != NodeKind::Directory {
            return Err(FsError::NotADirectory);
        }

        for i in 1..MAX_NODES {
            if self.nodes[i].is_used
                && self.nodes[i].parent == parent_idx
                && self.nodes[i].name().eq_ignore_ascii_case(name)
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
        Ok(())
    }

    pub fn read_file(&self, path: &str) -> Result<&str, FsError> {
        let idx = self.resolve_path(path)?;
        if self.nodes[idx].kind != NodeKind::File {
            return Err(FsError::IsADirectory);
        }
        Ok(self.nodes[idx].content_str())
    }
}

pub static FILESYSTEM: Mutex<FileSystem> = Mutex::new(FileSystem::new());

fn split_parent_and_name(path: &str) -> (&str, &str) {
    let trimmed = path.trim();
    if let Some(pos) = trimmed.rfind('/') {
        let parent = if pos == 0 { "/" } else { &trimmed[..pos] };
        let name = &trimmed[pos + 1..];
        (parent, name)
    } else {
        (".", trimmed)
    }
}
