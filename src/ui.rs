use iced::widget::{
    Space, Stack, button, column, container, image, mouse_area, responsive, row, text, toggler,
};
use iced::{ContentFit, Element, Fill, Length, Padding, Size};

mod translated;

use translated::translate_x;

use crate::{
    app::{BASE_WIDTH, Message, State, transition_offset},
    resources::{AppTheme, DimensionToken, IconId, ResourceContext, TextKey, TextSizeToken},
    theme,
};

/// Builds the application's current view from its state.
pub(crate) fn view(state: &State) -> Element<'_, Message> {
    let resources = ResourceContext::new(state.locale, AppTheme::Dark, 1.0, 1.0);
    if state.initializing {
        return loading_view(state, resources);
    }
    if state.desktop.is_none() {
        return unsupported_view(state, resources);
    }

    responsive(move |available| immersive_view(state, available))
        .width(Fill)
        .height(Fill)
        .into()
}

/// Builds the full-bleed wallpaper view and its floating controls.
fn immersive_view(state: &State, available: Size) -> Element<'_, Message> {
    let scale = (available.width / BASE_WIDTH).max(1.0);
    let resources = ResourceContext::new(state.locale, AppTheme::Dark, scale, scale);
    let background = pager_background(state, available, resources);
    let controls = controls(state, resources);

    Stack::new()
        .push(background)
        .push(controls)
        .width(Fill)
        .height(Fill)
        .into()
}

/// Builds the centered status view shown while startup is in progress.
fn loading_view(state: &State, resources: ResourceContext) -> Element<'_, Message> {
    container(text(&state.status).size(resources.text_size(TextSizeToken::Standalone)))
        .padding(resources.dimension(DimensionToken::StandalonePadding))
        .center(Fill)
        .style(move |iced_theme| theme::fallback_background(resources, iced_theme))
        .into()
}

/// Builds the message shown when the current desktop is unsupported.
fn unsupported_view(state: &State, resources: ResourceContext) -> Element<'_, Message> {
    container(text(&state.status).size(resources.text_size(TextSizeToken::Standalone)))
        .padding(resources.dimension(DimensionToken::StandalonePadding))
        .center(Fill)
        .style(move |iced_theme| theme::fallback_background(resources, iced_theme))
        .into()
}

