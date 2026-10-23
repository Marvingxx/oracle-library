//! Asking another Oracle app to do something with a file ("Send to Oracle Vector…"): a request is
//! a small file in that app's inbox folder (`%LOCALAPPDATA%\Oracle\inbox\<app id>`), which the app
//! takes when it looks (about every second, see [`take`]). If the app isn't running, it is started.
//!
//! A running app says so by touching its `alive` file each time it looks; one that hasn't for a few
//! seconds is taken as closed. Requests are written whole (a temporary name, then renamed), so the
//! app never reads half of one. Nothing here needs a network or a service: two programs and a folder.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::App;

/// What the receiving app does with the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Place it in the active document, linked (a new document when none is open).
    Place,
    /// Open it as a document.
    Open,
}

impl Action {
    fn word(self) -> &'static str {
        match self {
            Action::Place => "place",
            Action::Open => "open",
        }
    }

    fn parse(word: &str) -> Option<Action> {
        match word {
            "place" => Some(Action::Place),
            "open" => Some(Action::Open),
            _ => None,
        }
    }
}

/// A request taken from an inbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub action: Action,
    pub file: PathBuf,
}

/// How long after its last look an app still counts as running.
const ALIVE: Duration = Duration::from_secs(5);
/// Requests older than this are dropped unread (the app was gone and they no longer make sense).
const STALE: Duration = Duration::from_secs(600);

/// The inbox folder of `app`.
pub fn inbox_dir(app: App) -> Option<PathBuf> {
    Some(dirs::data_local_dir()?.join("Oracle").join("inbox").join(app.id()))
}

/// Sends `file` to `app` to `action` it, starting the app when it isn't running. `Err` (French, for
/// the interface) says why it couldn't.
pub fn send(app: App, action: Action, file: &Path) -> Result<(), String> {
    let dir = inbox_dir(app).ok_or("dossier de l'utilisateur introuvable")?;
    let running = is_running_in(&dir);
    send_in(&dir, action, file)?;
    if !running {
        let exe = app.exe().ok_or_else(|| format!("{} est introuvable sur cet ordinateur", app.name()))?;
        std::process::Command::new(&exe).spawn().map_err(|e| format!("impossible de lancer {} : {e}", app.name()))?;
    }
    Ok(())
}

/// Keeps `app` marked as running and wakes it when requests arrive, from a background thread: an
/// idle app draws no frames, so it would otherwise be taken for closed (and started a second time)
/// and answer only when next used. `wake` is called (egui's `request_repaint`) while requests are
/// waiting; the app takes them ([`take`]) on the frame it then draws. Call once, at start-up: the
/// thread lasts as long as the program.
pub fn watch(app: App, wake: impl Fn() + Send + 'static) {
    if let Some(dir) = inbox_dir(app) {
        watch_in(dir, app.id(), wake);
    }
}

fn watch_in(dir: PathBuf, id: &str, wake: impl Fn() + Send + 'static) {
    let spawned = std::thread::Builder::new().name(format!("{id}-inbox")).spawn(move || {
        loop {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("alive"), b""); // Best effort, as in take_in.
            if has_requests(&dir) {
                wake();
            }
            std::thread::sleep(WATCH_EVERY);
        }
    });
    if let Err(e) = spawned {
        log::warn!("{id}: inbox watch not started: {e}");
    }
}

/// How often [`watch`] looks: well within [`ALIVE`].
const WATCH_EVERY: Duration = Duration::from_millis(500);

fn has_requests(dir: &Path) -> bool {
    std::fs::read_dir(dir).into_iter().flatten().flatten().any(|e| e.path().extension().is_some_and(|x| x == "request"))
}

/// Whether `app` looked at its inbox in the last few seconds (it is running).
pub fn is_running(app: App) -> bool {
    inbox_dir(app).is_some_and(|d| is_running_in(&d))
}

fn is_running_in(dir: &Path) -> bool {
    std::fs::metadata(dir.join("alive"))
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < ALIVE)
}

fn send_in(dir: &Path, action: Action, file: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    let stamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let name = format!("{stamp:032}-{}", std::process::id());
    let part = dir.join(format!("{name}.part"));
    std::fs::write(&part, format!("{}\n{}\n", action.word(), file.display())).map_err(|e| format!("{} : {e}", part.display()))?;
    std::fs::rename(&part, dir.join(format!("{name}.request"))).map_err(|e| format!("{} : {e}", dir.display()))
}

/// The requests waiting for `app`, oldest first, removed from its inbox; also marks `app` as
/// running. Cheap: call it about once a second from the app's frame.
pub fn take(app: App) -> Vec<Request> {
    inbox_dir(app).map(|d| take_in(&d)).unwrap_or_default()
}

fn take_in(dir: &Path) -> Vec<Request> {
    if std::fs::create_dir_all(dir).is_err() {
        return Vec::new();
    }
    let _ = std::fs::write(dir.join("alive"), b""); // Best effort: a sender then also starts the app.
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "request"))
        .collect();
    names.sort();
    let mut requests = Vec::new();
    for path in names {
        let fresh = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < STALE);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let _ = std::fs::remove_file(&path); // Taken; one that can't be removed is taken once more at worst.
        let mut lines = text.lines();
        if let (true, Some(action), Some(file)) = (fresh, lines.next().and_then(Action::parse), lines.next()) {
            requests.push(Request { action, file: PathBuf::from(file) });
        }
    }
    requests
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The watch marks the app running, and wakes it while a request waits.
    #[test]
    fn the_watch_keeps_the_app_alive_and_wakes_it_for_a_request() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let dir = temp("watch");
        let woken = Arc::new(AtomicUsize::new(0));
        let count = woken.clone();
        watch_in(dir.clone(), "test", move || {
            count.fetch_add(1, Ordering::Relaxed);
        });
        std::thread::sleep(Duration::from_millis(300));
        assert!(is_running_in(&dir));
        assert_eq!(woken.load(Ordering::Relaxed), 0, "nothing to take");
        send_in(&dir, Action::Place, Path::new("C:/logo.oracle-vector")).unwrap();
        std::thread::sleep(WATCH_EVERY * 3);
        assert!(woken.load(Ordering::Relaxed) > 0, "woken for the request");
        assert_eq!(take_in(&dir).len(), 1);
    }

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("oracle-inbox-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn requests_arrive_in_order_once() {
        let dir = temp("order");
        send_in(&dir, Action::Place, Path::new("C:/logos/a.oracle-vector")).unwrap();
        send_in(&dir, Action::Open, Path::new("C:/logos/b.pcraft")).unwrap();
        let got = take_in(&dir);
        assert_eq!(got.iter().map(|r| r.action).collect::<Vec<_>>(), [Action::Place, Action::Open]);
        assert!(got[0].file.ends_with("a.oracle-vector") && got[1].file.ends_with("b.pcraft"));
        assert!(take_in(&dir).is_empty(), "taken once");
    }

    #[test]
    fn an_app_that_looks_is_running_and_half_written_requests_wait() {
        let dir = temp("alive");
        assert!(!is_running_in(&dir));
        assert!(take_in(&dir).is_empty());
        assert!(is_running_in(&dir), "looking marks it running");
        std::fs::write(dir.join("123.part"), "place\nC:/x.svg\n").unwrap();
        assert!(take_in(&dir).is_empty(), "a request still being written isn't taken");
    }
}
