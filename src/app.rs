use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    label::Label,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    v_flex, ActiveTheme, Icon, IconName, IndexPath, Root, Sizable as _, Theme, ThemeMode,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use pifile::fs::{
    copy_file, delete_permanent, home_dir, initial_cwd, list_dir, mkdir, move_path, parent_of,
    places, rename, trash_path, DirEntry, FileKind, Place, SortKey,
};
use pifile::theme::{detect_system_dark, omarchy_watch_paths, OmarchyPalette};

actions!(
    pifile_actions,
    [
        GoParent,
        OpenSelected,
        OpenWithSystem,
        NewFolder,
        RenameSelected,
        TrashSelected,
        DeleteSelected,
        CopySelected,
        CutSelected,
        PasteClipboard,
        ToggleHidden,
        Refresh,
        SelectHome,
        ConfirmPrompt,
        CancelPrompt,
        Quit,
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", GoParent, None),
        KeyBinding::new("alt-up", GoParent, None),
        KeyBinding::new("enter", OpenSelected, None),
        KeyBinding::new("ctrl-enter", OpenWithSystem, None),
        KeyBinding::new("ctrl-shift-n", NewFolder, None),
        KeyBinding::new("f2", RenameSelected, None),
        KeyBinding::new("delete", TrashSelected, None),
        KeyBinding::new("shift-delete", DeleteSelected, None),
        KeyBinding::new("ctrl-c", CopySelected, None),
        KeyBinding::new("ctrl-x", CutSelected, None),
        KeyBinding::new("ctrl-v", PasteClipboard, None),
        KeyBinding::new("ctrl-h", ToggleHidden, None),
        KeyBinding::new("f5", Refresh, None),
        KeyBinding::new("alt-home", SelectHome, None),
        KeyBinding::new("escape", CancelPrompt, None),
        KeyBinding::new("ctrl-q", Quit, None),
    ]);
}

pub fn open_window(path: Option<PathBuf>, cx: &AsyncApp) -> anyhow::Result<WindowHandle<Root>> {
    cx.open_window(window_options(), move |window, cx| {
        let view = cx.new(|cx| Pifile::new(path.clone(), window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    })
    .map_err(|e| anyhow::anyhow!("{e}"))
}

fn window_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(80.), px(60.)),
            size: size(px(1024.), px(680.)),
        })),
        window_min_size: Some(size(px(640.), px(400.))),
        titlebar: Some(TitlebarOptions {
            title: Some("Pifile".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        app_id: Some("pifile".into()),
        ..Default::default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    NewFolder,
    Rename,
}

#[derive(Clone)]
enum ClipboardOp {
    Copy(PathBuf),
    Cut(PathBuf),
}

/// A notify watcher on the Omarchy theme paths, drained on each frame so a
/// theme switch re-tints the file browser live.
struct ThemeWatch {
    events: Arc<Mutex<Vec<PathBuf>>>,
    _watcher: Option<RecommendedWatcher>,
}

impl ThemeWatch {
    fn new() -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        let tx = events.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Create(_)
                    ) {
                        if let Ok(mut queue) = tx.lock() {
                            queue.extend(event.paths);
                        }
                    }
                }
            },
            notify::Config::default(),
        )
        .ok();
        if let Some(watcher) = watcher.as_mut() {
            for path in omarchy_watch_paths() {
                if path.exists() {
                    let _ = watcher.watch(&path, RecursiveMode::NonRecursive);
                }
            }
        }
        Self {
            events,
            _watcher: watcher,
        }
    }

    fn drain(&self) -> Vec<PathBuf> {
        self.events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
}

pub struct DirDelegate {
    entries: Vec<DirEntry>,
    selected: Option<IndexPath>,
}

impl DirDelegate {
    fn get(&self, ix: IndexPath) -> Option<&DirEntry> {
        self.entries.get(ix.row)
    }
}

pub struct Pifile {
    palette: OmarchyPalette,
    places: Vec<Place>,
    cwd: PathBuf,
    show_hidden: bool,
    sort: SortKey,
    selected: Option<IndexPath>,
    list: Entity<ListState<DirDelegate>>,
    status: SharedString,
    clipboard: Option<ClipboardOp>,
    prompt: Option<PromptKind>,
    prompt_input: Entity<InputState>,
    theme_watch: ThemeWatch,
    last_theme_check: Instant,
    _subs: Vec<Subscription>,
}

