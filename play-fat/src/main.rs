#![allow(dead_code)]

use play_fat::block_device::virt::*;
use play_fat::fat::prim::*;
use play_fat::fat::*;
use std::fs::File;
use std::io::Result;

fn main() -> Result<()> {
    let image = "/home/stears/data/simon/nox-rust/target/x86-nox/release/nox-rust.img";
    let offset = 1048576;

    let file = File::open(image)?;
    let device = Box::new(FileBlockDevice::new(file, offset));

    let mut fs = FATFileSystem::open(device);

    let mut cluster_buffer = vec![0u8; fs.cluster_bytes() as usize];

    for entry in fs.ls_root(cluster_buffer.as_mut_slice()) {
        process_entry(&mut fs, 0, entry)
    }

    Ok(())
}

fn process_entry<'a>(fs: &mut FATFileSystem, level: usize, entry: DirectoryEntry<'a>) {
    match entry {
        DirectoryEntry::LongFileName(entry) => {
            for _ in 0..level {
                print!("  ");
            }

            println!(
                "LFN: {:?}",
                std::char::decode_utf16(entry.chars())
                    .filter_map(|ch| ch.ok())
                    .collect::<String>()
            );
        }

        DirectoryEntry::Standard(entry) => {
            for _ in 0..level {
                print!("  ");
            }

            if entry.is_directory() {
                println!("Dir: {}", std::str::from_utf8(entry.name()).unwrap(),);

                let mut dir_cluster = vec![0u8; fs.cluster_bytes() as usize];

                if entry.name()[0] != b'.' {
                    for child_entry in fs.ls(entry.first_cluster(), dir_cluster.as_mut_slice()) {
                        process_entry(fs, level + 1, child_entry)
                    }
                }
            } else {
                println!(
                    "File: {} ({} bytes)",
                    std::str::from_utf8(entry.name()).unwrap(),
                    entry.size(),
                );
            }
        }
    }
}
