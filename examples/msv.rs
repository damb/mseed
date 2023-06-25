//! Port of [libmseed
//! `mseedview.c`](https://github.com/EarthScope/libmseed/blob/main/example/mseedview.c) example.
//!
//! For further information on how to use this example program, simply invoke:
//!
//! ```sh
//! cargo run --example msv -- --help
//! ``

use std::fmt;
use std::path::PathBuf;

use clap::Parser;

use mseed::{MSControlFlags, MSReader, MSSampleType};

const NUM_COLUMNS: usize = 6;

#[derive(Parser, Debug)]
#[command(author, version)]
#[command(about = "Illustrates reading and viewing miniSEED data", long_about = None)]
struct Args {
    /// Pring header details.
    #[arg(short='p', long, action = clap::ArgAction::Count)]
    header_detail: u8,

    /// Print first 6 sample values.
    ///
    /// If the sample type is text, the first 6 characters are printed.
    #[arg(short = 'd')]
    print_data: bool,

    /// Print all sample values.
    #[arg(short = 'D')]
    print_data_all: bool,

    /// Print basic summary.
    #[arg(short = 's', long)]
    basic_summary: bool,

    /// Path to miniSEED file.
    ///
    /// Parsing a byte range is possible with the '@'-syntax, e.g.
    /// path/to/data.mseed[@[FROM][-TO]]
    #[arg(value_name = "FILE[@[FROM][-TO]]")]
    in_file: PathBuf,
}

fn print_numeric_data_samples<T: fmt::Display>(data_samples: &[T], print_all: bool) {
    let mut col = 0;
    for sample in data_samples {
        print!("{:10.8} ", sample);

        col += 1;
        if col == NUM_COLUMNS {
            col = 0;
            print!("\n");

            if !print_all {
                return;
            }
        }
    }
    if col != 0 {
        print!("\n");
    }
}

fn main() {
    let args = Args::parse();

    // Validate CRCs when reading
    let mut flags = MSControlFlags::MSF_VALIDATECRC;
    // Enable parsing a byte range
    flags |= MSControlFlags::MSF_PNAMERANGE;

    let print_data = args.print_data || args.print_data_all;
    if print_data {
        flags |= MSControlFlags::MSF_UNPACKDATA;
    }

    let mut rec_cnt = 0;
    let mut sample_cnt = 0;

    // Create reader
    let mut reader = MSReader::new_with_flags(args.in_file, flags).unwrap();
    // Loop over miniSEED records
    while let Some(msr) = reader.next() {
        let mut msr = msr.unwrap();

        rec_cnt += 1;
        sample_cnt += msr.sample_cnt();
        print!("{}", msr.display(args.header_detail as i8));

        if print_data {
            match msr.sample_type() {
                MSSampleType::Text => {
                    let data_samples = msr.data_samples::<u8>();
                    if data_samples.is_none() {
                        continue;
                    }

                    let text = String::from_utf8_lossy(data_samples.unwrap());
                    if args.print_data_all {
                        println!("{}", text);
                    } else {
                        println!("{}", &text[..NUM_COLUMNS]);
                    }
                }
                MSSampleType::Integer32 => {
                    let data_samples = msr.data_samples::<i32>();
                    if data_samples.is_none() {
                        continue;
                    }
                    print_numeric_data_samples(data_samples.unwrap(), args.print_data_all);
                }
                MSSampleType::Float32 => {
                    let data_samples = msr.data_samples::<f32>();
                    if data_samples.is_none() {
                        continue;
                    }
                    print_numeric_data_samples(data_samples.unwrap(), args.print_data_all);
                }
                MSSampleType::Float64 => {
                    let data_samples = msr.data_samples::<f64>();
                    if data_samples.is_none() {
                        continue;
                    }
                    print_numeric_data_samples(data_samples.unwrap(), args.print_data_all);
                }
                _ => continue,
            }
        }
    }

    if args.basic_summary {
        println!("Records: {}, Samples: {}", rec_cnt, sample_cnt);
    }
}
