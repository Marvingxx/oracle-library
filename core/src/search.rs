//! Search: every word of the query must match, in any language, with or without accents.
//!
//! A query word matches an asset when one of the asset's words (name, tags, folders, the user's
//! own tags) starts with it — "met" finds "metal" — or equals one of its French/English
//! synonyms — "bois" finds "wood". Name matches rank first, then tags, then the rest.

use std::collections::{BTreeMap, BTreeSet};

use crate::fold::{synonyms, words};
use crate::index::{Asset, FormatGroup, Library};
use crate::user::UserData;

/// What to show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    /// Only this top-level section (`Asset::kind`).
    pub kind: Option<String>,
    /// Only these format groups (empty: all).
    pub groups: BTreeSet<FormatGroup>,
    pub favorites_only: bool,
    /// Only assets carrying this tag (folded).
    pub tag: Option<String>,
}

/// One query word and the words that count as it.
struct Term {
    word: String,
    exact: Vec<String>,
}

fn terms(text: &str) -> Vec<Term> {
    words(text).into_iter().map(|w| Term { exact: synonyms(&w).into_iter().skip(1).collect(), word: w }).collect()
}

/// The kind of file a query word asks for ("police" wants fonts), which ranks those first.
fn group_hint(term: &Term) -> Option<FormatGroup> {
    let all = std::iter::once(&term.word).chain(term.exact.iter());
    for w in all {
        let g = match w.as_str() {
            "font" | "police" | "typo" | "typographie" | "typeface" => FormatGroup::Font,
            "vector" | "vecteur" | "vectoriel" | "svg" => FormatGroup::Vector,
            "brush" | "pinceau" | "abr" | "psd" => FormatGroup::Photoshop,
            "photo" | "image" | "jpg" | "png" => FormatGroup::Image,
            _ => continue,
        };
        return Some(g);
    }
    None
}

/// Whether `term` matches one of `candidates`.
fn hit(term: &Term, candidates: &[String]) -> bool {
    candidates.iter().any(|c| c.starts_with(&term.word) || term.exact.iter().any(|e| e == c))
}

/// The ids of the assets that match, best first.
pub fn search(library: &Library, user: &UserData, q: &Query) -> Vec<usize> {
    let terms = terms(&q.text);
    let tag = q.tag.as_deref().map(crate::fold::fold);
    let mut scored: Vec<(u32, usize)> = Vec::new();
    for (i, a) in library.assets.iter().enumerate() {
        if let Some(k) = &q.kind
            && &a.kind != k
        {
            continue;
        }
        if !q.groups.is_empty() && !q.groups.contains(&a.group) {
            continue;
        }
        if q.favorites_only && !user.is_favorite(&a.id) {
            continue;
        }
        let own = user.tags_of(&a.id);
        if let Some(t) = &tag
            && !own.iter().any(|o| crate::fold::fold(o) == *t)
            && !a.tags.iter().any(|o| o == t)
        {
            continue;
        }
        match score(a, &own, &terms) {
            Some(s) => scored.push((s, i)),
            None => continue,
        }
    }
    // Stable: equal scores keep folder order.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.into_iter().map(|(_, i)| i).collect()
}

fn score(a: &Asset, own_tags: &[String], terms: &[Term]) -> Option<u32> {
    if terms.is_empty() {
        return Some(0);
    }
    let own: Vec<String> = own_tags.iter().flat_map(|t| words(t)).collect();
    let mut total = 0;
    for t in terms {
        let s = if hit(t, &a.name_words) {
            3
        } else if hit(t, &own) {
            3
        } else if hit(t, &a.tags) {
            2
        } else if hit(t, &words(&a.format)) {
            1
        } else {
            return None;
        };
        total += s;
        if group_hint(t) == Some(a.group) {
            total += 4;
        }
    }
    Some(total)
}

/// How many assets each section holds after the other filters, for the sidebar counts.
pub fn counts_by_kind(library: &Library, indices: &[usize]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for &i in indices {
        if let Some(a) = library.assets.get(i) {
            *out.entry(a.kind.clone()).or_insert(0) += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::Library;

    fn lib() -> Library {
        let dir = std::env::temp_dir().join(format!("oracle-library-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for f in [
            "01_Textures photo/Metal/Metal_1.jpg",
            "01_Textures photo/Bois/Wood_texture (1).jpg",
            "01_Textures photo/Papiers/Papier froisse/crumple_03.png",
            "06_Typographies/Montserrat/Montserrat-Bold.ttf",
            "07_Icônes/arrow-left.svg",
        ] {
            let p = crate::index::resolve(&dir, f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        }
        Library::open(&dir).unwrap()
    }

    fn names(l: &Library, q: &str) -> Vec<String> {
        let user = UserData::default();
        search(l, &user, &Query { text: q.into(), ..Default::default() }).iter().map(|&i| l.assets[i].name.clone()).collect()
    }

    #[test]
    fn metal_with_or_without_accent() {
        let l = lib();
        assert_eq!(names(&l, "métal"), ["Metal_1"]);
        assert_eq!(names(&l, "metal"), ["Metal_1"]);
        assert_eq!(names(&l, "METAL"), ["Metal_1"]);
        assert_eq!(names(&l, "met"), ["Metal_1"]);
    }

    #[test]
    fn french_finds_english_and_back() {
        let l = lib();
        assert_eq!(names(&l, "bois"), ["Wood_texture (1)"]);
        assert_eq!(names(&l, "wood"), ["Wood_texture (1)"]);
        assert_eq!(names(&l, "paper crumpled"), ["crumple_03"]);
        assert_eq!(names(&l, "papier froissé"), ["crumple_03"]);
        assert_eq!(names(&l, "police"), ["Montserrat-Bold"]);
        assert_eq!(names(&l, "icone fleche"), ["arrow-left"]);
    }

    #[test]
    fn every_word_must_match_and_filters_apply() {
        let l = lib();
        assert!(names(&l, "metal bois").is_empty());
        let user = UserData::default();
        let q = Query { groups: [FormatGroup::Font].into(), ..Default::default() };
        assert_eq!(search(&l, &user, &q).len(), 1);
        let q = Query { kind: Some("Textures photo".into()), ..Default::default() };
        assert_eq!(search(&l, &user, &q).len(), 3);
    }

    #[test]
    fn the_users_own_tags_are_searched() {
        let l = lib();
        let mut user = UserData::default();
        let id = l.assets.iter().find(|a| a.name == "Metal_1").unwrap().id.clone();
        user.add_tag(&id, "Campagne été");
        let found: Vec<_> = search(&l, &user, &Query { text: "campagne ete".into(), ..Default::default() }).into_iter().map(|i| l.assets[i].name.clone()).collect();
        assert_eq!(found, ["Metal_1"]);
    }
}
