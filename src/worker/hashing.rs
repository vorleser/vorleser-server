use std::fs::File;
use ring::digest;
use std::io::Read;
use walkdir::{WalkDir, WalkDirIterator};
use std::path::{Path, PathBuf};
use humanesort::HumaneOrder;

use super::error::*;

/// Checksum of a whole directory.
pub fn checksum_file(path: &AsRef<Path>) -> Result<Vec<u8>> {
    let mut ctx = digest::Context::new(&digest::SHA256);
    update_hash_from_file(&mut ctx, path)?;
    let mut res = Vec::new();
    res.extend_from_slice(ctx.finish().as_ref());
    Ok(res)
}

/// Update hash object using file content
fn update_hash_from_file(ctx: &mut digest::Context, path: &AsRef<Path>) -> Result<()> {
    let mut file = File::open(path.as_ref())?;
    let mut buf: [u8; 1024] = [0; 1024];
    loop {
        let count = file.read(&mut buf[..])?;
        ctx.update(&buf[0..count]);
        if count == 0 { break }
    }
    Ok(())
}

/// Checksum a whole directory
pub fn checksum_dir(path: &AsRef<Path>) -> Result<Vec<u8>> {
    let walker = WalkDir::new(path.as_ref())
        .follow_links(true)
        .sort_by(
            |s, o| s.to_string_lossy().humane_cmp(&o.to_string_lossy())
        );
    let mut ctx = digest::Context::new(&digest::SHA256);
    for entry in walker {
        if let Ok(e) = entry {
            let p = e.path();
            if e.file_type().is_file() {
                update_hash_from_file(&mut ctx, &p)?;
            }
            ctx.update(p.to_string_lossy().as_bytes());
        }
    }
    let mut res = Vec::new();
    res.extend_from_slice(ctx.finish().as_ref());
    Ok(res)
}

