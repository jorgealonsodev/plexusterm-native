//! PlexusTerm Native (M0 spike): one window with one terminal pane on a local shell.
//!
//! No Tokio bridge yet: the local PTY runs on plain threads (proto-pty, term-engine), so the
//! `gpui_tokio` pattern of PRD §6.1 is added with the first network protocol.

mod colors;
mod grid;
mod keys;
mod terminal;

use gpui::{App, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, prelude::*, px, size};
use ui_theme::{DEFAULT_SCHEME_ID, scheme_by_id, ui_tokens};

use crate::terminal::{LocalSession, TerminalView};

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let scheme = scheme_by_id(DEFAULT_SCHEME_ID);
        // The pane is painted on the interface `bg` token (TERM-2), which is the scheme background.
        let palette = scheme.palette;
        debug_assert_eq!(ui_tokens(scheme).bg, palette.background);
        let bounds = Bounds::centered(None, size(px(1200.0), px(760.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("PlexusTerm".into()),
                ..TitlebarOptions::default()
            }),
            window_decorations: Some(WindowDecorations::Server),
            app_id: Some("plexusterm".to_owned()),
            ..WindowOptions::default()
        };
        let local = match LocalSession::start() {
            Ok(local) => local,
            Err(error) => {
                // A local pane that cannot start closes like one whose shell ended (ST-8); it is
                // the only pane, so the app ends.
                eprintln!("plexusterm: could not start the terminal: {}", error.0);
                cx.quit();
                return;
            }
        };
        let opened = cx.open_window(options, |window, cx| {
            cx.new(|cx| TerminalView::new(local, palette, window, cx))
        });
        if let Err(error) = opened {
            eprintln!("plexusterm: could not open the window: {error:#}");
            cx.quit();
            return;
        }
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
