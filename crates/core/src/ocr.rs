//! Local OCR assist: read a photo or scan of a document with `ocrs` (pure Rust, runs on this
//! computer), or the text layer of a PDF, so the expiry date and number can be prefilled.
//! Nothing is sent anywhere; the two small models (~12 MB) are downloaded once, only when the
//! user asks for it.
use std::path::{Path, PathBuf};

pub const DETECTION_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx";
pub const RECOGNITION_URL: &str = "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx";
const DETECTION_FILE: &str = "text-detection.onnx";
const RECOGNITION_FILE: &str = "text-recognition.onnx";

#[derive(Debug, Clone, PartialEq)]
pub enum OcrError {
    /// The models aren't downloaded yet.
    ModelsMissing,
    /// Built without the `ocr` feature.
    Unavailable,
    Unsupported,
    Failed(String),
}

impl std::fmt::Display for OcrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrError::ModelsMissing => write!(f, "{}", i18n::t!("core.ocr.models_missing")),
            OcrError::Unavailable => write!(f, "{}", i18n::t!("core.ocr.unavailable")),
            OcrError::Unsupported => write!(f, "{}", i18n::t!("core.ocr.unsupported")),
            OcrError::Failed(e) => write!(f, "{}", i18n::t!("core.ocr.failed", detail = e)),
        }
    }
}

pub fn model_dir() -> Option<PathBuf> {
    crate::paths::ocr_dir()
}

pub fn models_present() -> bool {
    model_dir().is_some_and(|d| [DETECTION_FILE, RECOGNITION_FILE].iter().all(|f| d.join(f).metadata().is_ok_and(|m| m.len() > 100_000)))
}

pub fn available() -> bool {
    cfg!(feature = "ocr")
}

/// Whether a file can be read: images (OCR) and PDFs (text layer).
pub fn readable(path: &Path) -> bool {
    matches!(path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref(), Some("png" | "jpg" | "jpeg" | "webp" | "pdf"))
}

/// Blocking download of both models (skips the ones already there).
pub fn download_models() -> Result<(), OcrError> {
    use std::io::{Read as _, Write as _};
    let dir = model_dir().ok_or(OcrError::Unavailable)?;
    std::fs::create_dir_all(&dir).map_err(|e| OcrError::Failed(e.to_string()))?;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_connect(Some(std::time::Duration::from_secs(10))).timeout_global(Some(std::time::Duration::from_secs(600))).build().into();
    for (url, file) in [(DETECTION_URL, DETECTION_FILE), (RECOGNITION_URL, RECOGNITION_FILE)] {
        let dest = dir.join(file);
        if dest.metadata().is_ok_and(|m| m.len() > 100_000) {
            continue;
        }
        let mut resp = agent.get(url).call().map_err(|e| OcrError::Failed(e.to_string()))?;
        let tmp = dir.join(format!("{file}.part"));
        let mut out = std::fs::File::create(&tmp).map_err(|e| OcrError::Failed(e.to_string()))?;
        let mut reader = resp.body_mut().with_config().limit(200_000_000).reader();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|e| OcrError::Failed(e.to_string()))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| OcrError::Failed(e.to_string()))?;
        }
        out.sync_all().map_err(|e| OcrError::Failed(e.to_string()))?;
        drop(out);
        std::fs::rename(&tmp, &dest).map_err(|e| OcrError::Failed(e.to_string()))?;
    }
    Ok(())
}

/// The text of a document: OCR for images, the text layer for PDFs. Blocking and CPU-heavy.
pub fn read_text(path: &Path) -> Result<String, OcrError> {
    if !readable(path) {
        return Err(OcrError::Unsupported);
    }
    imp::read_text(path)
}

#[cfg(feature = "ocr")]
mod imp {
    use super::*;
    use std::sync::{Arc, Mutex, OnceLock};

    fn engine() -> Result<Arc<ocrs::OcrEngine>, OcrError> {
        static ENGINE: OnceLock<Mutex<Option<Arc<ocrs::OcrEngine>>>> = OnceLock::new();
        let slot = ENGINE.get_or_init(|| Mutex::new(None));
        let mut g = slot.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(e) = g.as_ref() {
            return Ok(e.clone());
        }
        if !models_present() {
            return Err(OcrError::ModelsMissing);
        }
        let dir = model_dir().ok_or(OcrError::ModelsMissing)?;
        let det = rten::Model::load_file(dir.join(DETECTION_FILE)).map_err(|e| OcrError::Failed(e.to_string()))?;
        let rec = rten::Model::load_file(dir.join(RECOGNITION_FILE)).map_err(|e| OcrError::Failed(e.to_string()))?;
        let e = Arc::new(ocrs::OcrEngine::new(ocrs::OcrEngineParams { detection_model: Some(det), recognition_model: Some(rec), ..Default::default() }).map_err(|e| OcrError::Failed(e.to_string()))?);
        *g = Some(e.clone());
        Ok(e)
    }

    pub fn read_text(path: &Path) -> Result<String, OcrError> {
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
            let bytes = std::fs::read(path).map_err(|e| OcrError::Failed(e.to_string()))?;
            return pdf_extract::extract_text_from_mem(&bytes).map_err(|e| OcrError::Failed(e.to_string()));
        }
        let engine = engine()?;
        let img = image::open(path).map_err(|e| OcrError::Failed(e.to_string()))?.into_rgb8();
        let src = ocrs::ImageSource::from_bytes(img.as_raw(), img.dimensions()).map_err(|e| OcrError::Failed(e.to_string()))?;
        let input = engine.prepare_input(src).map_err(|e| OcrError::Failed(e.to_string()))?;
        engine.get_text(&input).map_err(|e| OcrError::Failed(e.to_string()))
    }
}

#[cfg(not(feature = "ocr"))]
mod imp {
    use super::*;
    pub fn read_text(_: &Path) -> Result<String, OcrError> {
        Err(OcrError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_types() {
        assert!(readable(Path::new("scan.JPG")) && readable(Path::new("a.pdf")) && !readable(Path::new("a.docx")) && !readable(Path::new("noext")));
        assert_eq!(read_text(Path::new("a.docx")), Err(OcrError::Unsupported));
    }
}
