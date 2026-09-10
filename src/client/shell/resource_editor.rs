use super::*;

pub(super) fn render_resource_editor_overlay(
    b: &mut Buffer,
    editor: &ClientResourceEditorOverlay,
    p: &Palette,
) -> Option<OverlayRender> {
    let popup = popup(b.area, 86, 22)?;
    let inner = panel(b, popup, p.accent, p.panel_bg)?;
    put_text(
        b,
        inner.x,
        inner.y,
        inner.width,
        if editor.resource_id.is_some() {
            "edit resource"
        } else {
            "new resource"
        },
        Style::default()
            .fg(p.text)
            .bg(p.panel_bg)
            .add_modifier(Modifier::BOLD),
    );
    let fields = [
        ("name", &editor.name, ClientResourceEditorField::Name),
        (
            "kind (prompt/skill/mcp)",
            &editor.kind,
            ClientResourceEditorField::Kind,
        ),
        (
            "scope (global/project/task)",
            &editor.scope,
            ClientResourceEditorField::Scope,
        ),
        (
            "provider (optional)",
            &editor.provider,
            ClientResourceEditorField::Provider,
        ),
    ];
    for (index, (label, value, field)) in fields.iter().enumerate() {
        let label_y = inner.y + 2 + index as u16 * 3;
        put_text(
            b,
            inner.x,
            label_y,
            inner.width,
            &format!(" {label}"),
            Style::default().fg(p.overlay0).bg(p.panel_bg),
        );
        let rect = Rect::new(inner.x, label_y + 1, inner.width, 1);
        let style = Style::default().fg(p.text).bg(if editor.field == *field {
            p.surface1
        } else {
            p.surface0
        });
        put_text(b, rect.x, rect.y, rect.width, &format!(" {}", value), style);
    }
    let content_label_y = inner.y + 15;
    put_text(
        b,
        inner.x,
        content_label_y,
        inner.width,
        " content (prompt/skill instructions or MCP JSON)",
        Style::default().fg(p.overlay0).bg(p.panel_bg),
    );
    let content = Rect::new(inner.x, content_label_y + 1, inner.width, 1);
    put_text(
        b,
        content.x,
        content.y,
        content.width,
        &format!(" {}", editor.content),
        Style::default()
            .fg(p.text)
            .bg(if editor.field == ClientResourceEditorField::Content {
                p.surface1
            } else {
                p.surface0
            }),
    );
    let status_y = inner.bottom().saturating_sub(2);
    let status = if editor.saving {
        " saving…".to_owned()
    } else if let Some(error) = editor.error.as_deref() {
        format!(" {error}")
    } else {
        " Tab field · Enter save · Esc cancel".to_owned()
    };
    put_text(
        b,
        inner.x,
        status_y,
        inner.width,
        &status,
        Style::default()
            .fg(if editor.error.is_some() {
                p.red
            } else {
                p.overlay0
            })
            .bg(p.panel_bg),
    );
    let value = match editor.field {
        ClientResourceEditorField::Name => &editor.name,
        ClientResourceEditorField::Kind => &editor.kind,
        ClientResourceEditorField::Scope => &editor.scope,
        ClientResourceEditorField::Provider => &editor.provider,
        ClientResourceEditorField::Content => &editor.content,
    };
    Some(OverlayRender {
        cursor: (!editor.saving).then(|| crate::protocol::CursorState {
            x: (inner.x + 1 + display_width(value)).min(inner.right().saturating_sub(1)),
            y: match editor.field {
                ClientResourceEditorField::Name => inner.y + 3,
                ClientResourceEditorField::Kind => inner.y + 6,
                ClientResourceEditorField::Scope => inner.y + 9,
                ClientResourceEditorField::Provider => inner.y + 12,
                ClientResourceEditorField::Content => content.y,
            },
            visible: true,
            shape: 0,
        }),
        ..OverlayRender::default()
    })
}
