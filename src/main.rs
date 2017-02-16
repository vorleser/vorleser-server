#![feature(libc)]
extern crate ffmpeg_sys as ffmpeg;
extern crate libc;

mod metadata;

use std::env;

fn main() {
    let mut args = env::args();
    args.next();
    for s in args {
        println!("{}", s);
        get_metadata(&s);
    }
}


fn get_metadata(file_name: &str) {
    let mut c = metadata::Context::new();
    c.read_file(file_name);
    println!("{:?}", c.get_mediainfo());

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
}

