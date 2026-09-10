use super::*;

pub(super) fn render_task_web_browser_overlay(
    b: &mut Buffer,
    browser: &ClientTaskWebBrowserOverlay,
    p: &Palette,
) -> Option<OverlayRender> {
    let outer = popup(b.area, 120, 32)?;
    let inner = panel(b, outer, p.accent, p.panel_bg)?;
    if inner.height < 12 || inner.width < 30 {
        return Some(OverlayRender::default());
    }
    let base = Style::default()
        .bg(p.panel_bg)
        .remove_modifier(Modifier::DIM);
    put_text(
        b,
        inner.x,
        inner.y,
        inner.width,
        &format!("task browser · {}", browser.task_id),
        base.fg(p.text).add_modifier(Modifier::BOLD),
    );
    let subtitle = if browser.preview_url.is_some() {
        "task-local browser preview · press x to open externally"
    } else {
        "HTTP/HTTPS only · profiles persist per task"
    };
    put_text(
        b,
        inner.x,
        inner.y.saturating_add(1),
        inner.width,
        subtitle,
        base.fg(p.overlay1),
    );
    let profiles_y = inner.y.saturating_add(3);
    put_text(
        b,
        inner.x,
        profiles_y,
        inner.width,
        " profiles / tabs",
        base.fg(p.overlay1).add_modifier(Modifier::BOLD),
    );
    for (index, profile) in browser.profiles.iter().enumerate().take(4) {
        let row = Rect::new(inner.x, profiles_y + 1 + index as u16, inner.width, 1);
        let style = if index == browser.selected_profile {
            base.fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            base.fg(p.text)
        };
        b.set_style(row, style);
        put_text(
            b,
            row.x,
            row.y,
            row.width,
            &format!(" {} · {}", profile.name, profile.url),
            style,
        );
    }
    let url_y = profiles_y.saturating_add(6);
    let url = Rect::new(inner.x, url_y, inner.width, 1);
    let url_style = if browser.field == ClientTaskBrowserField::Url {
        base.fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        base.fg(p.text).bg(p.surface0)
    };
    b.set_style(url, url_style);
    put_text(
        b,
        url.x,
        url.y,
        url.width,
        &format!(" url: {}", browser.url),
        url_style,
    );
    let name = Rect::new(inner.x, url_y.saturating_add(2), inner.width, 1);
    let name_style = if browser.field == ClientTaskBrowserField::ProfileName {
        base.fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        base.fg(p.text).bg(p.surface0)
    };
    b.set_style(name, name_style);
    put_text(
        b,
        name.x,
        name.y,
        name.width,
        &format!(" profile name: {}", browser.profile_name),
        name_style,
    );
    let preview_title = browser.preview_url.as_deref().map_or_else(
        || " page preview".to_owned(),
        |url| format!(" page preview · {url}"),
    );
    let preview_title_y = name.y.saturating_add(2);
    put_text(
        b,
        inner.x,
        preview_title_y,
        inner.width,
        &preview_title,
        base.fg(p.overlay1).add_modifier(Modifier::BOLD),
    );
    let preview_body_y = preview_title_y.saturating_add(1);
    let preview_bottom = inner.bottom().saturating_sub(4);
    if browser.preview_url.is_some() && browser.preview_body.is_empty() {
        put_text(
            b,
            inner.x,
            preview_body_y,
            inner.width,
            " loading or empty response",
            base.fg(p.overlay1),
        );
    } else {
        for (index, line) in browser
            .preview_body
            .iter()
            .enumerate()
            .take(usize::from(preview_bottom.saturating_sub(preview_body_y)))
        {
            put_text(
                b,
                inner.x,
                preview_body_y.saturating_add(index as u16),
                inner.width,
                line,
                base.fg(p.text),
            );
        }
    }
    if let Some(error) = browser.error.as_deref() {
        put_text(
            b,
            inner.x,
            inner.bottom().saturating_sub(3),
            inner.width,
            &format!(" error: {error}"),
            base.fg(p.red),
        );
    }
    put_text(
        b,
        inner.x,
        inner.bottom().saturating_sub(1),
        inner.width,
        " tab switch · enter preview/save · n new · d delete · c clear profile · x external · esc close ",
        base.fg(p.overlay0),
    );
    Some(OverlayRender {
        primary: url,
        ..OverlayRender::default()
    })
}
