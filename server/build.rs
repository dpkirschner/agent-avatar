//! Embeds the kiosk page (`app/`) into the binary: writes
//! `$OUT_DIR/assets.rs` with every file under `app/` as
//! `(path, bytes)`, sorted by path. Adding a card, status template,
//! theme or avatar folder needs no Rust change to be served.
//!
//! Nothing here may hold an absolute path: deploy.sh builds several
//! worktrees into one target dir, and Cargo reuses this script's output
//! across them. The watched path is relative to the package and the
//! generated `include_bytes!` resolve through `CARGO_MANIFEST_DIR` at
//! compile time, so each worktree embeds its own `app/`.
use std::path::{Path, PathBuf};

fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for p in entries {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            walk(&p, root, out);
        } else {
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../app");
    // Relative to the package root, so it means this worktree's app/.
    println!("cargo:rerun-if-changed=../app");
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    let mut src = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for rel in &files {
        src.push_str(&format!(
            "    ({rel:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../app/\", {rel:?}))),\n"
        ));
    }
    src.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("assets.rs");
    std::fs::write(out, src).unwrap();
}
