use std::f32::consts::PI;

use iced::gradient;
use iced::widget::{
    Space, Stack, button, column, container, image, mouse_area, responsive, row, text, toggler,
};
use iced::{Background, Color, ContentFit, Element, Fill, Length, Padding, Size, Theme};

mod translated;

use translated::translate_x;

use crate::{
    app::{BASE_WIDTH, Message, State, transition_offset},
    locale::{Locale, TextKey},
};

/// Builds the application's current view from its state.
pub(crate) fn view(state: &State) -> Element<'_, Message> {
    if state.initializing {
        return loading_view(state);
    }
    if state.desktop.is_none() {
        return unsupported_view(state);
    }

    responsive(move |available| immersive_view(state, available))
        .width(Fill)
        .height(Fill)
        .into()
}

/// Builds the full-bleed wallpaper view and its floating controls.
fn immersive_view(state: &State, available: Size) -> Element<'_, Message> {
    let scale = (available.width / BASE_WIDTH).max(1.0);
    let background = pager_background(state, available);
    let controls = controls(state, scale);

    Stack::new()
        .push(background)
        .push(controls)
        .width(Fill)
        .height(Fill)
        .into()
}

/// Builds the centered status view shown while startup is in progress.
fn loading_view(state: &State) -> Element<'_, Message> {
    container(text(&state.status).size(18))
        .padding(32)
        .center(Fill)
        .style(dark_background)
        .into()
}

/// Builds the message shown when the current desktop is unsupported.
fn unsupported_view(state: &State) -> Element<'_, Message> {
    container(text(&state.status).size(18))
        .padding(32)
        .center(Fill)
        .style(dark_background)
        .into()
}

/// Builds the selected wallpaper and its adjacent page at full-window size.
fn pager_background(state: &State, available: Size) -> Element<'_, Message> {
    let width = available.width.max(1.0);
    let height = available.height.max(1.0);
    let offset = transition_offset(state);
    let (center, neighbor) = visible_pages(state, offset);
    let locale = state.locale;

    let mut pages = Stack::new().push(translated_page(
        preview_image(state.preview_handle(center), locale),
        offset * width,
    ));
    if let Some(neighbor) = neighbor {
        let direction = if neighbor > center { 1.0 } else { -1.0 };
        pages = pages.push(translated_page(
            preview_image(state.preview_handle(neighbor), locale),
            (offset + direction) * width,
        ));
    }

    let pages = container(pages.width(width).height(height).clip(true))
        .width(width)
        .height(height)
        .clip(true);

    mouse_area(pages)
        .on_move(Message::PagerPointerMoved)
        .on_press(Message::PagerPressed(width))
        .on_release(Message::PagerReleased)
        .interaction(iced::mouse::Interaction::Grabbing)
        .into()
}

/// Translates a fully laid-out page without changing its image bounds.
fn translated_page(page: Element<'static, Message>, x: f32) -> Element<'static, Message> {
    translate_x(page, x)
}

/// Chooses the centered page and the adjacent page exposed by the current offset.
fn visible_pages(state: &State, offset: f32) -> (usize, Option<usize>) {
    if let Some(transition) = &state.transition
        && transition.from != transition.to
    {
        return (transition.from, Some(transition.to));
    }

    let neighbor = if offset < 0.0 {
        state.selected.checked_add(1)
    } else if offset > 0.0 {
        state.selected.checked_sub(1)
    } else {
        None
    }
    .filter(|index| *index < state.entries.len());
    (state.selected, neighbor)
}

/// Builds the top setting, edge navigation, and bottom metadata overlays.
fn controls(state: &State, scale: f32) -> Element<'_, Message> {
    let top = top_controls(state, scale);
    let arrows = navigation_controls(state, scale);
    let bottom = bottom_controls(state, scale);

    Stack::new()
        .push(
            column![top, Space::new().height(Fill), bottom]
                .width(Fill)
                .height(Fill),
        )
        .push(arrows)
        .width(Fill)
        .height(Fill)
        .into()
}

/// Builds the title-free top overlay containing only the daily-change setting.
fn top_controls(state: &State, scale: f32) -> Element<'_, Message> {
    let daily_toggle = toggler(state.settings.daily_change)
        .label(state.locale.text(TextKey::DailyChange))
        .size(22.0 * scale)
        .text_size(16.0 * scale)
        .spacing(10.0 * scale)
        .on_toggle_maybe((!state.busy).then_some(Message::ToggleDaily));

    container(row![Space::new().width(Fill), daily_toggle].align_y(iced::Center))
        .padding([24.0 * scale, 32.0 * scale])
        .width(Fill)
        .height(104.0 * scale)
        .style(top_gradient)
        .into()
}

