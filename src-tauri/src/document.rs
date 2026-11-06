use png_inspector::{Detail, Report, read_detail, read_preview, scan};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    ffi::OsString,
    fs::{self, File},
    path::{Path, PathBuf},
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
    path: PathBuf,
    id: u32,
    file: File,
    report: Arc<Report>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Previous,
    Next,
}

#[derive(Default)]
pub struct Session(Mutex<Option<OpenFile>>);

impl Session {
    pub fn open(&self, path: &Path) -> Result<DocumentInfo, String> {
        self.open_path(path, None)
    }

    fn open_path(&self, path: &Path, expected_id: Option<u32>) -> Result<DocumentInfo, String> {
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
        if let Some(id) = expected_id {
            current(&mut active, id)?;
        }
        let id = active
            .as_ref()
            .map_or(1, |current| current.id.wrapping_add(1));
        *active = Some(OpenFile {
            id,
            path: path.to_owned(),
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

    pub fn adjacent(&self, id: u32, direction: Direction) -> Result<Option<DocumentInfo>, String> {
        let path = current(&mut *self.lock()?, id)?.path.clone();
        let directory = path.parent().ok_or("Missing PNG folder")?;
        let name = path.file_name().ok_or("Missing file name")?;
        let order = match direction {
            Direction::Previous => Ordering::Less,
            Direction::Next => Ordering::Greater,
        };
        let mut nearest: Option<OsString> = None;
        for entry in fs::read_dir(directory).map_err(folder_error)? {
            let entry = entry.map_err(folder_error)?;
            let candidate = entry.file_name();
            if candidate.as_os_str().cmp(name) != order
                || nearest
                    .as_ref()
                    .is_some_and(|best| candidate.cmp(best) != order.reverse())
            {
                continue;
            }
            let supported = Path::new(&candidate).extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("apng")
            });
            if supported && entry.file_type().map_err(folder_error)?.is_file() {
                nearest = Some(candidate);
            }
        }
        nearest
            .map(|name| self.open_path(&directory.join(name), Some(id)))
            .transpose()
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

fn folder_error(error: std::io::Error) -> String {
    format!("Could not read the PNG folder: {error}")
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
