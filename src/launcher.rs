//! The launcher's state, interactions, and view.

use iced::keyboard;
use iced::widget::{
    button, column, container, grid, image, operation, row, scrollable, space, svg, text,
    text_input, tooltip,
};
use iced::{Border, Center, Color, Element, Fill, Subscription, Task, Theme, Widget, color};

use crate::{catalog, launch};

const SEARCH: &str = "search";
const SURFACE: Color = Color::from_packed_rgb8(0x131313);
const MUTED: Color = Color::from_packed_rgb8(0x989ca6);

/// The single launcher screen.
pub struct State {
    query: String,
    apps: Vec<catalog::App>,
    icons: Vec<Icon>,
    visible: Vec<usize>,
    loading: bool,
    launching: bool,
    error: Option<String>,
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
    /// Close the launcher.
    Close,
}

impl State {
    /// Creates the visible loading screen and initial runtime tasks.
    pub fn new() -> (Self, Task<Message>) {
        let (state, focus) = Self::empty();
        (state, Task::batch([focus, scan()]))
    }

    fn empty() -> (Self, Task<Message>) {
        (
            Self {
                query: String::new(),
                apps: Vec::new(),
                icons: Vec::new(),
                visible: Vec::new(),
                loading: true,
                launching: false,
                error: None,
            },
            operation::focus(SEARCH),
        )
    }

