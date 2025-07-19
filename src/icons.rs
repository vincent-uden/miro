use iced::{
    Length, Theme,
    widget::{self, svg},
};

const SVG_DELETE: &[u8] = include_bytes!("../assets/icons/delete.svg");
const SVG_TABLE_OF_CONTENTS: &[u8] = include_bytes!("../assets/icons/table_of_contents.svg");
const SVG_BOOKMARK: &[u8] = include_bytes!("../assets/icons/bookmark.svg");

pub fn delete() -> svg::Handle {
    svg::Handle::from_memory(SVG_DELETE)
}

pub fn table_of_contents() -> svg::Handle {
    svg::Handle::from_memory(SVG_TABLE_OF_CONTENTS)
}

pub fn bookmark() -> svg::Handle {
    svg::Handle::from_memory(SVG_BOOKMARK)
}

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub enum ButtonVariant {
    Primary,
    Danger,
    Subtle,
}

pub fn icon_button<'a, T>(
    handle: svg::Handle,
    variant: ButtonVariant,
) -> iced::widget::Button<'a, T> {
    const BTN_SIZE: f32 = 18.0;
    widget::button(widget::svg(handle).width(BTN_SIZE).height(BTN_SIZE).style(
        move |theme: &Theme, _| {
            let palette = theme.extended_palette();
            widget::svg::Style {
                color: Some(match variant {
                    ButtonVariant::Primary => palette.primary.base.text,
                    ButtonVariant::Danger => palette.danger.base.text,
                    ButtonVariant::Subtle => palette.background.base.text,
                }),
            }
        },
    ))
    .width(Length::Shrink)
    .padding(4.0)
    .style(move |theme: &Theme, status| {
        let palette = theme.extended_palette();
        widget::button::Style {
            background: match status {
                widget::button::Status::Hovered => Some(
                    (match variant {
                        ButtonVariant::Primary => palette.primary.weak,
                        ButtonVariant::Danger => palette.danger.weak,
                        ButtonVariant::Subtle => palette.background.weak,
                    })
                    .color
                    .into(),
                ),
                widget::button::Status::Pressed => Some(
                    (match variant {
                        ButtonVariant::Primary => palette.primary.strong,
                        ButtonVariant::Danger => palette.danger.strong,
                        ButtonVariant::Subtle => palette.background.strong,
                    })
                    .color
                    .into(),
                ),
                widget::button::Status::Active => Some(
                    (match variant {
                        ButtonVariant::Primary => palette.primary.base,
                        ButtonVariant::Danger => palette.danger.base,
                        ButtonVariant::Subtle => palette.background.base,
                    })
                    .color
                    .into(),
                ),
                _ => None,
            },
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
}
