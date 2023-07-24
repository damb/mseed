# mseed

[![Crates.io](https://img.shields.io/crates/v/mseed)](https://crates.io/crates/mseed)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/damb/mseed/rust.yml?branch=main)](https://github.com/damb/mseed/actions/workflows/rust.yml?query=branch%3Amain)

Rust bindings for [libmseed](https://github.com/EarthScope/libmseed) - The miniSEED data format library.

## Usage

mseed uses [Cargo](https://crates.io), so add it with `cargo add mseed` or
modify `Cargo.toml`:

```toml
[dependencies]
mseed = "0.3"
```

## Documentation

For the crate's documentation please refer to
[docs.rs/mseed](https://docs.rs/mseed/).

## Examples

Please refer to the libraries' [examples](examples/).

## Version of libmseed

Currently this library requires `libmseed` version 3.0.15 (or newer patch
versions). The source for `libmseed` is included in the `libmseed-sys` crate so
there's no need to pre-install the `libmseed` library, the `libmseed-sys` crate
will figure that and/or build that for you.

## Building mseed

```sh
git clone https://github.com/damb/mseed
cd mseed
cargo build
```

## Contribute

Any PR is very welcomed!

## License

Licensed under the [Apache-2.0 license](https://www.apache.org/licenses/LICENSE-2.0).
For more information see the [LICENSE](/LICENSE) file.

