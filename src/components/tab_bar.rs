use freya::prelude::*;
use freya::radio::*;

use crate::state::{AppChannel, TabId};

#[derive(PartialEq, Clone)]
pub struct TabBar;

impl Component for TabBar {
    fn render(&self) -> impl IntoElement {
        let mut radio = use_radio(AppChannel::Tabs);

        let tabs: Vec<(TabId, String, bool)> = {
            let state = radio.read();
            state
                .tabs
                .iter()
                .enumerate()
                .map(|(i, t)| (t.id, t.title.clone(), i == state.active_tab))
                .collect()
        };

        rect()
            .width(Size::fill())
            .height(Size::px(36.))
            .background((20, 20, 20))
            .padding(4.)
            .spacing(4.)
            .direction(Direction::Horizontal)
            .children(tabs.into_iter().map(|(tab_id, title, is_active)| {
                TabButton {
                    tab_id,
                    title,
                    is_active,
                }
                .into_element()
            }))
            .child(
                rect()
                    .width(Size::px(36.))
                    .height(Size::fill())
                    .center()
                    .corner_radius(4.)
                    .color((180, 180, 180))
                    .on_mouse_up(move |_: Event<MouseEventData>| {
                        radio.write_channel(AppChannel::Tabs).new_tab();
                    })
                    .child("+"),
            )
    }
}

#[derive(PartialEq, Clone)]
struct TabButton {
    tab_id: TabId,
    title: String,
    is_active: bool,
}

impl Component for TabButton {
    fn render(&self) -> impl IntoElement {
        let tab_id = self.tab_id;
        let is_active = self.is_active;
        let mut radio = use_radio(AppChannel::Tabs);

        let background: Color = if is_active {
            (35, 35, 35).into()
        } else {
            (25, 25, 25).into()
        };
        let text_color: Color = if is_active {
            (230, 230, 230).into()
        } else {
            (140, 140, 140).into()
        };

        Button::new()
            .height(Size::fill())
            .padding((4., 12., 4., 12.))
            .flat()
            .rounded_full()
            .background(background)
            .hover_background((45, 45, 45))
            .color(text_color)
            .on_press(move |_: Event<PressEventData>| {
                let mut state = radio.write_channel(AppChannel::Tabs);
                if let Some(idx) = state.tabs.iter().position(|t| t.id == tab_id) {
                    state.active_tab = idx;
                }
            })
            .child(
                rect()
                    .horizontal()
                    .cross_align(Alignment::Center)
                    .spacing(4.)
                    .child(
                        label()
                            .text(self.title.clone())
                            .text_overflow(TextOverflow::Ellipsis),
                    )
                    .child(
                        Button::new()
                            .flat()
                            .width(Size::px(20.))
                            .height(Size::px(20.))
                            .compact()
                            .rounded_full()
                            .on_press(move |e: Event<PressEventData>| {
                                e.stop_propagation();
                                let mut state = radio.write_channel(AppChannel::Tabs);
                                if let Some(idx) = state.tabs.iter().position(|t| t.id == tab_id) {
                                    if state.tabs.len() > 1 {
                                        state.tabs.remove(idx);
                                        if state.active_tab >= state.tabs.len() {
                                            state.active_tab = state.tabs.len() - 1;
                                        }
                                        if let Some(tab) = state.tabs.get(state.active_tab) {
                                            Focus::new_for_id(tab.active_panel).request_focus();
                                        }
                                    }
                                }
                            })
                            .child(label().text("X").font_size(14.)),
                    ),
            )
    }

    fn render_key(&self) -> DiffKey {
        DiffKey::from(&self.tab_id.0)
    }
}
