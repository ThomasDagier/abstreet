use std::collections::BTreeMap;
use std::io::{BufReader, Read};
use std::process::Command;

use anyhow::Result;
use fs_err::File;
use structopt::StructOpt;
use walkdir::WalkDir;

use abstio::{DataPacks, Entry, Manifest};
use abstutil::{must_run_cmd, prettyprint_usize, Timer};

const MD5_BUF_READ_SIZE: usize = 4096;

#[derive(StructOpt)]
#[structopt(
    name = "updater",
    about = "Download the latest version of per-city data"
)]
enum Task {
    /// Synchronize the source-of-truth in S3 with data in the local directory. Based on current
    /// permissions, only Dustin runs this.
    Upload,
    /// This uploads to S3 from cloud VMs that import maps. This never deletes files from S3, only
    /// updates or creates ne ones.
    IncrementalUpload {
        /// Upload data to a temporary version managed by the cloud scripts.
        #[structopt(long)]
        version: String,
    },
    /// Just compare data in the current directory with the manifest, and describe any new,
    /// deleted, or modified files.
    DryRun {
        /// Just check if one file has changed.
        #[structopt(long)]
        single_file: Option<String>,
    },
    /// Print the JSON list of all possible city data packs to download. You can write this output
    /// to `data/player/data.json`, then download everything.
    OptIntoAll,
    /// Synchronize the local `data` directory with the source-of-truth in S3.
    Download {
        /// The Github Actions build uses this to include only a few files for the release to be
        /// usable. People can use the UI to open another map and download more data.
        #[structopt(long)]
        minimal: bool,
        /// Only update files from the manifest. Leave extra files alone.
        #[structopt(long)]
        dont_delete: bool,
        /// Only useful for Dustin. "Download" from my local S3 source-of-truth, not from the
        /// network.
        #[structopt(long)]
        dl_from_local: bool,
        /// Download data tied to a named release. See
        /// https://a-b-street.github.io/docs/tech/dev/data.html.
        #[structopt(long, default_value = "dev")]
        version: String,
    },
}

#[tokio::main]
async fn main() {
    if !std::path::Path::new("data").exists() {
        panic!("Your current directory doesn't have the data/ directory. Run from the git root: cd ../; cargo run --bin updater -- args");
    }

    abstutil::logger::setup();
    match Task::from_args() {
        Task::Upload => {
            upload("dev");
        }
        Task::IncrementalUpload { version } => {
            // We DON'T want to override the main data immediately from the batch Docker jobs. If
            // running locally, can temporarily disable this assertion.
            assert_ne!(version, "dev");
            incremental_upload(version);
        }
        Task::DryRun { single_file } => {
            if let Some(path) = single_file {
                let local = md5sum(&path);
                let truth = Manifest::load()
                    .entries
                    .remove(&path)
                    .unwrap_or_else(|| panic!("{} not in data/MANIFEST.txt", path))
                    .checksum;
                if local != truth {
                    println!("{} has changed", path);
                }
            } else {
                just_compare();
            }
        }
        Task::OptIntoAll => {
            println!("{}", abstutil::to_json(&DataPacks::all_data_packs()));
        }
        Task::Download {
            minimal,
            dont_delete,
            dl_from_local,
            version,
        } => {
            download_updates(version, minimal, !dont_delete, dl_from_local).await;
        }
    }
}

async fn download_updates(version: String, minimal: bool, delete_local: bool, dl_from_local: bool) {
    let data_packs = DataPacks::load_or_create();
    let truth = Manifest::load().filter(data_packs);
    let local = generate_manifest(&truth);

    // Anything local need deleting?
    if delete_local {
        for path in local.entries.keys() {
            if !truth.entries.contains_key(path) {
                rm(path);
            }
        }
    }

    // Anything missing or needing updating?
    let mut failed = Vec::new();
    for (path, entry) in truth.entries {
        if local.entries.get(&path).map(|x| &x.checksum) != Some(&entry.checksum) {
            if minimal && !path.contains("montlake") && path != "data/system/us/seattle/city.bin" {
                continue;
            }

            fs_err::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
            match download_file(&version, &path, dl_from_local).await {
                Ok(bytes) => {
                    println!(
                        "> decompress {}, which is {} bytes compressed",
                        path,
                        prettyprint_usize(bytes.len())
                    );
                    let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
                    let mut out = File::create(&path).unwrap();
                    if let Err(err) = std::io::copy(&mut decoder, &mut out) {
                        println!("{}, but continuing", err);
                        failed.push(format!("{} failed: {}", path, err));
                    }
                }
                Err(err) => {
                    println!("{}, but continuing", err);
                    failed.push(format!("{} failed: {}", path, err));
                }
            };
        }
    }
    if !failed.is_empty() {
        // Fail the build.
        panic!("Failed to download stuff: {:?}", failed);
    }

    remove_empty_directories("data/input");
    remove_empty_directories("data/system");
}

