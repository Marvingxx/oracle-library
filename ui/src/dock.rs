//! Right-hand studio dock for the companion apps that have no existing docking adapter.
use craft_ui::docking::{Action, DockArea, DockStyle, Layout, Node, Permissions};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LibraryPanel {
    Properties,
    Library,
}

#[derive(Default)]
pub struct LibraryDock {
    layout: Layout<LibraryPanel>,
    initialized: bool,
    has_properties: bool,
}

impl LibraryDock {
    pub fn initialize(&mut self, properties: bool) {
        if !self.initialized {
            self.initialized = true;
            self.has_properties = properties;
            if properties {
                self.layout.root = Some(Node::Tabs { panels: vec![LibraryPanel::Properties], active: 0 });
            }
        }
    }

    pub fn open_library(&mut self) {
        let panel = LibraryPanel::Library;
        let action = if self.layout.contains(&panel) {
            Action::Activate { panel }
        } else {
            let root = Layout { root: self.layout.root.clone(), floating: Vec::new() };
            Action::Open { panel, anchor: root.panels().first().map(|p| **p) }
        };
        if let Err(error) = self.layout.apply(action) {
            log::warn!("Cannot open library dock: {error}");
        }
    }

    pub fn menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Fenêtre", |ui| {
            if ui.selectable_label(self.layout.contains(&LibraryPanel::Library), "Bibliothèque Oracle").clicked() {
                self.open_library();
                ui.close();
            }
            if self.has_properties && !self.layout.contains(&LibraryPanel::Properties)
                && ui.button("Propriétés").clicked() {
                let _ = self.layout.apply(Action::Open { panel: LibraryPanel::Properties, anchor: None });
                ui.close();
            }
        });
    }

    pub fn show(&mut self, ui: &mut egui::Ui, width: f32, body: impl FnMut(&mut egui::Ui, &LibraryPanel)) {
        let rooted = self.layout.root.is_some();
        let floating = !self.layout.floating.is_empty();
        // Craft's docking surface needs a root panel for its normal drop targets.
        // Keep a right-edge target available when the last tab was detached.
        let available = ui.available_rect_before_wrap();
        let empty_target = egui::Rect::from_min_max(
            egui::pos2((available.right() - width).max(available.left()), available.top()), available.max,
        );
        let render = |ui: &mut egui::Ui| {
            let mut style = DockStyle::from_ui(ui);
            style.float_label = "Détacher".into();
            style.close_label = "Fermer".into();
            style.move_label = "Déplacer dans le groupe".into();
            let output = DockArea::new(egui::Id::new("oracle-library-studio-dock")).show(
                ui, &self.layout, &style,
                |panel| match panel { LibraryPanel::Properties => "Propriétés", LibraryPanel::Library => "Bibliothèque Oracle" }.into(),
                |_| Permissions::default(), body,
            );
            for action in output.actions {
                if !rooted && ui.input(|i| i.pointer.primary_released()
                    && i.pointer.interact_pos().is_some_and(|p| empty_target.contains(p)))
                    && let Action::Float { panel, .. } = &action
                {
                    let mut restored = self.layout.clone();
                    if restored.apply(Action::Close { panel: *panel }).is_ok()
                        && restored.apply(Action::Open { panel: *panel, anchor: None }).is_ok()
                    {
                        self.layout = restored;
                        continue;
                    }
                }
                if let Err(error) = self.layout.apply(action) {
                    log::warn!("Cannot change library dock: {error}");
                }
            }
        };
        if rooted {
            egui::Panel::right("oracle-library-studio").default_size(width).min_size(260.0).resizable(true).show(ui, render);
        } else if floating {
            if ui.ctx().dragged_id().is_some()
                && ui.input(|i| i.pointer.interact_pos().is_some_and(|p| empty_target.contains(p)))
            {
                ui.painter().rect_stroke(empty_target.shrink(1.0), 0.0,
                    egui::Stroke::new(2.0, ui.visuals().selection.stroke.color), egui::StrokeKind::Inside);
            }
            // An overlay child renders detached groups without reserving a sidebar.
            let mut overlay = ui.new_child(egui::UiBuilder::new().id_salt("oracle-library-overlay").max_rect(ui.available_rect_before_wrap()));
            render(&mut overlay);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_library_tab_drag_detaches_and_redocks_in_the_companion_studio() {
        check_pointer_drag(true);
    }

    #[test]
    fn oracle_library_last_tab_redocks_into_an_empty_studio() {
        check_pointer_drag(false);
    }

    fn check_pointer_drag(properties: bool) {
        use egui::{Event, Modifiers, PointerButton, pos2, vec2};
        let mut dock = LibraryDock::default();
        dock.initialize(properties);
        dock.open_library();
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1280.0, 800.0)).wgpu().build_ui_state(
            |ui, dock: &mut LibraryDock| {
                dock.show(ui, 360.0, |ui, panel| { ui.label(format!("{panel:?}")); });
                egui::CentralPanel::default().show(ui, |ui| { ui.label("Oracle studio"); });
            }, dock,
        );
        h.run_steps(4);
        let capture = |h: &mut egui_kittest::Harness<'_, LibraryDock>, state: &str| {
            if let Some(dir) = std::env::var_os("ORACLE_LIBRARY_DOCK_EVIDENCE") {
                let dir = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&dir).unwrap();
                h.render().unwrap().save(dir.join(format!("companion-{}-{state}.png", if properties { "properties" } else { "empty" }))).unwrap();
            }
        };
        capture(&mut h, "docked");
        let id = egui::Id::new("oracle-library-studio-dock");
        let tab = |h: &egui_kittest::Harness<'_, LibraryDock>, panel| h.ctx.read_response(id.with(("tab", &panel))).unwrap().rect.center();
        let drag = |h: &mut egui_kittest::Harness<'_, LibraryDock>, from, to| {
            h.event(Event::PointerMoved(from));
            h.run_steps(1);
            h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
            h.run_steps(1);
            for step in 1..=8 {
                h.event(Event::PointerMoved(from + (to - from) * (step as f32 / 8.0)));
                h.run_steps(1);
            }
            h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
            h.run_steps(3);
        };
        let source = tab(&h, LibraryPanel::Library);
        assert!(source.x > 640.0);
        assert!(h.state().layout.floating.is_empty());
        drag(&mut h, source, pos2(160.0, 140.0));
        assert!(h.state().layout.floating.iter().any(|f| f.panels.contains(&LibraryPanel::Library)));
        capture(&mut h, "detached");
        let source = tab(&h, LibraryPanel::Library);
        let target = if properties { tab(&h, LibraryPanel::Properties) } else { pos2(1150.0, 300.0) };
        drag(&mut h, source, target);
        assert!(h.state().layout.floating.is_empty());
        assert!(h.state().layout.contains(&LibraryPanel::Library));
        capture(&mut h, "redocked");
    }

    #[test]
    fn library_opens_beside_properties_and_reopens_without_floating() {
        let mut dock = LibraryDock::default();
        dock.initialize(true);
        dock.open_library();
        assert!(matches!(&dock.layout.root, Some(Node::Tabs { panels, active: 1 }) if panels == &[LibraryPanel::Properties, LibraryPanel::Library]));
        assert!(dock.layout.floating.is_empty());
        dock.layout.apply(Action::Close { panel: LibraryPanel::Library }).unwrap();
        dock.open_library();
        assert!(dock.layout.floating.is_empty());
        assert_eq!(dock.layout.panels().len(), 2);
    }

    #[test]
    fn activating_an_explicitly_detached_tab_preserves_its_position() {
        let mut dock = LibraryDock::default();
        dock.initialize(false);
        dock.open_library();
        dock.layout.apply(Action::Float { panel: LibraryPanel::Library, rect: [30.0, 40.0, 400.0, 500.0] }).unwrap();
        let expected = dock.layout.clone();
        dock.open_library();
        assert_eq!(dock.layout, expected);
    }
}
