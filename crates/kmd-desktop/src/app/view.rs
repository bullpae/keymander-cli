//! UI 렌더링 — view, 결과 목록, 상세 패널, 치트시트

use super::*;
use super::{context_actions_for, estimate_title_max_chars, truncate_str};

impl App {
    fn view_brand_mark(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let u = self.ui;
        let copper = t.peach;

        // copper 색으로 아주 연한 원형 뱃지 배경
        let badge_bg = Color { a: 0.12, ..copper };
        let badge_border = Color { a: 0.35, ..copper };

        let badge_size = u.pill_height - 16.0;
        let icon = container(
            text("\u{00BB}")                 // »
                .size((u.font * 1.15).round())
                .color(copper),
        )
        .center_x(badge_size)   // center_x(len)은 너비를 len으로 설정하며 가운데 정렬
        .center_y(badge_size)
        .style(move |_: &_| container::Style {
            background: Some(Background::Color(badge_bg)),
            border: Border {
                radius: (badge_size / 2.0).into(),
                width: 1.0,
                color: badge_border,
            },
            ..Default::default()
        });

        mouse_area(icon)
            .on_press(Message::BrandClicked)
            .on_right_press(Message::BrandRightClicked)
            .interaction(iced::mouse::Interaction::Pointer)
            .into()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let has_results = !self.results.is_empty();
        let text_color = t.text;
        let overlay_color = t.overlay;
        let accent_color = t.accent;
        let bg = t.background_with_opacity();

        let divider_c = t.border;

        // ── 드래그 스트립 (투명) ──
        let drag_strip = mouse_area(container(text("")).width(Fill).height(DRAG_STRIP))
            .on_press(Message::StartWindowDrag)
            .interaction(iced::mouse::Interaction::Grab);

        // ── 검색바 콘텐츠 ──
        let u = self.ui;
        let brand = self.view_brand_mark();

        let input = text_input("Search anything...", &self.query)
            .id(self.input_id.clone())
            .on_input(Message::QueryChanged)
            .on_submit(Message::Submit)
            .width(Fill)
            .size(u.font)
            .padding(Padding::from([4, 8]))
            .style(move |_theme, status| {
                let is_focused = matches!(status, text_input::Status::Focused { .. });
                let ph_color = if is_focused {
                    Color {
                        a: overlay_color.a * 0.5,
                        ..overlay_color
                    }
                } else {
                    overlay_color
                };
                text_input::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: Border::default(),
                    icon: overlay_color,
                    placeholder: ph_color,
                    value: text_color,
                    selection: Color {
                        a: 0.3,
                        ..accent_color
                    },
                }
            });