fn just_compare() {
    let data_packs = DataPacks::load_or_create();
    let truth = Manifest::load().filter(data_packs);
    let local = generate_manifest(&truth);

    // Anything local need deleting?
    for path in local.entries.keys() {
        if !truth.entries.contains_key(path) {
            println!("- Remove {}", path);
        }
    }

    // Anything missing or needing updating?
    for (path, entry) in truth.entries {
        if local.entries.get(&path).map(|x| &x.checksum) != Some(&entry.checksum) {
            if let Some(old_bytes) = local.entries.get(&path).map(|x| x.uncompressed_size_bytes) {
                if old_bytes == entry.uncompressed_size_bytes {
                    println!("- Update {}. Same size, md5sum changed", path);
                } else {
                    println!(
                        "- Update {}. {} bytes difference",
                        path,
                        (old_bytes as isize) - (entry.uncompressed_size_bytes as isize)
                    );
                }
            } else {
                println!("- Add {}", path);
            }
        }
    }
}

fn upload(version: &str) {
    let remote_base = format!("/home/dabreegster/s3_abst_data/{}", version);

    let remote: Manifest = abstio::maybe_read_json(
        format!("{}/MANIFEST.json", remote_base),
        &mut Timer::throwaway(),
    )
    .unwrap_or(Manifest {
        entries: BTreeMap::new(),
    });
    let mut local = generate_manifest(&remote);

    // Anything remote need deleting?
    for path in remote.entries.keys() {
        if !local.entries.contains_key(path) {
            rm(&format!("{}/{}.gz", remote_base, path));
        }
    }

    // Anything missing or needing updating?
    let local_entries = std::mem::take(&mut local.entries);
    for (path, entry) in Timer::new("compress files").parallelize(
        "compress files",
        local_entries.into_iter().collect(),
        |(path, mut entry)| {
            let remote_path = format!("{}/{}.gz", remote_base, path);
            let changed = remote.entries.get(&path).map(|x| &x.checksum) != Some(&entry.checksum);
            if changed {
                compress(&path, &remote_path);
            }
            // Always do this -- even if nothing changed, compressed_size_bytes isn't filled out by
            // generate_manifest.
            entry.compressed_size_bytes = fs_err::metadata(&remote_path)
                .unwrap_or_else(|_| panic!("Compressed {} not there?", remote_path))
                .len();
            (path, entry)
        },
    ) {
        local.entries.insert(path, entry);
    }

    abstio::write_json(format!("{}/MANIFEST.json", remote_base), &local);
    abstio::write_json("data/MANIFEST.json".to_string(), &local);

    must_run_cmd(
        Command::new("aws")
            .arg("s3")
            .arg("sync")
            .arg("--delete")
            .arg(format!("{}/data", remote_base))
            .arg(format!("s3://abstreet/{}/data", version)),
    );
    // Because of the directory structure, do this one separately, without --delete. The wasm files
    // also live in /dev/.
    must_run_cmd(
        Command::new("aws")
            .arg("s3")
            .arg("cp")
            .arg(format!("{}/MANIFEST.json", remote_base))
            .arg(format!("s3://abstreet/{}/MANIFEST.json", version)),
    );
}

fn incremental_upload(version: String) {
    let remote_base = "tmp_incremental_upload";

    // Assume the local copy of the manifest from git is the current source of truth.
    let mut truth = Manifest::load();
    let local = generate_manifest(&truth);

    // Anything missing or needing updating?
    let mut changes = false;
    for (path, entry) in Timer::new("compress files")
        .parallelize(
            "compress files",
            local.entries.into_iter().collect(),
            |(path, mut entry)| {
                if truth.entries.get(&path).map(|x| &x.checksum) != Some(&entry.checksum) {
                    let remote_path = format!("{}/{}.gz", remote_base, path);
                    compress(&path, &remote_path);
                    entry.compressed_size_bytes = fs_err::metadata(&remote_path)
                        .unwrap_or_else(|_| panic!("Compressed {} not there?", remote_path))
                        .len();
                    Some((path, entry))
                } else {
                    None
                }
            },
        )
        .into_iter()
        .flatten()
    {
        truth.entries.insert(path, entry);
        changes = true;
    }
    if !changes {
        return;
    }

    // TODO /home/dabreegster/s3_abst_data/{version}/MANIFEST.json will get out of sync...
    abstio::write_json("data/MANIFEST.json".to_string(), &truth);

    must_run_cmd(
        Command::new("aws")
            .arg("s3")
            .arg("sync")
            .arg(format!("{}/data", remote_base))
            .arg(format!("s3://abstreet/{}/data", version)),
    );
    // Upload the new manifest file to S3.
    // TODO This won't work from AWS Batch; the workers will stomp over each other.
    must_run_cmd(
        Command::new("aws")
            .arg("s3")
            .arg("cp")
            .arg("data/MANIFEST.json")
            .arg(format!("s3://abstreet/{}/MANIFEST.json", version)),
    );

    // Nuke the temporary workspace
    must_run_cmd(Command::new("rm").arg("-rfv").arg(remote_base));
}

