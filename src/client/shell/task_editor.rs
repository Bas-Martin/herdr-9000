use super::*;

pub(super) fn render_task_editor_overlay(
    b: &mut Buffer,
    editor: &ClientTaskFileEditorOverlay,
    p: &Palette,
) -> Option<OverlayRender> {
    let outer = popup(b.area, 110, 30)?;
    let inner = panel(b, outer, p.accent, p.panel_bg)?;
    if inner.height < 10 || inner.width < 28 {
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
        &format!("edit task file · {}", editor.task_id),
        base.fg(p.text).add_modifier(Modifier::BOLD),
    );
    let subtitle = if editor.loading {
        "reading file…"
    } else if editor.saving {
        "saving file…"
    } else if editor.path.is_empty() {
        "enter a repository-relative path"
    } else {
        "edit content, then press ctrl+s"
    };
    put_text(
        b,
        inner.x,
        inner.y.saturating_add(1),
        inner.width,
        subtitle,
        base.fg(p.overlay1),
    );
    if let Some(error) = editor.error.as_deref() {
        put_text(
            b,
            inner.x,
            inner.y.saturating_add(2),
            inner.width,
            &format!(" error: {error}"),
            base.fg(p.red),
        );
    }

    let path_area = Rect::new(inner.x, inner.y.saturating_add(3), inner.width, 1);
    let path_style = if editor.field == ClientTaskFileEditorField::Path {
        base.fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        base.fg(p.text).bg(p.surface0)
    };
    b.set_style(path_area, path_style);
    put_text(
        b,
        path_area.x,
        path_area.y,
        path_area.width,
        &format!(" path: {}", editor.path),
        path_style,
    );

    let content_area = Rect::new(
        inner.x,
        inner.y.saturating_add(5),
        inner.width,
        inner.height.saturating_sub(8),
    );
    let content_style = if editor.field == ClientTaskFileEditorField::Content {
        base.fg(p.text).bg(p.surface0)
    } else {
        base.fg(p.overlay1).bg(p.surface0)
    };
    b.set_style(content_area, content_style);
    for (line_index, line) in editor.content.split('\n').enumerate() {
        let Some(y) = content_area.y.checked_add(line_index as u16) else {
            break;
        };
        if y >= content_area.bottom() {
            break;
        }
        put_text(
            b,
            content_area.x,
            y,
            content_area.width,
            line,
            content_style,
        );
    }

    let save = Rect::new(
        inner.right().saturating_sub(13),
        inner.bottom().saturating_sub(2),
        13.min(inner.width),
        1,
    );
    button(
        b,
        save,
        " ctrl+s save ",
        Style::default()
            .fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD)
            .remove_modifier(Modifier::DIM),
    );
    put_text(
        b,
        inner.x,
        inner.bottom().saturating_sub(1),
        inner.width.saturating_sub(save.width.saturating_add(1)),
        " tab switch field · enter read · esc close ",
        base.fg(p.overlay0),
    );

    let cursor = editor_cursor(editor, path_area, content_area);
    Some(OverlayRender {
        primary: save,
        task_editor_path: path_area,
        task_editor_content: content_area,
        task_editor_save: save,
        cursor,
        ..OverlayRender::default()
    })
}

fn editor_cursor(
    editor: &ClientTaskFileEditorOverlay,
    path_area: Rect,
    content_area: Rect,
) -> Option<crate::protocol::CursorState> {
    if editor.loading || editor.saving {
        return None;
    }
    match editor.field {
        ClientTaskFileEditorField::Path => Some(crate::protocol::CursorState {
            x: path_area
                .x
                .saturating_add(8)
                .saturating_add(display_width(&editor.path))
                .min(path_area.right().saturating_sub(1)),
            y: path_area.y,
            visible: true,
            shape: 0,
        }),
        ClientTaskFileEditorField::Content => {
            let cursor = editor.cursor.min(editor.content.len());
            let prefix = &editor.content[..cursor];
            let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
            let column = prefix
                .rsplit_once('\n')
                .map_or(prefix, |(_, current)| current)
                .chars()
                .count();
            let y = content_area.y.checked_add(u16::try_from(line).ok()?)?;
            if y >= content_area.bottom() {
                return None;
            }
            Some(crate::protocol::CursorState {
                x: content_area
                    .x
                    .saturating_add(u16::try_from(column).unwrap_or(u16::MAX))
                    .min(content_area.right().saturating_sub(1)),
                y,
                visible: true,
                shape: 0,
            })
        }
    }
}
