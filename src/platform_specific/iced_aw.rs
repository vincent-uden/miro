use iced::{
    Background, Border, Element, Length, Padding, Shadow, Theme, alignment, border,
    widget::{self, button, container, row, text},
};
use iced_aw::menu::{self, primary, DrawPath};
use keybinds2::{KeySeq, Keybind};
use std::path::PathBuf;

use crate::{
    CONFIG,
    app::{self, AppMessage},
    common_menu::{self, CommonMenuItem},
    config::BindableMessage,
};

pub fn create_menu_bar(_pdf_idx: usize, recent_files: &[PathBuf]) -> Element<'static, AppMessage> {
    let cfg = CONFIG.read().unwrap();
    let mut bar_items = Vec::new();

    for (category_name, skeleton_items) in &common_menu::items() {
        let mut descs: Vec<ItemDesc> = Vec::new();

        for item in skeleton_items {
            match item {
                CommonMenuItem::Button(msg) => {
                    descs.push(ItemDesc::Button(*msg));
                }
                CommonMenuItem::RecentFiles => {
                    if !recent_files.is_empty() {
                        descs.push(ItemDesc::Label("Recent".to_string()));
                        for path in recent_files {
                            descs.push(ItemDesc::RecentFile(path.clone()));
                        }
                    }
                }
                CommonMenuItem::Separator => {
                    descs.push(ItemDesc::Separator);
                }
            }
        }

        let last_button_idx = descs.iter().rposition(|d| matches!(d, ItemDesc::Button(_)));

        let mut menu_items = Vec::new();
        for (i, desc) in descs.into_iter().enumerate() {
            match desc {
                ItemDesc::Button(msg) => {
                    let label = msg.default_menu_label().unwrap_or("(unnamed)").to_string();
                    let binding = cfg.get_binding_for_msg(msg);
                    let is_last = Some(i) == last_button_idx;
                    let widget: Element<'static, AppMessage> = if is_last {
                        menu_button_last(label, msg, binding).into()
                    } else {
                        menu_button(label, msg, binding).into()
                    };
                    menu_items.push(menu::Item::new(widget));
                }
                ItemDesc::Label(text) => {
                    menu_items.push(menu::Item::new(menu_label(text)));
                }
                ItemDesc::RecentFile(path) => {
                    menu_items.push(menu::Item::new(create_recent_file_button(path)));
                }
                ItemDesc::Separator => {
                    menu_items.push(menu::Item::new(menu_separator()));
                }
            }
        }

        let cat_button = menu_category_button(category_name.clone());
        let cat_menu = iced_aw::Menu::new(menu_items)
            .max_width(300.0)
            .offset(0.0)
            .spacing(0.0);
        bar_items.push(menu::Item::with_menu(cat_button, cat_menu));
    }

    drop(cfg);

    container(row![
        menu::MenuBar::new(bar_items)
            .draw_path(DrawPath::Backdrop)
            .style(
                |theme: &Theme, status: iced_aw::style::Status| menu::Style {
                    menu_background: theme.extended_palette().background.weak.color.into(),
                    bar_background: theme.extended_palette().background.weak.color.into(),
                    menu_border: Border {
                        radius: border::Radius::new(0.0).bottom(8.0),
                        ..Default::default()
                    },
                    bar_shadow: Shadow::default(),
                    menu_shadow: Shadow::default(),
                    ..primary(theme, status)
                },
            ),
        container(widget::space::horizontal())
            .width(Length::Fill)
            .height(28.0)
            .style(|theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.weak.color.into()),
                ..Default::default()
            })
    ])
    .width(Length::Fill)
    .padding(Padding::default().bottom(2.0))
    .style(|theme| container::Style {
        background: Some(Background::Color(
            theme.extended_palette().background.weak.color,
        )),
        ..Default::default()
    })
    .into()
}

enum ItemDesc {
    Button(BindableMessage),
    Label(String),
    RecentFile(PathBuf),
    Separator,
}

fn menu_category_button(
    label: String,
) -> button::Button<'static, AppMessage, Theme, iced::Renderer> {
    app::base_button(
        text(label).align_y(alignment::Vertical::Center),
        AppMessage::None,
    )
    .width(Length::Shrink)
    .style(move |theme, status| {
        let palette = theme.extended_palette();
        let pair = match status {
            button::Status::Active => palette.background.weak,
            button::Status::Hovered | button::Status::Disabled => palette.background.base,
            button::Status::Pressed => palette.primary.base,
        };
        button::Style {
            text_color: pair.text,
            background: Some(Background::Color(pair.color)),
            ..Default::default()
        }
    })
}

