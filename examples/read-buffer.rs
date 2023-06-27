//! Port of [libmseed
//! `lm_read_buffer.c`](https://github.com/EarthScope/libmseed/blob/main/example/lm_read_buffer.c)
//! example.
//!
//! For further information on how to use this example program, simply invoke:
//!
//! ```sh
//! cargo run --example read-buffer -- --help
//! ```

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;

use clap::Parser;

use mseed::{MSControlFlags, MSTimeFormat, MSTraceList};

#[derive(Parser, Debug)]
#[command(author, version)]
#[command(about = "Illustrates reading miniSEED from buffers", long_about = None)]
struct Args {
    /// Path to output file.
    #[arg(value_name = "FILE")]
    in_file: PathBuf,
}

fn main() {
    let args = Args::parse();

    // Read specified file into buffer
    let file = File::open(args.in_file).unwrap();
    let mut reader = BufReader::new(file);

    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).unwrap();

    // Set control flags to validate CRC and unpack data samples
    let flags = MSControlFlags::MSF_VALIDATECRC | MSControlFlags::MSF_UNPACKDATA;
    // Create a `MSTraceList` from a buffer
    let mstl = MSTraceList::from_buffer(&mut buf, flags).unwrap();
    // Print summary
    print!("{}", mstl.display(MSTimeFormat::IsoMonthDay, 1, 1, 0));
}
