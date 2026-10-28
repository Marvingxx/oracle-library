//! oracle-grids: the brand tools from the command line.
//!
//!   oracle-grids base         --area 1200x800 --kind square|isometric|hexagon|golden --size 40 -o grille.svg
//!   oracle-grids construction logo.svg [--elements h,v,d,c,a,p,o,s,m] [--clean] -o construction.svg
//!   oracle-grids clearspace   logo.svg --unit logomark|px|cm|in --size 1 [--invert] [--fill] -o zone.svg
//!   oracle-grids lockup       logo.svg --kind horizontal|vertical|condensed|partner [--partner p.svg] -o lockup.svg
//!   oracle-grids bento        --area 1080x1080 --style simple|centered|brick|random --sections 6 -o bento.svg
//!
//! `--params réglages.json` reads every setting of the tool at once (the JSON the apps save);
//! `--json sortie.json` also writes the layers as JSON. Without `-o`, the SVG goes to stdout.

use oracle_brandtools::*;
use std::collections::BTreeMap;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("oracle-grids : {e}");
            ExitCode::FAILURE
        }
    }
}

struct Args {
    tool: String,
    input: Option<String>,
    opts: BTreeMap<String, String>,
    flags: Vec<String>,
}

fn parse(args: Vec<String>) -> Result<Args, String> {
    let mut it = args.into_iter().peekable();
    let tool = it.next().ok_or_else(usage)?;
    let mut a = Args { tool, input: None, opts: BTreeMap::new(), flags: Vec::new() };
    const FLAGS: [&str; 6] = ["--clean", "--invert", "--fill", "--numbers", "--artboard", "--no-logo"];
    while let Some(x) = it.next() {
        if FLAGS.contains(&x.as_str()) {
            a.flags.push(x[2..].to_string());
        } else if let Some(k) = x.strip_prefix("--").or_else(|| x.strip_prefix('-')) {
            let v = it.next().ok_or_else(|| format!("valeur manquante après {x}"))?;
            a.opts.insert(k.to_string(), v);
        } else if a.input.is_none() {
            a.input = Some(x);
        } else {
            return Err(format!("argument inattendu : {x}"));
        }
    }
    Ok(a)
}

fn usage() -> String {
    "outil manquant : base, construction, clearspace, lockup ou bento".into()
}

impl Args {
    fn num(&self, k: &str) -> Result<Option<f64>, String> {
        self.opts.get(k).map(|v| v.replace(',', ".").parse::<f64>().map_err(|_| format!("--{k} : nombre attendu, reçu {v}"))).transpose()
    }
    fn flag(&self, k: &str) -> bool {
        self.flags.iter().any(|f| f == k)
    }
    fn params<T: serde::de::DeserializeOwned + Default>(&self) -> Result<T, String> {
        match self.opts.get("params") {
            Some(path) => {
                let s = std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?;
                serde_json::from_str(&s).map_err(|e| format!("{path} : {e}"))
            }
            None => Ok(T::default()),
        }
    }
    fn logo(&self) -> Result<Logo, String> {
        let path = self.input.as_ref().ok_or("fichier SVG du logo manquant")?;
        let src = std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?;
        Logo::from_svg(&src).map_err(|e| format!("{path} : {e}"))
    }
    fn area(&self, default: (f64, f64)) -> Result<Rect, String> {
        let (w, h) = match self.opts.get("area") {
            Some(v) => {
                let (w, h) = v.split_once(['x', 'X', '×']).ok_or("--area : LARGEURxHAUTEUR attendu")?;
                (w.trim().parse().map_err(|_| "--area : largeur")?, h.trim().parse().map_err(|_| "--area : hauteur")?)
            }
            None => default,
        };
        Ok(Rect::from_size(0.0, 0.0, w, h))
    }
}

fn pick<T: Copy>(v: &str, options: &[(&str, T)], what: &str) -> Result<T, String> {
    options.iter().find(|(k, _)| *k == v).map(|(_, t)| *t).ok_or_else(|| {
        format!("{what} : {v} inconnu ({})", options.iter().map(|(k, _)| *k).collect::<Vec<_>>().join(", "))
    })
}