/// Builds the previous and next buttons centered on the window edges.
fn navigation_controls(state: &State, scale: f32) -> Element<'_, Message> {
    let motion_idle = state.transition.is_none();
    let previous = button(text("‹").size(38.0 * scale))
        .padding([8.0 * scale, 16.0 * scale])
        .style(edge_button)
        .on_press_maybe(
            (state.selected > 0 && !state.busy && motion_idle).then_some(Message::Previous),
        );
    let next = button(text("›").size(38.0 * scale))
        .padding([8.0 * scale, 16.0 * scale])
        .style(edge_button)
        .on_press_maybe(
            (state.selected + 1 < state.entries.len() && !state.busy && motion_idle)
                .then_some(Message::Next),
        );

    container(
        row![previous, Space::new().width(Fill), next]
            .spacing(16.0 * scale)
            .align_y(iced::Center),
    )
    .padding([0.0, 24.0 * scale])
    .width(Fill)
    .height(Fill)
    .center_y(Fill)
    .into()
}

/// Builds the metadata, actions, and status overlay at the bottom of the image.
fn bottom_controls(state: &State, scale: f32) -> Element<'_, Message> {
    let details = selected_details(state, scale);
    let set_button = button(text(state.locale.text(TextKey::SetWallpaper)).size(16.0 * scale))
        .padding([10.0 * scale, 16.0 * scale])
        .style(button::primary)
        .on_press_maybe(
            (state.selected_preview_is_ready() && !state.busy).then_some(Message::SetWallpaper),
        );
    let refresh_button = button(text(state.locale.text(TextKey::Refresh)).size(16.0 * scale))
        .padding([10.0 * scale, 16.0 * scale])
        .style(button::secondary)
        .on_press_maybe((!state.busy).then_some(Message::Refresh));

    container(
        column![
            details,
            row![set_button, refresh_button].spacing(12.0 * scale),
            text(&state.status).size(14.0 * scale)
        ]
        .spacing(12.0 * scale),
    )
    .padding(Padding {
        top: 48.0 * scale,
        right: 32.0 * scale,
        bottom: 24.0 * scale,
        left: 32.0 * scale,
    })
    .width(Fill)
    .style(bottom_gradient)
    .into()
}

/// Builds the date, position, and description for the selected wallpaper.
fn selected_details(state: &State, scale: f32) -> Element<'_, Message> {
    let Some(entry) = state.entries.get(state.selected) else {
        return text(state.locale.text(TextKey::LoadingFeed))
            .size(16.0 * scale)
            .into();
    };
    column![
        row![
            text(&entry.date).size(16.0 * scale),
            Space::new().width(Fill),
            text(format!("{} / {}", state.selected + 1, state.entries.len())).size(14.0 * scale),
        ],
        text(&entry.description).size(20.0 * scale),
    ]
    .spacing(6.0 * scale)
    .into()
}

/// Displays an allocated image handle or a localized full-window loading placeholder.
fn preview_image(
    handle: Option<iced::widget::image::Handle>,
    locale: Locale,
) -> Element<'static, Message> {
    match handle {
        Some(handle) => image(handle)
            .width(Fill)
            .height(Fill)
            .content_fit(ContentFit::Cover)
            .into(),
        None => container(text(locale.text(TextKey::LoadingPreview)))
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Fill)
            .style(dark_background)
            .into(),
    }
}

/// Paints a dark fallback behind loading and unsupported states.
fn dark_background(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(Color::WHITE),
        background: Some(Background::Color(Color::from_rgb8(18, 18, 18))),
        ..container::Style::default()
    }
}

/// Paints a top-to-transparent scrim behind the daily-change setting.
fn top_gradient(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(Color::WHITE),
        background: Some(
            gradient::Linear::new(PI)
                .add_stop(0.0, Color::from_rgba8(0, 0, 0, 0.72))
                .add_stop(1.0, Color::TRANSPARENT)
                .into(),
        ),
        ..container::Style::default()
    }
}

/// Paints a transparent-to-bottom scrim behind metadata and actions.
fn bottom_gradient(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(Color::WHITE),
        background: Some(
            gradient::Linear::new(PI)
                .add_stop(0.0, Color::TRANSPARENT)
                .add_stop(0.45, Color::from_rgba8(0, 0, 0, 0.42))
                .add_stop(1.0, Color::from_rgba8(0, 0, 0, 0.82))
                .into(),
        ),
        ..container::Style::default()
    }
}

/// Paints compact translucent navigation buttons over the image.
fn edge_button(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::secondary(theme, status);
    style.text_color = Color::WHITE;
    style.background = Some(Background::Color(match status {
        button::Status::Hovered => Color::from_rgba8(0, 0, 0, 0.72),
        button::Status::Disabled => Color::from_rgba8(0, 0, 0, 0.18),
        button::Status::Active | button::Status::Pressed => Color::from_rgba8(0, 0, 0, 0.52),
    }));
    style.border.radius = 6.0.into();
    style
}
