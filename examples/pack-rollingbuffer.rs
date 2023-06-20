//! Port of the [libmseed
//! `lm_pack_rollingbuffer.c`](https://github.com/EarthScope/libmseed/blob/develop/example/lm_pack_rollingbuffer.c)
//! example.
//!
//! For further information on how to use this example program, simply invoke:
//!
//! ```sh
//! cargo run --example pack-rollingbuffer -- --help
//! ```

use std::path::PathBuf;

use clap::Parser;

use mseed::{MSControlFlags, MSDataEncoding, MSReader, MSRecord, MSTraceList, TlPackInfo};

#[derive(Parser, Debug)]
#[command(author, version)]
#[command(about = "Illustrates using a trace list as an itermediate rolling buffer")]
#[command( long_about = None)]
struct Args {
    /// Path to miniSEED file.
    #[arg(value_name = "FILE")]
    in_file: PathBuf,
}

fn main() {
    let args = Args::parse();

    let mut mstl = MSTraceList::new().unwrap();

    let pack_info = TlPackInfo {
        encoding: MSDataEncoding::Steim2,
        rec_len: 256,
        extra_headers: None,
    };

    // A simple record handler callback function that parses and prints records
    let record_handler = |rec: &[u8]| {
        let mut buf = rec.to_vec();
        let msr = MSRecord::parse(&mut buf, MSControlFlags::MSF_UNPACKDATA).unwrap();

        println!("{}", msr);
    };

    // Create a reader
    let mut reader = MSReader::new_with_flags(
        args.in_file,
        MSControlFlags::MSF_VALIDATECRC | MSControlFlags::MSF_UNPACKDATA,
    )
    .unwrap();

    // Loop over the reader
    while let Some(msr) = reader.next() {
        let msr = msr.unwrap();

        mstl.insert(msr, true).unwrap();

        let (cnt_records, cnt_samples) = mstl
            .pack(record_handler, &pack_info, MSControlFlags::empty())
            .unwrap();

        println!(
            "mstl.pack() created {} records containing {} samples, totally",
            cnt_records, cnt_samples
        );
    }

    // Final call to flush data buffers - now with `MSControlFlags::MSF_FLUSHDATA` enabled
    let (cnt_records, cnt_samples) = mstl
        .pack(record_handler, &pack_info, MSControlFlags::MSF_FLUSHDATA)
        .unwrap();

    println!(
        "Final mstl.pack() created {} records containing {} samples, totally",
        cnt_records, cnt_samples
    );
}