    /// Processes input and runs indexing or launch work away from the UI thread.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Query(query) => {
                self.query = query;
                self.error = None;
                self.visible = self.order();
                Task::none()
            }
            Message::Loaded(result) => {
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
                        self.error = None;
                    }
                    Err(error) => self.error = Some(error),
                }
                Task::none()
            }
            Message::Submit => {
                let Some(&index) = self.visible.first() else {
                    return Task::none();
                };
                self.update(Message::Launch(index))
            }
            Message::Launch(index) => {
                if self.launching || !self.visible.contains(&index) {
                    return Task::none();
                }
                let Some(app) = self.apps.get(index) else {
                    return Task::none();
                };
                self.launching = true;
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
            Message::Launched(Ok(())) | Message::Close => iced::exit(),
            Message::Launched(Err(error)) => {
                self.launching = false;
                self.error = Some(format!("Could not launch application: {error}"));
                operation::focus(SEARCH)
            }
            Message::Reload => {
                if self.loading || self.launching {
                    return operation::focus(SEARCH);
                }
                self.loading = true;
                self.error = None;
                Task::batch([operation::focus(SEARCH), scan()])
            }
            Message::Focus => operation::focus(SEARCH),
        }
    }

    fn order(&self) -> Vec<usize> {
        // The model's exact order is also the grid's order and Enter's target.
        let entries: Vec<_> = self.apps.iter().map(|app| app.entry.clone()).collect();
        model::search::order(&entries, &self.query)
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
                _ => None,
            }),
        ])
    }

    /// Builds the dark reference layout with responsive application tiles.
    pub fn view(&self) -> Element<'_, Message> {
        let search = container(
            row![
                svg(svg::Handle::from_memory(
                    include_bytes!("../assets/search.svg").as_slice()
                ))
                .width(25)
                .height(25),
                text_input("Search", &self.query)
                    .id(SEARCH)
                    .size(19)
                    .padding(0)
                    .on_input(Message::Query)
                    .on_submit(Message::Submit)
                    .style(|_, _| text_input::Style {
                        background: Color::TRANSPARENT.into(),
                        border: Border::default(),
                        placeholder: color!(0xe4e4e7),
                        value: color!(0xf0f0f2),
                        selection: color!(0x485571),
                    }),
            ]
            .spacing(13)
            .align_y(Center),
        )
        .padding([13, 12])
        .height(52)
        .width(Fill)
        .style(|_| container::Style {
            background: Some(color!(0x202020).into()),
            border: Border {
                color: color!(0x373737),
                width: 1.2,
                radius: 10.into(),
            },
            ..Default::default()
        });

        let rail = container(
            column![
                space().height(Fill),
                rail_button(
                    include_bytes!("../assets/refresh.svg"),
                    "Refresh applications",
                    Some(Message::Reload)
                ),
                rail_button(
                    include_bytes!("../assets/moon.svg"),
                    "Dark appearance",
                    None
                ),
                rail_button(
                    include_bytes!("../assets/exit.svg"),
                    "Close launcher",
                    Some(Message::Close)
                ),
                rail_button(
                    include_bytes!("../assets/power.svg"),
                    "Session actions unavailable",
                    None
                ),
            ]
            .spacing(9)
            .align_x(Center),
        )
        .padding([10, 8])
        .width(52)
        .height(Fill)
        .style(|_| container::Style {
            background: Some(SURFACE.into()),
            border: Border {
                radius: 11.into(),
                ..Default::default()
            },
            ..Default::default()
        });

        let content = iced::widget::responsive(move |size| {
            let columns = ((size.width - 20.0) / 104.0).floor().clamp(1.0, 8.0) as usize;
            let tiles = |indexes: Vec<usize>| {
                grid(indexes.into_iter().map(|index| self.tile(index)))
                    .columns(columns)
                    .width((columns as f32 * 104.0).min(size.width - 8.0))
                    .height(iced::widget::grid::aspect_ratio(104, 118))
                    .spacing(0)
            };
            let mut body: iced::widget::Column<Element<'_, Message>> =
                column![].spacing(12).width(Fill);
            if !self.query.trim().is_empty() {
                body = body
                    .push(text("APPLICATIONS").size(15).color(MUTED).boxed())
                    .push(tiles(self.visible.clone()).boxed());
            } else {
                let (pinned, remaining): (Vec<_>, Vec<_>) = self
                    .visible
                    .iter()
                    .copied()
                    .partition(|&index| model::search::is_pinned(&self.apps[index].entry));
                body = body.push(text("PINNED").size(15).color(MUTED).boxed());
                if pinned.is_empty() {
                    body = body.push(
                        container(
                            text(if self.loading {
                                ""
                            } else {
                                "No pinned applications installed"
                            })
                            .size(14)
                            .color(MUTED),
                        )
                        .height(110)
                        .padding([22, 12])
                        .boxed(),
                    );
                } else {
                    body = body.push(tiles(pinned).boxed());
                }
                body = body
                    .push(
                        iced::widget::rule::horizontal(1)
                            .style(|_| iced::widget::rule::Style {
                                color: color!(0x222222),
                                radius: 0.into(),
                                fill_mode: iced::widget::rule::FillMode::Full,
                                snap: true,
                            })
                            .boxed(),
                    )
                    .push(text("APPLICATIONS").size(15).color(MUTED).boxed())
                    .push(tiles(remaining).boxed());
            }
            if self.loading {
                body = body.push(text("Loading applications…").size(14).color(MUTED).boxed());
            } else if self.visible.is_empty() {
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
            if let Some(error) = &self.error {
                body = body.push(text(error).size(14).color(color!(0xefb0ad)).boxed());
            }
            scrollable(body.padding([12, 10])).height(Fill).width(Fill)
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
        container(column![search, row![rail, panel].spacing(10).height(Fill)].spacing(10))
            .padding(10)
            .height(Fill)
            .width(Fill)
            .style(|_| container::Style {
                background: Some(color!(0x090909).into()),
                border: Border {
                    radius: 12.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .boxed()
    }

    fn tile(&self, index: usize) -> Element<'_, Message> {
        let app = &self.apps[index];
        let icon: Element<'_, Message> = match &self.icons[index] {
            Icon::Svg(handle) => svg(handle.clone()).width(44).height(44).boxed(),
            Icon::Raster(handle) => image(handle.clone()).width(44).height(44).boxed(),
        };
        let first = !self.query.trim().is_empty() && self.visible.first() == Some(&index);
        button(
            column![
                icon,
                text(&app.entry.name)
                    .size(15)
                    .line_height(1.2)
                    .height(36)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .ellipsis(text::Ellipsis::End)
                    .color(MUTED)
                    .align_x(Center)
                    .width(Fill)
            ]
            .spacing(10)
            .align_x(Center),
        )
        .padding([16, 4])
        .width(Fill)
        .height(118)
        .on_press_maybe((!self.launching).then_some(Message::Launch(index)))
        .style(move |_, status| button::Style {
            background: (matches!(status, button::Status::Hovered | button::Status::Pressed))
                .then_some(color!(0x242426).into()),
            border: Border {
                radius: 8.into(),
                width: if first { 1.0 } else { 0.0 },
                color: color!(0x34343b),
            },
            text_color: MUTED,
            ..Default::default()
        })
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

fn rail_button(
    bytes: &'static [u8],
    label: &'static str,
    message: Option<Message>,
) -> Element<'static, Message> {
    tooltip(
        button(svg(svg::Handle::from_memory(bytes)).width(23).height(23))
            .on_press_maybe(message)
            .padding(6)
            .style(|_, status| button::Style {
                background: Some(
                    if status == button::Status::Hovered {
                        color!(0x29292b)
                    } else {
                        color!(0x1d1d1e)
                    }
                    .into(),
                ),
                border: Border {
                    radius: 10.into(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        text(label).size(13),
        tooltip::Position::Right,
    )
    .boxed()
}

/// The fixed dark appearance used by the launcher.
pub fn theme(_: &State) -> Theme {
    Theme::Dark
}

#[cfg(test)]
mod tests;
