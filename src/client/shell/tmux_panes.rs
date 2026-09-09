use super::*;

pub(super) fn render_tmux_panes_overlay(
    b: &mut Buffer,
    overlay: &ClientTmuxPaneOverlay,
    p: &Palette,
) -> Option<OverlayRender> {
    let outer = popup(b.area, 118, 28)?;
    let inner = panel(b, outer, p.accent, p.panel_bg)?;
    if inner.height < 8 || inner.width < 32 {
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
        if overlay.watching {
            "tmux pane · live"
        } else {
            "tmux panes"
        },
        base.fg(p.text).add_modifier(Modifier::BOLD),
    );
    let subtitle = if overlay.loading {
        "reading tmux panes…"
    } else if overlay.watching {
        "live capture; esc returns to the pane list"
    } else if overlay.panes.is_empty() {
        "no tmux panes found"
    } else {
        "Herdr subagents are marked; external panes are read-only"
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
    let mut rows = Vec::new();
    if let Some(error) = overlay.error.as_deref() {
        put_text(
            b,
            body.x,
            body.y,
            body.width,
            &format!(" error: {error}"),
            base.fg(p.red),
        );
    } else if overlay.watching {
        for (index, line) in overlay
            .output
            .lines()
            .enumerate()
            .take(body.height as usize)
        {
            put_text(
                b,
                body.x,
                body.y + index as u16,
                body.width,
                line,
                base.fg(p.text),
            );
        }
    } else if overlay.loading {
        put_text(
            b,
            body.x,
            body.y,
            body.width,
            " reading…",
            base.fg(p.overlay1),
        );
    } else {
        for (index, pane) in overlay.panes.iter().enumerate().take(body.height as usize) {
            let rect = Rect::new(body.x, body.y + index as u16, body.width, 1);
            rows.push((rect, index));
            let origin = match pane.origin {
                crate::api::schema::TmuxPaneOrigin::HerdrSubagent => "herdr",
                crate::api::schema::TmuxPaneOrigin::External => "external",
            };
            let agent = pane.agent_name.as_deref().unwrap_or(&pane.current_command);
            let activity = pane
                .last_output
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("")
                .trim();
            let label = if activity.is_empty() {
                format!(
                    " {origin:<8} {agent:<16} {}:{}:{}  {:?}",
                    pane.session_name, pane.window_index, pane.pane_index, pane.status
                )
            } else {
                format!(
                    " {origin:<8} {agent:<16} {}:{}:{}  {:?}  · {activity}",
                    pane.session_name, pane.window_index, pane.pane_index, pane.status
                )
            };
            let style = if index == overlay.selected {
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
    let footer = if overlay.watching {
        " esc list · r refresh capture · ctrl-c send interrupt "
    } else {
        " j/k select · enter live view · s send keys · x close Herdr pane · r refresh · esc close "
    };
    put_text(
        b,
        inner.x,
        inner.bottom().saturating_sub(1),
        inner.width,
        footer,
        base.fg(p.overlay0),
    );
    Some(OverlayRender {
        tmux_rows: rows,
        ..OverlayRender::default()
    })
}
