//! Ported from PlexusTerm v0.2.15 `uiTheme.test.ts` and `themes.test.ts` (PRD §8.1, Appendix C).

use ui_theme::{DEFAULT_SCHEME_ID, Rgb, mix, readable_on, scheme_by_id, schemes, ui_tokens};

#[allow(clippy::expect_used, reason = "test helper over literal hex strings")]
fn hex(value: &str) -> Rgb {
    Rgb::parse(value).expect("valid hex")
}

#[test]
fn ships_57_schemes_with_unique_ids_and_the_requested_names() {
    let all = schemes();
    assert_eq!(all.len(), 57);
    let mut ids: Vec<_> = all.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 57);
    let names: Vec<_> = all.iter().map(|s| s.name).collect();
    for name in [
        "PlexusTerm",
        "Catppuccin Mocha",
        "Catppuccin Frappé",
        "Rosé Pine Dawn",
        "Laser",
        "3024 Day",
    ] {
        assert!(names.contains(&name), "{name} missing");
    }
    assert_eq!(all[0].id, "plexusterm", "PlexusTerm is listed first");
}

#[test]
fn counts_43_dark_and_14_light_schemes() {
    let light = schemes().iter().filter(|s| !s.dark).count();
    assert_eq!(light, 14);
}

#[test]
fn marks_light_and_dark_themes_by_their_background() {
    assert!(!scheme_by_id("catppuccin-latte").dark);
    assert!(!scheme_by_id("solarized-light").dark);
    assert!(scheme_by_id("catppuccin-mocha").dark);
    assert!(scheme_by_id("plexusterm").dark);
}

#[test]
fn uses_catppuccin_mocha_by_default_and_for_unknown_ids() {
    assert_eq!(DEFAULT_SCHEME_ID, "catppuccin-mocha");
    assert_eq!(scheme_by_id("no-such-theme").id, "catppuccin-mocha");
    assert_eq!(scheme_by_id("catppuccin-mocha").palette.background, hex("#1e1e2e"));
}

#[test]
fn keeps_the_selection_alpha_of_imported_schemes() {
    let mocha = scheme_by_id("catppuccin-mocha").palette.selection;
    assert_eq!((mocha.r, mocha.g, mocha.b, mocha.a), (0xf5, 0xe0, 0xdc, 0x80));
    let own = scheme_by_id("plexusterm").palette.selection;
    assert_eq!(own.a, 0xff, "a #rrggbb selection is opaque");
}

#[test]
fn blends_two_colours_by_a_weight_and_ignores_alpha() {
    assert_eq!(mix(hex("#000000"), hex("#ffffff"), 0.0), hex("#000000"));
    assert_eq!(mix(hex("#000000"), hex("#ffffff"), 1.0), hex("#ffffff"));
    assert_eq!(mix(hex("#000000"), hex("#ffffff80"), 0.5), hex("#808080"));
}

#[test]
fn parse_rejects_malformed_hex() {
    for bad in ["", "#12345", "123456", "#gg0000", "#1234567"] {
        assert!(Rgb::parse(bad).is_none(), "{bad:?} accepted");
    }
}

#[test]
fn builds_the_interface_colours_from_the_scheme() {
    let nord = scheme_by_id("nord");
    let t = ui_tokens(nord);
    assert_eq!(t.bg, hex("#2e3440"));
    assert_eq!(t.text, nord.palette.foreground);
    assert_eq!(t.accent, nord.palette.blue());
    assert_eq!(t.danger, nord.palette.red());
    assert_eq!(t.warning, nord.palette.yellow());
    assert_eq!(t.success, nord.palette.green());
    assert!(t.dark);
}

#[test]
fn layers_panels_and_borders_between_the_background_and_the_text() {
    let t = ui_tokens(scheme_by_id("catppuccin-mocha"));
    let (bg, fg) = (hex("#1e1e2e"), hex("#cdd6f4"));
    assert_eq!(t.surface, mix(bg, fg, 0.04));
    assert_eq!(t.surface_2, mix(bg, fg, 0.08));
    assert_eq!(t.border, mix(bg, fg, 0.16));
    assert_eq!(t.muted, mix(bg, fg, 0.6));
    assert_eq!(t.surface, hex("#252536"), "golden value from the v0.2.15 formula");
}

#[test]
fn picks_dark_or_light_text_for_buttons_by_the_accents_brightness() {
    assert_eq!(ui_tokens(scheme_by_id("catppuccin-mocha")).on_accent, hex("#000000"));
    assert_eq!(ui_tokens(scheme_by_id("github-light")).on_accent, hex("#ffffff"));
    assert_eq!(ui_tokens(scheme_by_id("catppuccin-mocha")).on_warning, hex("#000000"));
    assert_eq!(
        readable_on(hex("#676767")),
        hex("#000000"),
        "just above the 0.4 threshold"
    );
    assert_eq!(
        readable_on(hex("#656565")),
        hex("#ffffff"),
        "just below the 0.4 threshold"
    );
}

#[test]
fn keeps_the_original_look_for_the_plexusterm_scheme() {
    let t = ui_tokens(scheme_by_id("plexusterm"));
    assert_eq!(t.bg, hex("#0f1419"));
    assert_eq!(t.accent, hex("#5ec4ff"));
}

#[test]
fn colours_git_statuses_with_vs_codes_defaults_for_dark_and_light_schemes() {
    let dark = ui_tokens(scheme_by_id("nord")).git;
    assert_eq!(
        [dark.modified, dark.added, dark.deleted, dark.renamed, dark.untracked],
        ["#e2c08d", "#81b88b", "#c74e39", "#73c991", "#73c991"].map(hex)
    );
    assert_eq!(
        [dark.ignored, dark.stage_modified, dark.stage_deleted, dark.conflicting],
        ["#8c8c8c", "#e2c08d", "#c74e39", "#e4676b"].map(hex)
    );
    let light = ui_tokens(scheme_by_id("solarized-light")).git;
    assert_eq!(
        [
            light.modified,
            light.added,
            light.deleted,
            light.renamed,
            light.untracked
        ],
        ["#895503", "#587c0c", "#ad0707", "#007100", "#007100"].map(hex)
    );
    assert_eq!(
        [
            light.ignored,
            light.stage_modified,
            light.stage_deleted,
            light.conflicting
        ],
        ["#8e8e90", "#895503", "#ad0707", "#ad0707"].map(hex)
    );
}

#[test]
fn colours_code_in_the_file_viewer_from_the_ansi_palette() {
    let nord = scheme_by_id("nord");
    let p = &nord.palette;
    let s = ui_tokens(nord).syntax;
    assert_eq!(s.keyword, p.magenta());
    assert_eq!(s.string, p.green());
    assert_eq!(s.number, p.yellow());
    assert_eq!(s.function, p.blue());
    assert_eq!(s.type_, p.cyan());
    assert_eq!(s.tag, p.red());
    assert_eq!(s.property, p.bright_blue());
    assert_eq!(s.comment, mix(p.background, p.foreground, 0.6));
}
