//! The bundle script and the Linux entry declare exactly what otherwise-link generates.
use otherwise_link::register;
use vigia_core::links::{BUNDLE_ID, DESKTOP_FILE, SCHEME};

fn repo(p: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)).unwrap().replace("\r\n", "\n")
}

#[test]
fn macos_info_plist_registers_the_scheme() {
    assert!(repo("script/bundle-macos.sh").contains(&register::macos_url_types(BUNDLE_ID, &[SCHEME])));
}

#[test]
fn linux_desktop_entry_takes_links() {
    let d = repo(&format!("packaging/linux/{DESKTOP_FILE}"));
    assert_eq!(register::linux_desktop_patch(&d, &[SCHEME]), d, "already patched");
    assert!(d.contains("MimeType=x-scheme-handler/otherwise-vigia;") && d.contains("Exec=vigia-app %u"));
    assert_eq!(otherwise_link::apps::known("vigia").map(|a| a.bundle_id), Some(BUNDLE_ID));
}
