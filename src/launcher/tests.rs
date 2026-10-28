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
    let messages: Vec<_> = ui.into_messages().collect();
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
    for size in [Size::new(1200.0, 760.0), Size::new(420.0, 360.0)] {
        let mut ui = Simulator::with_size(Settings::default(), size, state.view());
        let search = ui.find(selector::id(super::SEARCH)).unwrap().bounds();
        assert!(search.x >= 10.0 && search.x + search.width <= size.width - 10.0);
        for label in ["Chrome", "One", "Two"] {
            let bounds = ui.find(label).unwrap().bounds();
            assert!(bounds.width > 0.0 && bounds.x >= 72.0);
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
    let mut ui = Simulator::with_size(Settings::default(), (1200.0, 760.0), state.view());
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
