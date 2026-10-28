//! Exercise the real widget tree and runtime tasks without opening a window.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use iced::keyboard::key::Named;
use iced::{Point, Settings, Size, Task, Theme};
use iced_test::Simulator;
use iced_test::futures::futures::StreamExt;
use iced_test::runtime::{self, Action};
use iced_test::selector;

use super::{Icon, Message, State};
use crate::catalog;

static NEXT: AtomicU64 = AtomicU64::new(0);

fn app(id: &str, name: &str) -> catalog::App {
    catalog::App {
        entry: model::Entry::parse(
            id.into(),
            PathBuf::from(format!("/fixture/{id}")),
            &format!("[Desktop Entry]\nType=Application\nName={name}\nExec=touch\n"),
            "C",
        )
        .unwrap()
        .unwrap(),
        icon: None,
    }
}

fn loaded(apps: Vec<catalog::App>) -> (State, Task<Message>) {
    let (mut state, focus) = State::empty();
    let _ = state.update(Message::Loaded(Ok(apps)));
    (state, focus)
}

// Immutable render snapshots let update mutate the real state while rebuild
// preserves the simulator's widget cache, including keyboard focus.
fn render_state(state: &State) -> State {
    State {
        query: state.query.clone(),
        calculation: state.calculation.clone(),
        copied: state.copied,
        apps: state.apps.clone(),
        icons: state
            .icons
            .iter()
            .map(|icon| match icon {
                Icon::Svg(handle) => Icon::Svg(handle.clone()),
                Icon::Raster(handle) => Icon::Raster(handle.clone()),
            })
            .collect(),
        visible: state.visible.clone(),
        loading: state.loading,
        launching: state.launching,
        error: state.error.clone(),
        pins: state.pins.clone(),
        pins_ready: state.pins_ready,
        saving_pins: state.saving_pins,
        context: state.context,
        cursor: state.cursor,
    }
}

fn focus(ui: &mut Simulator<'_, Message>, task: Task<Message>) {
    let executor = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    executor.block_on(async {
        let mut actions = runtime::task::into_stream(task).expect("boot must schedule focus");
        while let Some(action) = actions.next().await {
            if let Action::Widget(mut operation) = action {
                ui.operate(operation.as_mut());
                assert!(ui.find(selector::is_focused()).is_ok());
                return;
            }
        }
        panic!("boot task did not emit its widget focus operation");
    });
}

fn task_output(task: Task<Message>) -> Message {
    let executor = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    executor.block_on(async {
        let mut actions = runtime::task::into_stream(task).expect("launch must schedule work");
        while let Some(action) = actions.next().await {
            if let Action::Output(message) = action {
                return message;
            }
        }
        panic!("launch task completed without reporting its result");
    })
}

#[test]
fn actual_boot_task_focuses_search_before_typing_or_clicking() {
    let (mut state, boot) = State::new();
    let rendered = render_state(&state);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), rendered.view());
    assert!(ui.find(selector::is_focused()).is_err());
    focus(&mut ui, boot);
    let _ = ui.typewrite("fire");
    let messages: Vec<_> = ui.drain().collect();
    assert!(
        messages
            .iter()
            .all(|message| matches!(message, Message::Query(_)))
    );
    for message in messages {
        let _ = state.update(message);
    }
    assert_eq!(state.query, "fire");
    assert!(state.loading);
    let _ = state.update(Message::Loaded(Ok(vec![
        app("firefox.desktop", "Firefox"),
        app("other.desktop", "Other"),
    ])));
    assert_eq!(state.query, "fire");
    assert_eq!(state.visible, [0]);
    let updated = render_state(&state);
    let mut ui = ui.rebuild(updated.view());
    assert!(ui.find("Firefox").is_ok());
    assert!(ui.find("Other").is_err());
    assert!(
        ui.find(selector::is_focused()).is_ok(),
        "index completion must preserve focus"
    );
    let _ = ui.typewrite("fox");
    for message in ui.into_messages() {
        let _ = state.update(message);
    }
    assert_eq!(state.query, "firefox");
}

