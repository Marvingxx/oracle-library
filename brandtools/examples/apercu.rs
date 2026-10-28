//! Renders an SVG to PNG, to look at what the tools produce: `cargo run --example apercu -- in.svg out.png [largeur]`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output, rest @ ..] = args.as_slice() else {
        eprintln!("apercu ENTRÉE.svg SORTIE.png [LARGEUR]");
        std::process::exit(2);
    };
    let width: f32 = rest.first().and_then(|w| w.parse().ok()).unwrap_or(1600.0);
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let src = std::fs::read_to_string(input).expect("lecture");
    let tree = resvg::usvg::Tree::from_str(&src, &opt).expect("SVG");
    let k = width / tree.size().width();
    let mut pix = resvg::tiny_skia::Pixmap::new(width as u32, (tree.size().height() * k).ceil() as u32).expect("taille");
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(k, k), &mut pix.as_mut());
    pix.save_png(output).expect("écriture");
}
