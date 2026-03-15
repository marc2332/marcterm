use std::time::{Duration, Instant};

use async_io::Timer;
use freya::prelude::*;
use freya::radio::*;
use freya::terminal::{TerminalHandle, TerminalId};

use crate::{
    components::{tab_bar::TabBar, tab_content::TabContent},
    state::{AppChannel, AppState, NavDirection, TabId},
};

enum WatchResult {
    TitleChanged(TabId, String),
    Closed(TerminalId),
    OutputReceived(TabId),
}

async fn watch_handle(tab_id: TabId, handle: TerminalHandle) -> WatchResult {
    let id = handle.id();
    let h1 = handle.clone();
    let h2 = handle.clone();
    futures::future::select_all([
        Box::pin(async move {
            h1.title_changed().await;
            WatchResult::TitleChanged(tab_id, h1.title().unwrap_or_default())
        }) as std::pin::Pin<Box<dyn futures::Future<Output = WatchResult>>>,
        Box::pin(async move {
            h2.closed().await;
            WatchResult::Closed(id)
        }),
        Box::pin(async move {
            handle.output_received().await;
            WatchResult::OutputReceived(tab_id)
        }),
    ])
    .await
    .0
}

#[derive(PartialEq, Clone)]
pub struct App {
    pub font_size: f32,
    pub shell: String,
}

