#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod document;

use document::{Direction, DocumentInfo, Session};
use png_inspector::Detail;
use std::{path::PathBuf, sync::Mutex};
use tauri::{
    AppHandle, DragDropEvent, Emitter, Manager, WebviewWindow, WindowEvent, ipc::Response,
};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
struct PendingDrop(Mutex<Option<Result<PathBuf, String>>>);

#[tauri::command]
async fn pick_png(window: WebviewWindow) -> Result<Option<DocumentInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = window
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("PNG", &["png", "apng"])
            .blocking_pick_file();
        path.map(|path| {
            let path = path.into_path().map_err(|error| error.to_string())?;
            window.state::<Session>().open(&path)
        })
        .transpose()
    })
    .await
    .map_err(worker_error)?
}

#[tauri::command]
async fn open_drop(app: AppHandle) -> Result<Option<DocumentInfo>, String> {
    let path = app
        .state::<PendingDrop>()
        .0
        .lock()
        .map_err(|_| "Drop state is unavailable")?
        .take();
    tauri::async_runtime::spawn_blocking(move || {
        path.map(|path| app.state::<Session>().open(&path?))
            .transpose()
    })
    .await
    .map_err(worker_error)?
}

#[tauri::command]
async fn adjacent_png(
    app: AppHandle,
    id: u32,
    direction: Direction,
) -> Result<Option<DocumentInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Session>().adjacent(id, direction))
        .await
        .map_err(worker_error)?
}

#[tauri::command]
async fn chunk_detail(app: AppHandle, id: u32, index: usize) -> Result<Detail, String> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Session>().detail(id, index))
        .await
        .map_err(worker_error)?
}

#[tauri::command]
async fn png_preview(app: AppHandle, id: u32) -> Result<Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Session>().preview(id).map(Response::new)
    })
    .await
    .map_err(worker_error)?
}

fn worker_error(error: tauri::Error) -> String {
    format!("Inspection worker failed: {error}")
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(Session::default())
        .manage(PendingDrop::default())
        .on_window_event(|window, event| {
            if let WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event {
                let path = match paths.as_slice() {
                    [path] => Ok(path.clone()),
                    _ => Err("PNG ファイルは 1 個ずつドロップしてください。".into()),
                };
                if let Ok(mut pending) = window.state::<PendingDrop>().0.lock() {
                    *pending = Some(path);
                }
                let _ = window.emit("png-drop", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            pick_png,
            open_drop,
            adjacent_png,
            chunk_detail,
            png_preview
        ])
        .run(tauri::generate_context!())
        .expect("could not run PNG Structure Inspector");
}