#[test]
fn enter_launches_the_first_ranked_visible_result_exactly_once() {
    let directory = std::env::temp_dir().join(format!(
        "baka-gui-enter-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&directory).unwrap();
    let mut pinned = app("google-chrome.desktop", "Chrome Browser");
    pinned.entry.command = vec!["touch".into(), "wrong".into()];
    pinned.entry.directory = Some(directory.clone());
    let mut exact = app("browser.desktop", "Browser");
    exact.entry.command = vec!["touch".into(), "correct".into()];
    exact.entry.directory = Some(directory.clone());
    let (mut state, boot) = loaded(vec![pinned, exact]);
    let rendered = render_state(&state);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), rendered.view());
    focus(&mut ui, boot);
    let _ = ui.typewrite("browser");
    for message in ui.drain().collect::<Vec<_>>() {
        let _ = state.update(message);
    }
    assert_eq!(state.visible, [1, 0]);
    let updated = render_state(&state);
    let mut ui = ui.rebuild(updated.view());
    assert!(ui.find("Browser").unwrap().bounds().x < ui.find("Chrome Browser").unwrap().bounds().x);
    let _ = ui.tap_key(Named::Enter);
    let messages: Vec<_> = ui.drain().collect();
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0], Message::Submit));
    let task = state.update(messages.into_iter().next().unwrap());
    assert!(state.launching);
    assert!(
        runtime::task::into_stream(state.update(Message::Submit)).is_none(),
        "repeated Enter while spawning must not spawn again"
    );
    let result = task_output(task);
    assert!(matches!(result, Message::Launched(Ok(()))));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !directory.join("correct").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(directory.join("correct").exists());
    assert!(!directory.join("wrong").exists());
    let exit = state.update(result);
    let mut actions = runtime::task::into_stream(exit).unwrap();
    assert!(matches!(
        iced_test::futures::futures::executor::block_on(actions.next()),
        Some(Action::Exit)
    ));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn no_match_and_empty_catalog_enter_do_not_schedule_a_launch() {
    for apps in [Vec::new(), vec![app("one.desktop", "One")]] {
        let (mut state, boot) = loaded(apps);
        let _ = state.update(Message::Query("unmatched".into()));
        let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), state.view());
        focus(&mut ui, boot);
        assert!(
            ui.find(if state.apps.is_empty() {
                "No applications found"
            } else {
                "No matching applications"
            })
            .is_ok()
        );
        let _ = ui.tap_key(Named::Enter);
        let messages: Vec<_> = ui.into_messages().collect();
        assert_eq!(messages.len(), 1);
        assert!(
            runtime::task::into_stream(state.update(messages.into_iter().next().unwrap()))
                .is_none()
        );
        assert!(!state.launching);
    }
}

#[test]
fn clicking_a_tile_launches_that_tile_instead_of_the_first_result() {
    let (state, _) = loaded(vec![app("one.desktop", "One"), app("two.desktop", "Two")]);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), state.view());
    let _ = ui.click("Two").unwrap();
    let messages: Vec<_> = ui
        .into_messages()
        .filter(|message| !matches!(message, Message::Cursor(_)))
        .collect();
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0], Message::Launch(1)));
}

#[test]
fn failed_launch_keeps_the_screen_and_restores_usable_search() {
    let mut broken = app("broken.desktop", "Broken");
    broken.entry.command = vec!["/definitely/not/an/installed/application".into()];
    let (mut state, _) = loaded(vec![broken]);
    let result = task_output(state.update(Message::Submit));
    assert!(matches!(result, Message::Launched(Err(_))));
    let refocus = state.update(result);
    assert!(!state.launching);
    let rendered = render_state(&state);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), rendered.view());
    focus(&mut ui, refocus);
    assert!(ui.find(state.error.as_ref().unwrap().as_str()).is_ok());
    let _ = ui.typewrite("new");
    for message in ui.into_messages() {
        let _ = state.update(message);
    }
    assert_eq!(state.query, "new");
    assert!(state.error.is_none());
}

