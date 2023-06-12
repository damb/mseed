use std::path::PathBuf;

pub fn test_data_base_dir() -> PathBuf {
    let mut base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base_dir.push("tests/data");

    base_dir
}
