use iced::widget::{
    Space, Stack, button, column, container, image as iced_image, mouse_area, responsive, row,
    text as iced_text, toggler,
};
use iced::{ContentFit, Element, Fill, Length, Padding, Size};

mod translated;

use translated::translate_x;

use crate::{
    app::{Message, State, transition_offset},
    theme,
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
    let background = pager_background(state, available);
    let controls = controls(state);

    Stack::new()
        .push(background)
        .push(controls)
        .width(Fill)
        .height(Fill)
        .into()
}

/// Builds the centered status view shown while startup is in progress.
fn loading_view(state: &State) -> Element<'_, Message> {
    container(iced_text(state.status.resolve()).size(dimension!(text_standalone)))
        .padding(dimension!(standalone_padding))
        .center(Fill)
        .style(theme::fallback_background)
        .into()
}

/// Builds the message shown when the current desktop is unsupported.
fn unsupported_view(state: &State) -> Element<'_, Message> {
    container(iced_text(state.status.resolve()).size(dimension!(text_standalone)))
        .padding(dimension!(standalone_padding))
        .center(Fill)
        .style(theme::fallback_background)
        .into()
}

/// Builds the selected wallpaper and its adjacent page at full-window size.
fn pager_background(state: &State, available: Size) -> Element<'_, Message> {
    let width = available.width.max(1.0);
    let height = available.height.max(1.0);
    let offset = transition_offset(state);
    let (center, neighbor) = visible_pages(state, offset);

    let mut pages = Stack::new().push(translated_page(
        preview_image(state.preview_handle(center)),
        offset * width,
    ));
    if let Some(neighbor) = neighbor {
        let direction = if neighbor > center { 1.0 } else { -1.0 };
        pages = pages.push(translated_page(
            preview_image(state.preview_handle(neighbor)),
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
fn controls(state: &State) -> Element<'_, Message> {
    let top = top_controls(state);
    let arrows = navigation_controls(state);
    let bottom = bottom_controls(state);

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
fn top_controls(state: &State) -> Element<'_, Message> {
    let daily_toggle = toggler(state.settings.daily_change)
        .label(text!(daily_change))
        .size(dimension!(toggle_size))
        .text_size(dimension!(text_label))
        .spacing(dimension!(toggle_spacing))
        .on_toggle_maybe((!state.busy).then_some(Message::ToggleDaily));

    container(row![Space::new().width(Fill), daily_toggle].align_y(iced::Center))
        .padding([
            dimension!(top_padding_vertical),
            dimension!(top_padding_horizontal),
        ])
        .width(Fill)
        .height(dimension!(top_overlay_height))
        .style(theme::top_scrim)
        .into()
}

/// Builds the previous and next placeholder buttons centered on the window edges.
fn navigation_controls(state: &State) -> Element<'_, Message> {
    let motion_idle = state.transition.is_none();
    let previous =
        button(iced_text(image!(previous).placeholder()).size(dimension!(text_navigation_icon)))
            .padding([
                dimension!(navigation_button_padding_vertical),
                dimension!(navigation_button_padding_horizontal),
            ])
            .style(theme::edge_navigation)
            .on_press_maybe(
                (state.selected > 0 && !state.busy && motion_idle).then_some(Message::Previous),
            );
    let next = button(iced_text(image!(next).placeholder()).size(dimension!(text_navigation_icon)))
        .padding([
            dimension!(navigation_button_padding_vertical),
            dimension!(navigation_button_padding_horizontal),
        ])
        .style(theme::edge_navigation)
        .on_press_maybe(
            (state.selected + 1 < state.entries.len() && !state.busy && motion_idle)
                .then_some(Message::Next),
        );

    container(
        row![previous, Space::new().width(Fill), next]
            .spacing(dimension!(navigation_spacing))
            .align_y(iced::Center),
    )
    .padding([0.0, dimension!(navigation_horizontal_inset)])
    .width(Fill)
    .height(Fill)
    .center_y(Fill)
    .into()
}

/// Builds the metadata, actions, and status overlay at the bottom of the image.
fn bottom_controls(state: &State) -> Element<'_, Message> {
    let details = selected_details(state);
    let set_button = button(iced_text(text!(set_wallpaper)).size(dimension!(text_label)))
        .padding([
            dimension!(action_button_padding_vertical),
            dimension!(action_button_padding_horizontal),
        ])
        .style(theme::primary_action)
        .on_press_maybe(
            (state.selected_preview_is_ready() && !state.busy).then_some(Message::SetWallpaper),
        );
    let refresh_button = button(iced_text(text!(refresh)).size(dimension!(text_label)))
        .padding([
            dimension!(action_button_padding_vertical),
            dimension!(action_button_padding_horizontal),
        ])
        .style(theme::secondary_action)
        .on_press_maybe((!state.busy).then_some(Message::Refresh));

    container(
        column![
            details,
            row![set_button, refresh_button].spacing(dimension!(action_spacing)),
            iced_text(state.status.resolve()).size(dimension!(text_status))
        ]
        .spacing(dimension!(action_spacing)),
    )
    .padding(Padding {
        top: dimension!(bottom_padding_top),
        right: dimension!(bottom_padding_horizontal),
        bottom: dimension!(bottom_padding_bottom),
        left: dimension!(bottom_padding_horizontal),
    })
    .width(Fill)
    .style(theme::bottom_scrim)
    .into()
}

/// Builds the date, position, and description for the selected wallpaper.
fn selected_details(state: &State) -> Element<'_, Message> {
    let Some(entry) = state.entries.get(state.selected) else {
        return iced_text(text!(loading_feed))
            .size(dimension!(text_loading))
            .into();
    };
    column![
        row![
            iced_text(&entry.date).size(dimension!(text_label)),
            Space::new().width(Fill),
            iced_text(text!(page_counter, state.selected + 1, state.entries.len()))
                .size(dimension!(text_counter)),
        ],
        iced_text(&entry.description).size(dimension!(text_description)),
    ]
    .spacing(dimension!(metadata_spacing))
    .into()
}

/// Displays an allocated image handle or a localized full-window loading placeholder.
fn preview_image(handle: Option<iced::widget::image::Handle>) -> Element<'static, Message> {
    match handle {
        Some(handle) => iced_image(handle)
            .width(Fill)
            .height(Fill)
            .content_fit(ContentFit::Cover)
            .into(),
        None => container(iced_text(text!(loading_preview)))
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Fill)
            .style(theme::fallback_background)
            .into(),
    }
}
