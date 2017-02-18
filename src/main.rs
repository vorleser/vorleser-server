#![feature(libc)]
extern crate ffmpeg_sys as ffmpeg;
extern crate libc;

mod metadata;

use std::fs::File;
use std::io::Write;
use std::env;

fn main() {
    let mut args = env::args();
    args.next();
    for s in args {
        println!("{}", s);
        match metadata::MediaFile::read_file(s) {
            Ok(ref mut c) => {
                println!("{:?}", c.get_mediainfo());
                save(c.get_cover_art());
            },
            Err(e) => println!("Error: {}", e)
        }
    }
}

fn save(buf: &[u8]) {
    let mut f = File::create("lul.jpg").unwrap();
    if let Ok(_) = f.write_all(buf) {
        println!("Successfully wrote image!")
    }
}


// let a = Audiobook {
//     chapters: Vec::new(),
//     name: "lul".to_string(),
//     length: apply_timebase((*ctx).duration, &AV_TIME_BASE_Q)
// };
// let ref mut chaps = av_chapter_vec(ctx)[0];
// let c = Chapter::from_av_chapter(chap);
// unsafe {
//     let mut chaps = av_chapter_vec(c.ctx);
//     let c = Chapter::from_av_chapters(chaps);
//     println!("{:?}", c);
// }
