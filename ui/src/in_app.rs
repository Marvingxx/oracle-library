//! The shared library inside an Oracle app, in a dock or a floating window, with assets dragged
//! onto the document.
//!
//! A drag that ends outside the library window becomes a file drop, exactly as if the file had
//! been dragged in from the Explorer: [`InAppLibrary::inject_drops`] adds it to egui's
//! `dropped_files` before the frame, so each app places it with its own drop handling (a picture
//! becomes a layer in Oracle Edit, an SVG becomes artwork in Oracle Vector).

use std::path::{Path, PathBuf};

use egui::{Id, Order, RichText};

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
    panel_rect: Option<egui::Rect>,
    panel_frame: u64,
    /// The app window's client origin (points) and points-to-pixels scale, as of the last frame:
    /// to read the pointer from the OS at a drop.
    client: Option<(egui::Pos2, f32)>,
}

impl InAppLibrary {
    pub fn new(app_id: &'static str) -> InAppLibrary {
        InAppLibrary { open: false, app_id, browser: None, dragging: None, drops: Vec::new(), window_rect: None, panel_rect: None, panel_frame: 0, client: None }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Updates asset dragging for a host-managed dock. Returns an activation request
    /// (for example after Add to Library); the host opens its existing library tab.
    /// Floating and closing are owned by the host's docking layout, never by this browser.
    pub fn update_docked(&mut self, ctx: &egui::Context) -> bool {
        self.update_drag(ctx);
        self.window_rect = None;
        std::mem::take(&mut self.open)
    }

    fn update_drag(&mut self, ctx: &egui::Context) {
        self.client = ctx.input(|i| i.viewport().inner_rect).map(|r| (r.min, ctx.pixels_per_point()));
        self.follow_drag(ctx);
        if self.panel_frame != ctx.cumulative_frame_nr() {
            self.panel_rect = None;
        }
    }

    /// Files dropped on the library window go into the library rather than onto the document. Call
    /// from `eframe::App::raw_input_hook`, before the app looks at `dropped_files`, with the
    /// pointer's position at the drop if the app knows it (`None`: it is read from the OS where it
    /// can be; unknown, the drop is left to the app). Returns whether the drop was taken.
    pub fn claim_drops(&mut self, raw: &mut egui::RawInput, at: Option<egui::Pos2>) -> bool {
        if raw.dropped_files.is_empty() {
            return false;
        }
        let at = at.or_else(|| self.os_pointer());
        let over = at.is_some_and(|p| self.contains(p));
        if !over || raw.dropped_files.is_empty() {
            return false;
        }
        let files: Vec<PathBuf> = raw.dropped_files.drain(..).map(|f| f.path().to_path_buf()).filter(|p| p.is_absolute()).collect();
        let browser = self.browser.get_or_insert_with(LibraryBrowser::new);
        let _ = browser.add_files(&files);
        true
    }

    fn contains(&self, p: egui::Pos2) -> bool {
        (self.open && self.window_rect.is_some_and(|r| r.contains(p)))
            || self.panel_rect.is_some_and(|r| r.contains(p))
    }

    /// The same shared library inside a dock. Its compact layout keeps search and tiles visible.
    pub fn panel_ui(&mut self, ui: &mut egui::Ui, french: bool) {
        let browser = self.browser.get_or_insert_with(LibraryBrowser::new);
        browser.french = french;
        browser.accent = ui.visuals().hyperlink_color;
        self.panel_rect = Some(ui.available_rect_before_wrap());
        self.panel_frame = ui.ctx().cumulative_frame_nr();
        browser.ui(ui);
        let requests = browser.take_requests();
        for request in requests {
            self.handle(request, ui.ctx());
        }
    }

    /// The pointer in points of the app window, read from the OS (winit's file drops carry no
    /// position, and during an OS drag the window gets no pointer events).
    #[cfg(windows)]
    fn os_pointer(&self) -> Option<egui::Pos2> {
        let (origin, ppp) = self.client?;
        let pt = winsafe::GetCursorPos().ok()?;
        // Both in physical pixels: the client origin is egui's points times the scale.
        Some(((egui::pos2(pt.x as f32, pt.y as f32) - origin * ppp) / ppp).to_pos2())
    }

    #[cfg(not(windows))]
    fn os_pointer(&self) -> Option<egui::Pos2> {
        None
    }

    /// Adds `files` to the library (its "Mes créations" folder) and shows them: what an app's
    /// File › Add to Library sends. Opens the window. Returns where they are now in the library
    /// (empty when they couldn't be added: the browser says why).
    pub fn add_files(&mut self, files: &[PathBuf]) -> Vec<PathBuf> {
        self.open = true;
        let browser = self.browser.get_or_insert_with(LibraryBrowser::new);
        browser.add_files(files).unwrap_or_default()
    }

    /// Like [`add_files`](Self::add_files) without opening the window: a document sent to another
    /// app goes through the library quietly.
    pub fn store_files(&mut self, files: &[PathBuf]) -> Vec<PathBuf> {
        let browser = self.browser.get_or_insert_with(LibraryBrowser::new);
        browser.add_files(files).unwrap_or_default()
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
        // Logic runs before docks are drawn: the previous frame's rectangle is the drop target.
        self.update_drag(ctx);
        if !self.open {
            self.window_rect = None;
            return;
        }
        let browser = self.browser.get_or_insert_with(LibraryBrowser::new);
        browser.french = french;
        browser.accent = ctx.global_style().visuals.hyperlink_color;
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
            let over_window = pos.is_some_and(|p| self.contains(p));
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
                let accent = ui.visuals().hyperlink_color;
                egui::Frame::popup(ui.style()).fill(accent).show(ui, |ui| {
                    ui.label(RichText::new(format!("+ {name}")).color(crate::on_accent(accent)));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docked_library_consumes_reveal_requests_without_opening_the_legacy_window() {
        let ctx = egui::Context::default();
        let mut library = InAppLibrary::new("oracle-edit");
        library.open = true;
        assert!(library.update_docked(&ctx));
        assert!(!library.open);
        assert!(!library.update_docked(&ctx));
    }

    #[test]
    fn dock_remains_a_drop_target_when_the_floating_window_is_closed() {
        let mut library = InAppLibrary::new("oracle-vector");
        library.panel_rect = Some(egui::Rect::from_min_size(egui::pos2(700.0, 40.0), egui::vec2(300.0, 500.0)));
        library.window_rect = Some(egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(500.0, 400.0)));
        assert!(library.contains(egui::pos2(800.0, 100.0)));
        assert!(!library.contains(egui::pos2(100.0, 100.0)));
        library.open = true;
        assert!(library.contains(egui::pos2(100.0, 100.0)));
        assert!(!library.contains(egui::pos2(600.0, 600.0)));
    }

    #[test]
    fn releasing_in_the_previous_frames_dock_does_not_place_an_asset() {
        let ctx = egui::Context::default();
        let mut library = InAppLibrary::new("oracle-vector");
        let rect = egui::Rect::from_min_size(egui::pos2(700.0, 40.0), egui::vec2(300.0, 500.0));
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            library.panel_rect = Some(rect);
            library.panel_frame = ui.ctx().cumulative_frame_nr();
        });
        out.textures_delta.clear();
        library.dragging = Some(PathBuf::from("asset.svg"));
        let mut out = ctx.run_ui(egui::RawInput {
            events: vec![egui::Event::PointerMoved(egui::pos2(800.0, 100.0))],
            ..Default::default()
        }, |ui| library.ui(ui.ctx(), false));
        out.textures_delta.clear();
        assert!(library.drops.is_empty());
        assert!(library.dragging.is_none());
        assert!(library.panel_rect.is_none());

        library.dragging = Some(PathBuf::from("asset.svg"));
        let mut out = ctx.run_ui(egui::RawInput {
            events: vec![egui::Event::PointerMoved(egui::pos2(200.0, 100.0))],
            ..Default::default()
        }, |ui| library.ui(ui.ctx(), false));
        out.textures_delta.clear();
        assert_eq!(library.drops, vec![PathBuf::from("asset.svg")]);
    }
}
