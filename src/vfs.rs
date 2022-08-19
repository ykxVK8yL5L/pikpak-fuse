//! FUSE adaptor
//!
//! https://github.com/gz/btfs is used as a reference.
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::{SystemTime,UNIX_EPOCH};
use std::{collections::BTreeMap, time::Duration};
use std::sync::atomic::{AtomicU64, Ordering};

use httpdate;
use hmacsha::HmacSha;
use sha1::{Sha1, Digest};
use hex_literal::hex;
use base64::encode;

use bytes::Bytes;
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory,ReplyCreate, ReplyEmpty, ReplyEntry,
    ReplyOpen,ReplyWrite, Request, FUSE_ROOT_ID,
};
use tracing::{debug,info};

use crate::drive::{PikpakDrive, PikpakFile};
use crate::drive::model::*;

use crate::error::Error;
use crate::file_cache::FileCache;

const TTL: Duration = Duration::from_secs(1);
const BLOCK_SIZE: u64 = 4194304;

const FILE_HANDLE_READ_BIT: u64 = 1 << 63;
const FILE_HANDLE_WRITE_BIT: u64 = 1 << 62;


#[derive(Debug, Clone)]
pub struct Inode {
    children: BTreeMap<OsString, u64>,
    parent: u64,
}

impl Inode {
    fn new(parent: u64) -> Self {
        Self {
            children: BTreeMap::new(),
            parent,
        }
    }

    fn add_child(&mut self, name: OsString, inode: u64) {
        self.children.insert(name, inode);
    }
}

pub struct PikpakDriveFileSystem {
    drive: PikpakDrive,
    file_cache: FileCache,
    files: BTreeMap<u64, PikpakFile>,
    inodes: BTreeMap<u64, Inode>,
    next_inode: u64,
    next_fh: AtomicU64,
    oss_args: Option<OssArgs>,
    upload_tags:CompleteMultipartUpload,
}

impl PikpakDriveFileSystem {
    pub fn new(drive: PikpakDrive, read_buffer_size: usize) -> Self {
        let file_cache = FileCache::new(drive.clone(), read_buffer_size);
        let mut upload_tags = CompleteMultipartUpload{Part:vec![]};
        Self {
            drive,
            file_cache,
            files: BTreeMap::new(),
            inodes: BTreeMap::new(),
            next_inode: 1,
            next_fh: AtomicU64::new(1),
            oss_args:None,
            upload_tags:upload_tags,
        }
    }

    /// Next inode number
    fn next_inode(&mut self) -> u64 {
        self.next_inode = self.next_inode.wrapping_add(1);
        self.next_inode
    }

    /// Next file handler
    fn next_fh(&mut self) -> u64 {
        let mut fh = self.next_fh.fetch_add(1, Ordering::SeqCst);
        fh
    }


    fn allocate_next_file_handle(&self, read: bool, write: bool) -> u64 {
        let mut fh = self.next_fh.fetch_add(1, Ordering::SeqCst);
        // Assert that we haven't run out of file handles
        assert!(fh < FILE_HANDLE_WRITE_BIT && fh < FILE_HANDLE_READ_BIT);
        if read {
            fh |= FILE_HANDLE_READ_BIT;
        }
        if write {
            fh |= FILE_HANDLE_WRITE_BIT;
        }

        fh
    }

    fn init(&mut self) -> Result<(), Error> {
        let mut root_file = PikpakFile::new_root();
        // let (used_size, _) = self.drive.get_quota().map_err(|_| Error::ApiCallFailed)?;
        // root_file.size = used_size.to_string();
        let root_inode = Inode::new(0);
        self.inodes.insert(FUSE_ROOT_ID, root_inode);
        self.files.insert(FUSE_ROOT_ID, root_file);
        Ok(())
    }

    fn lookup(&mut self, parent: u64, name: &OsStr) -> Result<FileAttr, Error> {
        let mut parent_inode = self
            .inodes
            .get(&parent)
            .ok_or(Error::ParentNotFound)?
            .clone();
        if parent_inode.children.is_empty() {
            // Parent inode isn't loaded yet
            debug!(parent = parent, "readdir missing parent in lookup");
            self.readdir(parent, 0)?;
            parent_inode = self
                .inodes
                .get(&parent)
                .ok_or(Error::ParentNotFound)?
                .clone();
        }
        let inode = parent_inode
            .children
            .get(name)
            .ok_or(Error::ChildNotFound)?;
        let file = self.files.get(inode).ok_or(Error::NoEntry)?;
        Ok(file.to_file_attr(*inode))
    }