fn run(args: Vec<String>) -> Result<(), String> {
    let a = parse(args)?;
    let mut logo_under = None;
    let drawing = match a.tool.as_str() {
        "base" => {
            let mut p: BaseParams = a.params()?;
            if let Some(k) = a.opts.get("kind") {
                p.kind = pick(
                    k,
                    &[("square", BaseKind::Square), ("isometric", BaseKind::Isometric), ("hexagon", BaseKind::Hexagon), ("golden", BaseKind::GoldenRatio)],
                    "--kind",
                )?;
            }
            p.size = a.num("size")?.unwrap_or(p.size);
            p.golden_height = a.num("golden-height")?.unwrap_or(p.golden_height);
            p.stroke = a.num("stroke")?.unwrap_or(p.stroke);
            if let Some(c) = a.opts.get("color") {
                p.color = c.clone();
            }
            let area = match &a.input {
                Some(_) => {
                    let l = a.logo()?;
                    let r = Rect::from_size(0.0, 0.0, l.width, l.height);
                    logo_under = Some(l);
                    r
                }
                None => a.area((1200.0, 800.0))?,
            };
            base_grid(area, &p)
        }
        "construction" => {
            let mut p: ConstructionParams = a.params()?;
            if let Some(list) = a.opts.get("elements") {
                let on: Vec<&str> = list.split(',').map(str::trim).collect();
                let has = |k: &str| on.contains(&k);
                p.horizontals = has("h");
                p.verticals = has("v");
                p.diagonals = has("d");
                p.circles = has("c");
                p.anchors = has("a");
                p.handles = has("p");
                p.outlines = has("o");
                p.spacing = has("s");
                p.measurements = has("m");
            }
            p.angle_tolerance_deg = a.num("tolerance")?.unwrap_or(p.angle_tolerance_deg);
            p.sensitivity = a.num("sensitivity")?.unwrap_or(p.sensitivity);
            if let Some(c) = a.opts.get("circles") {
                p.circle_filter = pick(c, &[("all", CircleFilter::All), ("big", CircleFilter::Big), ("small", CircleFilter::Small)], "--circles")?;
            }
            if let Some(s) = a.opts.get("spacing") {
                p.spacing_placement = pick(s, &[("inside", Placement::Inside), ("outside", Placement::Outside)], "--spacing")?;
                p.spacing = true;
            }
            p.clean |= a.flag("clean");
            let l = a.logo()?;
            let artboard = Rect::from_size(0.0, 0.0, l.width, l.height);
            if a.flag("artboard") {
                p.extent = Extent::Artboard;
            }
            let d = construction(&l, Some(artboard), &p);
            logo_under = Some(l);
            d
        }
        "clearspace" => {
            let mut p: ClearspaceParams = a.params()?;
            if let Some(u) = a.opts.get("unit") {
                p.unit = pick(u, &[("logomark", ClearUnit::Logomark), ("px", ClearUnit::Px), ("cm", ClearUnit::Cm), ("in", ClearUnit::In)], "--unit")?;
            }
            p.size = a.num("size")?.unwrap_or(p.size);
            p.invert |= a.flag("invert");
            p.fill |= a.flag("fill");
            p.min_print_mm = a.num("min-print")?.unwrap_or(p.min_print_mm);
            p.min_screen_px = a.num("min-screen")?.unwrap_or(p.min_screen_px);
            let l = a.logo()?;
            let (d, cs) = clearspace(&l, &p);
            eprintln!("unité : {:.3} px", cs.unit);
            logo_under = Some(l);
            d
        }
        "lockup" => {
            let mut p: LockupParams = a.params()?;
            if let Some(k) = a.opts.get("kind") {
                p.kind = pick(
                    k,
                    &[("horizontal", LockupKind::Horizontal), ("vertical", LockupKind::Vertical), ("condensed", LockupKind::Condensed), ("partner", LockupKind::Partner)],
                    "--kind",
                )?;
            }
            p.spacing = a.num("spacing")?.unwrap_or(p.spacing);
            p.invert |= a.flag("invert");
            let l = a.logo()?;
            let partner = match a.opts.get("partner") {
                Some(path) => {
                    let s = std::fs::read_to_string(path).map_err(|e| format!("{path} : {e}"))?;
                    Some(Logo::from_svg(&s).map_err(|e| format!("{path} : {e}"))?)
                }
                None => None,
            };
            let r = lockup(&l, partner.as_ref(), &p)?;
            eprintln!("unité : {:.3} px, espace : {:.3} px", r.unit, r.gap);
            r.drawing
        }
        "bento" => {
            let mut p: BentoParams = a.params()?;
            if let Some(s) = a.opts.get("style") {
                p.style = pick(s, &[("simple", BentoStyle::Simple), ("centered", BentoStyle::Centered), ("brick", BentoStyle::Brick), ("random", BentoStyle::Random)], "--style")?;
            }
            if let Some(n) = a.num("sections")? {
                p.sections = n as usize;
            }
            p.radius = a.num("radius")?.unwrap_or(p.radius);
            p.gutter = a.num("gutter")?.unwrap_or(p.gutter);
            if let Some(s) = a.num("seed")? {
                p.seed = s as u64;
            }
            p.numbers |= a.flag("numbers");
            bento(a.area((1080.0, 1080.0))?, &p)
        }
        other => return Err(format!("outil inconnu : {other} ({})", usage())),
    };
    let under = if a.flag("no-logo") { None } else { logo_under.as_ref() };
    let svg = drawing.to_svg(under, a.opts.get("background").map(String::as_str).or(Some("#ffffff")));
    if let Some(path) = a.opts.get("json") {
        let json = serde_json::to_string_pretty(&drawing).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("{path} : {e}"))?;
    }
    match a.opts.get("o") {
        Some(path) => std::fs::write(path, svg).map_err(|e| format!("{path} : {e}"))?,
        None => print!("{svg}"),
    }
    Ok(())
}