#[test]
fn layout_keeps_search_and_tiles_inside_default_and_minimum_windows() {
    let (state, _) = loaded(vec![
        app("google-chrome.desktop", "Chrome"),
        app("one.desktop", "One"),
        app("two.desktop", "Two"),
    ]);
    for size in [Size::new(1000.0, 650.0), Size::new(420.0, 360.0)] {
        let mut ui = Simulator::with_size(Settings::default(), size, state.view());
        let search = ui.find(selector::id(super::SEARCH)).unwrap().bounds();
        assert!(search.x >= 10.0 && search.x + search.width <= size.width - 10.0);
        for label in ["Chrome", "One", "Two"] {
            let bounds = ui.find(label).unwrap().bounds();
            assert!(bounds.width > 0.0 && bounds.x >= 12.0);
            assert!(
                bounds.x + bounds.width <= size.width - 10.0,
                "{label} overflows at {size:?}: {bounds:?}"
            );
        }
    }
}

#[test]
fn many_apps_scroll_to_the_last_tile_and_keep_long_names_inside_columns() {
    let apps: Vec<_> = (0..40)
        .map(|index| {
            app(
                &format!("app-{index:02}.desktop"),
                &format!("Application {index:02} with a longer name"),
            )
        })
        .collect();
    let (state, _) = loaded(apps);
    let mut ui = Simulator::with_size(Settings::default(), (420.0, 360.0), state.view());
    let label = "Application 39 with a longer name";
    assert!(ui.find(label).unwrap().visible_bounds().is_none());
    ui.point_at(Point::new(250.0, 200.0));
    let _ = ui.scroll(iced::mouse::ScrollDelta::Pixels {
        x: 0.0,
        y: -100_000.0,
    });
    ui.draw(&Theme::Dark);
    let last = ui.find(label).unwrap();
    assert!(
        last.visible_bounds().is_some(),
        "last tile must be reachable by scrolling"
    );
    assert!(last.bounds().x + last.bounds().width <= 410.0);
    let _ = ui.click(label).unwrap();
    assert!(
        ui.into_messages()
            .any(|message| matches!(message, Message::Launch(39)))
    );
}

#[test]
fn render_reference_preview() {
    let fixture = [
        ("google-chrome.desktop", "Chrome"),
        ("brave-browser.desktop", "Brave"),
        ("code.desktop", "VS Code"),
        ("org.gnome.Nautilus.desktop", "Files"),
        ("obsidian.desktop", "Obsidian"),
        ("terminal.desktop", "Terminal"),
        ("firefox.desktop", "Firefox"),
        ("editor.desktop", "Text Editor"),
        ("calculator.desktop", "Calculator"),
        ("settings.desktop", "Settings"),
        ("software.desktop", "Software"),
        ("monitor.desktop", "System Monitor"),
        ("disks.desktop", "Disks"),
    ];
    let installed = catalog::scan().unwrap();
    let apps = fixture
        .into_iter()
        .map(|(id, name)| {
            let mut fixture = app(id, name);
            fixture.icon = installed
                .iter()
                .find(|candidate| {
                    candidate.entry.id.eq_ignore_ascii_case(id)
                        || candidate.entry.name.eq_ignore_ascii_case(name)
                })
                .and_then(|candidate| candidate.icon.clone());
            fixture
        })
        .collect();
    let (state, boot) = loaded(apps);
    let mut ui = Simulator::with_size(Settings::default(), (1000.0, 650.0), state.view());
    focus(&mut ui, boot);
    let preview = ui.snapshot(&Theme::Dark).unwrap();
    // Snapshot baselines are normally immutable; render to a fresh directory
    // and publish this inspection artifact after every run instead.
    let directory = PathBuf::from(format!("target/preview-{}", std::process::id()));
    assert!(
        preview
            .matches_image(directory.join("launcher-preview.png"))
            .unwrap()
    );
    for entry in fs::read_dir(&directory).unwrap().flatten() {
        fs::copy(
            entry.path(),
            PathBuf::from("target").join(entry.file_name()),
        )
        .unwrap();
    }
    fs::remove_dir_all(directory).unwrap();
}

fn right_click(ui: &mut Simulator<'_, Message>, label: &str) -> Vec<Message> {
    let position = ui.find(label).unwrap().bounds().center();
    ui.point_at(position);
    let _ = ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
        position,
    })]);
    let _ = ui.simulate([
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
            iced::mouse::Button::Right,
        )),
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
            iced::mouse::Button::Right,
        )),
    ]);
    ui.drain().collect()
}