impl Pifile {
    pub fn new(path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let palette = OmarchyPalette::load(detect_system_dark());
        let cwd = path
            .as_ref()
            .filter(|p| p.is_dir())
            .cloned()
            .or_else(|| {
                path.as_ref()
                    .and_then(|p| p.parent().map(PathBuf::from))
                    .filter(|p| p.is_dir())
            })
            .unwrap_or_else(initial_cwd);
        let entries = list_dir(&cwd, false, SortKey::Name).unwrap_or_default();
        let selected = (!entries.is_empty()).then(IndexPath::default);
        let count = entries.len();
        let list = cx.new(|cx| ListState::new(DirDelegate { entries, selected }, window, cx));
        let prompt_input = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        apply_palette(&palette, Some(window), cx);
        window.set_window_title(&format!("Pifile — {}", cwd.display()));
        let focus = list.focus_handle(cx);
        window.defer(cx, move |window, cx| {
            focus.focus(window, cx);
        });

        let mut this = Self {
            status: format!("{}  ·  {count} items", cwd.display()).into(),
            palette,
            places: places(),
            cwd,
            show_hidden: false,
            sort: SortKey::Name,
            selected,
            list: list.clone(),
            clipboard: None,
            prompt: None,
            prompt_input,
            theme_watch: ThemeWatch::new(),
            last_theme_check: Instant::now(),
            _subs: Vec::new(),
        };
        this._subs.push(
            cx.subscribe_in(
                &list,
                window,
                |this, _, ev: &ListEvent, window, cx| match ev {
                    ListEvent::Select(ix) => {
                        this.selected = Some(*ix);
                        cx.notify();
                    }
                    ListEvent::Confirm(ix) => {
                        this.selected = Some(*ix);
                        this.open_selected(window, cx);
                    }
                    ListEvent::Cancel => {}
                },
            ),
        );
        this
    }

    fn selected_entry(&self, cx: &App) -> Option<DirEntry> {
        self.list.read(cx).delegate().get(self.selected?).cloned()
    }

    fn set_status(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status = text.into();
        cx.notify();
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match list_dir(&self.cwd, self.show_hidden, self.sort) {
            Ok(entries) => {
                let n = entries.len();
                self.selected = (!entries.is_empty()).then(IndexPath::default);
                self.list.update(cx, |state, cx| {
                    state.delegate_mut().entries = entries;
                    state.delegate_mut().selected = self.selected;
                    cx.notify();
                });
                window.set_window_title(&format!("Pifile — {}", self.cwd.display()));
                self.set_status(format!("{}  ·  {n} items", self.cwd.display()), cx);
            }
            Err(err) => self.set_status(format!("error: {err}"), cx),
        }
    }

    fn set_cwd(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.cwd = path.canonicalize().unwrap_or(path);
        self.prompt = None;
        self.reload(window, cx);
    }

