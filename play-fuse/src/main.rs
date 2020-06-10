use fuse::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
    FUSE_ROOT_ID,
};
use libc::ENOENT;
use play_fat::block_device::virt::*;
use play_fat::fat::prim::*;
use play_fat::fat::*;
use slab::Slab;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::time::{Duration, UNIX_EPOCH};

const MIN_INODE: u64 = FUSE_ROOT_ID + 1;

const TTL: Duration = Duration::from_secs(1);

struct NodeDetails {
    attr: FileAttr,
}

struct FSImpl {
    fs: FATFileSystem,
    buffer: Vec<u8>,
    nodes: Slab<NodeDetails>,
}

impl FSImpl {
    const INITIAL_NODE_CAPACITY: usize = 1024;

    fn open(image_path: impl AsRef<std::path::Path>, offset: u64) -> Self {
        let image = File::open(image_path).unwrap();
        let device = FileBlockDevice::new(image, offset);
        let fs = FATFileSystem::open(Box::new(device));

        let buffer = vec![0u8; fs.cluster_bytes() as usize];
        let nodes = Slab::with_capacity(Self::INITIAL_NODE_CAPACITY);

        Self { fs, buffer, nodes }
    }

    fn get_root_attr(&mut self, req: &Request, reply: ReplyAttr) {
        let root_attr = FileAttr {
            ino: FUSE_ROOT_ID,
            size: 0,
            blocks: 0,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: req.uid(),
            gid: req.gid(),
            rdev: 0,
            flags: 0,
        };

        reply.attr(&TTL, &root_attr);
    }

    fn read_root_dir(&mut self, _req: &Request, offset: i64, mut reply: ReplyDirectory) {
        let mut index = MIN_INODE;

        if offset > 0 {
            return;
        }

        println!("Starting root dir enumeration with offset {}", offset);

        // TODO: what about "." and ".."
        for entry in self.fs.ls_root(self.buffer.as_mut_slice()) {
            match entry {
                DirectoryEntry::LongFileName(_entry) => {}

                DirectoryEntry::Standard(entry) => {
                    let entry_name = std::str::from_utf8(entry.name()).unwrap().trim();

                    // TODO: proper inode alloc
                    let inode = index;
                    let next_offset = index as i64 + 1;

                    if entry.is_directory() {
                        println!(
                            "Returning directory entry {:?} with inode {}",
                            entry_name, inode
                        );
                        reply.add(inode, next_offset, FileType::Directory, entry_name);
                    } else {
                        println!("Returning file entry {:?} with inode {}", entry_name, inode);
                        reply.add(inode, next_offset, FileType::RegularFile, entry_name);
                    }

                    index += 1;
                }
            }
        }

        reply.ok();
    }

    fn root_lookup(&mut self, req: &Request, name: &OsStr, reply: ReplyEntry) {
        for (i, entry) in self.fs.ls_root(self.buffer.as_mut_slice()).enumerate() {
            match entry {
                DirectoryEntry::LongFileName(_entry) => {}

                DirectoryEntry::Standard(entry) => {
                    let entry_name = std::str::from_utf8(entry.name()).unwrap().trim();

                    if name != entry_name {
                        continue;
                    }

                    let slot = self.nodes.vacant_entry();

                    let attr = FileAttr {
                        ino: MIN_INODE + slot.key() as u64,
                        size: 0,
                        blocks: 0,
                        atime: UNIX_EPOCH,
                        mtime: UNIX_EPOCH,
                        ctime: UNIX_EPOCH,
                        crtime: UNIX_EPOCH,
                        kind: if entry.is_directory() {
                            FileType::Directory
                        } else {
                            FileType::RegularFile
                        },
                        perm: 0o755,
                        nlink: 2,
                        uid: req.uid(),
                        gid: req.gid(),
                        rdev: 0,
                        flags: 0,
                    };

                    let node_details = NodeDetails { attr };

                    reply.entry(&TTL, &node_details.attr, 0);

                    slot.insert(node_details);

                    println!("Found entry {:?} with inode {}", name, attr.ino);

                    return;
                }
            }
        }

        println!("Could not find entry {:?}", name);
        reply.error(ENOENT);
    }
}

impl Filesystem for FSImpl {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if parent == FUSE_ROOT_ID {
            self.root_lookup(req, name, reply);
        } else {
            println!("Request to lookup '{:?}' with parent {}", name, parent);
            reply.error(ENOENT);
        }
    }

    fn forget(&mut self, _req: &Request, ino: u64, nlookup: u64) {
        println!("Request to forget {} for count {}", ino, nlookup);
        let index = (ino - MIN_INODE) as usize;
        self.nodes.remove(index);
    }

    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == FUSE_ROOT_ID {
            self.get_root_attr(req, reply);
            return;
        }

        let index = (ino - MIN_INODE) as usize;

        if let Some(details) = self.nodes.get(index) {
            println!("Request to get attributes for {} succeeded", ino);
            reply.attr(&TTL, &details.attr);
            return;
        }

        println!("Request to get attributes for {} returning enoent", ino);
        reply.error(ENOENT);
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        _size: u32,
        reply: ReplyData,
    ) {
        println!("Request to read inode {} with offset {}", ino, offset);
        //if ino == 2 {
        //    reply.data(&HELLO_TXT_CONTENT.as_bytes()[offset as usize..]);
        //} else {
        //    reply.error(ENOENT);
        //}
        reply.error(ENOENT);
    }

    fn readdir(&mut self, req: &Request, ino: u64, _fh: u64, offset: i64, reply: ReplyDirectory) {
        if ino == FUSE_ROOT_ID {
            self.read_root_dir(req, offset, reply);
        } else {
            println!("Request to readdir with ino {}", ino);
            reply.error(ENOENT);
        }
    }
}

fn main() {
    env_logger::init();

    let mountpoint = env::args_os().nth(1).unwrap();

    let options = ["-o", "ro", "-o", "fsname=hello"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();

    let image = "/home/stears/data/simon/nox-rust/target/x86-nox/release/nox-rust.img";
    let offset = 1048576;
    let fs = FSImpl::open(image, offset);

    fuse::mount(fs, mountpoint, &options).unwrap();
}
