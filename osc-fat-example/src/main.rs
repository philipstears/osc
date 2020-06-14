#![allow(dead_code)]

use osc_fat::block_device::virt::*;
use osc_fat::fat::*;
use std::fs::File;
use std::io::Result;

fn main() -> Result<()> {
    let image = "/home/stears/data/simon/nox-rust/target/x86-nox/release/nox-rust.img";
    let offset = 1048576;

    let file = File::open(image)?;
    let device = Box::new(FileBlockDevice::new(file, offset));

    let fs = FATFileSystem::open(device);

    let mut read_buffer = vec![0u8; fs.required_read_buffer_size()];

    fs.walk_directory(read_buffer.as_mut_slice(), DirectorySelector::Root)
        .enumerate_occupied_entries(|entry| {
            process_entry(&fs, 0, entry);
        });

    Ok(())
}

fn process_entry<'a>(fs: &FATFileSystem, level: usize, entry: DirectoryEntry<'a>) {
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

                if entry.name()[0] != b'.' {
                    let mut read_buffer = vec![0u8; fs.required_read_buffer_size()];

                    fs.walk_directory(
                        read_buffer.as_mut_slice(),
                        DirectorySelector::Normal(entry.first_cluster()),
                    )
                    .enumerate_occupied_entries(|child_entry| {
                        process_entry(&fs, level + 1, child_entry);
                    });
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