    fn go_parent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(parent) = parent_of(&self.cwd) {
            if parent != self.cwd {
                self.set_cwd(parent, window, cx);
            }
        }
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.selected_entry(cx) else {
            return;
        };
        if entry.is_dir() {
            self.cwd = entry.path.canonicalize().unwrap_or(entry.path);
            self.prompt = None;
            match list_dir(&self.cwd, self.show_hidden, self.sort) {
                Ok(entries) => {
                    let count = entries.len();
                    self.selected = (!entries.is_empty()).then(IndexPath::default);
                    self.list.update(cx, |state, cx| {
                        state.delegate_mut().entries = entries;
                        state.delegate_mut().selected = self.selected;
                        cx.notify();
                    });
                    window.set_window_title(&format!("Pifile — {}", self.cwd.display()));
                    self.set_status(format!("{}  ·  {count} items", self.cwd.display()), cx);
                }
                Err(err) => self.set_status(format!("error: {err}"), cx),
            }
        } else {
            self.open_with_system(&entry.path, cx);
        }
    }

    /// Launch the file's default application. `open::that` blocks until the
    /// launched process exits — a terminal editor means the UI freezes — so
    /// the launcher runs on a background thread. `gio open` is preferred over
    /// `xdg-open` because xdg-open's generic branch executes `Terminal=true`
    /// desktop entries directly (`env nvim …`) instead of opening a terminal,
    /// which orphans the editor when the caller has no TTY.
    fn open_with_system(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        self.set_status(format!("opening {}…", path.display()), cx);
        let path = path.to_path_buf();
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let task = cx.background_spawn(async move {
            let mut commands = Vec::new();
            let mut gio = std::process::Command::new("gio");
            gio.arg("open").arg(&path);
            commands.push(gio);
            let mut xdg = std::process::Command::new("xdg-open");
            xdg.arg(&path);
            commands.push(xdg);
            let mut last_err = None;
            for mut cmd in commands {
                match spawn_detached(&mut cmd) {
                    Ok(()) => return Ok(()),
                    Err(err) => last_err = Some(err),
                }
            }
            Err(last_err.unwrap_or_else(|| std::io::Error::other("no launcher available")))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(()) => this.set_status(format!("opened {name}"), cx),
                Err(err) => this.set_status(format!("open {name}: {err:#}"), cx),
            })
            .ok();
        })
        .detach();
    }

    fn trash_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.selected_entry(cx) else {
            return;
        };
        match trash_path(&entry.path) {
            Ok(()) => {
                self.reload(window, cx);
                self.set_status(format!("trashed {}", entry.name), cx);
            }
            Err(err) => self.set_status(format!("{err:#}"), cx),
        }
    }

    fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.selected_entry(cx) else {
            return;
        };
        match delete_permanent(&entry.path) {
            Ok(()) => {
                self.reload(window, cx);
                self.set_status(format!("deleted {}", entry.name), cx);
            }
            Err(err) => self.set_status(format!("{err:#}"), cx),
        }
    }

    fn copy_selected(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Some(entry) = self.selected_entry(cx) else {
            return;
        };
        self.clipboard = Some(if cut {
            ClipboardOp::Cut(entry.path)
        } else {
            ClipboardOp::Copy(entry.path)
        });
        self.set_status(if cut { "cut" } else { "copied" }, cx);
    }

    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(op) = self.clipboard.clone() else {
            return;
        };
        let result = match &op {
            ClipboardOp::Copy(src) => copy_file(src, &self.cwd),
            ClipboardOp::Cut(src) => move_path(src, &self.cwd),
        };
        match result {
            Ok(dest) => {
                if matches!(op, ClipboardOp::Cut(_)) {
                    self.clipboard = None;
                }
                self.reload(window, cx);
                self.set_status(format!("pasted {}", dest.display()), cx);
            }
            Err(err) => self.set_status(format!("{err:#}"), cx),
        }
    }

    fn begin_prompt(&mut self, kind: PromptKind, window: &mut Window, cx: &mut Context<Self>) {
        let seed = match kind {
            PromptKind::NewFolder => String::new(),
            PromptKind::Rename => self.selected_entry(cx).map(|e| e.name).unwrap_or_default(),
        };
        if kind == PromptKind::Rename && seed.is_empty() {
            return;
        }
        self.prompt = Some(kind);
        self.prompt_input.update(cx, |input, cx| {
            input.set_value(seed, window, cx);
        });
        cx.notify();
    }

    fn confirm_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(kind) = self.prompt else {
            return;
        };
        let name = self.prompt_input.read(cx).value().to_string();
        let result = match kind {
            PromptKind::NewFolder => mkdir(&self.cwd, &name).map(|_| format!("created {name}")),
            PromptKind::Rename => match self.selected_entry(cx) {
                Some(entry) => rename(&entry.path, &name).map(|_| format!("renamed {name}")),
                None => Err(anyhow::anyhow!("nothing selected")),
            },
        };
        self.prompt = None;
        match result {
            Ok(msg) => {
                self.reload(window, cx);
                self.set_status(msg, cx);
            }
            Err(err) => self.set_status(format!("{err:#}"), cx),
        }
    }

    /// Re-check the Omarchy theme when the watcher fires so the browser
    /// re-tints live, matching the other GPUI Kit apps.
    fn poll_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.last_theme_check.elapsed() <= Duration::from_millis(400) {
            return;
        }
        self.last_theme_check = Instant::now();
        if !self.theme_watch.drain().is_empty() {
            let palette = OmarchyPalette::load(self.palette.dark);
            if palette != self.palette {
                self.palette = palette;
                apply_palette(&self.palette, Some(window), cx);
                cx.notify();
            }
        }
    }
}

impl Render for Pifile {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.poll_theme(window, cx);
        let page = hex_to_hsla(&self.palette.background).unwrap_or(cx.theme().background);
        let fg = hex_to_hsla(&self.palette.foreground).unwrap_or(cx.theme().foreground);
        let muted = hex_to_hsla(&self.palette.muted).unwrap_or(cx.theme().muted_foreground);
        let surface = hex_to_hsla(&self.palette.surface).unwrap_or(cx.theme().secondary);
        let accent = hex_to_hsla(&self.palette.accent).unwrap_or(cx.theme().primary);
        let cwd = self.cwd.display().to_string();
        let hidden_label = if self.show_hidden {
            "Hide dotfiles"
        } else {
            "Show hidden"
        };