#[test]
fn right_click_menu_pins_and_unpins_and_persists_across_restart() {
    let directory = std::env::temp_dir().join(format!(
        "baka-context-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let path = directory.join("pins.conf");
    let apps = vec![app("one.desktop", "One"), app("two.desktop", "Two")];
    let (mut state, _) = loaded(apps.clone());
    state.pins = crate::pins::Preferences::load(path.clone()).unwrap();
    let rendered = render_state(&state);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), rendered.view());
    let messages = right_click(&mut ui, "Two");
    assert!(
        messages
            .iter()
            .any(|message| matches!(message, Message::Context(1)))
    );
    assert!(
        !messages
            .iter()
            .any(|message| matches!(message, Message::Launch(_) | Message::Submit))
    );
    for message in messages {
        let _ = state.update(message);
    }
    let menu_state = render_state(&state);
    let mut ui = ui.rebuild(menu_state.view());
    let _ = ui
        .snapshot(&Theme::Dark)
        .unwrap()
        .matches_image(format!("target/pin-menu-{}.png", std::process::id()))
        .unwrap();
    let _ = ui.click("Pin to Pinned").unwrap();
    let mut save = None;
    for message in ui.into_messages() {
        let task = state.update(message);
        if state.saving_pins && save.is_none() {
            save = Some(task);
        }
    }
    assert!(state.saving_pins);
    let result = task_output(save.unwrap());
    assert!(matches!(result, Message::PreferencesSaved(Ok(_))));
    let _ = state.update(result);
    assert!(state.pins.is_pinned(&state.apps[1].entry));
    assert_eq!(
        state.visible,
        [1, 0],
        "empty-query Enter order follows custom pin"
    );
    let (mut restarted, _) = loaded(apps);
    let _ = restarted.update(Message::PreferencesLoaded(Ok(
        crate::pins::Preferences::load(path.clone()).unwrap(),
    )));
    assert_eq!(restarted.visible, [1, 0]);
    let rendered = render_state(&restarted);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), rendered.view());
    assert!(ui.find("Two").unwrap().bounds().y < ui.find("One").unwrap().bounds().y);
    for message in right_click(&mut ui, "Two") {
        let _ = restarted.update(message);
    }
    let menu_state = render_state(&restarted);
    let mut ui = ui.rebuild(menu_state.view());
    let _ = ui.click("Unpin from Pinned").unwrap();
    let message = ui
        .into_messages()
        .find(|message| matches!(message, Message::TogglePin))
        .unwrap();
    let result = task_output(restarted.update(message));
    let _ = restarted.update(result);
    assert!(
        !crate::pins::Preferences::load(path)
            .unwrap()
            .is_pinned(&restarted.apps[1].entry)
    );
    assert_eq!(restarted.visible, [0, 1]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn menu_dismisses_without_launching_and_stays_inside_small_window() {
    let (mut state, _) = loaded(vec![app("one.desktop", "One")]);
    let _ = state.update(Message::Cursor(Point::new(419.0, 359.0)));
    let _ = state.update(Message::Context(0));
    let rendered = render_state(&state);
    let mut ui = Simulator::with_size(Settings::default(), (420.0, 360.0), rendered.view());
    let bounds = ui.find("Pin to Pinned").unwrap().bounds();
    assert!(bounds.x >= 0.0 && bounds.y >= 0.0);
    assert!(bounds.x + bounds.width <= 420.0 && bounds.y + bounds.height <= 360.0);
    ui.point_at(Point::new(40.0, 40.0));
    let _ = ui.simulate(iced_test::simulator::click());
    for message in ui.into_messages() {
        assert!(!matches!(message, Message::Launch(_)));
        let _ = state.update(message);
    }
    assert!(state.context.is_none());
    let _ = state.update(Message::Context(0));
    let close = state.update(Message::Close);
    assert!(state.context.is_none());
    let mut actions = runtime::task::into_stream(close).unwrap();
    assert!(!matches!(
        iced_test::futures::futures::executor::block_on(actions.next()),
        Some(Action::Exit)
    ));
}

#[test]
fn failed_pin_save_keeps_original_pins_and_shows_error_without_launching() {
    let (mut state, _) = loaded(vec![app("one.desktop", "One")]);
    // A default in-memory configuration has no writable path.
    let _ = state.update(Message::Context(0));
    let result = task_output(state.update(Message::TogglePin));
    assert!(matches!(result, Message::PreferencesSaved(Err(_))));
    let _ = state.update(result);
    assert!(!state.pins.is_pinned(&state.apps[0].entry));
    assert!(!state.saving_pins && !state.launching);
    let mut ui = Simulator::with_size(Settings::default(), (420.0, 360.0), state.view());
    let error = ui.find(state.error.as_ref().unwrap().as_str()).unwrap();
    assert!(error.visible_bounds().is_some());
}

#[test]
fn custom_pins_do_not_override_search_ranking_and_default_unpins_sort_alphabetically() {
    let (mut state, _) = loaded(vec![
        app("code.desktop", "Zulu Browser"),
        app("alpha.desktop", "Alpha"),
        app("exact.desktop", "Browser"),
    ]);
    state.pins.set("code.desktop".into(), false);
    state.visible = state.order();
    assert_eq!(state.visible, [1, 2, 0]);
    state.pins.set("code.desktop".into(), true);
    let _ = state.update(Message::Query("browser".into()));
    assert_eq!(state.visible, [2, 0]);
}

fn clipboard_result(
    task: Task<Message>,
    expected: &str,
    status: Result<(), iced::clipboard::Error>,
) -> Message {
    let executor = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    executor.block_on(async {
        let mut actions =
            runtime::task::into_stream(task).expect("copy must schedule a clipboard write");
        let mut status = Some(status);
        let mut writes = 0;
        while let Some(action) = actions.next().await {
            match action {
                Action::Clipboard(runtime::clipboard::Action::Write { content, channel }) => {
                    assert_eq!(
                        content,
                        iced_test::core::clipboard::Content::Text(expected.to_owned())
                    );
                    writes += 1;
                    channel
                        .send(status.take().expect("exactly one clipboard write"))
                        .unwrap();
                }
                Action::Widget(_) => {}
                Action::Output(message) => {
                    assert_eq!(writes, 1);
                    return message;
                }
                _ => panic!("copy must neither launch applications nor exit"),
            }
        }
        panic!("clipboard write did not report its result");
    })
}

#[test]
fn typing_an_expression_shows_the_result_and_enter_copies_instead_of_launching() {
    let (mut state, boot) = State::new();
    let rendered = render_state(&state);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), rendered.view());
    focus(&mut ui, boot);
    let _ = ui.typewrite("2 + 3 * 4");
    for message in ui.drain().collect::<Vec<_>>() {
        let _ = state.update(message);
    }
    assert_eq!(state.calculation.as_deref(), Some("14"));
    assert!(state.visible.is_empty());
    let updated = render_state(&state);
    let mut ui = ui.rebuild(updated.view());
    assert!(ui.find("CALCULATOR").is_ok());
    assert!(ui.find("14").is_ok());
    let _ = ui
        .snapshot(&Theme::Dark)
        .unwrap()
        .matches_image(format!(
            "target/calculator-preview-{}.png",
            std::process::id()
        ))
        .unwrap();
    assert!(ui.find("Loading applications…").is_err());
    // Completing indexing must not replace an already calculated result.
    let _ = state.update(Message::Loaded(Ok(vec![app("match.desktop", "2 + 3 * 4")])));
    assert!(state.visible.is_empty());
    let _ = ui.tap_key(Named::Enter);
    let message = ui
        .into_messages()
        .find(|message| matches!(message, Message::Submit))
        .unwrap();
    let task = state.update(message);
    let result = clipboard_result(task, "14", Ok(()));
    assert!(runtime::task::into_stream(state.update(result)).is_none());
    assert!(state.copied && !state.launching);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), state.view());
    assert!(ui.find("Copied to clipboard").is_ok());
    assert!(ui.find("APPLICATIONS").is_err());
}

