//! The library browser: search, sections, tags and favourites over the user's asset folder, with
//! thumbnails, a details pane and drag-and-drop. Shared by Oracle Suite (full window) and the
//! Oracle apps. The browser never touches the OS itself: what it can't do in egui (choose a folder,
//! start a native drag, open a file in an app) it asks for through [`Request`]s the host carries out.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod in_app;
mod thumbs;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use egui::{Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, vec2};
use oracle_library::{Asset, FormatGroup, Library, Query, UserData, search};

pub use in_app::InAppLibrary;
pub use thumbs::Thumbs;

/// Oracle's brand red, #d31c3b.
pub const ORACLE_RED: Color32 = Color32::from_rgb(0xd3, 0x1c, 0x3b);

/// Something the host application does for the browser.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Ask the user for the library folder, then call [`LibraryBrowser::set_root`].
    PickFolder,
    /// Start a native drag of this file (to Oracle Edit, Oracle Vector, the Explorer…).
    Drag { file: PathBuf, preview: Option<PathBuf> },
    /// Open the file in an Oracle app (`oracle-edit`, `oracle-vector`).
    OpenWith { app: &'static str, file: PathBuf },
    /// Open the file with the system's default app.
    Open(PathBuf),
    /// Show the file in the system file manager.
    Reveal(PathBuf),
}

/// Which Oracle app suits a file best.
pub fn app_for(asset: &Asset) -> Option<(&'static str, &'static str)> {
    match asset.group {
        FormatGroup::Image | FormatGroup::Photoshop => Some(("oracle-edit", "Oracle Edit")),
        FormatGroup::Vector => Some(("oracle-vector", "Oracle Vector")),
        _ => None,
    }
}

enum State {
    NoFolder,
    Loading(Receiver<Result<Library, String>>),
    Ready(Arc<Library>),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shelf {
    All,
    Favorites,
    Recent,
}

pub struct LibraryBrowser {
    /// Interface language: French when true, English otherwise. The host sets it each frame.
    pub french: bool,
    /// The accent colour for what is on or chosen (Oracle red unless the host says otherwise).
    pub accent: Color32,
    user: UserData,
    user_path: Option<PathBuf>,
    user_dirty: Option<Instant>,
    state: State,
    query: Query,
    shelf: Shelf,
    results: Vec<usize>,
    results_for: Option<(Query, u64)>,
    /// Bumped when anything that affects the results changes (favourites, tags, recents).
    revision: u64,
    selected: Option<String>,
    new_tag: String,
    tile: f32,
    thumbs: Thumbs,
    requests: Vec<Request>,
    status: Option<(String, Instant)>,
}

impl Default for LibraryBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryBrowser {
    /// Loads the user's library settings and starts indexing the library folder.
    pub fn new() -> LibraryBrowser {
        Self::with_settings(UserData::default_path())
    }

    /// Like [`new`](Self::new), with the settings file at `user_path` (None: nothing is saved).
    pub fn with_settings(user_path: Option<PathBuf>) -> LibraryBrowser {
        let user = user_path.as_deref().map(UserData::load).unwrap_or_default();
        let mut b = LibraryBrowser {
            french: true,
            accent: ORACLE_RED,
            user,
            user_path,
            user_dirty: None,
            state: State::NoFolder,
            query: Query::default(),
            shelf: Shelf::All,
            results: Vec::new(),
            results_for: None,
            revision: 0,
            selected: None,
            new_tag: String::new(),
            tile: 132.0,
            thumbs: Thumbs::new(),
            requests: Vec::new(),
            status: None,
        };
        b.reload();
        b
    }

    /// Uses `root` as the library folder from now on (and remembers it).
    pub fn set_root(&mut self, root: PathBuf) {
        self.user.root = root.display().to_string();
        self.mark_dirty();
        self.reload();
    }

    /// Whether the library is indexed and ready.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, State::Ready(_))
    }

    /// Selects the first search result, showing its details.
    pub fn select_first(&mut self) {
        if let (State::Ready(lib), Some(&i)) = (&self.state, self.results.first()) {
            self.selected = lib.assets.get(i).map(|a| a.id.clone());
        }
    }

    /// Adds `tag` to the selected asset, as typing it in the details pane does.
    pub fn tag_selected(&mut self, tag: &str) {
        if let Some(id) = self.selected.clone() {
            self.user.add_tag(&id, tag);
            self.revision += 1;
            self.mark_dirty();
        }
    }

