//! Builds ZIP fixtures in temporary directories.
#![allow(dead_code)]
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn build_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("create zip");
    let mut writer = zip::ZipWriter::new(file);
    for (name, content) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .expect("start entry");
        writer.write_all(content).expect("write entry");
    }
    writer.finish().expect("finish zip");
}
