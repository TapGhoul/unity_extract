use clap::Parser;
use flate2::read::GzDecoder;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use tar::{Archive, Entry};

#[derive(Parser, Debug)]
/// Extract a unity unitypackage file
struct Args {
    /// The unitypackage file to extract from
    input: PathBuf,
    /// The destination directory (current if not set)
    output: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();

    let extract_base = args.output.unwrap_or_default();

    let f = &mut File::open(args.input).expect("No such file");

    let mut paths = HashMap::new();

    for_archive(f, "pathname", |mut entry, id| {
        let mut buf = String::new();
        entry
            .read_to_string(&mut buf)
            .expect("failed to read path name");

        let trimmed = buf
            .split_terminator(&['\0', '\r', '\n'])
            .next()
            .expect("Invalid pathname");

        let parsed_path = PathBuf::from(trimmed);

        // This isn't very good, but eh
        if !parsed_path.is_relative() || parsed_path.components().any(|e| e.as_os_str() == "..") {
            println!("ERROR: {id} is a potentially dangerous path - {buf}");
            return;
        }

        paths.insert(id, parsed_path);
    });

    for_archive(f, "asset.meta", |mut entry, id| {
        let mut buf = String::new();
        entry
            .read_to_string(&mut buf)
            .expect("failed to read asset meta");

        if buf.lines().any(|e| e == "folderAsset: yes") {
            paths.remove(&id);
        }
    });

    for_archive(f, "asset", |mut entry, id| {
        let path = paths.remove(&id).expect("Can't unpack asset with no path");
        let path = extract_base.join(path);

        let mut dir = path.clone();
        dir.pop();
        std::fs::create_dir_all(dir).expect("Failed to unpack");

        let mut w = File::create(path).expect("failed to create file");
        std::io::copy(&mut entry, &mut w).expect("failed to write");
    });

    for (id, path) in paths.into_iter() {
        let path = path.to_string_lossy();
        println!("WARN: {path} ({id}) was never unpacked");
    }
}

fn for_archive<F>(f: &mut File, asset_type: &str, mut cb: F)
where
    F: FnMut(Entry<GzDecoder<&mut File>>, String) -> (),
{
    f.seek(SeekFrom::Start(0)).expect("failed to seek");
    let decoder = GzDecoder::new(f);
    let mut archive = Archive::new(decoder);
    let entries = archive.entries().expect("no entries");

    for entry in entries.filter_map(|e| e.ok()) {
        let id = {
            let path = entry.path().expect("no path");
            let mut p_iter = path.iter();
            let id = p_iter.next().expect("no ID").to_string_lossy().into_owned();
            let Some(ext) = p_iter.next() else { continue };

            if ext != asset_type {
                continue;
            }

            id
        };
        cb(entry, id);
    }
}