    /// Puts `text` in the search field.
    pub fn set_search(&mut self, text: &str) {
        self.query.text = text.to_string();
    }

    /// Indexes the library folder again (after files were added).
    pub fn reload(&mut self) {
        let Some(root) = self.user.library_root() else {
            self.state = State::NoFolder;
            return;
        };
        let (tx, rx) = channel();
        let _ = std::thread::Builder::new().name("library-index".into()).spawn(move || {
            let _ = tx.send(Library::open(&root));
        });
        self.state = State::Loading(rx);
        self.results_for = None;
    }

    /// What the host should do now. Drain it every frame.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// Notes that a request about `file` succeeded (a drag or an open), for the recent list.
    pub fn used(&mut self, file: &std::path::Path) {
        let State::Ready(lib) = &self.state else { return };
        if let Some(a) = lib.assets.iter().find(|a| a.file(&lib.root) == file) {
            self.user.touch(&a.id);
            self.revision += 1;
            self.mark_dirty();
        }
    }

    /// Shows a short message in the browser's status line.
    pub fn say(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), Instant::now()));
    }

    fn tr<'a>(&self, fr: &'a str, en: &'a str) -> &'a str {
        if self.french { fr } else { en }
    }

    fn mark_dirty(&mut self) {
        self.user_dirty.get_or_insert_with(Instant::now);
    }

    fn save_if_due(&mut self, force: bool) {
        let Some(since) = self.user_dirty else { return };
        if !force && since.elapsed() < Duration::from_millis(800) {
            return;
        }
        if let Some(path) = &self.user_path
            && let Err(e) = self.user.save(path)
        {
            log::warn!("couldn't save the library settings: {e}");
        }
        self.user_dirty = None;
    }

    /// Draws the browser in all of `ui`.
    pub fn ui(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.thumbs.poll(&ctx);
        self.save_if_due(false);
        if self.user_dirty.is_some() {
            ctx.request_repaint_after(Duration::from_millis(900));
        }
        if let State::Loading(rx) = &self.state {
            match rx.try_recv() {
                Ok(Ok(lib)) => self.state = State::Ready(Arc::new(lib)),
                Ok(Err(e)) => self.state = State::Failed(e),
                Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.state = State::Failed("indexation interrompue".into()),
            }
        }
        let lib = match &self.state {
            State::Ready(lib) => lib.clone(),
            State::NoFolder => return self.empty_state(ui, None),
            State::Failed(e) => {
                let e = e.clone();
                return self.empty_state(ui, Some(e));
            }
            State::Loading(_) => {
                ui.centered_and_justified(|ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(self.tr("Indexation de la bibliothèque…", "Indexing the library…"));
                    });
                });
                return;
            }
        };
        self.refresh_results(&lib);

        let sidebar_w = 240.0;
        let details_w = if self.selected.is_some() { 300.0 } else { 0.0 };
        let full = ui.available_rect_before_wrap();
        let side = Rect::from_min_size(full.min, vec2(sidebar_w, full.height()));
        let details = Rect::from_min_max(egui::pos2(full.right() - details_w, full.top()), full.max);
        let main = Rect::from_min_max(egui::pos2(side.right() + 16.0, full.top()), egui::pos2(details.left() - if details_w > 0.0 { 16.0 } else { 0.0 }, full.bottom()));

        ui.scope_builder(egui::UiBuilder::new().max_rect(side), |ui| self.sidebar(ui, &lib));
        ui.painter().vline(side.right() + 8.0, full.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
        ui.scope_builder(egui::UiBuilder::new().max_rect(main), |ui| self.main(ui, &lib));
        if details_w > 0.0 {
            ui.painter().vline(details.left() - 8.0, full.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
            ui.scope_builder(egui::UiBuilder::new().max_rect(details), |ui| self.details(ui, &lib));
        }
        ui.allocate_rect(full, Sense::hover());
    }

    fn empty_state(&mut self, ui: &mut Ui, error: Option<String>) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            ui.heading(self.tr("Bibliothèque", "Library"));
            ui.add_space(8.0);
            let text = match &error {
                Some(e) => format!("{} {e}", self.tr("Impossible d'ouvrir la bibliothèque :", "Couldn't open the library:")),
                None => self.tr("Choisissez le dossier qui contient vos ressources (textures, pinceaux, polices, vecteurs…).", "Choose the folder that holds your assets (textures, brushes, fonts, vectors…).").to_string(),
            };
            ui.label(RichText::new(text).weak());
            ui.add_space(12.0);
            if ui.add(egui::Button::new(self.tr("Choisir le dossier…", "Choose folder…")).min_size(vec2(180.0, 32.0))).clicked() {
                self.requests.push(Request::PickFolder);
            }
        });
    }

    fn refresh_results(&mut self, lib: &Library) {
        let mut q = self.query.clone();
        q.favorites_only = self.shelf == Shelf::Favorites;
        let key = (q.clone(), self.revision + if self.shelf == Shelf::Recent { 1 << 40 } else { 0 });
        if self.results_for.as_ref() == Some(&key) {
            return;
        }
        self.results = if self.shelf == Shelf::Recent {
            let found = search(lib, &self.user, &q);
            let set: std::collections::HashSet<usize> = found.into_iter().collect();
            self.user.recent.iter().filter_map(|id| lib.assets.iter().position(|a| &a.id == id)).filter(|i| set.contains(i)).collect()
        } else {
            search(lib, &self.user, &q)
        };
        self.results_for = Some(key);
    }

    fn sidebar(&mut self, ui: &mut Ui, lib: &Library) {
        egui::ScrollArea::vertical().id_salt("library-sidebar").auto_shrink(false).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let all = lib.assets.len();
            let fav = self.user.favorites.len();
            let recent = self.user.recent.len();
            if side_item(ui, self.accent, self.tr("Tout", "All"), Some(all), self.shelf == Shelf::All && self.query.kind.is_none() && self.query.tag.is_none()).clicked() {
                self.shelf = Shelf::All;
                self.query.kind = None;
                self.query.tag = None;
            }
            if side_item(ui, self.accent, &format!("★ {}", self.tr("Favoris", "Favourites")), Some(fav), self.shelf == Shelf::Favorites).clicked() {
                self.shelf = Shelf::Favorites;
            }
            if side_item(ui, self.accent, self.tr("Récents", "Recent"), Some(recent), self.shelf == Shelf::Recent).clicked() {
                self.shelf = Shelf::Recent;
            }
            ui.add_space(12.0);
            section_title(ui, self.tr("SECTIONS", "SECTIONS"));
            for (kind, n) in &lib.kinds {
                let on = self.query.kind.as_deref() == Some(kind.as_str());
                if side_item(ui, self.accent, kind, Some(*n), on).clicked() {
                    self.query.kind = if on { None } else { Some(kind.clone()) };
                    self.shelf = Shelf::All;
                }
            }
            let tags = self.user.all_tags();
            if !tags.is_empty() {
                ui.add_space(12.0);
                section_title(ui, self.tr("MES TAGS", "MY TAGS"));
                for (tag, n) in tags {
                    let on = self.query.tag.as_deref() == Some(tag.as_str());
                    if side_item(ui, self.accent, &format!("# {tag}"), Some(n), on).clicked() {
                        self.query.tag = if on { None } else { Some(tag.clone()) };
                    }
                }
            }
            ui.add_space(16.0);
            section_title(ui, self.tr("DOSSIER", "FOLDER"));
            ui.label(RichText::new(lib.root.display().to_string()).small().weak());
            ui.horizontal_wrapped(|ui| {
                if ui.small_button(self.tr("Changer…", "Change…")).clicked() {
                    self.requests.push(Request::PickFolder);
                }
                if ui.small_button(self.tr("Actualiser", "Refresh")).on_hover_text(self.tr("Réindexer après avoir ajouté des fichiers", "Index again after adding files")).clicked() {
                    self.reload();
                }
            });
        });
    }

    fn main(&mut self, ui: &mut Ui, lib: &Arc<Library>) {
        // Search field, big and first.
        ui.horizontal(|ui| {
            let hint = self.tr("Rechercher : métal, papier froissé, fumée, police…", "Search: metal, crumpled paper, smoke, font…");
            let edit = egui::TextEdit::singleline(&mut self.query.text).hint_text(hint).desired_width((ui.available_width() - 40.0).max(120.0)).min_size(vec2(0.0, 30.0)).font(egui::TextStyle::Heading);
            let r = ui.add(edit);
            if ui.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.command) {
                r.request_focus();
            }
            if !self.query.text.is_empty() && ui.button("×").on_hover_text(self.tr("Effacer", "Clear")).clicked() {
                self.query.text.clear();
            }
        });
        ui.add_space(8.0);
        // Format chips.
        ui.horizontal_wrapped(|ui| {
            let groups: [(Option<FormatGroup>, &str, &str); 8] = [
                (None, "Tout", "All"),
                (Some(FormatGroup::Image), "Images", "Images"),
                (Some(FormatGroup::Vector), "Vecteurs", "Vectors"),
                (Some(FormatGroup::Font), "Polices", "Fonts"),
                (Some(FormatGroup::Photoshop), "Photoshop & pinceaux", "Photoshop & brushes"),
                (Some(FormatGroup::Layout), "Mise en page", "Layout"),
                (Some(FormatGroup::Video), "Vidéo", "Video"),
                (Some(FormatGroup::Other), "Autres", "Other"),
            ];
            for (g, fr, en) in groups {
                let on = match g {
                    None => self.query.groups.is_empty(),
                    Some(g) => self.query.groups.contains(&g),
                };
                if chip(ui, self.accent, self.tr(fr, en), on).clicked() {
                    match g {
                        None => self.query.groups.clear(),
                        Some(g) => {
                            if !self.query.groups.remove(&g) {
                                self.query.groups.insert(g);
                            }
                        }
                    }
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let n = self.results.len();
            let label = if self.french { format!("{n} ressource{}", if n > 1 { "s" } else { "" }) } else { format!("{n} asset{}", if n == 1 { "" } else { "s" }) };
            ui.label(RichText::new(label).weak());
            if let Some((text, at)) = &self.status
                && at.elapsed() < Duration::from_secs(4)
            {
                ui.label(RichText::new(format!("· {text}")).weak());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("+").on_hover_text(self.tr("Vignettes plus grandes", "Larger thumbnails")).clicked() {
                    self.tile = (self.tile + 24.0).min(264.0);
                }
                if ui.button("−").on_hover_text(self.tr("Vignettes plus petites", "Smaller thumbnails")).clicked() {
                    self.tile = (self.tile - 24.0).max(84.0);
                }
            });
        });
        ui.add_space(6.0);
        self.grid(ui, lib);
    }

    fn grid(&mut self, ui: &mut Ui, lib: &Arc<Library>) {
        if self.results.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(self.tr("Aucune ressource ne correspond.", "No asset matches.")).weak());
            });
            return;
        }
        let gap = 10.0;
        let tile = self.tile;
        let label_h = 30.0;
        let width = ui.available_width();
        let cols = (((width + gap) / (tile + gap)).floor() as usize).max(1);
        let rows = self.results.len().div_ceil(cols);
        let row_h = tile + label_h + gap;
        let results = self.results.clone();
        egui::ScrollArea::vertical().id_salt("library-grid").auto_shrink(false).show_rows(ui, row_h, rows, |ui, range| {
            for row in range {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for col in 0..cols {
                        let Some(&i) = results.get(row * cols + col) else { break };
                        let Some(asset) = lib.assets.get(i) else { break };
                        self.tile_ui(ui, lib, asset, tile, label_h);
                    }
                });
                ui.add_space(gap - ui.spacing().item_spacing.y);
            }
        });
    }

    fn tile_ui(&mut self, ui: &mut Ui, lib: &Arc<Library>, asset: &Asset, tile: f32, label_h: f32) {
        let (rect, resp) = ui.allocate_exact_size(vec2(tile, tile + label_h), Sense::click_and_drag());
        if !ui.is_rect_visible(rect) {
            return;
        }
        let visuals = ui.visuals().clone();
        let accent = self.accent;
        let selected = self.selected.as_deref() == Some(asset.id.as_str());
        let pic = Rect::from_min_size(rect.min, vec2(tile, tile));
        let painter = ui.painter_at(rect);
        let bg = if visuals.dark_mode { Color32::from_gray(36) } else { Color32::from_gray(232) };
        painter.rect_filled(pic, CornerRadius::same(4), bg);
        if may_be_transparent(asset) {
            checkerboard(&painter, pic.shrink(1.0), visuals.dark_mode);
        }
        let (source, original) = thumb_source(lib, asset);
        match self.thumbs.get(&asset.id, source, original) {
            Some(tex) => {
                let size = tex.size_vec2();
                let fit = (pic.width() - 8.0) / size.x.max(size.y).max(1.0);
                let r = Rect::from_center_size(pic.center(), size * fit);
                painter.image(tex.id(), r, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            }
            None => {
                let text = if asset.group == FormatGroup::Font { "Aa".to_string() } else { asset.format.clone() };
                let size = if asset.group == FormatGroup::Font { tile * 0.32 } else { 15.0 };
                painter.text(pic.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(size), visuals.weak_text_color());
            }
        }
        // Format badge.
        let badge = painter.layout_no_wrap(asset.format.clone(), egui::FontId::monospace(9.5), Color32::WHITE);
        let br = Rect::from_min_size(pic.left_bottom() + vec2(5.0, -badge.size().y - 7.0), badge.size() + vec2(6.0, 2.0));
        painter.rect_filled(br, CornerRadius::same(2), Color32::from_black_alpha(150));
        painter.galley(br.min + vec2(3.0, 1.0), badge, Color32::WHITE);
        if self.user.is_favorite(&asset.id) {
            painter.text(pic.right_top() + vec2(-8.0, 6.0), egui::Align2::RIGHT_TOP, "★", egui::FontId::proportional(15.0), Color32::from_rgb(255, 196, 61));
        }
        let border = if selected {
            Stroke::new(2.0, accent)
        } else if resp.hovered() {
            Stroke::new(1.0, visuals.widgets.hovered.bg_stroke.color)
        } else {
            Stroke::NONE
        };
        painter.rect_stroke(pic, CornerRadius::same(4), border, StrokeKind::Inside);
        let name = painter.layout(asset.name.clone(), egui::FontId::proportional(11.5), visuals.text_color(), tile);
        let clipped = Rect::from_min_size(egui::pos2(rect.left(), pic.bottom() + 4.0), vec2(tile, label_h - 4.0));
        ui.painter().with_clip_rect(clipped).galley(clipped.min, name, visuals.text_color());

        let resp = resp.on_hover_text(format!("{}\n{}", asset.name, asset.path));
        if resp.clicked() {
            self.selected = Some(asset.id.clone());
            self.new_tag.clear();
        }
        if resp.double_clicked() {
            self.open(lib, asset);
        }
        if resp.drag_started() {
            self.requests.push(Request::Drag { file: asset.file(&lib.root), preview: asset.preview_file(&lib.root) });
        }
        resp.context_menu(|ui| self.context_menu(ui, lib, asset));
    }

    fn context_menu(&mut self, ui: &mut Ui, lib: &Library, asset: &Asset) {
        if let Some((app, name)) = app_for(asset)
            && ui.button(format!("{} {name}", self.tr("Ouvrir dans", "Open in"))).clicked()
        {
            self.requests.push(Request::OpenWith { app, file: asset.file(&lib.root) });
            ui.close();
        }
        if ui.button(self.tr("Ouvrir avec l'application par défaut", "Open with the default app")).clicked() {
            self.requests.push(Request::Open(asset.file(&lib.root)));
            ui.close();
        }
        if ui.button(self.tr("Afficher dans l'Explorateur", "Show in folder")).clicked() {
            self.requests.push(Request::Reveal(asset.file(&lib.root)));
            ui.close();
        }
        if ui.button(self.tr("Copier le chemin", "Copy path")).clicked() {
            ui.ctx().copy_text(asset.file(&lib.root).display().to_string());
            self.say(self.tr("Chemin copié", "Path copied").to_string());
            ui.close();
        }
        ui.separator();
        let fav = self.user.is_favorite(&asset.id);
        if ui.button(if fav { self.tr("Retirer des favoris", "Remove from favourites") } else { self.tr("Ajouter aux favoris", "Add to favourites") }).clicked() {
            self.user.toggle_favorite(&asset.id);
            self.revision += 1;
            self.mark_dirty();
            ui.close();
        }
    }

    fn open(&mut self, lib: &Library, asset: &Asset) {
        let file = asset.file(&lib.root);
        match app_for(asset) {
            Some((app, _)) => self.requests.push(Request::OpenWith { app, file }),
            None => self.requests.push(Request::Open(file)),
        }
    }

    fn details(&mut self, ui: &mut Ui, lib: &Arc<Library>) {
        let Some(id) = self.selected.clone() else { return };
        let Some(asset) = lib.by_id(&id).cloned() else {
            self.selected = None;
            return;
        };
        egui::ScrollArea::vertical().id_salt("library-details").auto_shrink(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.tr("DÉTAILS", "DETAILS")).small().weak());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("×").clicked() {
                        self.selected = None;
                    }
                });
            });
            ui.add_space(6.0);
            let w = ui.available_width();
            let (pic, resp) = ui.allocate_exact_size(vec2(w, w * 0.75), Sense::click_and_drag());
            let dark = ui.visuals().dark_mode;
            ui.painter().rect_filled(pic, CornerRadius::same(4), if dark { Color32::from_gray(30) } else { Color32::from_gray(236) });
            if may_be_transparent(&asset) {
                checkerboard(ui.painter(), pic.shrink(1.0), dark);
            }
            let (source, original) = thumb_source(lib, &asset);
            if let Some(tex) = self.thumbs.get(&asset.id, source, original) {
                let size = tex.size_vec2();
                let fit = ((pic.width() - 12.0) / size.x).min((pic.height() - 12.0) / size.y);
                ui.painter().image(tex.id(), Rect::from_center_size(pic.center(), size * fit), Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            }
            if resp.drag_started() {
                self.requests.push(Request::Drag { file: asset.file(&lib.root), preview: asset.preview_file(&lib.root) });
            }
            if resp.double_clicked() {
                self.open(lib, &asset);
            }
            ui.label(RichText::new(self.tr("Glissez l'aperçu vers Oracle Edit, Oracle Vector ou un autre logiciel.", "Drag the preview to Oracle Edit, Oracle Vector or any other app.")).small().weak());
            ui.add_space(8.0);
            ui.label(RichText::new(&asset.name).heading().strong());
            ui.add_space(4.0);
            egui::Grid::new("library-info").num_columns(2).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                let rows: Vec<(&str, String)> = vec![
                    (self.tr("Format", "Format"), asset.format.clone()),
                    (self.tr("Poids", "Size"), human_bytes(asset.bytes)),
                    (self.tr("Dimensions", "Dimensions"), asset.dimensions().unwrap_or_else(|| "—".into())),
                    (self.tr("Section", "Section"), asset.kind.clone()),
                    (self.tr("Licence", "Licence"), if asset.license.is_empty() { "—".into() } else { asset.license.clone() }),
                ];
                for (k, v) in rows {
                    ui.label(RichText::new(k).weak());
                    ui.add(egui::Label::new(v).wrap());
                    ui.end_row();
                }
            });
            if !asset.source.is_empty() {
                ui.hyperlink_to(self.tr("Source", "Source"), &asset.source);
            }
            ui.add_space(10.0);

            // Actions.
            let fav = self.user.is_favorite(&asset.id);
            ui.horizontal_wrapped(|ui| {
                let label = if fav { format!("★ {}", self.tr("Favori", "Favourite")) } else { self.tr("+ Favori", "+ Favourite").to_string() };
                if ui.button(label).on_hover_text(if fav { self.tr("Retirer des favoris", "Remove from favourites") } else { self.tr("Ajouter aux favoris", "Add to favourites") }).clicked() {
                    self.user.toggle_favorite(&asset.id);
                    self.revision += 1;
                    self.mark_dirty();
                }
                if let Some((app, name)) = app_for(&asset)
                    && ui.button(format!("{} {name}", self.tr("Ouvrir dans", "Open in"))).clicked()
                {
                    self.requests.push(Request::OpenWith { app, file: asset.file(&lib.root) });
                }
                if ui.button(self.tr("Afficher dans l'Explorateur", "Show in folder")).clicked() {
                    self.requests.push(Request::Reveal(asset.file(&lib.root)));
                }
                if ui.button(self.tr("Copier le chemin", "Copy path")).clicked() {
                    ui.ctx().copy_text(asset.file(&lib.root).display().to_string());
                    self.say(self.tr("Chemin copié", "Path copied").to_string());
                }
            });
            ui.add_space(12.0);

            // The user's tags: editable.
            ui.label(RichText::new(self.tr("MES TAGS", "MY TAGS")).small().weak());
            ui.horizontal_wrapped(|ui| {
                for tag in self.user.tags_of(&asset.id) {
                    let r = ui.add(egui::Button::new(RichText::new(format!("{tag}  ×")).color(Color32::WHITE)).fill(self.accent).corner_radius(4.0));
                    if r.on_hover_text(self.tr("Retirer ce tag", "Remove this tag")).clicked() {
                        self.user.remove_tag(&asset.id, &tag);
                        self.revision += 1;
                        self.mark_dirty();
                    }
                }
            });
            let hint = self.tr("+ Ajouter un tag (Entrée)", "+ Add a tag (Enter)").to_string();
            let r = ui.add(egui::TextEdit::singleline(&mut self.new_tag).hint_text(hint).desired_width(f32::INFINITY));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let tag = std::mem::take(&mut self.new_tag);
                self.user.add_tag(&asset.id, &tag);
                self.revision += 1;
                self.mark_dirty();
                r.request_focus();
            }
            ui.add_space(10.0);
            ui.label(RichText::new(self.tr("MOTS-CLÉS DU CATALOGUE", "CATALOGUE KEYWORDS")).small().weak());
            ui.horizontal_wrapped(|ui| {
                for t in asset.tags.iter().take(24) {
                    if ui.add(egui::Button::new(RichText::new(t).small()).corner_radius(4.0)).on_hover_text(self.tr("Rechercher ce mot", "Search for this word")).clicked() {
                        self.query.text = t.clone();
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(RichText::new(&asset.path).small().weak());
        });
    }
}

