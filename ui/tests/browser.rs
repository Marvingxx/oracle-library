//! The browser over a small library: search, select, tag. Writes `tests/snapshots/*.png` to look at.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use oracle_library_ui::LibraryBrowser;

fn library() -> std::path::PathBuf {
    // A fixed folder: the browser shows its path, which is in the snapshot.
    let dir = std::env::temp_dir().join("oracle-library-ui");
    let _ = std::fs::remove_dir_all(&dir);
    let files: [(&str, [u8; 3]); 6] = [
        ("01_Textures photo/Metal/Metal brosse.png", [150, 150, 160]),
        ("01_Textures photo/Metal/Rouille.png", [150, 80, 40]),
        ("01_Textures photo/Bois/Chene.png", [140, 100, 60]),
        ("01_Textures photo/Papiers/Papier froisse.png", [235, 235, 230]),
        ("02_Effets et overlays/Fumee/Fumee 01.png", [90, 90, 90]),
        ("06_Typographies/Montserrat/Montserrat-Bold.ttf", [0, 0, 0]),
    ];
    for (rel, rgb) in files {
        let path = oracle_library::index::resolve(&dir, rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        if rel.ends_with(".png") {
            image::RgbaImage::from_pixel(64, 48, image::Rgba([rgb[0], rgb[1], rgb[2], 255])).save(&path).unwrap();
        } else {
            std::fs::write(&path, b"font").unwrap();
        }
    }
    dir
}

#[test]
fn search_select_and_tag() {
    let root = library();
    let settings = root.join("settings.json");
    let mut browser = LibraryBrowser::with_settings(Some(settings.clone()));
    browser.set_root(root.clone());
    browser.set_search("métal");
    let mut harness = Harness::builder().with_size(egui::vec2(1100.0, 720.0)).build_ui_state(|ui, b: &mut LibraryBrowser| b.ui(ui), browser);
    for _ in 0..200 {
        harness.step();
        if harness.state().is_ready() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(harness.state().is_ready());
    harness.step();
    harness.get_by_label_contains("2 ressources");
    harness.state_mut().select_first();
    harness.state_mut().tag_selected("Campagne été");
    for _ in 0..30 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    harness.get_by_label_contains("Campagne été");
    harness.get_by_label_contains("DÉTAILS");
    harness.snapshot("browser_details");
    drop(harness);
    // Tags are saved for every Oracle app.
    let saved = std::fs::read_to_string(&settings).unwrap();
    assert!(saved.contains("Campagne été"), "{saved}");
}