    fn readdir(&mut self, ino: u64, offset: i64) -> Result<Vec<(u64, FileType, String)>, Error> {
        //info!(ino = ino, "readdir");
        let mut entries = Vec::new();
        let mut inode = self.inodes.get(&ino).ok_or(Error::NoEntry)?.clone();
        if offset == 0 {
            entries.push((ino, FileType::Directory, ".".to_string()));
            entries.push((inode.parent, FileType::Directory, String::from("..")));
            let file = self.files.get(&ino).ok_or(Error::NoEntry)?;
            let parent_file_id = &file.id;
            let files = self
                .drive
                .list_all(parent_file_id)
                .map_err(|_| Error::ApiCallFailed)?;
            debug!(
                inode = ino,
                "total {} files in directory {}",
                files.len(),
                file.name
            );

            let mut to_remove = inode.children.keys().cloned().collect::<Vec<_>>();
            for file in &files {
                let name = OsString::from(file.name.clone());
                if inode.children.contains_key(&name) {
                    // file already exists
                    to_remove.retain(|n| n != &name);
                } else {
                    let new_inode = self.next_inode();
                    inode.add_child(name, new_inode);
                    self.files.insert(new_inode, file.clone());
                    self.inodes
                        .entry(new_inode)
                        .or_insert_with(|| Inode::new(ino));
                }
            }

            if !to_remove.is_empty() {
                for name in to_remove {
                    if let Some(ino_remove) = inode.children.remove(&name) {
                        debug!(inode = ino_remove, name = %Path::new(&name).display(), "remove outdated inode");
                        self.files.remove(&ino_remove);
                        self.inodes.remove(&ino_remove);
                    }
                }
            }
            self.inodes.insert(ino, inode.clone());
        }

        for child_ino in inode.children.values().skip(offset as usize) {
            let file = self.files.get(child_ino).ok_or(Error::ChildNotFound)?;
            let kind = if file.kind.eq("drive#folder"){
                FileType::Directory
            }else{
                FileType::RegularFile
            };

            entries.push((*child_ino, kind, file.name.clone()));
        }
        Ok(entries)
    }

    fn read(&mut self, ino: u64, fh: u64, offset: i64, size: u32) -> Result<Bytes, Error> {
        let file = self.files.get(&ino).ok_or(Error::NoEntry)?;
        debug!(inode = ino, name = %file.name, fh = fh, offset = offset, size = size, "read");
        if offset >= file.size.parse::<i64>().unwrap() {
            return Ok(Bytes::new());
        }
        let size = std::cmp::min(size, file.size.parse::<u64>().unwrap().saturating_sub(offset as u64) as u32);
        self.file_cache.read(fh, offset, size)
    }
    
}

impl Filesystem for PikpakDriveFileSystem {
    fn init(
        &mut self,
        _req: &Request<'_>,
        _config: &mut fuser::KernelConfig,
    ) -> Result<(), libc::c_int> {
        if let Err(e) = self.init() {
            return Err(e.into());
        }
        Ok(())
    }

    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let dirname = Path::new(name);
        debug!(parent = parent, name = %dirname.display(), "lookup");
        match self.lookup(parent, name) {
            Ok(attr) => reply.entry(&TTL, &attr, 0),
            Err(e) => reply.error(e.into()),
        }
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        if let Some(file) = self.files.get(&ino) {
            debug!(inode = ino, name = %file.name, "getattr");
            reply.attr(&TTL, &file.to_file_attr(ino))
        } else {
            debug!(inode = ino, "getattr");
            reply.error(libc::ENOENT);
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        debug!(inode = ino, offset = offset, "readdir");
        match self.readdir(ino, offset) {
            Ok(entries) => {
                // Offset of 0 means no offset.
                // Non-zero offset means the passed offset has already been seen,
                // and we should start after it.
                let offset_add = if offset == 0 { 0 } else { offset + 1 };
                for (i, (ino, kind, name)) in entries.into_iter().enumerate() {
                    let buffer_full = reply.add(ino, offset_add + i as i64, kind, name);
                    if buffer_full {
                        break;
                    }
                }
                reply.ok();
            }
            Err(e) => reply.error(e.into()),
        }
    }

