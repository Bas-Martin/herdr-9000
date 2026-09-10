use super::*;

pub(super) fn render_worktree_apps_overlay(
    b: &mut Buffer,
    apps: &ClientWorktreeAppsOverlay,
    p: &Palette,
) -> Option<OverlayRender> {
    let popup = popup(b.area, 72, 14)?;
    let inner = panel(b, popup, p.accent, p.panel_bg)?;
    put_text(
        b,
        inner.x,
        inner.y,
        inner.width,
        "open worktree in",
        Style::default()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD),
    );
    put_text(
        b,
        inner.x,
        inner.y + 2,
        inner.width,
        &format!(" path: {}", apps.path),
        Style::default().fg(p.subtext0).bg(p.panel_bg),
    );
    let choices = [
        ClientWorktreeExternalApp::VsCode,
        ClientWorktreeExternalApp::FileExplorer,
        ClientWorktreeExternalApp::Terminal,
        ClientWorktreeExternalApp::VisualStudio,
    ];
    for (index, app) in choices.iter().enumerate() {
        let row = inner.y + 4 + index as u16;
        let marker = if apps.selected == index { ">" } else { " " };
        put_text(
            b,
            inner.x,
            row,
            inner.width,
            &format!(" {marker} {}", app.label()),
            Style::default()
                .fg(if apps.selected == index {
                    p.accent
                } else {
                    p.text
                })
                .bg(if apps.selected == index {
                    p.surface1
                } else {
                    p.panel_bg
                }),
        );
    }
    if apps.opening {
        put_text(
            b,
            inner.x,
            inner.y + 9,
            inner.width,
            " opening…",
            Style::default().fg(p.accent).bg(p.panel_bg),
        );
    } else if let Some(error) = apps.error.as_deref() {
        put_text(
            b,
            inner.x,
            inner.y + 9,
            inner.width,
            &format!(" {error}"),
            Style::default().fg(p.red).bg(p.panel_bg),
        );
    } else {
        put_text(
            b,
            inner.x,
            inner.y + 9,
            inner.width,
            " ↑/↓ select · Enter open · Esc cancel",
            Style::default().fg(p.overlay0).bg(p.panel_bg),
        );
    }
    Some(OverlayRender {
        primary: Rect::default(),
        cancel: Rect::default(),
        ..OverlayRender::default()
    })
}
