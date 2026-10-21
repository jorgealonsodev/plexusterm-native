//! Ported from PlexusTerm v0.2.15 `src/features/settings/fontCatalog.test.ts` (PRD FONT-1, SET-3a).
//! v0.2.15 found installed fonts by measuring canvas text; natively the caller enumerates the
//! system's families and `installed_fonts` picks the candidates among them.

use settings_model::fonts::{BUNDLED_FONTS, INSTALLED_CANDIDATES, catalog_choice, installed_fonts};
use settings_model::settings::DEFAULT_FONT_FAMILY;

/// True when `family` is `"<name> Variable", monospace` or `"<name> Web", monospace`.
fn is_bundled_family_list(family: &str) -> bool {
    let Some(quoted) = family
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix("\", monospace"))
    else {
        return false;
    };
    !quoted.contains('"') && (quoted.ends_with(" Variable") || quoted.ends_with(" Web")) && quoted.len() > 4
}

// describe("BUNDLED_FONTS")
// it("lists the fonts shipped with the app, each with a CSS family list that ends in monospace")
#[test]
fn lists_the_fonts_shipped_with_the_app_each_with_a_family_list_that_ends_in_monospace() {
    let names: Vec<&str> = BUNDLED_FONTS.iter().map(|f| f.name).collect();
    assert_eq!(
        names,
        [
            "JetBrains Mono",
            "Fira Code",
            "Cascadia Code",
            "Source Code Pro",
            "Hack"
        ]
    );
    for font in BUNDLED_FONTS {
        assert!(is_bundled_family_list(font.family), "{}", font.family);
    }
    // Bundled names never take over a font of the same name installed on the computer.
    for font in BUNDLED_FONTS {
        assert!(!INSTALLED_CANDIDATES.contains(&font.name));
    }
}

// describe("detectInstalledFonts")
// it("finds the candidates whose text is drawn differently from every fallback")
#[test]
fn finds_the_installed_candidates_in_catalogue_order() {
    let system = ["Ubuntu Mono".to_owned(), "Arial".to_owned(), "Consolas".to_owned()];
    assert_eq!(installed_fonts(&system), ["Consolas", "Ubuntu Mono"]);
}

// it("finds none when nothing can be measured")
#[test]
fn finds_none_when_nothing_can_be_enumerated() {
    let none: [String; 0] = [];
    assert!(installed_fonts(&none).is_empty());
}

// it("only tries known monospaced families, each once")
#[test]
fn only_tries_known_monospaced_families_each_once() {
    let mut unique = INSTALLED_CANDIDATES.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), INSTALLED_CANDIDATES.len());
    assert!(installed_fonts(&["Comic Sans MS".to_owned()]).is_empty());
    // A family the system lists twice is offered once.
    assert_eq!(
        installed_fonts(&["Consolas".to_owned(), "Consolas".to_owned()]),
        ["Consolas"]
    );
}

// describe("catalogChoice") it("names the catalogue entry a font family setting matches")
#[test]
fn names_the_catalogue_entry_a_font_family_setting_matches() {
    let installed = ["Consolas".to_owned()];
    assert_eq!(catalog_choice(DEFAULT_FONT_FAMILY, &installed), DEFAULT_FONT_FAMILY);
    assert_eq!(
        catalog_choice("\"Fira Code Variable\", monospace", &installed),
        "\"Fira Code Variable\", monospace"
    );
    assert_eq!(catalog_choice(" Consolas ", &installed), "Consolas");
}

// it("is empty for anything typed by hand")
#[test]
fn is_empty_for_anything_typed_by_hand() {
    let installed = ["Consolas".to_owned()];
    assert_eq!(catalog_choice("Hack Nerd Font", &installed), "");
    assert_eq!(catalog_choice("\"Consolas\", serif", &installed), "");
}