#[test]
fn clicking_the_result_copies_its_text_and_reports_clipboard_errors() {
    let (mut state, _) = loaded(Vec::new());
    let _ = state.update(Message::Query("7 % 4".into()));
    let mut ui = Simulator::with_size(Settings::default(), (420.0, 360.0), state.view());
    let bounds = ui.find("3").unwrap().bounds();
    assert!(bounds.x + bounds.width <= 410.0);
    let _ = ui.click("3").unwrap();
    let message = ui
        .into_messages()
        .find(|message| matches!(message, Message::CopyResult))
        .unwrap();
    let task = state.update(message);
    let result = clipboard_result(task, "3", Err(iced::clipboard::Error::ClipboardUnavailable));
    let _ = state.update(result);
    assert!(!state.copied && !state.launching);
    assert_eq!(state.calculation.as_deref(), Some("3"));
    let mut ui = Simulator::with_size(Settings::default(), (420.0, 360.0), state.view());
    assert!(ui.find("Could not copy result to the clipboard").is_ok());
    let _ = ui.click("3").unwrap();
    let message = ui
        .into_messages()
        .find(|message| matches!(message, Message::CopyResult))
        .unwrap();
    let task = state.update(message);
    let result = clipboard_result(task, "3", Ok(()));
    let _ = state.update(result);
    assert!(state.copied && state.error.is_none());
}