    fn open(&mut self, _req: &Request<'_>, ino: u64, _flags: i32, reply: ReplyOpen) {
        debug!(inode = ino, "open");
        if let Some((file_id, file_name, file_size)) = self
            .files
            .get(&ino)
            .map(|f| (f.id.clone(), f.name.clone(), f.size.parse::<u64>().unwrap()))
        {
            debug!(inode = ino, name = %file_name, "open file");
            let fh = self.next_fh();
            self.file_cache.open(fh, file_id, file_size);
            reply.opened(fh, 0);
        } else {
            debug!(inode = ino, "open file");
            reply.error(libc::ENOENT);
        }
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        debug!(inode = ino, fh = fh, "release file");
        let mut upload_tags = CompleteMultipartUpload{Part:vec![]};
        self.upload_tags = upload_tags;
        self.oss_args = None;
        self.file_cache.release(fh);
        reply.ok();
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        debug!(inode = ino, fh = fh, offset = offset, size = size, "read work here");
        match self.read(ino, fh, offset, size) {
            Ok(data) => reply.data(&data),
            Err(e) => reply.error(e.into()),
        }
    }


    fn rename(
        &mut self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        new_parent: u64,
        new_name: &OsStr,
        flags: u32,
        reply: ReplyEmpty,
    ) {
        let file = match self.lookup(parent, name) {
            Ok(attrs) => attrs,
            Err(error_code) => {
                reply.error(error_code.into());
                return;
            }
        };

        let file_id = self.files.get(&file.ino).unwrap().id.clone();

        let res:PikpakFile = match self.drive.rename_file(&file_id, &new_name.to_string_lossy()) {
            Ok(res) => {
                 reply.ok();
                 return;
            },
            Err(error_code) => {
                debug!("rename error: {:?}", error_code);
                reply.error(libc::EFAULT);
                return;
            }
        };
        reply.ok();
    }

    fn copy_file_range(
        &mut self,
        _req: &Request<'_>,
        src_inode: u64,
        src_fh: u64,
        src_offset: i64,
        dest_inode: u64,
        dest_fh: u64,
        dest_offset: i64,
        size: u64,
        _flags: u32,
        reply: ReplyWrite,
    ) {
        debug!(
            "copy_file_range() called with src ({}, {}, {}) dest ({}, {}, {}) size={}",
            src_fh, src_inode, src_offset, dest_fh, dest_inode, dest_offset, size
        );


        let src_file = match self.files.get(&src_inode) {
            Some(file) => file,
            None => {
                reply.error(libc::EFAULT);
                return;
            }
        };

        let src_file_id = src_file.id.clone();
        let dest_file_id = self.files.get(&src_inode).unwrap().id.clone();
        let res:TaskResponse = match self.drive.copy_file(&src_file_id, &dest_file_id){
            Ok(res) => {
                reply.written(src_file.size.parse::<u32>().unwrap());
                return;
            }
            Err(error_code) => {
                debug!("copy error: {:?}", error_code);
                reply.error(libc::EFAULT);
                return;
            }
        };
    }

    //目录操作
    fn mkdir(
        &mut self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        mut mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        debug!("mkdir() called with {:?} {:?} {:o}", parent, name, mode);
        if self.lookup(parent, name).is_ok() {
            reply.error(libc::EEXIST);
            return;
        }
        let parent_file = match self.files.get(&parent).ok_or(Error::NoEntry){
            Ok(file) => file,
            Err(e) => {
                reply.error(Error::ParentNotFound.into());
                return;
            }
        };
        let new_folder_name = name.to_string_lossy().to_string();
        let parent_file_id = parent_file.id.clone();
        let new_dir_res:CreateFolderResponse = match self.drive.create_folder(&parent_file_id,&new_folder_name) {
            Ok(res) => res,
            Err(error_code) => {
                debug!("create_folder error: {:?}", error_code);
                reply.error(libc::EFAULT);
                return;
            }
        };
        let new_dir = new_dir_res.file;

        let new_inode = self.next_inode();
        let attrs = new_dir.to_file_attr(new_inode);

        reply.entry(&TTL, &attrs, 0);
    }


    fn rmdir(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        debug!("rmdir() called with {:?} {:?}", parent, name);

        let file = match self.lookup(parent, name) {
            Ok(file) => file,
            Err(e) => {
                reply.error(e.into());
                return;
            }
        };
        let file_id = self.files.get(&file.ino).unwrap().id.clone();

        let res:TaskResponse = match self.drive.remove_file(&file_id) {
            Ok(res) => {
                 reply.ok();
                 return;
            },
            Err(error_code) => {
                debug!("delete_folder error: {:?}", error_code);
                reply.error(libc::EFAULT);
                return;
            }
        };
        reply.ok()
    }