impl Drop for LibraryBrowser {
    fn drop(&mut self) {
        self.save_if_due(true);
    }
}

/// The picture to show for `asset`: the catalogue's preview, or the file itself for images and
/// SVGs (decoded, then cached on disk). The flag says whether it is an original.
fn thumb_source(lib: &Library, asset: &Asset) -> (Option<PathBuf>, bool) {
    if let Some(p) = &asset.preview {
        let original = p == &asset.path;
        return (Some(oracle_library::index::resolve(&lib.root, p)), original);
    }
    (None, false)
}

/// Formats that can carry transparency, drawn over a checkerboard.
fn may_be_transparent(asset: &Asset) -> bool {
    matches!(asset.format.as_str(), "PNG" | "SVG" | "SVGZ" | "GIF" | "WEBP" | "TIF" | "TIFF" | "PSD" | "AI" | "EPS" | "PDF")
}

fn human_bytes(b: u64) -> String {
    let b = b as f64;
    if b >= 1e9 {
        format!("{:.1} Go", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} Mo", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} Ko", b / 1e3)
    } else {
        format!("{b:.0} o")
    }
}

fn checkerboard(painter: &egui::Painter, r: Rect, dark: bool) {
    let (a, b) = if dark { (Color32::from_gray(44), Color32::from_gray(54)) } else { (Color32::from_gray(246), Color32::from_gray(226)) };
    painter.rect_filled(r, CornerRadius::same(3), a);
    let s = 10.0;
    let mut y = r.top();
    let mut row = 0;
    while y < r.bottom() {
        let mut x = r.left() + if row % 2 == 0 { 0.0 } else { s };
        while x < r.right() {
            let cell = Rect::from_min_max(egui::pos2(x, y), egui::pos2((x + s).min(r.right()), (y + s).min(r.bottom())));
            painter.rect_filled(cell, CornerRadius::ZERO, b);
            x += 2.0 * s;
        }
        y += s;
        row += 1;
    }
}

