//! Backups: one zip with a consistent copy of the database (`vigia.db`) and the attachments
//! folder (`vigia-files/…`). Restoring unpacks it into a folder, ready to open.
use crate::db::{FILES_DIR, Store, StoreError};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

fn add_dir(zip: &mut zip::ZipWriter<std::fs::File>, dir: &Path, prefix: &str) -> std::io::Result<usize> {
    let mut n = 0;
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let name = format!("{prefix}/{}", e.file_name().to_string_lossy());
        if p.is_dir() {
            n += add_dir(zip, &p, &name)?;
        } else {
            zip.start_file(name, opts).map_err(std::io::Error::other)?;
            let mut f = std::fs::File::open(&p)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            zip.write_all(&buf)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Writes the backup zip; returns how many attachment files went in.
pub fn create(store: &Store, dest: &Path) -> Result<usize, StoreError> {
    let tmp = dest.with_extension("tmp.db");
    store.snapshot(&tmp)?;
    let result = (|| -> std::io::Result<usize> {
        let file = std::fs::File::create(dest)?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("vigia.db", opts).map_err(std::io::Error::other)?;
        zip.write_all(&std::fs::read(&tmp)?)?;
        let files = store.files_dir();
        let n = if files.is_dir() { add_dir(&mut zip, &files, FILES_DIR)? } else { 0 };
        zip.finish().map_err(std::io::Error::other)?;
        Ok(n)
    })();
    let _ = std::fs::remove_file(&tmp);
    Ok(result?)
}

/// Unpacks a backup into `dir` (which must be empty or missing); returns the database path.
pub fn restore(zip_path: &Path, dir: &Path) -> Result<PathBuf, StoreError> {
    std::fs::create_dir_all(dir)?;
    if std::fs::read_dir(dir)?.next().is_some() {
        return Err(StoreError::Io(std::io::Error::new(std::io::ErrorKind::AlreadyExists, dir.display().to_string())));
    }
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path)?).map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        // enclosed_name refuses `..` and absolute paths
        let Some(rel) = f.enclosed_name() else { continue };
        if !(rel == Path::new("vigia.db") || rel.starts_with(FILES_DIR)) || f.is_dir() {
            continue;
        }
        let out = dir.join(rel);
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        std::fs::write(out, buf)?;
    }
    let db = dir.join("vigia.db");
    Store::open(&db)?;
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Item, Kind};

    #[test]
    fn backup_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::create(&dir.path().join("live/vigia.db")).unwrap();
        let id = s.insert(&Item::new(Kind::Warranty, "Nevera")).unwrap();
        let src = dir.path().join("factura.pdf");
        std::fs::write(&src, b"%PDF-1.4 receipt").unwrap();
        s.attach(id, &src).unwrap();
        let zip = dir.path().join("backup.zip");
        assert_eq!(create(&s, &zip).unwrap(), 1);
        let db = restore(&zip, &dir.path().join("restored")).unwrap();
        let r = Store::open(&db).unwrap();
        assert_eq!(r.items().unwrap()[0].name, "Nevera");
        let a = &r.attachments(id).unwrap()[0];
        assert_eq!(std::fs::read(r.attachment_path(a)).unwrap(), b"%PDF-1.4 receipt");
        // never into a folder that has things in it
        assert!(restore(&zip, &dir.path().join("restored")).is_err());
    }
}
