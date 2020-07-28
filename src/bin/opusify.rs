use clap::{App, Arg};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
/// Test utility for opus reencoding
use vorleser_server::encoder::{EncoderError, OpusFile};

fn main() -> Result<(), EncoderError> {
    let parser = App::new("Opus test tool")
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("start-offset")
                .short("s")
                .long("initial-offset")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("repeat-offset")
                .default_value("20000")
                .short("o")
                .long("repeat-offset")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("file")
                .takes_value(true)
                .value_name("FILE")
                .required(true),
        )
        .arg(
            Arg::with_name("out-file")
                .takes_value(true)
                .value_name("OUT_FILE")
                .required(true),
        );

    let matches = parser.get_matches();

    let start_offset = matches
        .value_of("start-offset")
        .expect("You need to provide a start offset")
        .parse::<usize>()
        .unwrap();
    let mut out_file = File::create(matches.value_of("out-file").unwrap())
        .expect("Failed to write to output file");

    let mut in_file = OpusFile::create(&[matches.value_of("file").unwrap()])?;

    let mut total_pos = 0_u64;
    let mut start_data = Vec::with_capacity(start_offset);
    for _ in 0..start_offset {
        start_data.push(0);
    }
    total_pos += write_bytes_to_file(&mut out_file, &mut in_file, &mut start_data)?;

    let repeat_offset = matches
        .value_of("repeat-offset")
        .expect("You need to provide a repeat offset")
        .parse::<usize>()
        .unwrap();
    let mut data = Vec::with_capacity(repeat_offset);
    for _ in 0..repeat_offset {
        data.push(0);
    }
    loop {
        let mut in_file = OpusFile::create(&[matches.value_of("file").unwrap()])?;
        in_file
            .seek(SeekFrom::Start(total_pos))
            .expect("Seeking failed :(");
        let read = write_bytes_to_file(&mut out_file, &mut in_file, &mut data)?;
        total_pos += read;
        if read == 0 {
            break;
        }
    }
    Ok(())
}

fn write_bytes_to_file(
    out_file: &mut File,
    in_file: &mut OpusFile,
    buffer: &mut [u8],
) -> Result<u64, EncoderError> {
    let mut total_read = 0;
    loop {
        let read = in_file.read(&mut buffer[total_read..]).unwrap();
        total_read += read;

        if read == 0 {
            break;
        }
    }
    out_file
        .write_all(buffer)
        .expect("Failed to write data to out file");
    if total_read != buffer.len() {
        println!(
            "Only read {} bytes of targeted {}",
            total_read,
            buffer.len()
        );
    }
    Ok(total_read as u64)
}
