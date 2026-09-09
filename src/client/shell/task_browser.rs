use super::*;

pub(super) fn render_task_browser_overlay(
    b: &mut Buffer,
    browser: &ClientTaskBrowserOverlay,
    p: &Palette,
) -> Option<OverlayRender> {
    let outer = popup(b.area, 100, 24)?;
    let inner = panel(b, outer, p.accent, p.panel_bg)?;
    if inner.height < 7 || inner.width < 24 {
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
        "tasks",
        base.fg(p.text).add_modifier(Modifier::BOLD),
    );
    let subtitle = if browser.loading {
        "loading tasks…"
    } else if browser.opening {
        "opening task…"
    } else if browser.tasks.is_empty() {
        "no tasks"
    } else {
        "select a task to open its workspace"
    };
    put_text(
        b,
        inner.x,
        inner.y.saturating_add(1),
        inner.width,
        subtitle,
        base.fg(p.overlay1),
    );

    let body = Rect::new(
        inner.x,
        inner.y.saturating_add(3),
        inner.width,
        inner.height.saturating_sub(5),
    );
    let mut task_rows = Vec::new();
    if let Some(error) = browser.error.as_deref() {
        put_text(
            b,
            body.x,
            body.y,
            body.width,
            &format!(" error: {error}"),
            base.fg(p.red),
        );
    } else if browser.loading {
        put_text(
            b,
            body.x,
            body.y,
            body.width,
            " loading tasks…",
            base.fg(p.overlay1),
        );
    } else if browser.tasks.is_empty() {
        put_text(
            b,
            body.x,
            body.y,
            body.width,
            " no tasks found",
            base.fg(p.overlay1),
        );
    } else {
        for (index, task) in browser.tasks.iter().enumerate().take(body.height as usize) {
            let rect = Rect::new(body.x, body.y + index as u16, body.width, 1);
            task_rows.push((rect, index));
            let status = format!("{:?}", task.status).to_ascii_lowercase();
            let location = match task.location {
                crate::task::TaskLocationMode::Repository => "repo",
                crate::task::TaskLocationMode::Worktree => "worktree",
            };
            let label = format!(
                " {}  {} · {} · {}",
                task.task_id, task.name, status, location
            );
            let style = if index == browser.selected {
                base.fg(contrast(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                base.fg(p.text)
            };
            b.set_style(rect, style);
            put_text(b, rect.x, rect.y, rect.width, &label, style);
        }
    }

    let footer_y = inner.bottom().saturating_sub(1);
    put_text(
        b,
        inner.x,
        footer_y,
        inner.width,
        " j/k select · enter open · r refresh · esc close ",
        base.fg(p.overlay0),
    );
    Some(OverlayRender {
        task_rows,
        ..OverlayRender::default()
    })
}
