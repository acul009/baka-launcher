//! The launcher's state, interactions, and view.

use iced::keyboard;
use iced::widget::{
    button, column, container, grid, image, mouse_area, operation, pin, row, scrollable, space,
    stack, svg, text, text_input,
};
use iced::{Border, Center, Color, Element, Fill, Point, Subscription, Task, Theme, Widget, color};

use crate::{catalog, launch, pins};

const SEARCH: &str = "search";
const SURFACE: Color = Color::from_packed_rgb8(0x101113);
const MUTED: Color = Color::from_packed_rgb8(0x9da4af);

/// The single launcher screen.
pub struct State {
    query: String,
    calculation: Option<String>,
    copied: bool,
    apps: Vec<catalog::App>,
    icons: Vec<Icon>,
    visible: Vec<usize>,
    loading: bool,
    launching: bool,
    error: Option<String>,
    pins: pins::Preferences,
    pins_ready: bool,
    saving_pins: bool,
    context: Option<(usize, Point)>,
    cursor: Point,
}

enum Icon {
    Svg(svg::Handle),
    Raster(image::Handle),
}

/// Messages emitted by the launcher and its finite background tasks.
#[derive(Debug, Clone)]
pub enum Message {
    /// Search text changed.
    Query(String),
    /// Activate the first displayed result.
    Submit,
    /// Copy the displayed calculation result without launching an application.
    CopyResult,
    /// The clipboard reports success or failure for the specified result.
    CopyFinished {
        /// Result whose clipboard request completed.
        result: String,
        /// Clipboard write outcome.
        status: Result<(), iced::clipboard::Error>,
    },
    /// Activate one displayed application.
    Launch(usize),
    /// Initial or refreshed application scan completed.
    Loaded(Result<Vec<catalog::App>, String>),
    /// A process spawn completed.
    Launched(Result<(), String>),
    /// Refresh the application catalog.
    Reload,
    /// Focus the search after the native window opens.
    Focus,
    /// Open an application's context menu without launching it.
    Context(usize),
    /// Track the pointer for positioning the context menu.
    Cursor(Point),
    /// Dismiss the context menu.
    Dismiss,
    /// Toggle the pin preference of the menu's application.
    TogglePin,
    /// User pin preferences finished loading.
    PreferencesLoaded(Result<pins::Preferences, String>),
    /// A pin preference was saved, or failed without changing the current pins.
    PreferencesSaved(Result<pins::Preferences, String>),
    /// Close the launcher.
    Close,
    /// Exit when the native window loses keyboard focus, even with a menu open.
    Unfocused,
}

