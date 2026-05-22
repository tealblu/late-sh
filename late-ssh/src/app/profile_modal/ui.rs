use chrono::Utc;
use late_core::models::bonsai::Tree;
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use uuid::Uuid;

use crate::app::{
    bonsai::{state::stage_for, ui::render_tree_art_lines},
    chat::showcase::svc::ShowcaseFeedItem,
    common::{markdown::render_body_to_lines, theme, time::timezone_current_time},
    settings_modal::data::country_label,
};

use super::state::ProfileModalState;

const MODAL_WIDTH: u16 = 92;
const MODAL_HEIGHT: u16 = 28;
// Match the right-sidebar bonsai card width (see common/sidebar.rs).
const BONSAI_CARD_WIDTH: u16 = 24;
const FETCH_STRIP_HEIGHT: u16 = 5;

pub fn draw(
    frame: &mut Frame,
    area: Rect,
    state: &ProfileModalState,
    current_user_id: Uuid,
    profile_theming: bool,
) {
    // When profile_theming is enabled and we're viewing someone else's profile,
    // build a palette from the profiled user's preferred theme.
    // Otherwise fall back to the client's current theme (no custom background).
    let viewing_own = state.viewed_user_id() == Some(current_user_id);
    let use_profile_theme = profile_theming && !viewing_own;

    let theme_id = if use_profile_theme {
        state
            .profile()
            .and_then(|p| p.theme_id.as_deref())
            .unwrap_or(theme::DEFAULT_ID)
    } else {
        theme::DEFAULT_ID
    };
    let pal = theme::ModalPalette::from_theme_id(theme_id);

    let popup = centered_rect(MODAL_WIDTH, MODAL_HEIGHT, area);
    frame.render_widget(Clear, popup);
    if use_profile_theme {
        frame.render_widget(Block::default().style(pal.bg), popup);
    }

    let layout = Layout::vertical([
        Constraint::Min(10),
        Constraint::Length(FETCH_STRIP_HEIGHT),
        Constraint::Length(1),
    ])
    .split(popup);

    let wide = layout[0].width >= 80;
    if wide {
        let body = Layout::horizontal([Constraint::Min(50), Constraint::Length(BONSAI_CARD_WIDTH)])
            .split(layout[0]);
        draw_profile_card(frame, body[0], state, &pal);
        draw_bonsai_card(frame, body[1], state.bonsai(), &pal);
    } else {
        draw_profile_card(frame, layout[0], state, &pal);
    }

    draw_late_fetch_strip(frame, layout[1], state, &pal);
    draw_footer(frame, layout[2], &pal);
}

fn draw_footer(frame: &mut Frame, area: Rect, pal: &theme::ModalPalette) {
    let footer = Line::from(vec![
        Span::styled("↑↓ j/k", pal.amber_dim),
        Span::styled(" scroll  ", pal.text_dim),
        Span::styled("Esc/q", pal.amber_dim),
        Span::styled(" close", pal.text_dim),
    ]);
    frame.render_widget(Paragraph::new(footer), area);
}

fn draw_profile_card(
    frame: &mut Frame,
    area: Rect,
    state: &ProfileModalState,
    pal: &theme::ModalPalette,
) {
    let block = Block::default()
        .title(" profile ")
        .title_style(pal.title)
        .borders(Borders::ALL)
        .border_style(pal.border);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = inner.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    let lines = build_profile_lines(state, content.width as usize, pal);
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((state.scroll_offset(), 0)),
        content,
    );
}

fn draw_bonsai_card(frame: &mut Frame, area: Rect, tree: Option<&Tree>, pal: &theme::ModalPalette) {
    let block = Block::default()
        .title(" bonsai ")
        .title_style(pal.title)
        .borders(Borders::ALL)
        .border_style(pal.border);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(tree) = tree else {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(" no bonsai yet", pal.text_dim))),
            inner,
        );
        return;
    };

    let stage = stage_for(tree.is_alive, tree.growth_points);
    let age_days = (Utc::now().date_naive() - tree.created.date_naive())
        .num_days()
        .max(0);
    let wilting = tree.is_alive
        && tree
            .last_watered
            .map(|last| (Utc::now().date_naive() - last).num_days() >= 2)
            .unwrap_or(age_days >= 2);

    let mut lines =
        render_tree_art_lines(stage, tree.seed, wilting, inner.width as usize, 0.0, None);

    let visible = inner.height as usize;
    let label_line = Line::from(vec![Span::styled(
        format!("{} · {}d", stage.label(), age_days),
        pal.text_dim,
    )])
    .centered();

    if lines.len() + 1 < visible {
        let pad = visible.saturating_sub(lines.len() + 1);
        for _ in 0..pad {
            lines.insert(0, Line::from(""));
        }
    }
    lines.push(label_line);

    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_late_fetch_strip(
    frame: &mut Frame,
    area: Rect,
    state: &ProfileModalState,
    pal: &theme::ModalPalette,
) {
    let block = Block::default()
        .title(" late.fetch ")
        .title_style(pal.title)
        .borders(Borders::ALL)
        .border_style(pal.border);
    let inner = block.inner(area).inner(Margin {
        horizontal: 1,
        vertical: 0,
    });
    frame.render_widget(block, area);

    let Some(profile) = state.profile() else {
        return;
    };

    let theme_id = profile.theme_id.as_deref().unwrap_or(theme::DEFAULT_ID);
    let created = profile
        .created_at
        .as_ref()
        .map(format_created_at)
        .unwrap_or_else(|| "unknown".to_string());
    let ide = profile.ide.clone().unwrap_or_else(|| "—".to_string());
    let terminal = profile.terminal.clone().unwrap_or_else(|| "—".to_string());
    let os = profile.os.clone().unwrap_or_else(|| "—".to_string());
    let theme_label = theme::label_for_id(theme_id).to_string();
    let langs = if profile.langs.is_empty() {
        "—".to_string()
    } else {
        profile.langs.join(", ")
    };

    let inner_w = inner.width as usize;
    let col_w = inner_w / 2;

    let row1 = Line::from(format_two_cells(
        ("created", &created),
        ("theme", &theme_label),
        col_w,
        pal.amber_dim,
        pal.text,
        pal.text_dim,
    ));
    let row2 = Line::from(format_two_cells(
        ("ide", &ide),
        ("terminal", &terminal),
        col_w,
        pal.amber_dim,
        pal.text,
        pal.text_dim,
    ));
    let row3 = Line::from(format_two_cells(
        ("os", &os),
        ("langs", &langs),
        col_w,
        pal.amber_dim,
        pal.text,
        pal.text_dim,
    ));

    frame.render_widget(Paragraph::new(vec![row1, row2, row3]), inner);
}

