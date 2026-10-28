fn main() -> iced::Result {
    use baka_launcher::launcher;

    iced::application(
        launcher::State::new,
        launcher::State::update,
        launcher::State::view,
    )
    .title("Baka Launcher")
    .font(iced::Font::new("Fira Sans"))
    .theme(launcher::theme)
    .subscription(launcher::State::subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(1000.0, 650.0),
        // Matching size limits identify this launcher as a floating dialog
        // to tiling compositors, including COSMIC.
        min_size: Some(iced::Size::new(1000.0, 650.0)),
        max_size: Some(iced::Size::new(1000.0, 650.0)),
        resizable: false,
        #[cfg(target_os = "linux")]
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "baka-launcher".into(),
            ..Default::default()
        },
        position: iced::window::Position::Centered,
        decorations: false,
        ..Default::default()
    })
    .run()
}
