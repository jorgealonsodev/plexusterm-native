//! Every ui-kit widget in one window, for visual checks against PRD §8.5.
//!
//! `cargo run -p ui-kit --example gallery -- [--scheme <id>] [--open dialog|dropdown|context|help]`
//!
//! The window title is `ui-kit gallery - <scheme id>`. "Switch scheme" toggles Catppuccin Mocha
//! and Catppuccin Latte at runtime.

use gpui::{
    App, Bounds, Context, Entity, Focusable, MouseButton, MouseDownEvent, SharedString, Subscription, TitlebarOptions,
    Window, WindowBounds, WindowDecorations, WindowOptions, div, point, prelude::*, px, relative, size,
};
use ui_kit::theme::{LINE_HEIGHT, TextSize, Theme};
use ui_kit::{
    Assets, Button, Checkbox, ContextMenu, Dialog, DialogEvent, DropdownMenu, HelpTip, MenuEntry, MenuEvent, MenuItem,
    Radio, TextInput,
};
use ui_theme::Rgb;

const MOCHA: &str = "catppuccin-mocha";
const LATTE: &str = "catppuccin-latte";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
    Nothing,
    Dialog,
    Dropdown,
    Context,
    Help,
}

struct Args {
    scheme: String,
    open: Open,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        scheme: MOCHA.to_owned(),
        open: Open::Nothing,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--scheme" => args.scheme = it.next().ok_or("--scheme needs an id")?,
            "--open" => {
                args.open = match it.next().as_deref() {
                    Some("dialog") => Open::Dialog,
                    Some("dropdown") => Open::Dropdown,
                    Some("context") => Open::Context,
                    Some("help") => Open::Help,
                    _ => return Err("--open takes dialog, dropdown, context or help".into()),
                }
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(args)
}

struct Gallery {
    scheme: String,
    name: Entity<TextInput>,
    host: Entity<TextInput>,
    search: Entity<TextInput>,
    remember: bool,
    compress: bool,
    protocol: usize,
    pressed: bool,
    dropdown: Entity<DropdownMenu>,
    context: Entity<ContextMenu>,
    help: Entity<HelpTip>,
    dialog: Option<Entity<Dialog>>,
    last_event: SharedString,
    _subs: Vec<Subscription>,
}

fn menu_entries() -> Vec<MenuEntry> {
    vec![
        MenuItem::new("Open in new tab").into(),
        MenuItem::new("Rename").into(),
        MenuItem::new("Duplicate (unavailable)").disabled(true).into(),
        MenuEntry::Separator,
        MenuItem::new("Red").swatch(Rgb::new(0xf3, 0x8b, 0xa8)).into(),
        MenuItem::new("Blue").swatch(Rgb::new(0x89, 0xb4, 0xfa)).into(),
        MenuEntry::Separator,
        MenuItem::new("Delete").danger().into(),
    ]
}

impl Gallery {
    fn new(scheme: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| TextInput::new("Session name", cx).with_placeholder("Session name"));
        let host = cx.new(|cx| TextInput::new("Host", cx).with_text("plexus@build-01"));
        let search = cx.new(|cx| TextInput::new("Search", cx).with_placeholder("Search…").search());
        let dropdown = cx.new(|cx| DropdownMenu::new("Actions", menu_entries(), window, cx));
        let context = cx.new(|cx| ContextMenu::new("Session", menu_entries(), window, cx));
        let help = cx.new(|cx| {
            HelpTip::new(
                "Sessions are saved on this computer. Passwords go to the system keyring, never to disk.",
                window,
                cx,
            )
        });
        let subs = vec![
            cx.subscribe_in(&dropdown, window, |this: &mut Self, _, event: &MenuEvent, _, cx| {
                this.show_event(format!("dropdown: {event:?}"), cx)
            }),
            cx.subscribe_in(&context, window, |this: &mut Self, _, event: &MenuEvent, _, cx| {
                this.show_event(format!("context menu: {event:?}"), cx)
            }),
        ];
        Self {
            scheme,
            name,
            host,
            search,
            remember: true,
            compress: false,
            protocol: 0,
            pressed: true,
            dropdown,
            context,
            help,
            dialog: None,
            last_event: "".into(),
            _subs: subs,
        }
    }

    fn show_event(&mut self, text: String, cx: &mut Context<Self>) {
        self.last_event = text.into();
        cx.notify();
    }

    fn open_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new("New name", cx).with_text("build-01"));
        let input_focus = input.read(cx).focus_handle(cx);
        let dialog = cx.new(|cx| {
            Dialog::new("Rename session", cx)
                .message("Choose the name shown in the sidebar and the tab.")
                .body(input, vec![input_focus])
                .button("Cancel", ui_kit::ButtonKind::Default, cx)
                .button("Rename", ui_kit::ButtonKind::Primary, cx)
        });
        let sub = cx.subscribe_in(&dialog, window, |this: &mut Self, _, event: &DialogEvent, _, cx| {
            this.last_event = format!("dialog: {event:?}").into();
            this.dialog = None;
            cx.notify();
        });
        self._subs.push(sub);
        dialog.update(cx, |d, cx| d.focus_initial(window, cx));
        self.dialog = Some(dialog);
        cx.notify();
    }

    fn switch_scheme(&mut self, cx: &mut Context<Self>) {
        self.scheme = if self.scheme == LATTE { MOCHA } else { LATTE }.to_owned();
        let family = Theme::of(cx).font_family.clone();
        Theme::for_scheme(&self.scheme, 13.0)
            .with_font_family(family)
            .with_reduced_motion(cx.reduce_motion())
            .set_global(cx);
    }
}

