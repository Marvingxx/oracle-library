//! The library inside an Oracle app: a floating "Bibliothèque" window, and dragging an asset from
//! it onto the document.
//!
//! A drag that ends outside the library window becomes a file drop, exactly as if the file had
//! been dragged in from the Explorer: [`InAppLibrary::inject_drops`] adds it to egui's
//! `dropped_files` before the frame, so each app places it with its own drop handling (a picture
//! becomes a layer in Oracle Edit, an SVG becomes artwork in Oracle Vector).

use std::path::{Path, PathBuf};

use egui::{Color32, Id, Order, RichText};

use crate::{LibraryBrowser, Request};

pub struct InAppLibrary {
    /// Whether the window is shown.
    pub open: bool,
    /// This app's id (`oracle-edit`): "Open in" this app opens the file here.
    pub app_id: &'static str,
    browser: Option<LibraryBrowser>,
    /// The asset being dragged out of the window.
    dragging: Option<PathBuf>,
    /// Files to drop on the document at the next frame.
    drops: Vec<PathBuf>,
    window_rect: Option<egui::Rect>,
}

impl InAppLibrary {
    pub fn new(app_id: &'static str) -> InAppLibrary {
        InAppLibrary { open: false, app_id, browser: None, dragging: None, drops: Vec::new(), window_rect: None }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Adds the files dropped from the library to this frame's input. Call from
    /// `eframe::App::raw_input_hook`.
    pub fn inject_drops(&mut self, raw: &mut egui::RawInput) {
        for path in self.drops.drain(..) {
            raw.dropped_files.push(std::sync::Arc::new(LibraryDrop(path)) as egui::DroppedFileHandle);
        }
    }

    /// Draws the window (when open) and the drag in progress. Call once per frame. `french`
    /// picks the interface language.
    pub fn ui(&mut self, ctx: &egui::Context, french: bool) {
        self.follow_drag(ctx);
        if !self.open {
            self.window_rect = None;
            return;
        }
        let browser = self.browser.get_or_insert_with(LibraryBrowser::new);
        browser.french = french;
        let title = if french { "Bibliothèque Oracle" } else { "Oracle Library" };
        let mut open = self.open;
        let shown = egui::Window::new(title)
            .id(Id::new("oracle-library-window"))
            .open(&mut open)
            .default_size([980.0, 640.0])
            .min_size([640.0, 360.0])
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| browser.ui(ui));
        self.open = open;
        self.window_rect = shown.map(|r| r.response.rect);
        let requests = browser.take_requests();
        for r in requests {
            self.handle(r, ctx);
        }
    }

    fn handle(&mut self, request: Request, ctx: &egui::Context) {
        match request {
            Request::Drag { file, .. } => self.dragging = Some(file),
            Request::OpenWith { app, file } if app == self.app_id => {
                // Here: as if the file were dropped on the window.
                self.drops.push(file.clone());
                ctx.request_repaint();
                self.used(&file);
            }
            Request::OpenWith { app, file } => {
                let result = match other_app_exe(app) {
                    Some(exe) => std::process::Command::new(exe).arg(&file).spawn().map(|_| ()).map_err(|e| e.to_string()),
                    None => open_with_system(&file),
                };
                self.report(&file, result);
            }
            Request::Open(file) => {
                let result = open_with_system(&file);
                self.report(&file, result);
            }
            Request::Reveal(file) => {
                if let Err(e) = reveal(&file) {
                    log::warn!("couldn't show {}: {e}", file.display());
                }
            }
            Request::PickFolder => {
                if let Some(folder) = rfd::FileDialog::new().pick_folder()
                    && let Some(b) = self.browser.as_mut()
                {
                    b.set_root(folder);
                }
            }
        }
    }

    fn report(&mut self, file: &Path, result: Result<(), String>) {
        match result {
            Ok(()) => self.used(file),
            Err(e) => {
                if let Some(b) = self.browser.as_mut() {
                    b.say(e);
                }
            }
        }
    }

    fn used(&mut self, file: &Path) {
        if let Some(b) = self.browser.as_mut() {
            b.used(file);
        }
    }

    /// While an asset is dragged: a label under the pointer; on release outside the window, a
    /// drop on the document.
    fn follow_drag(&mut self, ctx: &egui::Context) {
        let Some(file) = self.dragging.clone() else { return };
        let (pos, released) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.any_released() || !i.pointer.any_down()));
        if released {
            self.dragging = None;
            let over_window = match (pos, self.window_rect) {
                (Some(p), Some(r)) => r.contains(p),
                _ => false,
            };
            if !over_window && pos.is_some() {
                self.drops.push(file.clone());
                self.used(&file);
                ctx.request_repaint();
            }
            return;
        }
        if let Some(p) = pos {
            let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            egui::Area::new(Id::new("oracle-library-drag")).order(Order::Tooltip).fixed_pos(p + egui::vec2(14.0, 14.0)).interactable(false).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(crate::ORACLE_RED).show(ui, |ui| {
                    ui.label(RichText::new(format!("+ {name}")).color(Color32::WHITE));
                });
            });
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        ctx.request_repaint();
    }
}

/// A library file handed to the app as a dropped file.
#[derive(Debug)]
struct LibraryDrop(PathBuf);

impl egui::DroppedFile for LibraryDrop {
    fn path(&self) -> &Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

/// Another Oracle app's program, where its installer puts it (`C:\Program Files\Oracle Vector`).
fn other_app_exe(app: &str) -> Option<PathBuf> {
    oracle_suite_link::App::from_id(app)?.exe()
}

fn open_with_system(file: &Path) -> Result<(), String> {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(file);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(file);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(file);
        c
    };
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn reveal(file: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Passed raw: Explorer wants `/select,"<path>"` as one argument, quoted inside.
        let arg = format!("/select,\"{}\"", file.display());
        return std::process::Command::new("explorer").raw_arg(arg).spawn().map(|_| ()).map_err(|e| e.to_string());
    }
    #[cfg(target_os = "macos")]
    {
        return std::process::Command::new("open").arg("-R").arg(file).spawn().map(|_| ()).map_err(|e| e.to_string());
    }
    #[allow(unreachable_code)]
    open_with_system(file.parent().unwrap_or(file))
}