fn format_two_cells(
    a: (&str, &str),
    b: (&str, &str),
    col_w: usize,
    label_style: Style,
    value_style: Style,
    sep_style: Style,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, (label, value)) in [a, b].into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("│ ", sep_style));
        }
        let label_padded = format!("{label:<9} ");
        let used = label_padded.chars().count() + value.chars().count();
        let pad = col_w.saturating_sub(used + if i == 0 { 2 } else { 0 });
        spans.push(Span::styled(label_padded, label_style));
        spans.push(Span::styled(value.to_string(), value_style));
        if i == 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
    }
    spans
}

fn format_created_at(created_at: &chrono::DateTime<Utc>) -> String {
    created_at.format("%Y-%m-%d").to_string()
}

fn build_profile_lines(
    state: &ProfileModalState,
    width: usize,
    pal: &theme::ModalPalette,
) -> Vec<Line<'static>> {
    if state.loading() {
        return Vec::new();
    }

    let Some(profile) = state.profile() else {
        return Vec::new();
    };

    let username = if profile.username.trim().is_empty() {
        "not set"
    } else {
        profile.username.trim()
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("Username: ", pal.text_dim),
            Span::styled(username.to_string(), pal.text),
        ]),
        Line::from(vec![
            Span::styled("Country:  ", pal.text_dim),
            Span::styled(country_label(profile.country.as_deref()), pal.text),
        ]),
        Line::from(vec![
            Span::styled("Timezone: ", pal.text_dim),
            Span::styled(
                profile.timezone.as_deref().unwrap_or("Not set").to_string(),
                pal.text,
            ),
        ]),
    ];

    if let Some(current_time) = timezone_current_time(Utc::now(), profile.timezone.as_deref()) {
        lines.push(Line::from(vec![
            Span::styled("Current time: ", pal.text_dim),
            Span::styled(current_time, pal.text),
        ]));
    }

    lines.extend([Line::from(""), section_heading("Bio", pal)]);

    if profile.bio.trim().is_empty() {
        lines.push(Line::from(Span::styled("Not set", pal.text_dim)));
    } else {
        lines.extend(render_body_to_lines(
            &profile.bio,
            width,
            Span::raw(""),
            pal.text,
        ));
    }

    let showcases = state.showcases_for_viewed();
    if !showcases.is_empty() {
        lines.push(Line::from(""));
        lines.push(section_heading(
            &format!("Showcases ({})", showcases.len()),
            pal,
        ));
        for item in showcases {
            lines.push(Line::from(""));
            lines.extend(render_body_to_lines(
                &showcase_markdown(item),
                width,
                Span::raw(""),
                pal.text,
            ));
        }
    }

    lines
}

fn showcase_markdown(item: &ShowcaseFeedItem) -> String {
    let s = &item.showcase;
    let mut out = String::new();
    out.push_str("### ");
    out.push_str(s.title.trim());
    out.push_str("\n\n> ");
    out.push_str(s.url.trim());
    let description = s.description.trim();
    if !description.is_empty() {
        out.push_str("\n\n");
        out.push_str(description);
    }
    if !s.tags.is_empty() {
        out.push_str("\n\n");
        let mut first = true;
        for tag in &s.tags {
            if !first {
                out.push(' ');
            }
            first = false;
            out.push('`');
            out.push('#');
            out.push_str(tag);
            out.push('`');
        }
    }
    out
}

fn section_heading(title: &str, pal: &theme::ModalPalette) -> Line<'static> {
    let accent = pal.amber.add_modifier(Modifier::BOLD);
    Line::from(vec![
        Span::styled("── ", pal.border),
        Span::styled(title.to_string(), accent),
        Span::styled(" ──", pal.border),
    ])
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .split(area);
    let horizontal = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .split(vertical[0]);
    horizontal[0]
}