impl Component for App {
    fn render(&self) -> impl IntoElement {
        let font_size = self.font_size;
        let shell = self.shell.clone();

        use_init_theme(|| DARK_THEME);
        use_init_radio_station::<AppState, AppChannel>(move || {
            AppState::new(font_size, shell.clone())
        });

        let mut radio = use_radio(AppChannel::Tabs);

        // Watch for title changes, terminal closures, and output activity.
        use_future(move || async move {
            let idle = Duration::from_secs(1);
            let mut closed_ids = std::collections::HashSet::<TerminalId>::new();
            let mut last_output: std::collections::HashMap<TabId, Instant> =
                std::collections::HashMap::new();

            loop {
                let watchers: Vec<_> = {
                    let state = radio.read();
                    state
                        .tabs
                        .iter()
                        .flat_map(|tab| {
                            let tab_id = tab.id;
                            tab.panels
                                .all_handles()
                                .into_iter()
                                .filter(|h| !closed_ids.contains(&h.id()))
                                .map(move |h| Box::pin(watch_handle(tab_id, h)))
                        })
                        .collect()
                };

                if watchers.is_empty() {
                    Timer::after(Duration::from_millis(100)).await;
                    continue;
                }

                // Race all handle watchers against the idle timeout.
                match futures::future::select(
                    Box::pin(futures::future::select_all(watchers)),
                    Box::pin(Timer::after(idle)),
                )
                .await
                {
                    futures::future::Either::Left(((result, _, _), _)) => match result {
                        WatchResult::TitleChanged(tab_id, title) if !title.is_empty() => {
                            if let Some(tab) = radio
                                .write_channel(AppChannel::Tabs)
                                .tabs
                                .iter_mut()
                                .find(|t| t.id == tab_id)
                            {
                                tab.title = title;
                            }
                        }
                        WatchResult::Closed(terminal_id) => {
                            closed_ids.insert(terminal_id);
                        }
                        WatchResult::OutputReceived(tab_id) => {
                            last_output.insert(tab_id, Instant::now());
                            let state = radio.read();
                            if state.tabs.iter().any(|t| t.id == tab_id && !t.outputting) {
                                drop(state);
                                if let Some(tab) = radio
                                    .write_channel(AppChannel::Tabs)
                                    .tabs
                                    .iter_mut()
                                    .find(|t| t.id == tab_id)
                                {
                                    tab.outputting = true;
                                }
                            }
                        }
                        _ => {}
                    },
                    futures::future::Either::Right(_) => {}
                }

                // Sweep stale outputting flags.
                let now = Instant::now();
                let is_stale = |tab: &crate::state::Tab| {
                    tab.outputting
                        && last_output
                            .get(&tab.id)
                            .map(|t| now.duration_since(*t) > idle)
                            .unwrap_or(true)
                };

                let state = radio.read();
                if state.tabs.iter().any(|t| is_stale(t)) {
                    drop(state);
                    let mut state = radio.write_channel(AppChannel::Tabs);
                    for tab in &mut state.tabs {
                        if is_stale(tab) {
                            tab.outputting = false;
                        }
                    }
                    last_output.retain(|id, _| state.tabs.iter().any(|t| t.id == *id));
                }
            }
        });

        rect()
            .expanded()
            .background((15, 15, 15))
            .color((220, 220, 220))
            .direction(Direction::Vertical)
            .on_key_down(move |e: Event<KeyboardEventData>| {
                let mods = e.modifiers;
                let ctrl = mods.contains(Modifiers::CONTROL);
                let ctrl_shift = mods.contains(Modifiers::CONTROL | Modifiers::SHIFT);
                let alt = mods.contains(Modifiers::ALT);

                match &e.key {
                    Key::Character(ch) if ctrl_shift && ch.eq_ignore_ascii_case("t") => {
                        radio.write_channel(AppChannel::Tabs).new_tab();
                    }
                    Key::Character(ch) if ctrl_shift && ch.eq_ignore_ascii_case("w") => {
                        radio.write_channel(AppChannel::Tabs).close_active_tab();
                    }
                    Key::Named(NamedKey::Tab) if ctrl && !mods.contains(Modifiers::SHIFT) => {
                        radio.write_channel(AppChannel::Tabs).next_tab();
                    }
                    Key::Named(NamedKey::Tab) if ctrl_shift => {
                        radio.write_channel(AppChannel::Tabs).prev_tab();
                    }
                    Key::Character(ch) if alt && ch.eq_ignore_ascii_case("p") => {
                        radio.write_channel(AppChannel::Tabs).split_vertical();
                    }
                    Key::Character(ch) if alt && (ch == "+" || ch == "=") => {
                        radio.write_channel(AppChannel::Tabs).split_horizontal();
                    }
                    Key::Character(ch) if alt && ch == "-" => {
                        radio.write_channel(AppChannel::Tabs).close_active_panel();
                    }
                    Key::Character(ch) if alt && ch == "4" => {
                        radio.write_channel(AppChannel::Tabs).split_into_grid();
                    }
                    Key::Character(ch) if alt && ch == "1" => {
                        radio
                            .write_channel(AppChannel::Tabs)
                            .close_all_except_active();
                    }
                    Key::Named(NamedKey::ArrowLeft) if alt => {
                        radio
                            .write_channel(AppChannel::Tabs)
                            .navigate(NavDirection::Left);
                    }
                    Key::Named(NamedKey::ArrowRight) if alt => {
                        radio
                            .write_channel(AppChannel::Tabs)
                            .navigate(NavDirection::Right);
                    }
                    Key::Named(NamedKey::ArrowUp) if alt => {
                        radio
                            .write_channel(AppChannel::Tabs)
                            .navigate(NavDirection::Up);
                    }
                    Key::Named(NamedKey::ArrowDown) if alt => {
                        radio
                            .write_channel(AppChannel::Tabs)
                            .navigate(NavDirection::Down);
                    }
                    Key::Character(ch) if ctrl && (ch == "+" || ch == "=") => {
                        radio.write_channel(AppChannel::Tabs).increase_font_size();
                    }
                    Key::Character(ch) if ctrl && ch == "-" => {
                        radio.write_channel(AppChannel::Tabs).decrease_font_size();
                    }
                    _ => {}
                }
            })
            .child(
                ResizableContainer::new()
                    .direction(Direction::Horizontal)
                    .panel(ResizablePanel::new(15.).child(TabBar))
                    .panel(ResizablePanel::new(85.).child(TabContent)),
            )
    }
}