fn section_title(ui: &mut Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(text).small().weak());
    ui.add_space(2.0);
}

fn side_item(ui: &mut Ui, accent: Color32, label: &str, count: Option<usize>, on: bool) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let v = ui.visuals();
    if on {
        ui.painter().rect_filled(rect, CornerRadius::same(4), accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(4), v.widgets.hovered.weak_bg_fill);
    }
    let text_color = if on { Color32::WHITE } else { v.text_color() };
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(13.0), text_color);
    let clip = Rect::from_min_max(rect.min, egui::pos2(rect.right() - 48.0, rect.bottom()));
    ui.painter().with_clip_rect(clip).galley(rect.left_center() + vec2(8.0, -galley.size().y / 2.0), galley, text_color);
    if let Some(n) = count {
        let c = if on { Color32::from_white_alpha(200) } else { v.weak_text_color() };
        ui.painter().text(rect.right_center() - vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, n.to_string(), egui::FontId::proportional(11.5), c);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn chip(ui: &mut Ui, accent: Color32, label: &str, on: bool) -> egui::Response {
    let v = ui.visuals().clone();
    let (fill, text) = if on { (accent, Color32::WHITE) } else { (v.widgets.inactive.weak_bg_fill, v.text_color()) };
    ui.add(egui::Button::new(RichText::new(label).color(text).size(12.5)).fill(fill).corner_radius(4.0).min_size(vec2(0.0, 24.0)))
}
