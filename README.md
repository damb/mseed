# mseed

Rust bindings for [libmseed](https://github.com/EarthScope/libmseed).

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

