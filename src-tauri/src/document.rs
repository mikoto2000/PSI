use png_inspector::{Detail, Report, read_detail, read_preview, scan};
use serde::Serialize;
use std::{
    fs::{self, File},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Serialize)]
pub struct DocumentInfo {
    id: u32,
    name: String,
    size: u64,
    report: Arc<Report>,
}

struct OpenFile {
    id: u32,
    file: File,
    report: Arc<Report>,
}

#[derive(Default)]
pub struct Session(Mutex<Option<OpenFile>>);

impl Session {
    pub fn open(&self, path: &Path) -> Result<DocumentInfo, String> {
        let metadata = fs::metadata(path).map_err(file_error)?;
        if !metadata.is_file() {
            return Err("Select a regular PNG file".into());
        }
        let mut file = File::open(path).map_err(file_error)?;
        let size = file.metadata().map_err(file_error)?.len();
        let report = Arc::new(scan(&mut file)?);
        let name = path
            .file_name()
            .ok_or("Missing file name")?
            .to_string_lossy()
            .into_owned();
        let mut active = self.lock()?;
        let id = active
            .as_ref()
            .map_or(1, |current| current.id.wrapping_add(1));
        *active = Some(OpenFile {
            id,
            file,
            report: report.clone(),
        });
        Ok(DocumentInfo {
            id,
            name,
            size,
            report,
        })
    }

    pub fn detail(&self, id: u32, index: usize) -> Result<Detail, String> {
        let mut active = self.lock()?;
        let active = current(&mut active, id)?;
        let chunk = active.report.chunks.get(index).ok_or("No such PNG chunk")?;
        read_detail(&mut active.file, chunk)
    }

    pub fn preview(&self, id: u32) -> Result<Vec<u8>, String> {
        let mut active = self.lock()?;
        let active = current(&mut active, id)?;
        read_preview(&mut active.file)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Option<OpenFile>>, String> {
        self.0
            .lock()
            .map_err(|_| "Inspection state is unavailable".into())
    }
}

fn current(active: &mut Option<OpenFile>, id: u32) -> Result<&mut OpenFile, String> {
    active
        .as_mut()
        .filter(|file| file.id == id)
        .ok_or_else(|| "PNG is no longer open".into())
}

fn file_error(error: std::io::Error) -> String {
    format!("Could not open PNG: {error}")
}
