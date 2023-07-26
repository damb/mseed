//! Convert miniSEED records into specified version format.
//!
//! The input may contain miniSEED records of different format versions.
//!
//! For further information on how to use this example program, simply invoke:
//!
//! ```sh
//! cargo run --example convert -- --help
//! ``

use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::rc::Rc;

use clap::Parser;

use mseed::{MSControlFlags, MSReader, MSSampleType, PackInfo};

const MSEED2_RECORD_LENGTH: i32 = 512;

#[derive(Parser, Debug)]
#[command(author, version)]
#[command(about = "Convert miniSEED record version format", long_about = None)]
struct Args {
    /// Specify target miniSEED version format.
    #[arg(short = 'F', long, value_name = "FORMAT", default_value_t = 3)]
    #[arg(value_parser = clap::value_parser!(u8).range(2..4))]
    format: u8,

    /// Print basic summary.
    #[arg(short = 's', long)]
    basic_summary: bool,

    /// Path to miniSEED input file.
    ///
    /// Parsing a byte range is possible with the '@'-syntax, e.g.
    /// path/to/data.mseed[@[FROM][-TO]]
    #[arg(value_name = "INFILE[@[FROM][-TO]]")]
    in_file: PathBuf,

    /// Path to output file.
    #[arg(value_name = "OUTFILE")]
    out_file: PathBuf,
}

fn main() {
    let args = Args::parse();

    // Create reader
    let mut reader =
        MSReader::new_with_flags(args.in_file, MSControlFlags::MSF_PNAMERANGE).unwrap();
    // Create sink
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(args.out_file)
        .unwrap();
    let writer = Rc::new(RefCell::new(BufWriter::new(file)));

    let mut rec_in_cnt = 0;
    let mut rec_out_cnt = 0;
    // Loop over miniSEED records
    while let Some(msr) = reader.next() {
        let mut msr = msr.unwrap();

        rec_in_cnt += 1;

        if msr.format_version() == args.format {
            writer.borrow_mut().write_all(msr.raw().unwrap()).unwrap();
            rec_out_cnt += 1;
        } else if msr.format_version() == 2 && args.format == 3 {
            let mut buf = vec![0; msr.raw().unwrap().len() * 2];
            let bytes_packed = mseed::repack_mseed3(&msr, &mut buf).unwrap();
            writer.borrow_mut().write_all(&buf[..bytes_packed]).unwrap();
            rec_out_cnt += 1;
        } else {
            // msr.format_version() == 3 && args.format == 2
            // requires manual repacking
            let writer = writer.clone();
            let record_handler = move |rec: &[u8]| {
                let _ = writer.borrow_mut().write_all(rec).unwrap();
            };

            msr.unpack_data().unwrap();
            let mut pack_info = PackInfo::new(msr.sid().unwrap()).unwrap();
            pack_info.rec_len = MSEED2_RECORD_LENGTH;
            pack_info.encoding = msr.encoding().unwrap();
            let flags = MSControlFlags::MSF_PACKVER2 | MSControlFlags::MSF_FLUSHDATA;

            let num_packed_recs = match msr.sample_type() {
                MSSampleType::Text => {
                    let mut data_samples = msr.data_samples::<u8>().unwrap().to_vec();
                    let (num_packed_recs, _) = mseed::pack_raw(
                        &mut data_samples,
                        &msr.start_time().unwrap(),
                        record_handler,
                        &pack_info,
                        flags,
                    )
                    .unwrap();

                    num_packed_recs
                }
                MSSampleType::Integer32 => {
                    let mut data_samples = msr.data_samples::<i32>().unwrap().to_vec();
                    let (num_packed_recs, _) = mseed::pack_raw(
                        &mut data_samples,
                        &msr.start_time().unwrap(),
                        record_handler,
                        &pack_info,
                        flags,
                    )
                    .unwrap();

                    num_packed_recs
                }
                MSSampleType::Float32 => {
                    let mut data_samples = msr.data_samples::<f32>().unwrap().to_vec();
                    let (num_packed_recs, _) = mseed::pack_raw(
                        &mut data_samples,
                        &msr.start_time().unwrap(),
                        record_handler,
                        &pack_info,
                        flags,
                    )
                    .unwrap();

                    num_packed_recs
                }
                MSSampleType::Float64 => {
                    let mut data_samples = msr.data_samples::<f32>().unwrap().to_vec();
                    let (num_packed_recs, _) = mseed::pack_raw(
                        &mut data_samples,
                        &msr.start_time().unwrap(),
                        record_handler,
                        &pack_info,
                        flags,
                    )
                    .unwrap();

                    num_packed_recs
                }
                _ => 0,
            };

            rec_out_cnt += num_packed_recs;
        }
    }

    if args.basic_summary {
        println!(
            "Records (in): {}, Records (out): {}",
            rec_in_cnt, rec_out_cnt
        );
    }
}