        let search_row = row![brand, input]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([0, 12]));

        let search_bar = container(search_row).width(Fill).center_y(u.pill_height);

        // ── 카드 본체 (항상 동일한 트리 구조 — 포커스/IME 안정성 보장) ──
        let sep_h = if has_results { 1.0 } else { 0.0 };
        let h_sep = container(text(""))
            .width(Fill)
            .height(sep_h)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(divider_c)),
                ..Default::default()
            });

        let h_sep2 = container(text(""))
            .width(Fill)
            .height(sep_h)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(divider_c)),
                ..Default::default()
            });

        let vert_divider = container(text(""))
            .width(if has_results { 1 } else { 0 })
            .height(Fill)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(divider_c)),
                ..Default::default()
            });

        let is_keys_mode = self.query.trim().starts_with(":keys")
            || self.query.trim() == ":k"
            || self.query.trim().starts_with(":k ");

        let results_body: Element<'_, Message> = if is_keys_mode && has_results {
            container(scrollable(self.view_cheatsheet()).width(Fill).height(Fill))
                .width(Fill)
                .height(Length::Fill)
                .into()
        } else {
            let left_col = Column::new()
                .push(self.view_results_list())
                .push(h_sep2)
                .push(self.view_status_bar());

            row![
                container(left_col).width(Length::FillPortion(2)),
                vert_divider,
                container(self.view_detail_panel())
                    .width(Length::FillPortion(1))
                    .clip(true),
            ]
            .width(Fill)
            .height(if has_results {
                Length::Fill
            } else {
                Length::Fixed(0.0)
            })
            .into()
        };

        let card_col = Column::new()
            .push(search_bar)
            .push(h_sep)
            .push(results_body);

        // ── 카드 스타일 (pill_height 기반 일관된 라운드) ──
        let border_color = Color {
            a: if has_results { 0.28 } else { 0.52 },
            ..accent_color
        };
        let inner_line = Color {
            a: 0.16,
            ..t.peach
        };
        let radius = u.pill_height / 2.0;

        let card_surface = container(card_col)
            .width(Fill)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: radius.into(),
                    width: 2.0,
                    color: border_color,
                },
                ..Default::default()
            });

        let card = container(card_surface)
            .width(Fill)
            .padding(2)
            .style(move |_: &_| container::Style {
                border: Border {
                    radius: (radius + 2.0).into(),
                    width: 1.0,
                    color: inner_line,
                },
                ..Default::default()
            });

        // ── 안정적 트리: 항상 동일한 4개 자식 (drag, card_row, hint, bg_dismiss) ──
        // edge_w를 항상 동일하게 유지하여 pill↔결과 전환 시 카드 너비 일관성 확보
        let edge_w: f32 = 4.0;
        let left_edge = mouse_area(container(text("")).width(edge_w).height(Fill))
            .on_press(Message::StartWindowResize(window::Direction::West))
            .interaction(iced::mouse::Interaction::ResizingHorizontally);
        let right_edge = mouse_area(container(text("")).width(edge_w).height(Fill))
            .on_press(Message::StartWindowResize(window::Direction::East))
            .interaction(iced::mouse::Interaction::ResizingHorizontally);

        let card_row = row![left_edge, card, right_edge]
            .width(Fill)
            .height(if has_results {
                Length::Fill
            } else {
                Length::Shrink
            });

        let hint_area: Element<'_, Message> = if !has_results && !self.query.trim().is_empty() {
            container(
                text("No results found")
                    .size(u.no_results_font)
                    .color(t.overlay)
                    .center(),
            )
            .width(Fill)
            .padding(Padding::from([8, 0]))
            .center_x(Fill)
            .into()
        } else {
            container(text("")).width(0).height(0).into()
        };

        let bg_dismiss = mouse_area(container(text("")).width(Fill).height(if has_results {
            Length::Fixed(0.0)
        } else {
            Length::Fill
        }))
        .on_press(Message::BackgroundClicked);

        let content = Column::new()
            .push(drag_strip)
            .push(card_row)
            .push(hint_area)
            .push(bg_dismiss);

        container(content).width(Fill).height(Fill).into()
    }

    pub(super) fn view_results_list(&self) -> Element<'_, Message> {
        let mut list = Column::new().spacing(0);
        for (i, result) in self.results.iter().enumerate() {
            list = list.push(self.view_result_row(i, result));
        }

        scrollable(container(list).width(Fill))
            .id(self.scrollable_id.clone())
            .height(Fill)
            .into()
    }

    pub(super) fn view_result_row<'a>(
        &'a self,
        index: usize,
        result: &'a kmd_core::SearchResult,
    ) -> Element<'a, Message> {
        let t = &self.theme;
        let is_selected = index == self.selected;
        let item = &result.item;

        let sel_color = if is_selected {
            t.accent
        } else {
            Color::TRANSPARENT
        };
        let u = self.ui;
        let left_bar = container(text(""))
            .width(4)
            .height(u.row_height - 8.0)
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(sel_color)),
                border: Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        let icon_size = u.result_icon;
        let icon_element: Element<'_, Message> = if let Some(handle) =
            crate::brand_icons::brand_icon_for_item(item.kind, &item.keywords, &item.path)
                .or_else(|| crate::brand_icons::brand_icon_for_settings(&item.keywords))
                .or_else(|| {
                    crate::app_icons::cached_icon_for_item(
                        item.kind,
                        &item.path,
                        item.icon_path.as_deref(),
                    )
                }) {
            image(handle)
                .content_fit(iced::ContentFit::Fill)
                .width(icon_size)
                .height(icon_size)
                .into()
        } else {
            container(text(&item.icon).size(icon_size - 6.0))
                .center_x(icon_size)
                .center_y(icon_size)
                .into()
        };
        let title = text(&item.name)
            .size(u.title_font)
            .color(t.text)
            .wrapping(Wrapping::None);
        let subtitle = text(&item.path)
            .size(u.subtitle_font)
            .color(t.subtext)
            .wrapping(Wrapping::None);
        let info = iced::widget::column![title, subtitle].spacing(2).width(Fill).clip(true);

        let kind_color = t.kind_color(item.kind);
        let kind_label = item.kind.to_string();
        let badge_bg = Color {
            a: 0.10,
            ..kind_color
        };
        let badge_border = Color {
            a: 0.20,
            ..kind_color
        };
        let badge = container(text(kind_label).size(u.badge_font).color(kind_color))
            .padding(Padding::from([2, 6]))
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(badge_bg)),
                border: Border {
                    radius: 4.0.into(),
                    width: 1.0,
                    color: badge_border,
                },
                ..Default::default()
            });

        let bg = if is_selected {
            t.surface2
        } else {
            Color::TRANSPARENT
        };

        let row_content = row![left_bar, icon_element, info, badge]
            .spacing(10)
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([4, 12]));

        mouse_area(
            container(row_content)
                .width(Fill)
                .height(u.row_height)
                .center_y(Fill)
                .style(move |_: &_| container::Style {
                    background: Some(Background::Color(bg)),
                    ..Default::default()
                }),
        )
        .on_press(Message::ResultClicked(index))
        .into()
    }

    /// :keys 치트시트 전체를 한 페이지 팝업 형태로 렌더링
    pub(super) fn view_cheatsheet(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let u = self.ui;
        let font_sm = u.hint_font;
        let font_md = font_sm + 2.0;
        let font_lg = font_md + 2.0;
        let hint_c = t.overlay;
        let text_c = t.text;
        let accent = t.accent;
        let border_c = Color { a: 0.12, ..accent };
        let shortcut_w = Length::Fixed(u.pill_height * 3.5);

        // 섹션 인덱스 수집: 헤더(━━) 위치 목록
        let header_indices: Vec<usize> = self
            .results
            .iter()
            .enumerate()
            .filter(|(_, r)| r.item.name.starts_with("━━"))
            .map(|(i, _)| i)
            .collect();

        let section_count = header_indices.len();
        let mid = (section_count + 1) / 2;

        let mut left_col = Column::new().spacing(8).width(Fill);
        let mut right_col = Column::new().spacing(8).width(Fill);

        for (sec_idx, &hdr_pos) in header_indices.iter().enumerate() {
            let hdr = &self.results[hdr_pos].item;
            let title = hdr
                .name
                .trim_start_matches('━')
                .trim_end_matches('━')
                .trim();

            let mut col = Column::new().spacing(2);
            let title_row = row![
                text(&hdr.icon).size(font_md).color(accent),
                text(title)
                    .size(font_lg)
                    .color(accent)
                    .wrapping(Wrapping::None),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center);
            col = col.push(title_row);
            col = col.push(Space::new().height(4));

            // 다음 헤더까지의 엔트리
            let end = header_indices
                .get(sec_idx + 1)
                .copied()
                .unwrap_or(self.results.len());
            for entry_idx in (hdr_pos + 1)..end {
                let item = &self.results[entry_idx].item;
                let r = row![
                    container(
                        text(&item.name)
                            .size(font_md)
                            .color(text_c)
                            .wrapping(Wrapping::None)
                    )
                    .width(shortcut_w),
                    text(&item.path)
                        .size(font_sm)
                        .color(hint_c)
                        .wrapping(Wrapping::None),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center);
                col = col.push(r);
            }

            let card: Element<'_, Message> = container(col)
                .padding(Padding::from([10, 14]))
                .width(Fill)
                .style(move |_: &_| container::Style {
                    border: Border {
                        radius: 8.0.into(),
                        width: 1.0,
                        color: border_c,
                    },
                    ..Default::default()
                })
                .into();

            if sec_idx < mid {
                left_col = left_col.push(card);
            } else {
                right_col = right_col.push(card);
            }
        }

        let grid = row![left_col, right_col].spacing(10).width(Fill);

        container(grid)
            .padding(Padding::from([10, 14]))
            .width(Fill)
            .into()
    }

    pub(super) fn view_status_bar(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let u = self.ui;
        let status_text = format!(
            "{}  \u{00B7}  {} results",
            self.search_mode.label(),
            self.results.len()
        );

        let left = text(status_text).size(u.status_font).color(t.overlay);
        let right = text("Esc to close").size(u.status_font).color(t.overlay);

        let bar = row![left, Space::new().width(Fill), right]
            .padding(Padding::from([4, 14]))
            .align_y(iced::Alignment::Center);

        container(bar)
            .width(Fill)
            .height(u.status_bar_height)
            .into()
    }

    pub(super) fn view_detail_panel(&self) -> Element<'_, Message> {
        let t = &self.theme;
        let u = self.ui;

        let Some(result) = self.results.get(self.selected) else {
            let hint_color = t.overlay;
            let shortcuts_col = iced::widget::column![
                text("Shortcuts").size(u.hint_font + 1.0).color(hint_color),
                container(text("")).height(8),
                text("Enter    Open").size(u.hint_font).color(hint_color),
                text("\u{2191}\u{2193}      Navigate")
                    .size(u.hint_font)
                    .color(hint_color),
                text("Esc       Close").size(u.hint_font).color(hint_color),
                text("Tab       Detail").size(u.hint_font).color(hint_color),
            ]
            .spacing(4);
            return container(
                container(shortcuts_col)
                    .width(Fill)
                    .padding(Padding::from([20, 16])),
            )
            .width(Fill)
            .height(Fill)
            .into();
        };
        let item = &result.item;

        // ── 아이콘 영역 (그라데이션 배경 위 큰 아이콘) ──
        let big_icon_size = u.detail_big_icon;
        let big_icon: Element<'_, Message> = if let Some(handle) =
            crate::brand_icons::brand_icon_for_item(item.kind, &item.keywords, &item.path)
                .or_else(|| crate::brand_icons::brand_icon_for_settings(&item.keywords))
                .or_else(|| {
                    crate::app_icons::cached_icon_for_item(
                        item.kind,
                        &item.path,
                        item.icon_path.as_deref(),
                    )
                }) {
            image(handle)
                .content_fit(iced::ContentFit::Fill)
                .width(big_icon_size)
                .height(big_icon_size)
                .into()
        } else {
            container(text(&item.icon).size(big_icon_size - 10.0))
                .center_x(big_icon_size)
                .center_y(big_icon_size)
                .into()
        };

        let kind_color = t.kind_color(item.kind);
        let icon_glow = Color {
            a: 0.06,
            ..kind_color
        };
        let icon_area = container(
            container(big_icon)
                .width(Fill)
                .center_x(Fill)
                .padding(Padding::from([20, 0])),
        )
        .width(Fill)
        .style(move |_: &_| container::Style {
            background: Some(Background::Color(icon_glow)),
            ..Default::default()
        });

        // ── 이름 (Bold) ──
        // 상세 패널 폭 기준으로 타이틀 최대 폭/글자 수를 동적으로 계산한다.
        // 좌우 마진을 항상 확보해 패널 경계를 넘지 않도록 한다.
        let detail_panel_width = (self.window_width / 3.0).max(220.0);
        let title_side_margin = 24.0;
        let title_width = (detail_panel_width - (title_side_margin * 2.0)).max(120.0);
        let title_max_chars = estimate_title_max_chars(title_width, u.detail_name_font);
        let name_str = truncate_str(&item.name, title_max_chars);
        let name_label = container(
            text(name_str)
                .size(u.detail_name_font)
                .color(t.text)
                .width(Length::Fixed(title_width))
                .wrapping(Wrapping::WordOrGlyph),
        )
        .width(Fill)
        .clip(true)
        .center_x(Fill)
        .padding(Padding::from([8, 12]));

        // ── "Path" 라벨 + 경로 (왼쪽 정렬, 말줄임) ──
        let path_text = if item.path.chars().count() > 35 {
            let end: String = item
                .path
                .chars()
                .rev()
                .take(32)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("...{end}")
        } else {
            item.path.clone()
        };
        let path_section = container(
            iced::widget::column![
                text("Path").size(u.path_label_font).color(t.overlay),
                text(path_text)
                    .size(u.path_text_font)
                    .color(t.subtext)
                    .wrapping(Wrapping::None),
            ]
            .spacing(2),
        )
        .width(Fill)
        .padding(Padding::from([4, 16]));

        // ── 뱃지 행 ──
        let kind_label = item.kind.to_string();
        let badge_bg = Color {
            a: 0.12,
            ..kind_color
        };
        let badge_border_c = Color {
            a: 0.22,
            ..kind_color
        };
        let badge = container(text(kind_label).size(u.badge_font).color(kind_color))
            .padding(Padding::from([2, 8]))
            .style(move |_: &_| container::Style {
                background: Some(Background::Color(badge_bg)),
                border: Border {
                    radius: 10.0.into(),
                    width: 1.0,
                    color: badge_border_c,
                },
                ..Default::default()
            });
        let badge_row = container(badge).width(Fill).padding(Padding::from([2, 16]));

        // ── 구분선 (패딩 있는 정돈된 선) ──
        let sep_color = t.border;
        let divider = container(
            container(text(""))
                .width(Fill)
                .height(1)
                .style(move |_: &_| container::Style {
                    background: Some(Background::Color(sep_color)),
                    ..Default::default()
                }),
        )
        .padding(Padding::from([4, 14]));

        // ── 액션 목록 ──
        let actions = context_actions_for(item.kind);
        let mut action_list = Column::new().spacing(1);
        for (i, action) in actions.iter().enumerate() {
            let action_clone = action.clone();
            let label_text = action.label_for_kind(item.kind).to_string();
            let shortcut_text = action.shortcut().to_string();
            let is_primary = i == 0;

            let text_color = if is_primary { t.accent } else { t.text };
            let accent_c = t.accent;
            let surface_c = t.surface2;

            let icon_text = match action {
                ContextAction::Open => "\u{2197}",
                ContextAction::OpenAsAdmin => "\u{26A1}",
                ContextAction::OpenFolder => "\u{1F4C1}",
                ContextAction::CopyPath => "\u{1F4CB}",
                ContextAction::CopyName => "\u{270D}",
            };

            let icon_label = text(icon_text).size(u.action_icon_font);
            let label = text(label_text).size(u.action_label_font).color(text_color);
            let shortcut = text(shortcut_text)
                .size(u.action_shortcut_font)
                .color(t.overlay);

            let action_row = row![
                container(icon_label).width(22).center_x(22),
                label,
                Space::new().width(Fill),
                shortcut
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
            .padding(Padding::from([6, 14]));

            let action_btn =
                container(action_row)
                    .width(Fill)
                    .style(move |_: &_| container::Style {
                        background: if is_primary {
                            Some(Background::Color(Color {
                                a: 0.10,
                                ..accent_c
                            }))
                        } else {
                            Some(Background::Color(Color {
                                a: 0.0,
                                ..surface_c
                            }))
                        },
                        border: if is_primary {
                            Border {
                                radius: 6.0.into(),
                                width: 1.0,
                                color: Color {
                                    a: 0.18,
                                    ..accent_c
                                },
                            }
                        } else {
                            Border::default()
                        },
                        ..Default::default()
                    });

            let wrapped = if is_primary {
                container(
                    mouse_area(action_btn)
                        .on_press(Message::RunAction(action_clone))
                        .interaction(iced::mouse::Interaction::Pointer),
                )
                .padding(Padding::from([2, 10]))
            } else {
                container(
                    mouse_area(action_btn)
                        .on_press(Message::RunAction(action_clone))
                        .interaction(iced::mouse::Interaction::Pointer),
                )
                .padding(Padding::from([0, 10]))
            };

            action_list = action_list.push(wrapped);
        }

        let panel_content = iced::widget::column![
            icon_area,
            name_label,
            path_section,
            badge_row,
            divider,
            action_list,
        ]
        .spacing(0);

        container(panel_content).width(Fill).height(Fill).into()
    }
}