        let sidebar = v_flex()
            .w(px(196.))
            .h_full()
            .flex_shrink_0()
            .bg(surface)
            .border_r_1()
            .border_color(page.blend(fg.opacity(0.08)))
            .py_2()
            .child(
                Label::new("Places")
                    .text_sm()
                    .text_color(muted)
                    .px_3()
                    .pb_1(),
            )
            .children(self.places.iter().map(|place| {
                let active = place.path == self.cwd;
                let path = place.path.clone();
                let label = place.label.clone();
                let icon = place.icon.clone();
                h_flex()
                    .id(SharedString::from(format!("place-{}", label)))
                    .px_3()
                    .py_1()
                    .gap_2()
                    .items_center()
                    .rounded_sm()
                    .mx_1()
                    .text_color(fg)
                    .when(active, |row| {
                        row.bg(page.blend(accent.opacity(0.14))).text_color(accent)
                    })
                    .hover(|row| row.bg(page.blend(accent.opacity(0.08))))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.set_cwd(path.clone(), window, cx);
                        }),
                    )
                    .child(
                        Icon::default()
                            .path(SharedString::from(format!("icons/{icon}.svg")))
                            .small()
                            .text_color(if active { accent } else { muted }),
                    )
                    .child(Label::new(label).text_sm())
            }));

        let toolbar =
            h_flex()
                .w_full()
                .px_2()
                .py_1()
                .gap_2()
                .items_center()
                .bg(surface)
                .border_b_1()
                .border_color(page.blend(fg.opacity(0.08)))
                .child(
                    Button::new("up")
                        .ghost()
                        .icon(IconName::ArrowUp)
                        .on_click(cx.listener(|this, _, window, cx| this.go_parent(window, cx))),
                )
                .child(Button::new("home").ghost().icon(IconName::Folder).on_click(
                    cx.listener(|this, _, window, cx| this.set_cwd(home_dir(), window, cx)),
                ))
                .child(
                    Button::new("refresh")
                        .ghost()
                        .icon(IconName::Redo)
                        .on_click(cx.listener(|this, _, window, cx| this.reload(window, cx))),
                )
                .child(
                    Button::new("new-dir")
                        .ghost()
                        .icon(IconName::Plus)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.begin_prompt(PromptKind::NewFolder, window, cx)
                        })),
                )
                .child(div().flex_1().child(Label::new(cwd).text_sm()))
                .child(
                    Button::new("hidden")
                        .ghost()
                        .label(hidden_label)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_hidden = !this.show_hidden;
                            this.reload(window, cx);
                        })),
                );

        let prompt_bar =
            self.prompt.map(|kind| {
                let label = match kind {
                    PromptKind::NewFolder => "New folder",
                    PromptKind::Rename => "Rename",
                };
                h_flex()
                    .w_full()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .items_center()
                    .child(Label::new(label).text_sm())
                    .child(div().flex_1().child(Input::new(&self.prompt_input)))
                    .child(Button::new("prompt-ok").primary().label("OK").on_click(
                        cx.listener(|this, _, window, cx| this.confirm_prompt(window, cx)),
                    ))
                    .child(
                        Button::new("prompt-cancel")
                            .ghost()
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.prompt = None;
                                cx.notify();
                            })),
                    )
            });

        v_flex()
            .id("pifile")
            .size_full()
            .bg(page)
            .text_color(fg)
            .on_action(cx.listener(|this, _: &GoParent, window, cx| this.go_parent(window, cx)))
            .on_action(cx.listener(|this, _: &OpenSelected, window, cx| {
                if this.prompt.is_some() {
                    this.confirm_prompt(window, cx);
                } else {
                    this.open_selected(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &OpenWithSystem, _, cx| {
                if let Some(entry) = this.selected_entry(cx) {
                    this.open_with_system(&entry.path, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NewFolder, window, cx| {
                this.begin_prompt(PromptKind::NewFolder, window, cx)
            }))
            .on_action(cx.listener(|this, _: &RenameSelected, window, cx| {
                this.begin_prompt(PromptKind::Rename, window, cx)
            }))
            .on_action(
                cx.listener(|this, _: &TrashSelected, window, cx| this.trash_selected(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &DeleteSelected, window, cx| {
                    this.delete_selected(window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &CopySelected, _, cx| this.copy_selected(false, cx)))
            .on_action(cx.listener(|this, _: &CutSelected, _, cx| this.copy_selected(true, cx)))
            .on_action(cx.listener(|this, _: &PasteClipboard, window, cx| this.paste(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleHidden, window, cx| {
                this.show_hidden = !this.show_hidden;
                this.reload(window, cx);
            }))
            .on_action(cx.listener(|this, _: &Refresh, window, cx| this.reload(window, cx)))
            .on_action(
                cx.listener(|this, _: &SelectHome, window, cx| {
                    this.set_cwd(home_dir(), window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ConfirmPrompt, window, cx| this.confirm_prompt(window, cx)),
            )
            .on_action(cx.listener(|this, _: &CancelPrompt, _, cx| {
                this.prompt = None;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Quit, _, cx| {
                let _ = this;
                cx.quit();
            }))
            .child(
                h_flex().flex_1().size_full().child(sidebar).child(
                    v_flex()
                        .flex_1()
                        .h_full()
                        .child(toolbar)
                        .children(prompt_bar)
                        .child(List::new(&self.list).flex_1())
                        .child(
                            h_flex()
                                .w_full()
                                .px_3()
                                .py_1()
                                .bg(surface)
                                .border_t_1()
                                .border_color(page.blend(fg.opacity(0.08)))
                                .child(Label::new(self.status.clone()).text_sm().text_color(muted)),
                        ),
                ),
            )
    }
}

impl ListDelegate for DirDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.entries.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        self.entries.get(ix.row).map(|file| {
            let icon = match file.kind {
                FileKind::Directory => IconName::Folder,
                _ => IconName::File,
            };
            ListItem::new(ix)
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .gap_3()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .flex_1()
                                .child(Icon::new(icon))
                                .child(Label::new(file.name.clone())),
                        )
                        .child(
                            Label::new(file.size_label())
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            Label::new(file.modified_label())
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                )
                .selected(Some(ix) == self.selected)
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
        cx.notify();
    }
}

/// Launch `cmd` detached from this process: stdio goes to /dev/null and the
/// child gets its own process group, so the launcher outlives the window and
/// the UI thread never waits on it.
#[cfg(unix)]
fn spawn_detached(cmd: &mut std::process::Command) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt as _;
    use std::process::Stdio;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
}

/// Non-Unix fallback: detach via null stdio only.
#[cfg(not(unix))]
fn spawn_detached(cmd: &mut std::process::Command) -> std::io::Result<()> {
    use std::process::Stdio;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

fn apply_palette(palette: &OmarchyPalette, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(
        if palette.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );
    let theme = Theme::global_mut(cx);
    let bg = hex_to_hsla(&palette.background);
    let fg = hex_to_hsla(&palette.foreground);
    if let Some(bg) = bg {
        theme.background = bg;
    }
    if let Some(fg) = fg {
        theme.foreground = fg;
    }
    if let Some(accent) = hex_to_hsla(&palette.accent) {
        theme.primary = accent;
        theme.accent = accent;
    }
    if let Some(surface) = hex_to_hsla(&palette.surface) {
        // Panels share the file area's tonal family: same hue, one step up.
        theme.secondary = surface;
        theme.sidebar = surface;
    }
    if let Some(muted) = hex_to_hsla(&palette.muted) {
        theme.muted_foreground = muted;
        theme.sidebar_foreground = muted;
    }
    if let (Some(bg), Some(accent_hex)) = (bg, hex_to_hsla(&palette.accent)) {
        // Row highlight: a translucent wash of the accent over the background,
        // with foreground text always readable on top.
        theme.list_active = bg.blend(accent_hex.opacity(0.18));
        theme.list_active_border = bg.blend(accent_hex.opacity(0.55));
        theme.list_hover = bg.blend(accent_hex.opacity(0.08));
        theme.selection = bg.blend(accent_hex.opacity(0.18));
    }
    Theme::sync_base(cx);
}

fn hex_to_hsla(value: &str) -> Option<Hsla> {
    let hex = value.trim().trim_start_matches('#');
    let expanded = if hex.len() == 3 {
        hex.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        hex.to_string()
    };
    let n = u32::from_str_radix(&expanded, 16).ok()?;
    Some(rgb(n).into())
}
