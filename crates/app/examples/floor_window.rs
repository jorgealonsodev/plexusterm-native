//! Floor reference for NFR-3 and NFR-4 (`docs/benchmarks/m0.md`, T6b): the same GPUI rev and
//! window options as the app, with one filled `div` and no terminal, PTY, fonts or text.
//!
//! `cargo build -p plexusterm --release --example floor_window`. With
//! `PLEXUS_EXIT_AFTER_FIRST_PROMPT=1` it prints the same `main at` and `phase` lines as a `bench`
//! build of the app and quits once its first frame has been submitted; otherwise it stays open
//! (for RSS readings) until the window is closed.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use gpui::{
    App, Bounds, Context, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions, canvas, div,
    prelude::*, px, rgb, size,
};

struct Floor {
    exit_after_frame: Option<Instant>,
}

impl Render for Floor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let started = self.exit_after_frame;
        let paint = move |_bounds: Bounds<gpui::Pixels>, (): (), window: &mut Window, _cx: &mut App| {
            if let Some(started) = started {
                eprintln!("plexusterm: phase first_paint {} µs", started.elapsed().as_micros());
                // This first paint happens in `open_window`, which does not present; the first
                // frame callback presents it, and the task runs once that callback has returned.
                window.on_next_frame(move |_, cx| {
                    cx.spawn(async move |cx| {
                        let micros = started.elapsed().as_micros();
                        eprintln!("plexusterm: phase first_frame_submitted {micros} µs");
                        eprintln!("plexusterm: first prompt frame submitted {micros} µs after start");
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                });
            }
        };
        div()
            .size_full()
            .bg(rgb(0x001e_1e2e))
            .child(canvas(|_, _, _| {}, paint).size_full())
    }
}

fn main() {
    let started = Instant::now();
    let measure = std::env::var_os("PLEXUS_EXIT_AFTER_FIRST_PROMPT").as_deref() == Some(std::ffi::OsStr::new("1"));
    let phase = move |name: &str| {
        if measure {
            eprintln!("plexusterm: phase {name} {} µs", started.elapsed().as_micros());
        }
    };
    if measure {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0);
        eprintln!("plexusterm: main at {epoch}");
    }
    let application = gpui_platform::application();
    phase("app_created");
    application.run(move |cx: &mut App| {
        phase("run_callback");
        let bounds = Bounds::centered(None, size(px(1200.0), px(760.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("PlexusTerm floor".into()),
                ..TitlebarOptions::default()
            }),
            window_decorations: Some(WindowDecorations::Server),
            app_id: Some("plexusterm-floor".to_owned()),
            ..WindowOptions::default()
        };
        let exit_after_frame = measure.then_some(started);
        let opened = cx.open_window(options, |_, cx| cx.new(|_| Floor { exit_after_frame }));
        phase("window_opened");
        if let Err(error) = opened {
            eprintln!("floor_window: could not open the window: {error:#}");
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