fn generate_manifest(truth: &Manifest) -> Manifest {
    let mut paths = Vec::new();
    for entry in WalkDir::new("data/input")
        .into_iter()
        .chain(WalkDir::new("data/system").into_iter())
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            continue;
        }
        let orig_path = entry.path().display().to_string();
        let path = orig_path.replace("\\", "/");
        if path.contains("system/assets/")
            || path.contains("system/ltn_proposals")
            || path.contains("system/proposals")
            || path.contains("system/study_areas")
        {
            continue;
        }
        paths.push((orig_path, path));
    }

    let mut kv = BTreeMap::new();
    for (path, entry) in
        Timer::new("compute md5sums").parallelize("compute md5sums", paths, |(orig_path, path)| {
            // If the file's modtime is newer than 12 hours or the uncompressed size has changed,
            // calculate md5sum. Otherwise assume no change. This heuristic saves lots of time and
            // doesn't stress my poor SSD as much.
            let metadata = fs_err::metadata(&orig_path).unwrap();
            let uncompressed_size_bytes = metadata.len();
            let recent_modtime = metadata.modified().unwrap().elapsed().unwrap()
                < std::time::Duration::from_secs(60 * 60 * 12);

            let checksum = if recent_modtime
                || truth
                    .entries
                    .get(&path)
                    .map(|entry| entry.uncompressed_size_bytes != uncompressed_size_bytes)
                    .unwrap_or(true)
            {
                md5sum(&orig_path)
            } else {
                truth.entries[&path].checksum.clone()
            };
            (
                path,
                Entry {
                    checksum,
                    uncompressed_size_bytes,
                    // Will calculate later
                    compressed_size_bytes: 0,
                },
            )
        })
    {
        kv.insert(path, entry);
    }

    Manifest { entries: kv }
}

fn md5sum(path: &str) -> String {
    // since these files can be very large, computes the md5 hash in chunks
    let mut file = File::open(path).unwrap();
    let mut buffer = [0_u8; MD5_BUF_READ_SIZE];
    let mut context = md5::Context::new();
    while let Ok(n) = file.read(&mut buffer) {
        if n == 0 {
            break;
        }
        context.consume(&buffer[..n]);
    }
    format!("{:x}", context.compute())
}

fn rm(path: &str) {
    println!("> rm {}", path);
    match fs_err::remove_file(path) {
        Ok(_) => {}
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                println!("file {} does not exist, continuing", &path);
            }
            other_error => {
                panic!("problem removing file: {:?}", other_error);
            }
        },
    }
}

async fn download_file(version: &str, path: &str, dl_from_local: bool) -> Result<Vec<u8>> {
    if dl_from_local {
        return abstio::slurp_file(format!(
            "/home/dabreegster/s3_abst_data/{}/{}.gz",
            version, path
        ));
    }

    let url = format!("http://play.abstreet.org/{}/{}.gz", version, path);
    println!("> download {}", url);
    let (mut tx, rx) = futures_channel::mpsc::channel(1000);
    abstio::print_download_progress(rx);
    abstio::download_bytes(url, None, &mut tx).await
}

// download() will remove stray files, but leave empty directories around. Since some runtime code
// discovers lists of countries, cities, etc from the filesystem, this can get confusing.
//
// I'm sure there's a simpler way to do this, but I haven't found it.
fn remove_empty_directories(root: &str) {
    loop {
        // First just find all directories and files.
        let mut all_paths = Vec::new();
        let mut all_dirs = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path().display().to_string();
            all_paths.push(path.clone());
            if entry.file_type().is_dir() {
                all_dirs.push(path);
            }
        }

        // Now filter out directories that're a prefix of some path.
        all_dirs.retain(|dir| !all_paths.iter().any(|p| p != dir && p.starts_with(dir)));

        if all_dirs.is_empty() {
            break;
        } else {
            // Remove them! Then repeat, since we might have nested/empty/directories/.
            for x in all_dirs {
                println!("> Removing empty directory {}", x);
                // This fails if the directory isn't empty, which is a good sanity check. If
                // something weird happened, just bail.
                fs_err::remove_dir(&x).unwrap();
            }
        }
    }
}

fn compress(path: &str, remote_path: &str) {
    assert!(!path.ends_with(".gz"));
    assert!(remote_path.ends_with(".gz"));

    fs_err::create_dir_all(std::path::Path::new(remote_path).parent().unwrap()).unwrap();
    println!("> compressing {}", path);
    let mut input = BufReader::new(File::open(path).unwrap());
    let out = File::create(remote_path).unwrap();
    let mut encoder = flate2::write::GzEncoder::new(out, flate2::Compression::best());
    std::io::copy(&mut input, &mut encoder).unwrap();
    encoder.finish().unwrap();
}
