use fuse::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request,
    FUSE_ROOT_ID,
};
use libc::ENOENT;
use play_fat::block_device::virt::*;
use play_fat::fat::*;
use std::collections::{btree_map, BTreeMap};
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::time::{Duration, UNIX_EPOCH};

const TTL: Duration = Duration::from_secs(1);

struct NodeDetails {
    reference_count: u64,
    attr: FileAttr,
    first_cluster: u32,
}

struct FSImpl {
    fs: FATFileSystem,
    buffer: Vec<u8>,
    nodes_by_cluster: BTreeMap<u32, NodeDetails>,
}

impl FSImpl {
    fn open(image_path: impl AsRef<std::path::Path>, offset: u64) -> Self {
        let image = File::open(image_path).unwrap();
        let device = FileBlockDevice::new(image, offset);
        let fs = FATFileSystem::open(Box::new(device));

        let buffer = vec![0u8; fs.required_read_buffer_size()];
        let nodes_by_cluster = BTreeMap::new();

        Self {
            fs,
            buffer,
            nodes_by_cluster,
        }
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
            nlink: 1,
            uid: req.uid(),
            gid: req.gid(),
            rdev: 0,
            flags: 0,
        };

        reply.attr(&TTL, &root_attr);
    }

    // TODO: need to figure out the root cluster details for
    // all variants before committing to this
    fn cluster_index_to_inode(cluster_index: u32) -> u64 {
        (cluster_index + 16).into()
    }

    fn inode_to_cluster_index(inode: u64) -> u32 {
        (inode - 16) as u32
    }

    fn get_directory_selector(&self, inode: u64) -> Option<DirectorySelector> {
        if inode == FUSE_ROOT_ID {
            Some(DirectorySelector::Root)
        } else {
            self.nodes_by_cluster
                .get(&Self::inode_to_cluster_index(inode))
                .map(|details| DirectorySelector::Normal(details.first_cluster))
        }
    }
}

impl Filesystem for FSImpl {
    fn lookup(&mut self, req: &Request, parent_inode: u64, name: &OsStr, reply: ReplyEntry) {
        println!("Looking up {:?} in {}", name, parent_inode);

        let maybe_directory_selector = self.get_directory_selector(parent_inode);

        let mut directory_walker = match maybe_directory_selector {
            Some(directory_selector) => self
                .fs
                .walk_directory(self.buffer.as_mut_slice(), directory_selector),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        loop {
            for entry in directory_walker.occupied_entries() {
                match entry {
                    DirectoryEntry::LongFileName(_entry) => {}

                    DirectoryEntry::Standard(entry) => {
                        let entry_name = std::str::from_utf8(entry.name()).unwrap().trim();

                        if name != entry_name {
                            continue;
                        }

                        let node_details = self
                            .nodes_by_cluster
                            .entry(entry.first_cluster())
                            .or_insert_with(|| {
                                let attr = FileAttr {
                                    ino: Self::cluster_index_to_inode(entry.first_cluster()),
                                    size: entry.size() as u64,
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
                                    nlink: 1,
                                    uid: req.uid(),
                                    gid: req.gid(),
                                    rdev: 0,
                                    flags: 0,
                                };

                                let node_details = NodeDetails {
                                    reference_count: 0,
                                    attr,
                                    first_cluster: entry.first_cluster(),
                                };

                                node_details
                            });

                        node_details.reference_count += 1;

                        reply.entry(&TTL, &node_details.attr, 0);

                        println!(
                            "Found entry {:?} with inode {}",
                            name, node_details.attr.ino
                        );

                        return;
                    }
                }
            }

            if let Some(new_directory_walker) = directory_walker.next() {
                directory_walker = new_directory_walker;
            } else {
                break;
            }
        }

        println!("Could not find entry {:?}", name);
        reply.error(ENOENT);
    }

    fn forget(&mut self, _req: &Request, ino: u64, nlookup: u64) {
        match self
            .nodes_by_cluster
            .entry(Self::inode_to_cluster_index(ino))
        {
            btree_map::Entry::Vacant(_) => {
                println!(
                    "Request to forget {} for count {}, but the entry isn't present.",
                    ino, nlookup
                );
            }
            btree_map::Entry::Occupied(mut entry) => {
                if entry.get().reference_count > nlookup {
                    println!(
                        "Request to forget {} which has count {} for count {}.",
                        ino,
                        entry.get().reference_count,
                        nlookup
                    );
                    entry.get_mut().reference_count -= nlookup;
                } else {
                    println!(
                        "Request to forget {} which has count {} for count {}. Removing entry.",
                        ino,
                        entry.get().reference_count,
                        nlookup
                    );
                    entry.remove();
                }
            }
        };
    }

    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == FUSE_ROOT_ID {
            self.get_root_attr(req, reply);
            return;
        }

        let cluster_index = Self::inode_to_cluster_index(ino);

        if let Some(details) = self.nodes_by_cluster.get(&cluster_index) {
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
        size: u32,
        reply: ReplyData,
    ) {
        let cluster_index = Self::inode_to_cluster_index(ino);

        println!(
            "Request to read {} from offset {} with size {}",
            ino, offset, size
        );
        if let Some(details) = self.nodes_by_cluster.get(&cluster_index) {
            self.fs
                .read(details.first_cluster, self.buffer.as_mut_slice());
            reply.data(&self.buffer[offset as usize..]);
            return;
        }

        reply.error(ENOENT);
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        println!("Starting enumeration of {} with offset {}", ino, offset);

        let maybe_directory_selector = self.get_directory_selector(ino);

        let directory_walker = match maybe_directory_selector {
            Some(directory_selector) => self
                .fs
                .walk_directory(self.buffer.as_mut_slice(), directory_selector),
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        // TODO: what about "." and ".."
        let mut next_index = 0;

        directory_walker.enumerate_occupied_entries(|entry| {
            let index = next_index;
            next_index += 1;

            if index < offset {
                return;
            }

            match entry {
                DirectoryEntry::LongFileName(_entry) => {}

                DirectoryEntry::Standard(entry) => {
                    let entry_name = std::str::from_utf8(entry.name()).unwrap().trim();

                    let inode = Self::cluster_index_to_inode(entry.first_cluster());
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
                }
            }
        });

        reply.ok();
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