    // 文件操作
    fn create(
        &mut self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        mut mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        debug!("create() called with {:?} {:?}", parent, name);
        if self.lookup(parent, name).is_ok() {
            reply.error(libc::EEXIST);
            return;
        }

        let mut parent_inode = self.inodes.get(&parent).ok_or(Error::NoEntry).unwrap().clone();
        let parent_file = match self.files.get(&parent).ok_or(Error::NoEntry){
            Ok(file) => file,
            Err(e) => {
                reply.error(Error::ParentNotFound.into());
                return;
            }
        };
        let parent_file_id = parent_file.id.clone();
        
      
        let file_name =name.to_string_lossy().to_string();
        let now = SystemTime::now();
        let hash_str = format!("{}{}",&file_name,now.duration_since(UNIX_EPOCH).unwrap().as_secs());
        let mut hasher = Sha1::default();
        hasher.update(hash_str.as_bytes());
        let hash_code = hasher.finalize();
        let file_hash = format!("{:X}",&hash_code);

        
        let file = PikpakFile {
            name: file_name,
            kind: "drive#file".to_string(),
            id: "".to_string(),
            parent_id:parent_file_id,
            phase: "".to_string(),
            size: "0".to_string(),
            created_time: DateTime::new(now),
            modified_time: DateTime::new(now),
            file_extension: "".to_string(),
            mime_type: "".to_string(),
            web_content_link: "".to_string(),
            medias:Vec::new(),
            hash:Some(file_hash),
        };
        let new_file_inode = self.next_inode();
        let attrs = file.to_file_attr(new_file_inode);

        // self.files.insert(new_file_inode, file);
        // parent_inode.add_child(name.to_os_string(), new_file_inode);
        // self.inodes.insert(parent, parent_inode);
        // info!(new_file_indode=new_file_inode, "create new file inode is");
        // info!(parent_ino=parent, "parent ino is");

        let (read, write) = match flags & libc::O_ACCMODE {
            libc::O_RDONLY => (true, false),
            libc::O_WRONLY => (false, true),
            libc::O_RDWR => (true, true),
            // Exactly one access mode flag must be specified
            _ => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        reply.created(&TTL, &attrs, 0,self.allocate_next_file_handle(read, write), 0);

        // reply.created(
        //     &TTL,
        //     &attrs.into(),
        //     0,
        //     self.allocate_next_file_handle(read, write),
        //     0,
        // );


    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        debug!("unlink() called with {:?} {:?}", parent, name);
        let file = match self.lookup(parent, name) {
            Ok(file) => file,
            Err(e) => {
                reply.error(e.into());
                return;
            }
        };
        let file_id = self.files.get(&file.ino).unwrap().id.clone();
        let res:TaskResponse = match self.drive.remove_file(&file_id) {
            Ok(res) => {
                 reply.ok();
                 return;
            },
            Err(error_code) => {
                debug!("delete_folder error: {:?}", error_code);
                reply.error(libc::EFAULT);
                return;
            }
        };
        reply.ok()
    }

    fn write(
            &mut self,
            _req: &Request<'_>,
            ino: u64,
            fh: u64,
            offset: i64,
            data: &[u8],
            write_flags: u32,
            flags: i32,
            lock_owner: Option<u64>,
            reply: ReplyWrite,
        ) {
        info!("write() called with {:?} {:?}", ino, fh);
        reply.written(data.len() as u32);
    }


}

impl PikpakFile {
    fn to_file_attr(&self, ino: u64) -> FileAttr {
        //let kind = self.kind.into();
        let kind = if self.kind.eq("drive#folder"){
            FileType::Directory
        }else{
            FileType::RegularFile
        };
        
    
        let perm = if matches!(kind, FileType::Directory) {
            0o755
        } else {
            0o644
        };
        let nlink = if ino == FUSE_ROOT_ID { 2 } else { 1 };
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let blksize = BLOCK_SIZE;
        let blocks = self.size.parse::<u64>().unwrap() / blksize + 1;
        FileAttr {
            ino,
            size: self.size.parse::<u64>().unwrap(),
            blocks,
            atime: UNIX_EPOCH,
            mtime: *self.modified_time,
            ctime: *self.created_time,
            crtime: *self.created_time,
            kind,
            perm,
            nlink,
            uid,
            gid,
            rdev: 0,
            blksize: blksize as u32,
            flags: 0,
        }
    }
}