fn format_key_sequence(seq: &KeySeq) -> String {
    let parts = seq.as_slice().iter().map(|inp| format!("{inp} "));
    let mut out = String::from("(");
    for p in parts {
        out.push_str(&p);
    }
    out.pop();
    out.push(')');
    out
}

fn menu_label(label: String) -> button::Button<'static, AppMessage, Theme, iced::Renderer> {
    button(
        row![
            text(label).style(|theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style {
                    color: Some(palette.primary.base.color),
                }
            }),
            widget::space::horizontal(),
        ]
        .align_y(alignment::Vertical::Center),
    )
    .width(Length::Fill)
    .padding([4, 8])
    .style(move |theme, _status| {
        let palette = theme.extended_palette();
        button::Style {
            text_color: palette.primary.base.color,
            background: None,
            border: Border {
                radius: border::Radius::default(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(AppMessage::None)
}

fn menu_separator() -> Element<'static, AppMessage> {
    container(widget::rule::horizontal(1))
        .height(Length::Fixed(6.0))
        .padding(0.0)
        .into()
}

fn create_recent_file_button(
    path: PathBuf,
) -> button::Button<'static, AppMessage, Theme, iced::Renderer> {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    app::base_button(
        row![
            text(file_name),
            widget::space::horizontal(),
            text("").style(|theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style {
                    color: Some(palette.primary.base.color),
                }
            })
        ],
        AppMessage::OpenFile(path),
    )
    .width(Length::Fill)
    .style(move |theme, status| {
        let palette = theme.extended_palette();
        let pair = match status {
            button::Status::Active => palette.background.weak,
            button::Status::Hovered => palette.background.base,
            button::Status::Pressed => palette.background.strong,
            button::Status::Disabled => palette.secondary.weak,
        };
        button::Style {
            text_color: pair.text,
            background: Some(Background::Color(pair.color)),
            border: Border {
                radius: border::Radius::default(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
}

fn menu_button(
    label: String,
    msg: BindableMessage,
    binding: Option<Keybind<BindableMessage>>,
) -> button::Button<'static, AppMessage, Theme, iced::Renderer> {
    let txt = format!(
        " {}",
        binding.map_or(String::new(), |b| format_key_sequence(&b.seq))
    );
    app::base_button(
        row![
            text(label),
            widget::space::horizontal(),
            text(txt).style(|theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style {
                    color: Some(palette.primary.base.color),
                }
            })
        ],
        msg.into(),
    )
    .width(Length::Fill)
    .style(move |theme, status| {
        let palette = theme.extended_palette();
        let pair = match status {
            button::Status::Active => palette.background.weak,
            button::Status::Hovered => palette.background.base,
            button::Status::Pressed => palette.background.strong,
            button::Status::Disabled => palette.secondary.weak,
        };
        button::Style {
            text_color: pair.text,
            background: Some(Background::Color(pair.color)),
            border: Border {
                radius: border::Radius::default(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
}

fn menu_button_last(
    label: String,
    msg: BindableMessage,
    binding: Option<Keybind<BindableMessage>>,
) -> button::Button<'static, AppMessage, Theme, iced::Renderer> {
    let txt = format!(
        " {}",
        binding.map_or(String::new(), |b| format_key_sequence(&b.seq))
    );
    app::base_button(
        row![
            text(label),
            widget::space::horizontal(),
            text(txt).style(|theme: &Theme| {
                let palette = theme.extended_palette();
                text::Style {
                    color: Some(palette.primary.base.color),
                }
            })
        ],
        msg.into(),
    )
    .width(Length::Fill)
    .style(move |theme, status| {
        let palette = theme.extended_palette();
        let pair = match status {
            button::Status::Active => palette.background.weak,
            button::Status::Hovered => palette.background.base,
            button::Status::Pressed => palette.background.strong,
            button::Status::Disabled => palette.secondary.weak,
        };
        button::Style {
            text_color: pair.text,
            background: Some(Background::Color(pair.color)),
            border: Border {
                radius: border::Radius::default().bottom(8.0),
                color: theme.extended_palette().background.strong.color,
                width: 0.0,
            },
            ..Default::default()
        }
    })
}
