//! Indexes the library folder and runs a few searches: `cargo run --release --example probe -- [folder]`.

use oracle_library::{Library, Query, UserData, search};

fn main() -> Result<(), String> {
    let user = UserData::default();
    let root = std::env::args().nth(1).map(std::path::PathBuf::from).or_else(|| user.library_root()).ok_or("no library folder")?;
    let t = std::time::Instant::now();
    let lib = Library::open(&root)?;
    println!("{} assets ({} from the catalogue, {} found since) in {:?}", lib.assets.len(), lib.from_catalogue, lib.found_since, t.elapsed());
    for (kind, n) in &lib.kinds {
        println!("  {kind}: {n}");
    }
    for q in ["métal", "metal", "papier froissé", "grain", "bois", "police", "fumée", "icone fleche", "mockup"] {
        let t = std::time::Instant::now();
        let hits = search(&lib, &user, &Query { text: q.into(), ..Default::default() });
        let first: Vec<_> = hits.iter().take(3).map(|&i| lib.assets[i].path.clone()).collect();
        println!("{q:>16}: {:>6} in {:>9?}  {first:?}", hits.len(), t.elapsed());
    }
    Ok(())
}
