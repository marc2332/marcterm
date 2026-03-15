mod components;
mod config;
mod state;

use components::app::App;
use config::Config;
use freya::prelude::*;

/// Fall back to the Adwaita cursor theme when the host theme isn't reachable
/// inside the Flatpak sandbox (causes an invisible cursor on Wayland).
#[cfg(target_os = "linux")]
fn fix_flatpak_cursor_theme() {
    if std::env::var("FLATPAK_ID").is_err() {
        return;
    }

    let theme_name = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".into());

    if xcursor::CursorTheme::load(&theme_name)
        .load_icon("left_ptr")
        .is_none()
    {
        // SAFETY: called before any other threads are spawned.
        unsafe {
            std::env::set_var("XCURSOR_THEME", "Adwaita");
        }
    }

    if std::env::var("XCURSOR_SIZE").is_err() {
        // SAFETY: called before any other threads are spawned.
        unsafe {
            std::env::set_var("XCURSOR_SIZE", "24");
        }
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    fix_flatpak_cursor_theme();

    let config = Config::load();

    launch(
        LaunchConfig::new().with_window(
            WindowConfig::new(move || App {
                font_size: config.font_size,
                shell: config.shell.clone(),
            })
            .with_title("marcterm")
            .with_size(1024., 768.)
            .with_icon(LaunchConfig::window_icon(include_bytes!("../icon.png"))),
        ),
    );
}
