//! Text folding for search: "Métal", "metal" and "METAL" all become "metal".

use unicode_normalization::UnicodeNormalization;

/// Lower case, accents removed, ligatures expanded, and every run of non-alphanumeric
/// characters turned into one space. `"Papier froissé (1).jpg"` → `"papier froisse 1 jpg"`.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = true;
    for c in text.nfd() {
        if unicode_normalization::char::is_combining_mark(c) {
            continue;
        }
        let expanded: &str = match c {
            'œ' | 'Œ' => "oe",
            'æ' | 'Æ' => "ae",
            'ß' => "ss",
            _ => "",
        };
        if !expanded.is_empty() {
            out.push_str(expanded);
            space = false;
            continue;
        }
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// The words of `text`, folded: `"Bois_texture-02"` → `["bois", "texture", "02"]`.
pub fn words(text: &str) -> Vec<String> {
    fold(&text.replace('_', " ")).split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect()
}

/// French and English words that mean the same thing, folded. A query word matches any word of
/// its group, so "bois" finds wood textures and "paper" finds "Papiers".
const SYNONYMS: &[&[&str]] = &[
    &["metal", "metallique", "metallic"],
    &["acier", "steel"],
    &["cuivre", "copper"],
    &["or", "gold", "dore", "golden"],
    &["argent", "silver"],
    &["rouille", "rust"],
    &["bois", "wood", "wooden"],
    &["papier", "papiers", "paper"],
    &["carton", "cardboard"],
    &["froisse", "crumpled", "wrinkled"],
    &["pierre", "stone", "rock", "roche"],
    &["beton", "concrete"],
    &["mur", "murs", "wall", "walls"],
    &["marbre", "marble"],
    &["brique", "briques", "brick", "bricks"],
    &["tissu", "tissus", "fabric", "textile", "cloth"],
    &["cuir", "leather"],
    &["verre", "glass"],
    &["grain", "grains", "grainy"],
    &["bruit", "noise"],
    &["rayure", "rayures", "scratch", "scratches"],
    &["poussiere", "dust"],
    &["tache", "taches", "stain", "stains", "splat", "splatter"],
    &["eclaboussure", "eclaboussures", "splash"],
    &["encre", "ink"],
    &["peinture", "paint", "painting"],
    &["aquarelle", "watercolor", "watercolour"],
    &["pinceau", "pinceaux", "brush", "brushes"],
    &["crayon", "pencil"],
    &["fumee", "smoke"],
    &["feu", "fire", "flamme", "flame", "flames"],
    &["eau", "water"],
    &["ciel", "sky"],
    &["nuage", "nuages", "cloud", "clouds"],
    &["lumiere", "light", "lights"],
    &["ombre", "ombres", "shadow", "shadows"],
    &["lueur", "glow"],
    &["flou", "blur"],
    &["fuite", "leak", "leaks"],
    &["particule", "particules", "particle", "particles"],
    &["etincelle", "etincelles", "spark", "sparks"],
    &["neige", "snow"],
    &["pluie", "rain"],
    &["etoile", "etoiles", "star", "stars"],
    &["coeur", "heart"],
    &["fleur", "fleurs", "flower", "flowers"],
    &["feuille", "feuilles", "leaf", "leaves"],
    &["arbre", "arbres", "tree", "trees"],
    &["fond", "fonds", "background", "backgrounds"],
    &["motif", "motifs", "pattern", "patterns"],
    &["degrade", "degrades", "gradient", "gradients"],
    &["police", "polices", "font", "fonts", "typo", "typographie", "typographies", "typeface"],
    &["icone", "icones", "icon", "icons"],
    &["maquette", "maquettes", "mockup", "mockups"],
    &["affiche", "affiches", "poster", "posters"],
    &["publication", "post", "posts"],
    &["modele", "modeles", "template", "templates"],
    &["tampon", "tampons", "stamp", "stamps"],
    &["cadre", "cadres", "frame", "frames"],
    &["bordure", "bordures", "border", "borders"],
    &["forme", "formes", "shape", "shapes"],
    &["ligne", "lignes", "line", "lines"],
    &["cercle", "cercles", "circle", "circles"],
    &["fleche", "fleches", "arrow", "arrows"],
    &["texte", "text"],
    &["vieux", "ancien", "old", "vintage", "retro"],
    &["sale", "dirty", "grunge"],
    &["noir", "black"],
    &["blanc", "white"],
    &["rouge", "red"],
    &["bleu", "blue"],
    &["vert", "green"],
    &["jaune", "yellow"],
];

/// Every word that counts as `word` (itself first), folded.
pub fn synonyms(word: &str) -> Vec<String> {
    let w = fold(word);
    let mut out = vec![w.clone()];
    for group in SYNONYMS {
        if group.contains(&w.as_str()) {
            out.extend(group.iter().filter(|g| **g != w).map(|g| (*g).to_string()));
        }
    }
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_accents_case_and_punctuation() {
        assert_eq!(fold("Métal"), "metal");
        assert_eq!(fold("MÉTAL"), "metal");
        assert_eq!(fold("Papier froissé (1).jpg"), "papier froisse 1 jpg");
        assert_eq!(fold("Cœur  –  Éclaboussure"), "coeur eclaboussure");
        assert_eq!(fold("  "), "");
    }

    #[test]
    fn words_split_underscores_and_dashes() {
        assert_eq!(words("Wood_texture-02"), ["wood", "texture", "02"]);
    }

    #[test]
    fn synonyms_cross_languages() {
        assert!(synonyms("Métal").contains(&"metallic".to_string()));
        assert!(synonyms("wood").contains(&"bois".to_string()));
        assert_eq!(synonyms("zzz"), ["zzz"]);
    }
}
