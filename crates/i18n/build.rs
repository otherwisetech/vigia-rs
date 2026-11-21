//! Embeds every `locales/<lang>/*.ftl` file so adding a language or a module file needs no code.
use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut langs: Vec<_> = std::fs::read_dir(&root).unwrap().filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).map(|e| e.path()).collect();
    langs.sort();
    let mut out = String::from("pub(crate) static CATALOG: &[(&str, &[(&str, &str)])] = &[\n");
    for dir in langs {
        println!("cargo:rerun-if-changed={}", dir.display());
        let lang = dir.file_name().unwrap().to_string_lossy().into_owned();
        let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ftl")).collect();
        files.sort();
        writeln!(out, "    ({lang:?}, &[").unwrap();
        for f in files {
            println!("cargo:rerun-if-changed={}", f.display());
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            writeln!(out, "        ({name:?}, include_str!({:?})),", f.display().to_string()).unwrap();
        }
        out.push_str("    ]),\n");
    }
    out.push_str("];\n");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("catalog.rs"), out).unwrap();
}
