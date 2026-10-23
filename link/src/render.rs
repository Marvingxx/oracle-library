//! A native file drawn by the app that made it, for another app to place or show.
//!
//! The drawing is done by the owning app's command-line engine (`oracle-vector-cli convert`,
//! `oracle-edit-cli convert`, `oracle-guide-cli run --export`), so it looks exactly as in that app.
//! Results are kept in `%LOCALAPPDATA%\Oracle\renders`, named after the source's path, size and
//! modification time: an unchanged file is never drawn twice, a saved one is drawn again, and the
//! drawings of its older versions are removed. When the owning app is missing, the newest drawing
//! of the file is used if there is one, so a document placed earlier still shows.
//!
//! Drawing runs a program and can take seconds: call it off the interface thread.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{App, Preview, oracle_format};

/// The longest a drawing may take before it is given up (a stuck or very large document).
const TIMEOUT: Duration = Duration::from_secs(180);

/// A drawing of a native file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rendered {
    /// The drawing, in the cache.
    pub path: PathBuf,
    /// What it is.
    pub preview: Preview,
    /// `true` when it is of an older version of the file: the owning app couldn't draw the current
    /// one (missing, or it failed), and this is the last drawing made.
    pub stale: bool,
}

/// Where drawings are kept.
pub fn cache_dir() -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join("Oracle").join("renders"))
}

/// `source` (an Oracle file) drawn as `preview` by the app that made it. `Err` says why not, in
/// French, for the interface.
pub fn render(source: &Path, preview: Preview) -> Result<Rendered, String> {
    let format = oracle_format(source).ok_or_else(|| format!("{} n'est pas un fichier Oracle", source.display()))?;
    if !format.can(preview) {
        return Err(format!("{} ne se dessine pas en {}", format.label_fr, preview.ext().to_uppercase()));
    }
    let dir = cache_dir().ok_or("dossier de cache introuvable")?;
    render_in(&dir, source, preview, format.app.cli())
}

/// [`render`] with the cache folder and the owning app's engine given (tests).
fn render_in(dir: &Path, source: &Path, preview: Preview, cli: Option<PathBuf>) -> Result<Rendered, String> {
    let meta = std::fs::metadata(source).map_err(|e| format!("{} : {e}", source.display()))?;
    let stamp = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    let key = source_key(source);
    let name = format!("{key:016x}-{:x}-{stamp:x}.{}", meta.len(), preview.ext());
    let target = dir.join(&name);
    if target.is_file() {
        return Ok(Rendered { path: target, preview, stale: false });
    }
    let drawn = match cli {
        Some(cli) => draw(&cli, source, preview, dir, &target),
        None => Err(format!("{} est introuvable : il dessine ce fichier", oracle_format(source).map_or("L'app", |f| f.app.name()))),
    };
    match drawn {
        Ok(()) => {
            forget_older(dir, key, preview, &name);
            Ok(Rendered { path: target, preview, stale: false })
        }
        // The last drawing of an older version is better than nothing.
        Err(error) => match newest(dir, key, preview) {
            Some(path) => {
                log::warn!("{error}; dernier rendu utilisé : {}", path.display());
                Ok(Rendered { path, preview, stale: true })
            }
            None => Err(error),
        },
    }
}

/// A stable 64-bit name for the file's path (FNV-1a over its absolute form, case-folded on
/// Windows where paths are case-insensitive).
fn source_key(source: &Path) -> u64 {
    let abs = std::path::absolute(source).unwrap_or_else(|_| source.to_path_buf());
    let mut text = abs.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        text = text.to_lowercase();
    }
    text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// Runs the engine into a temporary file next to `target`, then puts it in place, so a reader never
/// sees half a drawing.
fn draw(cli: &Path, source: &Path, preview: Preview, dir: &Path, target: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    let part = dir.join(format!("part-{}-{}.{}", std::process::id(), unique(), preview.ext()));
    let mut cmd = Command::new(cli);
    match oracle_format(source).map(|f| f.app) {
        Some(App::Guide) => {
            cmd.arg("run").arg("--in").arg(source);
            if preview == Preview::Png {
                cmd.args(["--page", "1"]);
            }
            cmd.arg("--export").arg(&part);
        }
        _ => {
            cmd.arg("convert").arg(source).arg(&part);
        }
    }
    let result = run(cmd).and_then(|()| {
        if part.is_file() { std::fs::rename(&part, target).map_err(|e| format!("{} : {e}", target.display())) } else { Err("le rendu n'a produit aucun fichier".into()) }
    });
    let _ = std::fs::remove_file(&part); // Already moved, or a failed attempt's leftovers.
    result
}

fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Runs `cmd` without a console window, waiting at most [`TIMEOUT`].
fn run(mut cmd: Command) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let program = cmd.get_program().to_string_lossy().to_string();
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("impossible de lancer {program} : {e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                let mut err = String::new();
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut pipe, &mut err); // The message is a bonus.
                }
                return Err(format!("{program} a échoué ({status}) : {}", err.trim().chars().take(400).collect::<String>()));
            }
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill(); // Already gone is fine.
                return Err(format!("{program} n'a pas fini en {} s", TIMEOUT.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("{program} : {e}")),
        }
    }
}

