//! The browser over a small library: search, select, tag. Writes `tests/snapshots/*.png` to look at.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use oracle_library_ui::LibraryBrowser;

#[test]
fn compact_dock_indexes_and_searches_the_shared_folder() {
    let root = std::env::temp_dir().join(format!("oracle-library-compact-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for name in ["Metal.svg", "Papier.svg"] {
        std::fs::write(root.join(name), br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"#).unwrap();
    }
    let mut browser = LibraryBrowser::with_settings(None);
    browser.set_root(root);
    let mut harness = Harness::builder().with_size(egui::vec2(300.0, 720.0))
        .build_ui_state(|ui, b: &mut LibraryBrowser| b.ui(ui), browser);
    for _ in 0..200 {
        harness.step();
        if harness.state().is_ready() { break; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(harness.state().is_ready());
    harness.step();
    harness.get_by_label_contains("2 ressources");
    harness.state_mut().set_search("Metal");
    harness.step();
    harness.step();
    harness.get_by_label_contains("1 ressource");
    harness.get_by_label("Dossier…");
}

#[test]
fn short_dock_leaves_room_for_assets_and_keeps_format_filters_available() {
    let root = std::env::temp_dir().join(format!("oracle-library-short-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Metal.svg"), br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"/>"#).unwrap();
    let mut browser = LibraryBrowser::with_settings(None);
    browser.set_root(root);
    let mut harness = Harness::builder().with_size(egui::vec2(300.0, 250.0))
        .build_ui_state(|ui, b: &mut LibraryBrowser| b.ui(ui), browser);
    for _ in 0..200 {
        harness.step();
        if harness.state().is_ready() { break; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(harness.state().is_ready());
    harness.step();
    let counter = harness.get_by_label_contains("1 ressource");
    assert!(counter.rect().bottom() < 160.0, "the asset grid needs visible space below the controls");
    harness.get_by_label("Formats").click();
    harness.step();
    harness.get_by_label("Images").click();
    harness.step();
    harness.step();
    harness.get_by_label_contains("0 ressource");
}

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
    // In the details pane and among the sidebar's tags.
    assert!(harness.query_all_by_label_contains("Campagne été").count() >= 1);
    harness.get_by_label_contains("DÉTAILS");
    harness.snapshot("browser_details");
    drop(harness);
    // Tags are saved for every Oracle app.
    let saved = std::fs::read_to_string(&settings).unwrap();
    assert!(saved.contains("Campagne été"), "{saved}");
}

/// A brand of Oracle Brand Kit is a collection: its logo (the master itself, not a copy) and its
/// palette, read again when the brand changes.
#[test]
fn a_brand_is_a_collection_that_follows_its_project() {
    let dir = std::env::temp_dir().join("oracle-library-ui-brands");
    let _ = std::fs::remove_dir_all(&dir);
    let root = dir.join("library");
    std::fs::create_dir_all(root.join("01_Textures")).unwrap();
    image::RgbaImage::from_pixel(8, 8, image::Rgba([1, 2, 3, 255])).save(root.join("01_Textures").join("Papier.png")).unwrap();
    let project = dir.join("Maison Ete");
    std::fs::create_dir_all(project.join("Sources")).unwrap();
    let logo = project.join("Sources").join("logo.svg");
    std::fs::write(&logo, r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#d31c3b"/></svg>"##).unwrap();
    let brand = |red: &str| {
        format!(
            r#"{{"schemaVersion":1,"projectId":"p-1","brandName":"Maison Été","assets":[{{"id":"a-1","role":"primaryLogo","variant":"color","source":"Sources/logo.svg"}}],"colors":[{{"id":"c-1","name":"Rouge Été","hex":"{red}"}}]}}"#
        )
    };
    std::fs::write(project.join("brand.json"), brand("#d31c3b")).unwrap();
    let settings = dir.join("Oracle").join("library.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    oracle_suite_link::brands::Registry::at(dir.join("Oracle").join("brands.json")).register(&project).unwrap();

    let mut browser = LibraryBrowser::with_settings(Some(settings));
    browser.set_root(root);
    let mut harness = Harness::builder().with_size(egui::vec2(1100.0, 720.0)).build_ui_state(|ui, b: &mut LibraryBrowser| b.ui(ui), browser);
    for _ in 0..200 {
        harness.step();
        if harness.state().is_ready() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    harness.step();
    harness.get_by_label_contains("MARQUES");
    harness.get_by_label_contains("Maison Été").click();
    harness.step();
    harness.step();
    harness.get_by_label_contains("1 ressource");
    harness.get_by_label_contains("#D31C3B");
    harness.get_by_label_contains("Ouvrir dans Oracle Brand Kit");
    for _ in 0..20 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    harness.snapshot("browser_brand");
    // The designer changes the red in Brand Kit: the palette follows (checked every 2 s).
    std::fs::write(project.join("brand.json"), brand("#1c3bd3").replace("Rouge Été", "Bleu Été")).unwrap();
    let mut seen = false;
    for _ in 0..150 {
        harness.step();
        if harness.query_by_label_contains("#1C3BD3").is_some() {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    assert!(seen, "the new colour shows without a refresh");
}