fn section(title: &'static str, theme: &Theme) -> gpui::Div {
    div().flex().flex_col().gap(theme.space(8.0)).child(
        div()
            .text_size(theme.text(TextSize::Small))
            .text_color(theme.colors.muted)
            .child(title),
    )
}

fn row(theme: &Theme) -> gpui::Div {
    div().flex().flex_wrap().items_center().gap(theme.space(8.0))
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let c = theme.colors;
        let buttons = row(&theme)
            .child(Button::new("default", "Default"))
            .child(Button::new("primary", "Primary").primary())
            .child(Button::new("danger", "Danger").danger())
            .child(Button::new("small", "Small").small())
            .child(
                Button::new("pressed", "Pressed")
                    .pressed(self.pressed)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.pressed = !this.pressed;
                        cx.notify();
                    })),
            )
            .child(Button::new("recording", "● Recording").recording(true))
            .child(Button::new("disabled", "Disabled").disabled(true))
            .child(
                Button::new("disabled-primary", "Disabled primary")
                    .primary()
                    .disabled(true),
            );
        let inputs = row(&theme)
            .child(div().w(px(220.0)).child(self.name.clone()))
            .child(div().w(px(220.0)).child(self.host.clone()))
            .child(div().w(px(180.0)).child(self.search.clone()));
        let protocols = ["SSH", "Serial", "Telnet"];
        let toggles = row(&theme)
            .gap(theme.space(16.0))
            .child(
                Checkbox::new("remember", "Remember password")
                    .checked(self.remember)
                    .on_toggle(cx.listener(|this, checked: &bool, _, cx| {
                        this.remember = *checked;
                        cx.notify();
                    })),
            )
            .child(
                Checkbox::new("compress", "Compression")
                    .checked(self.compress)
                    .on_toggle(cx.listener(|this, checked: &bool, _, cx| {
                        this.compress = *checked;
                        cx.notify();
                    })),
            )
            .child(Checkbox::new("locked", "Locked option").checked(true).disabled(true))
            .children(protocols.iter().enumerate().map(|(i, label)| {
                Radio::new(("protocol", i), *label)
                    .checked(self.protocol == i)
                    .on_toggle(cx.listener(move |this, _: &bool, _, cx| {
                        this.protocol = i;
                        cx.notify();
                    }))
            }));
        let menus = row(&theme)
            .gap(theme.space(16.0))
            .child(self.dropdown.clone())
            .child(
                div()
                    .id("context-area")
                    .px(theme.space(12.0))
                    .py(theme.space(6.0))
                    .border(px(1.0))
                    .border_color(c.border)
                    .rounded(px(6.0))
                    .text_color(c.muted)
                    .child("Right-click here")
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.context
                                .update(cx, |m, cx| m.deploy(event.position, false, window, cx));
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(theme.space(6.0))
                    .child("Help tip")
                    .child(self.help.clone()),
            );
        let dialogs = row(&theme)
            .child(
                Button::new("open-dialog", "Open dialog…").on_click(cx.listener(|this, _, window, cx| {
                    this.open_dialog(window, cx);
                })),
            )
            .child(
                Button::new("switch-scheme", "Switch scheme")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| this.switch_scheme(cx))),
            )
            .child(div().text_color(c.muted).child(self.last_event.clone()));
        div()
            .id("gallery")
            .size_full()
            .flex()
            .flex_col()
            .gap(theme.space(18.0))
            .p(theme.space(20.0))
            .bg(c.bg)
            .text_color(c.text)
            .font_family(theme.font_family.clone())
            .text_size(theme.text(TextSize::Base))
            .line_height(relative(LINE_HEIGHT))
            .child(
                div()
                    .text_size(theme.text(TextSize::Title))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(format!("ui-kit gallery · {}", self.scheme)),
            )
            .child(section("Buttons", &theme).child(buttons))
            .child(section("Inputs", &theme).child(inputs))
            .child(section("Checkboxes and radios", &theme).child(toggles))
            .child(section("Menus and help", &theme).child(menus))
            .child(section("Dialog", &theme).child(dialogs))
            .child(self.context.clone())
            .children(self.dialog.clone())
    }
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("gallery: {error}");
            std::process::exit(2);
        }
    };
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            Theme::for_scheme(&args.scheme, 13.0)
                .with_installed_ui_font(cx)
                .with_reduced_motion(cx.reduce_motion())
                .set_global(cx);
            let bounds = Bounds::centered(None, size(px(980.0), px(560.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(format!("ui-kit gallery - {}", args.scheme).into()),
                    ..TitlebarOptions::default()
                }),
                window_decorations: Some(WindowDecorations::Server),
                app_id: Some("ui-kit-gallery".to_owned()),
                ..WindowOptions::default()
            };
            let (scheme, open) = (args.scheme.clone(), args.open);
            let opened = cx.open_window(options, |window, cx| {
                let gallery = cx.new(|cx| Gallery::new(scheme, window, cx));
                // Popups that anchor to a widget need one painted frame first.
                let handle = gallery.clone();
                window.on_next_frame(move |window, cx| {
                    handle.update(cx, |g, cx| match open {
                        Open::Nothing => {}
                        Open::Dialog => g.open_dialog(window, cx),
                        Open::Dropdown => g.dropdown.update(cx, |d, cx| d.open(true, window, cx)),
                        Open::Context => g
                            .context
                            .update(cx, |m, cx| m.deploy(point(px(420.0), px(330.0)), true, window, cx)),
                        Open::Help => {
                            let focus = g.help.focus_handle(cx);
                            window.focus(&focus, cx);
                        }
                    });
                });
                gallery
            });
            if let Err(error) = opened {
                eprintln!("gallery: could not open the window: {error:#}");
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