#[test]
fn evaluation_failure_restores_app_search_and_ignores_old_clipboard_feedback() {
    let (mut state, _) = loaded(vec![
        app("helper.desktop", "2 + utility"),
        app("firefox.desktop", "Firefox"),
    ]);
    let _ = state.update(Message::Query("2 + 2".into()));
    assert_eq!(state.calculation.as_deref(), Some("4"));
    let _ = state.update(Message::Query("2 +".into()));
    assert!(state.calculation.is_none());
    assert_eq!(state.visible, [0]);
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), state.view());
    assert!(ui.find("2 + utility").is_ok());
    assert!(ui.find("CALCULATOR").is_err());
    drop(ui);
    let _ = state.update(Message::Query("firefox".into()));
    assert_eq!(state.visible, [1]);
    let _ = state.update(Message::CopyFinished {
        result: "4".into(),
        status: Ok(()),
    });
    assert!(!state.copied);
    assert!(runtime::task::into_stream(state.update(Message::CopyResult)).is_none());
    let _ = state.update(Message::Query(String::new()));
    assert!(state.calculation.is_none());
    assert_eq!(state.visible.len(), 2);
}

#[test]
fn losing_window_focus_exits_even_with_a_menu_or_calculation_open() {
    for mode in 0..3 {
        let (mut state, _) = loaded(vec![app("one.desktop", "One")]);
        match mode {
            1 => {
                let _ = state.update(Message::Context(0));
            }
            2 => {
                let _ = state.update(Message::Query("2 + 2".into()));
            }
            _ => {}
        }
        let task = state.update(Message::Unfocused);
        let mut actions = runtime::task::into_stream(task).unwrap();
        assert!(matches!(
            iced_test::futures::futures::executor::block_on(actions.next()),
            Some(Action::Exit)
        ));
    }
}

#[test]
fn compact_grid_fits_six_apps_per_row_with_two_line_label_space() {
    let labels = [
        "Audio Player",
        "Bluetooth Manager",
        "Color Profiles",
        "Connections",
        "Google Chrome",
        "System Monitor",
    ];
    let (state, _) = loaded(
        labels
            .iter()
            .enumerate()
            .map(|(index, label)| app(&format!("fixture-{index}.desktop"), label))
            .collect(),
    );
    let mut ui = Simulator::with_size(Settings::default(), (1000.0, 650.0), state.view());
    let first = ui.find(labels[0]).unwrap().bounds();
    for label in labels {
        let bounds = ui.find(label).unwrap().bounds();
        assert_eq!(bounds.y, first.y, "six apps should share a row");
        assert!(bounds.width >= 120.0 && bounds.height >= 36.0);
        assert!(bounds.x >= 12.0 && bounds.x + bounds.width <= 988.0);
    }
}