/// Builds the selected wallpaper and its adjacent page at full-window size.
fn pager_background(
    state: &State,
    available: Size,
    resources: ResourceContext,
) -> Element<'_, Message> {
    let width = available.width.max(1.0);
    let height = available.height.max(1.0);
    let offset = transition_offset(state);
    let (center, neighbor) = visible_pages(state, offset);

    let mut pages = Stack::new().push(translated_page(
        preview_image(state.preview_handle(center), resources),
        offset * width,
    ));
    if let Some(neighbor) = neighbor {
        let direction = if neighbor > center { 1.0 } else { -1.0 };
        pages = pages.push(translated_page(
            preview_image(state.preview_handle(neighbor), resources),
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
fn controls(state: &State, resources: ResourceContext) -> Element<'_, Message> {
    let top = top_controls(state, resources);
    let arrows = navigation_controls(state, resources);
    let bottom = bottom_controls(state, resources);

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
fn top_controls(state: &State, resources: ResourceContext) -> Element<'_, Message> {
    let daily_toggle = toggler(state.settings.daily_change)
        .label(resources.text(TextKey::DailyChange))
        .size(resources.dimension(DimensionToken::ToggleSize))
        .text_size(resources.text_size(TextSizeToken::Label))
        .spacing(resources.dimension(DimensionToken::ToggleSpacing))
        .on_toggle_maybe((!state.busy).then_some(Message::ToggleDaily));

    container(row![Space::new().width(Fill), daily_toggle].align_y(iced::Center))
        .padding([
            resources.dimension(DimensionToken::TopPaddingVertical),
            resources.dimension(DimensionToken::TopPaddingHorizontal),
        ])
        .width(Fill)
        .height(resources.dimension(DimensionToken::TopOverlayHeight))
        .style(move |iced_theme| theme::top_scrim(resources, iced_theme))
        .into()
}

/// Builds the previous and next placeholder buttons centered on the window edges.
fn navigation_controls(state: &State, resources: ResourceContext) -> Element<'_, Message> {
    let motion_idle = state.transition.is_none();
    let previous = button(
        text(resources.icon(IconId::Previous))
            .size(resources.text_size(TextSizeToken::NavigationIcon)),
    )
    .padding([
        resources.dimension(DimensionToken::NavigationButtonPaddingVertical),
        resources.dimension(DimensionToken::NavigationButtonPaddingHorizontal),
    ])
    .style(move |iced_theme, status| theme::edge_navigation(resources, iced_theme, status))
    .on_press_maybe(
        (state.selected > 0 && !state.busy && motion_idle).then_some(Message::Previous),
    );
    let next = button(
        text(resources.icon(IconId::Next)).size(resources.text_size(TextSizeToken::NavigationIcon)),
    )
    .padding([
        resources.dimension(DimensionToken::NavigationButtonPaddingVertical),
        resources.dimension(DimensionToken::NavigationButtonPaddingHorizontal),
    ])
    .style(move |iced_theme, status| theme::edge_navigation(resources, iced_theme, status))
    .on_press_maybe(
        (state.selected + 1 < state.entries.len() && !state.busy && motion_idle)
            .then_some(Message::Next),
    );

    container(
        row![previous, Space::new().width(Fill), next]
            .spacing(resources.dimension(DimensionToken::NavigationSpacing))
            .align_y(iced::Center),
    )
    .padding([
        0.0,
        resources.dimension(DimensionToken::NavigationHorizontalInset),
    ])
    .width(Fill)
    .height(Fill)
    .center_y(Fill)
    .into()
}

/// Builds the metadata, actions, and status overlay at the bottom of the image.
fn bottom_controls(state: &State, resources: ResourceContext) -> Element<'_, Message> {
    let details = selected_details(state, resources);
    let set_button = button(
        text(resources.text(TextKey::SetWallpaper)).size(resources.text_size(TextSizeToken::Label)),
    )
    .padding([
        resources.dimension(DimensionToken::ActionButtonPaddingVertical),
        resources.dimension(DimensionToken::ActionButtonPaddingHorizontal),
    ])
    .style(theme::primary_action)
    .on_press_maybe(
        (state.selected_preview_is_ready() && !state.busy).then_some(Message::SetWallpaper),
    );
    let refresh_button = button(
        text(resources.text(TextKey::Refresh)).size(resources.text_size(TextSizeToken::Label)),
    )
    .padding([
        resources.dimension(DimensionToken::ActionButtonPaddingVertical),
        resources.dimension(DimensionToken::ActionButtonPaddingHorizontal),
    ])
    .style(theme::secondary_action)
    .on_press_maybe((!state.busy).then_some(Message::Refresh));

    container(
        column![
            details,
            row![set_button, refresh_button]
                .spacing(resources.dimension(DimensionToken::ActionSpacing)),
            text(&state.status).size(resources.text_size(TextSizeToken::Status))
        ]
        .spacing(resources.dimension(DimensionToken::ActionSpacing)),
    )
    .padding(Padding {
        top: resources.dimension(DimensionToken::BottomPaddingTop),
        right: resources.dimension(DimensionToken::BottomPaddingHorizontal),
        bottom: resources.dimension(DimensionToken::BottomPaddingBottom),
        left: resources.dimension(DimensionToken::BottomPaddingHorizontal),
    })
    .width(Fill)
    .style(move |iced_theme| theme::bottom_scrim(resources, iced_theme))
    .into()
}

/// Builds the date, position, and description for the selected wallpaper.
fn selected_details(state: &State, resources: ResourceContext) -> Element<'_, Message> {
    let Some(entry) = state.entries.get(state.selected) else {
        return text(resources.text(TextKey::LoadingFeed))
            .size(resources.text_size(TextSizeToken::Loading))
            .into();
    };
    column![
        row![
            text(&entry.date).size(resources.text_size(TextSizeToken::Label)),
            Space::new().width(Fill),
            text(resources.text(TextKey::PageCounter {
                current: state.selected + 1,
                total: state.entries.len(),
            }))
            .size(resources.text_size(TextSizeToken::Counter)),
        ],
        text(&entry.description).size(resources.text_size(TextSizeToken::Description)),
    ]
    .spacing(resources.dimension(DimensionToken::MetadataSpacing))
    .into()
}

/// Displays an allocated image handle or a localized full-window loading placeholder.
fn preview_image(
    handle: Option<iced::widget::image::Handle>,
    resources: ResourceContext,
) -> Element<'static, Message> {
    match handle {
        Some(handle) => image(handle)
            .width(Fill)
            .height(Fill)
            .content_fit(ContentFit::Cover)
            .into(),
        None => container(text(resources.text(TextKey::LoadingPreview)))
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Fill)
            .style(move |iced_theme| theme::fallback_background(resources, iced_theme))
            .into(),
    }
}
