use iced::widget::{
    Space, Stack, button, column, container, image, pin, responsive, row, text, toggler,
};
use iced::{ContentFit, Element, Fill, Length};

use crate::{
    app::{Message, State, transition_progress},
    locale::{Locale, TextKey},
};

pub(crate) fn view(state: &State) -> Element<'_, Message> {
    if state.initializing {
        return loading_view(state);
    }
    if state.desktop.is_none() {
        return unsupported_view(state);
    }

    let daily_toggle = toggler(state.settings.daily_change)
        .label(state.locale.text(TextKey::DailyChange))
        .on_toggle_maybe((!state.busy).then_some(Message::ToggleDaily));

    let header = row![
        text("Bingwall").size(28),
        Space::new().width(Fill),
        daily_toggle
    ]
    .align_y(iced::Center)
    .spacing(16);

    let previous = if state.selected > 0 && !state.busy {
        button(text("‹").size(34)).on_press(Message::Previous)
    } else {
        button(text("‹").size(34))
    };
    let next = if state.selected + 1 < state.entries.len() && !state.busy {
        button(text("›").size(34)).on_press(Message::Next)
    } else {
        button(text("›").size(34))
    };

    let pager = row![
        container(previous).center_y(Fill),
        container(preview(state))
            .width(Fill)
            .height(Fill)
            .clip(true),
        container(next).center_y(Fill),
    ]
    .height(Fill)
    .spacing(12)
    .align_y(iced::Center);

    let details = selected_details(state);
    let set_button = if state.selected_preview_is_ready() && !state.busy {
        button(state.locale.text(TextKey::SetWallpaper))
            .style(button::primary)
            .on_press(Message::SetWallpaper)
    } else {
        button(state.locale.text(TextKey::SetWallpaper)).style(button::primary)
    };
    let refresh_button = if state.busy {
        button(state.locale.text(TextKey::Refresh))
    } else {
        button(state.locale.text(TextKey::Refresh)).on_press(Message::Refresh)
    };
    let actions = row![set_button, refresh_button].spacing(12);

    container(
        column![
            header,
            pager,
            details,
            actions,
            text(&state.status).size(14)
        ]
        .spacing(16)
        .height(Fill),
    )
    .padding(24)
    .width(Fill)
    .height(Fill)
    .into()
}

fn loading_view(state: &State) -> Element<'_, Message> {
    container(text(&state.status).size(18))
        .padding(32)
        .center(Fill)
        .into()
}

fn unsupported_view(state: &State) -> Element<'_, Message> {
    container(
        column![text("Bingwall").size(30), text(&state.status).size(18)]
            .spacing(16)
            .align_x(iced::Center),
    )
    .padding(32)
    .center(Fill)
    .into()
}

fn selected_details(state: &State) -> Element<'_, Message> {
    let Some(entry) = state.entries.get(state.selected) else {
        return text(state.locale.text(TextKey::LoadingFeed)).into();
    };
    column![
        row![
            text(&entry.date).size(16),
            Space::new().width(Fill),
            text(format!("{} / {}", state.selected + 1, state.entries.len())).size(14),
        ],
        text(&entry.description).size(18),
    ]
    .spacing(6)
    .into()
}

fn preview(state: &State) -> Element<'_, Message> {
    let current = state.preview_handle(state.selected);
    let transition = state.transition.as_ref().map(|transition| {
        (
            state.preview_handle(transition.from),
            transition_progress(state),
            transition.direction,
        )
    });
    let locale = state.locale;

    responsive(move |available| {
        let width = available.width.min(available.height * 16.0 / 9.0);
        let height = width * 9.0 / 16.0;
        let content: Element<'static, Message> =
            if let Some((previous, progress, direction)) = transition.clone() {
                let old_x = -direction * progress * width;
                let new_x = direction * (1.0 - progress) * width;
                Stack::new()
                    .push(
                        pin(preview_image(previous, locale))
                            .x(old_x)
                            .width(width)
                            .height(height),
                    )
                    .push(
                        pin(preview_image(current.clone(), locale))
                            .x(new_x)
                            .width(width)
                            .height(height),
                    )
                    .width(width)
                    .height(height)
                    .clip(true)
                    .into()
            } else {
                preview_image(current.clone(), locale)
            };
        let frame = container(content)
            .width(width)
            .height(height)
            .clip(true)
            .style(container::rounded_box);
        container(frame)
            .width(Fill)
            .height(Fill)
            .center(Fill)
            .into()
    })
    .width(Fill)
    .height(Fill)
    .into()
}

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
            .into(),
    }
}
