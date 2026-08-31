use std::{env, fs, path::PathBuf};

const RULES_VERSION: u32 = 3;
const RULES_FILES: &[&str] = &[
    "src/clock.rs",
    "src/movegen.rs",
    "src/rules.rs",
    "src/state.rs",
];

fn main() {
    for path in RULES_FILES {
        println!("cargo:rerun-if-changed={path}");
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"tabula.rules.v1");
    hasher.update(&RULES_VERSION.to_le_bytes());
    for path in RULES_FILES {
        let bytes = fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
        hasher.update(&(path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(out.join("rules_hash.bin"), hasher.finalize().as_bytes())
        .expect("write rules_hash.bin");
}
