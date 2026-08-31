use std::{
    env, fs,
    path::{Path, PathBuf},
};

const RULES_VERSION_MARKER: &str = "const RULES_VERSION: RulesVersion = RulesVersion(";

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=game.toml");

    let sources = source_files();
    let source_version = rules_version_from_source(&sources);
    let manifest_version = rules_version_from_manifest();
    assert_eq!(
        source_version, manifest_version,
        "game.toml rules_version must match rules.rs RULES_VERSION"
    );

    // Deliberately hash every Rust source under src/. This conservative set
    // keeps behavior-affecting helpers from silently falling outside replay
    // identity when a new module is added.
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.rules.v1");
    hasher.update(&source_version.to_le_bytes());
    for (relative, path) in sources {
        let bytes =
            fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let relative = relative.to_string_lossy().replace('\\', "/");
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(out.join("rules_hash.bin"), hasher.finalize().as_bytes())
        .expect("write rules_hash.bin");
}

fn source_files() -> Vec<(PathBuf, PathBuf)> {
    let root = Path::new("src");
    let mut files = Vec::new();
    collect_rust_sources(root, root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn collect_rust_sources(root: &Path, directory: &Path, files: &mut Vec<(PathBuf, PathBuf)>) {
    let mut entries: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.unwrap_or_else(|error| panic!("read source entry: {error}")))
        .collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(root, &path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let relative = path
                .strip_prefix(root)
                .unwrap_or_else(|_| panic!("source path escaped src: {}", path.display()))
                .to_owned();
            files.push((relative, path));
        }
    }
}

fn rules_version_from_source(sources: &[(PathBuf, PathBuf)]) -> u32 {
    let (_, rules_path) = sources
        .iter()
        .find(|(relative, _)| relative == Path::new("rules.rs"))
        .expect("src/rules.rs is required");
    let source = fs::read_to_string(rules_path).expect("read src/rules.rs");
    let value = source
        .split_once(RULES_VERSION_MARKER)
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(value, _)| value.trim())
        .and_then(|value| value.parse().ok())
        .expect("src/rules.rs must define RulesVersion with a numeric RULES_VERSION");
    value
}

fn rules_version_from_manifest() -> u32 {
    let source = fs::read_to_string("game.toml").expect("read game.toml");
    source
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            if key.trim() != "rules_version" {
                return None;
            }
            value.split('#').next()?.trim().parse().ok()
        })
        .expect("game.toml must define numeric rules_version")
}