/// The drawings of `key` as `preview`, newest first.
fn drawings(dir: &Path, key: u64, preview: Preview) -> Vec<(std::time::SystemTime, PathBuf)> {
    let prefix = format!("{key:016x}-");
    let suffix = format!(".{}", preview.ext());
    let mut found: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with(&prefix) && n.ends_with(&suffix)))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path()))
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found
}

fn newest(dir: &Path, key: u64, preview: Preview) -> Option<PathBuf> {
    drawings(dir, key, preview).into_iter().next().map(|(_, p)| p)
}

/// Removes the drawings of older versions of the file, keeping `keep`.
fn forget_older(dir: &Path, key: u64, preview: Preview, keep: &str) {
    for (_, path) in drawings(dir, key, preview) {
        if path.file_name().and_then(|n| n.to_str()) != Some(keep) {
            let _ = std::fs::remove_file(&path); // In use elsewhere: it goes next time.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oracle-link-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn keys_are_stable_and_case_insensitive_on_windows() {
        assert_eq!(source_key(Path::new("C:/a/Logo.oracle-vector")), source_key(Path::new("C:/a/Logo.oracle-vector")));
        assert_ne!(source_key(Path::new("C:/a/one.oracle-vector")), source_key(Path::new("C:/a/two.oracle-vector")));
        if cfg!(windows) {
            assert_eq!(source_key(Path::new("C:\\A\\LOGO.oracle-vector")), source_key(Path::new("c:/a/logo.oracle-vector")));
        }
    }

    #[test]
    fn without_the_app_the_last_drawing_is_used_and_said_to_be_old() {
        let dir = temp("stale");
        let src = dir.join("logo.oracle-vector");
        std::fs::write(&src, "{}").unwrap();
        // No app, no drawing: a clear error.
        let err = render_in(&dir, &src, Preview::Svg, None).unwrap_err();
        assert!(err.contains("Oracle Vector"), "{err}");
        // A drawing of an older version: used, marked stale.
        let old = dir.join(format!("{:016x}-1-1.svg", source_key(&src)));
        std::fs::write(&old, "<svg/>").unwrap();
        let r = render_in(&dir, &src, Preview::Svg, None).unwrap();
        assert_eq!((r.path, r.stale), (old, true));
    }

    #[test]
    fn a_current_drawing_is_reused_without_running_anything() {
        let dir = temp("reuse");
        let src = dir.join("logo.oracle-vector");
        std::fs::write(&src, "{}").unwrap();
        let meta = std::fs::metadata(&src).unwrap();
        let stamp = meta.modified().unwrap().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let current = dir.join(format!("{:016x}-{:x}-{stamp:x}.svg", source_key(&src), meta.len()));
        std::fs::write(&current, "<svg/>").unwrap();
        let r = render_in(&dir, &src, Preview::Svg, Some(PathBuf::from("Z:/no/engine.exe"))).unwrap();
        assert_eq!((r.path, r.stale), (current, false));
    }

    #[test]
    fn a_failing_engine_is_an_error_not_a_hang() {
        let dir = temp("fail");
        let src = dir.join("photo.pcraft");
        std::fs::write(&src, "not a document").unwrap();
        // Any program that exits with an error stands for a broken engine.
        let bad = if cfg!(windows) { PathBuf::from(std::env::var("ComSpec").unwrap_or("cmd.exe".into())) } else { PathBuf::from("/bin/false") };
        let err = render_in(&dir, &src, Preview::Png, Some(bad)).unwrap_err();
        assert!(!err.is_empty());
    }

    /// The real engines, when this suite's builds are there: an `.oracle-vector` drawn as SVG and
    /// PNG, and drawn again once the file changes.
    #[test]
    fn real_engines_draw_and_redraw() {
        let Some(cli) = App::Vector.cli() else { return };
        let dir = temp("real");
        let svg = dir.join("in.svg");
        std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#d31c3b"/></svg>"##).unwrap();
        let src = dir.join("logo.oracle-vector");
        let mut c = Command::new(&cli);
        c.arg("convert").arg(&svg).arg(&src);
        run(c).unwrap();
        let cache = dir.join("cache");
        let first = render_in(&cache, &src, Preview::Svg, Some(cli.clone())).unwrap();
        assert!(std::fs::read_to_string(&first.path).unwrap().contains("<svg"));
        let png = render_in(&cache, &src, Preview::Png, Some(cli.clone())).unwrap();
        assert!(std::fs::read(&png.path).unwrap().starts_with(b"\x89PNG"));
        // Saved again (another size): a new drawing replaces the old one.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="20"><rect width="80" height="20" fill="#d31c3b"/></svg>"##).unwrap();
        let mut c = Command::new(&cli);
        c.arg("convert").arg(&svg).arg(&src);
        run(c).unwrap();
        let second = render_in(&cache, &src, Preview::Svg, Some(cli)).unwrap();
        assert_ne!(second.path, first.path);
        assert!(!first.path.exists(), "the older drawing is removed");
    }
}