impl State {
    /// Creates the visible loading screen and initial runtime tasks.
    pub fn new() -> (Self, Task<Message>) {
        let (mut state, focus) = Self::empty();
        state.pins_ready = false;
        let preferences = Task::perform(
            async {
                tokio::task::spawn_blocking(pins::Preferences::load_default)
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(|error| error.to_string()))
            },
            Message::PreferencesLoaded,
        );
        (state, Task::batch([focus, scan(), preferences]))
    }

    fn empty() -> (Self, Task<Message>) {
        (
            Self {
                query: String::new(),
                calculation: None,
                copied: false,
                apps: Vec::new(),
                icons: Vec::new(),
                visible: Vec::new(),
                loading: true,
                launching: false,
                error: None,
                pins: pins::Preferences::default(),
                pins_ready: true,
                saving_pins: false,
                context: None,
                cursor: Point::ORIGIN,
            },
            operation::focus(SEARCH),
        )
    }

    /// Processes input and runs indexing or launch work away from the UI thread.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Query(query) => {
                self.context = None;
                self.query = query;
                self.calculation = model::calculator::evaluate(&self.query);
                self.copied = false;
                self.error = None;
                self.visible = self.order();
                Task::none()
            }
            Message::Loaded(result) => {
                self.context = None;
                self.loading = false;
                match result {
                    Ok(apps) => {
                        self.icons = apps
                            .iter()
                            .map(|app| match &app.icon {
                                Some(catalog::Icon::Svg(bytes)) => {
                                    Icon::Svg(svg::Handle::from_memory(bytes.clone()))
                                }
                                Some(catalog::Icon::Raster(bytes)) => {
                                    Icon::Raster(image::Handle::from_bytes(bytes.clone()))
                                }
                                None => Icon::Svg(svg::Handle::from_memory(
                                    include_bytes!("../assets/application.svg").as_slice(),
                                )),
                            })
                            .collect();
                        self.apps = apps;
                        self.visible = self.order();
                    }
                    Err(error) => self.error = Some(error),
                }
                Task::none()
            }
            Message::Submit => {
                if self.calculation.is_some() {
                    return self.update(Message::CopyResult);
                }
                if self.context.is_some() {
                    return self.update(Message::TogglePin);
                }
                let Some(&index) = self.visible.first() else {
                    return Task::none();
                };
                self.update(Message::Launch(index))
            }
            Message::CopyResult => {
                let Some(result) = self.calculation.clone() else {
                    return Task::none();
                };
                self.copied = false;
                self.error = None;
                Task::batch([
                    iced::clipboard::write(result.clone()).map(move |status| {
                        Message::CopyFinished {
                            result: result.clone(),
                            status,
                        }
                    }),
                    operation::focus(SEARCH),
                ])
            }
            Message::CopyFinished { result, status } => {
                if self.calculation.as_ref() == Some(&result) {
                    match status {
                        Ok(()) => self.copied = true,
                        Err(_) => {
                            self.error = Some("Could not copy result to the clipboard".into())
                        }
                    }
                }
                Task::none()
            }
            Message::Launch(index) => {
                if self.launching || !self.visible.contains(&index) {
                    return Task::none();
                }
                let Some(app) = self.apps.get(index) else {
                    return Task::none();
                };
                self.launching = true;
                self.context = None;
                self.error = None;
                let entry = app.entry.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            launch::spawn(&entry).map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()))
                    },
                    Message::Launched,
                )
            }
            Message::Launched(Ok(())) | Message::Unfocused => iced::exit(),
            Message::Close if self.context.is_some() => self.update(Message::Dismiss),
            Message::Close => iced::exit(),
            Message::Launched(Err(error)) => {
                self.launching = false;
                self.error = Some(format!("Could not launch application: {error}"));
                operation::focus(SEARCH)
            }
            Message::Reload => {
                self.context = None;
                if self.loading || self.launching {
                    return operation::focus(SEARCH);
                }
                self.loading = true;
                self.error = None;
                Task::batch([operation::focus(SEARCH), scan()])
            }
            Message::Focus => operation::focus(SEARCH),
            Message::Cursor(position) => {
                self.cursor = position;
                Task::none()
            }
            Message::Context(index) => {
                if !self.launching && self.visible.contains(&index) {
                    self.context = Some((index, self.cursor));
                }
                Task::none()
            }
            Message::Dismiss => {
                self.context = None;
                operation::focus(SEARCH)
            }
            Message::TogglePin => {
                if !self.pins_ready || self.saving_pins || self.launching {
                    return Task::none();
                }
                let Some((index, _)) = self.context.take() else {
                    return Task::none();
                };
                let Some(app) = self.apps.get(index) else {
                    return Task::none();
                };
                let mut preferences = self.pins.clone();
                preferences.set(app.entry.id.clone(), !preferences.is_pinned(&app.entry));
                self.saving_pins = true;
                self.error = None;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            preferences.save().map_err(|error| error.to_string())?;
                            Ok(preferences)
                        })
                        .await
                        .unwrap_or_else(|error| Err(error.to_string()))
                    },
                    Message::PreferencesSaved,
                )
            }
            Message::PreferencesLoaded(result) => {
                match result {
                    Ok(preferences) => {
                        self.pins = preferences;
                        self.pins_ready = true;
                        self.visible = self.order();
                    }
                    Err(error) => {
                        self.error = Some(format!("Could not load pin preferences: {error}"))
                    }
                }
                Task::none()
            }
            Message::PreferencesSaved(result) => {
                self.saving_pins = false;
                match result {
                    Ok(preferences) => {
                        self.pins = preferences;
                        self.visible = self.order();
                    }
                    Err(error) => {
                        self.error = Some(format!("Could not save pin preferences: {error}"))
                    }
                }
                operation::focus(SEARCH)
            }
        }
    }

    fn order(&self) -> Vec<usize> {
        if self.calculation.is_some() {
            return Vec::new();
        }
        // The model's exact order is also the grid's order and Enter's target.
        let entries: Vec<_> = self.apps.iter().map(|app| app.entry.clone()).collect();
        let mut ordered = model::search::order(&entries, &self.query);
        if self.query.trim().is_empty() {
            ordered.sort_by(|&a, &b| {
                match (
                    self.pins.is_pinned(&entries[a]),
                    self.pins.is_pinned(&entries[b]),
                ) {
                    (true, true) => std::cmp::Ordering::Equal,
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    (false, false) => entries[a]
                        .name
                        .to_lowercase()
                        .cmp(&entries[b].name.to_lowercase())
                        .then_with(|| entries[a].id.cmp(&entries[b].id)),
                }
            });
        }
        ordered
    }

    /// Listens only while the launcher is running.
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                } => Some(Message::Close),
                _ => None,
            }),
            iced::window::events().filter_map(|(_, event)| match event {
                iced::window::Event::Opened { .. } => Some(Message::Focus),
                iced::window::Event::Unfocused => Some(Message::Unfocused),
                _ => None,
            }),
        ])
    }

    /// Builds a focused search bar and a quiet, responsive application grid.
    pub fn view(&self) -> Element<'_, Message> {
        let search = container(
            row![
                svg(svg::Handle::from_memory(
                    include_bytes!("../assets/search.svg").as_slice()
                ))
                .width(20)
                .height(20),
                text_input("Search apps or calculate…", &self.query)
                    .id(SEARCH)
                    .size(17)
                    .padding(0)
                    .on_input(Message::Query)
                    .on_submit(Message::Submit)
                    .style(|_, _| text_input::Style {
                        background: Color::TRANSPARENT.into(),
                        border: Border::default(),
                        placeholder: color!(0xa9b1bf),
                        value: color!(0xf0f0f2),
                        selection: color!(0x45474e),
                    }),
            ]
            .spacing(12)
            .align_y(Center),
        )
        .padding([10, 14])
        .height(50)
        .width(Fill)
        .style(|_| container::Style {
            background: Some(color!(0x18191b).into()),
            border: Border {
                color: color!(0x45474e),
                width: 1.0,
                radius: 9.into(),
            },
            ..Default::default()
        });

        let content = iced::widget::responsive(move |size| {
            let columns = ((size.width - 20.0) / 156.0).floor().clamp(1.0, 8.0) as usize;
            let tiles = |indexes: Vec<usize>| {
                grid(indexes.into_iter().map(|index| self.tile(index)))
                    .columns(columns)
                    .width((size.width - 16.0).max(1.0))
                    .height(iced::Shrink)
                    .spacing(12)
            };
            let mut body: iced::widget::Column<Element<'_, Message>> =
                column![].spacing(14).width(Fill);
            if let Some(result) = &self.calculation {
                body = body
                    .push(text("CALCULATOR").size(12).color(MUTED).boxed())
                    .push(
                        button(
                            column![
                                text(result)
                                    .size(32)
                                    .wrapping(text::Wrapping::WordOrGlyph)
                                    .color(color!(0xeeeeef)),
                                text(if self.copied {
                                    "Copied to clipboard"
                                } else {
                                    "Enter or click to copy"
                                })
                                .size(14)
                                .color(MUTED),
                            ]
                            .spacing(8)
                            .width(Fill),
                        )
                        .on_press(Message::CopyResult)
                        .padding(18)
                        .width(Fill)
                        .style(|_, status| button::Style {
                            background: Some(
                                if status == button::Status::Hovered {
                                    color!(0x29292d)
                                } else {
                                    color!(0x202023)
                                }
                                .into(),
                            ),
                            border: Border {
                                color: color!(0x38383e),
                                width: 1.0,
                                radius: 9.into(),
                            },
                            ..Default::default()
                        })
                        .boxed(),
                    );
            } else if !self.query.trim().is_empty() {
                body = body
                    .push(text("APPLICATIONS").size(12).color(MUTED).boxed())
                    .push(tiles(self.visible.clone()).boxed());
            } else {
                let (pinned, remaining): (Vec<_>, Vec<_>) = self
                    .visible
                    .iter()
                    .copied()
                    .partition(|&index| self.pins.is_pinned(&self.apps[index].entry));
                if !pinned.is_empty() {
                    body = body
                        .push(text("PINNED").size(12).color(MUTED).boxed())
                        .push(tiles(pinned).boxed())
                        .push(
                            iced::widget::rule::horizontal(1)
                                .style(|_| iced::widget::rule::Style {
                                    color: color!(0x272b31),
                                    radius: 0.into(),
                                    fill_mode: iced::widget::rule::FillMode::Full,
                                    snap: true,
                                })
                                .boxed(),
                        );
                }
                body = body
                    .push(text("APPLICATIONS").size(12).color(MUTED).boxed())
                    .push(tiles(remaining).boxed());
            }
            if self.loading && self.calculation.is_none() {
                body = body.push(text("Loading applications…").size(14).color(MUTED).boxed());
            } else if self.visible.is_empty() && self.calculation.is_none() {
                body = body.push(
                    text(if self.apps.is_empty() {
                        "No applications found"
                    } else {
                        "No matching applications"
                    })
                    .size(15)
                    .color(MUTED)
                    .boxed(),
                );
            }
            scrollable(body.padding([10, 4])).height(Fill).width(Fill)
        });
        let panel = container(content)
            .height(Fill)
            .width(Fill)
            .style(|_| container::Style {
                background: Some(SURFACE.into()),
                border: Border {
                    radius: 11.into(),
                    ..Default::default()
                },
                ..Default::default()
            });
        let mut body: iced::widget::Column<Element<'_, Message>> =
            column![search.boxed()].spacing(10);
        if let Some(error) = &self.error {
            body = body.push(text(error).size(14).color(color!(0xefb0ad)).boxed());
        }
        body = body.push(panel.boxed());
        let base = container(body)
            .padding(12)
            .height(Fill)
            .width(Fill)
            .style(|_| container::Style {
                background: Some(SURFACE.into()),
                border: Border {
                    radius: 12.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .boxed();
        let menu = iced::widget::responsive(move |size| -> Element<'_, Message> {
            let Some((index, position)) = self.context else {
                return space().boxed();
            };
            let label = if self.pins.is_pinned(&self.apps[index].entry) {
                "Unpin from Pinned"
            } else {
                "Pin to Pinned"
            };
            let item = button(text(label).size(15))
                .width(Fill)
                .padding([10, 14])
                .on_press_maybe(
                    (self.pins_ready && !self.saving_pins).then_some(Message::TogglePin),
                )
                .style(|_, status| button::Style {
                    background: Some(
                        if status == button::Status::Hovered {
                            color!(0x3c3c40)
                        } else {
                            color!(0x28282b)
                        }
                        .into(),
                    ),
                    text_color: color!(0xeeeeef),
                    border: Border {
                        radius: 6.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });
            let menu = container(item)
                .padding(4)
                .width(194)
                .style(|_| container::Style {
                    background: Some(color!(0x28282b).into()),
                    border: Border {
                        color: color!(0x45454a),
                        width: 1.0,
                        radius: 9.into(),
                    },
                    ..Default::default()
                });
            stack![
                mouse_area(space().width(Fill).height(Fill))
                    .on_press(Message::Dismiss)
                    .on_right_press(Message::Dismiss)
                    .on_scroll(|_| Message::Dismiss),
                pin(menu).position(Point::new(
                    position.x.clamp(8.0, (size.width - 202.0).max(8.0)),
                    position.y.clamp(8.0, (size.height - 60.0).max(8.0))
                )),
            ]
            .width(Fill)
            .height(Fill)
            .boxed()
        });
        mouse_area(stack![base, menu])
            .on_move(Message::Cursor)
            .boxed()
    }

    fn tile(&self, index: usize) -> Element<'_, Message> {
        let app = &self.apps[index];
        let pinned = self.query.trim().is_empty() && self.pins.is_pinned(&app.entry);
        let icon_size: u32 = if pinned { 32 } else { 40 };
        let icon: Element<'_, Message> = match &self.icons[index] {
            Icon::Svg(handle) => svg(handle.clone())
                .width(icon_size)
                .height(icon_size)
                .boxed(),
            Icon::Raster(handle) => image(handle.clone())
                .width(icon_size)
                .height(icon_size)
                .boxed(),
        };
        let first = self.visible.first() == Some(&index);
        mouse_area(
            button(
                column![
                    icon,
                    text(&app.entry.name)
                        .size(15)
                        .line_height(1.2)
                        .height(36)
                        .wrapping(text::Wrapping::WordOrGlyph)
                        .ellipsis(text::Ellipsis::End)
                        .color(if first {
                            color!(0xf0f2f6)
                        } else {
                            color!(0xc5cbd4)
                        })
                        .align_x(Center)
                        .width(Fill)
                ]
                .spacing(8)
                .align_x(Center),
            )
            .padding([if pinned { 10 } else { 14 }, 8])
            .width(Fill)
            .height(if pinned { 100 } else { 116 })
            .on_press_maybe((!self.launching).then_some(Message::Launch(index)))
            .style(move |_, status| button::Style {
                background: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                    Some(color!(0x292a2e).into())
                } else if first {
                    Some(color!(0x222327).into())
                } else {
                    Some(color!(0x191a1d).into())
                },
                border: Border {
                    radius: 8.into(),
                    width: 1.0,
                    color: if first {
                        color!(0x62656f)
                    } else {
                        color!(0x2d2f35)
                    },
                },
                text_color: MUTED,
                ..Default::default()
            }),
        )
        .on_right_press(Message::Context(index))
        .boxed()
    }
}

fn scan() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(catalog::scan)
                .await
                .map_err(|e| e.to_string())
                .and_then(|result| result.map_err(|e| e.to_string()))
        },
        Message::Loaded,
    )
}

/// The fixed dark appearance used by the launcher.
pub fn theme(_: &State) -> Theme {
    Theme::Dark
}

#[cfg(test)]
mod tests;
