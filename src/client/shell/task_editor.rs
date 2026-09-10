use super::*;

pub(super) fn render_task_editor_overlay(
    b: &mut Buffer,
    editor: &ClientTaskFileEditorOverlay,
    p: &Palette,
) -> Option<OverlayRender> {
    let outer = popup(b.area, 118, 30)?;
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
        &format!(
            "edit task files · {}{}",
            editor.task_id,
            if editor.dirty { " · unsaved" } else { "" }
        ),
        base.fg(p.text).add_modifier(Modifier::BOLD),
    );
    let subtitle = if editor.loading {
        "reading workspace…"
    } else if editor.saving {
        "saving file…"
    } else if editor.files.is_empty() {
        "enter a repository-relative path"
    } else {
        "select a file, edit content, then press ctrl+s"
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

    let tabs_y = inner.y.saturating_add(3);
    let mut tabs_x = inner.x;
    for (index, tab) in editor.tabs.iter().enumerate() {
        let marker = if index == editor.selected_tab {
            ">"
        } else {
            " "
        };
        let dirty = if tab.dirty { "*" } else { "" };
        let deleted = if tab.deleted { "!" } else { "" };
        let label = format!(" {marker}{dirty}{deleted} {} ", tab.path);
        if tabs_x >= inner.right() {
            break;
        }
        let width = u16::try_from(display_width(&label))
            .unwrap_or(u16::MAX)
            .min(inner.right().saturating_sub(tabs_x));
        let style = if index == editor.selected_tab {
            base.fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else if tab.deleted {
            base.fg(p.red).bg(p.surface0)
        } else {
            base.fg(p.text).bg(p.surface0)
        };
        b.set_style(Rect::new(tabs_x, tabs_y, width, 1), style);
        put_text(b, tabs_x, tabs_y, width, &label, style);
        tabs_x = tabs_x.saturating_add(width.saturating_add(1));
    }
    let body_y = inner.y.saturating_add(4);
    let body_height = inner.height.saturating_sub(9);
    let tree_width = if editor.files.is_empty() {
        0
    } else {
        inner.width.min(34).max(18)
    };
    let path_x = inner.x.saturating_add(tree_width.saturating_add(1));
    let path_width = inner.width.saturating_sub(tree_width.saturating_add(1));
    if tree_width > 0 {
        let tree_area = Rect::new(inner.x, body_y, tree_width, body_height);
        b.set_style(tree_area, base.bg(p.surface0));
        for (index, file) in editor.files.iter().enumerate().take(body_height as usize) {
            let rect = Rect::new(tree_area.x, tree_area.y + index as u16, tree_area.width, 1);
            let selected = index == editor.selected_file;
            let style = if selected {
                base.fg(contrast(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD)
            } else if file.is_dir {
                base.fg(p.overlay1)
            } else {
                base.fg(p.text)
            };
            b.set_style(rect, style);
            let marker = if file.is_dir { "▸ " } else { "  " };
            put_text(
                b,
                rect.x,
                rect.y,
                rect.width,
                &format!("{marker}{}", file.path),
                style,
            );
        }
    }

    let path_area = Rect::new(path_x, body_y, path_width, 1);
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

    let find_area = Rect::new(path_x, body_y.saturating_add(2), path_width, 1);
    let find_style = if editor.find_mode == Some(false) {
        base.fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        base.fg(p.text).bg(p.surface0)
    };
    b.set_style(find_area, find_style);
    put_text(
        b,
        find_area.x,
        find_area.y,
        find_area.width,
        &format!(" find: {}", editor.find_query),
        find_style,
    );
    let replace_area = Rect::new(path_x, body_y.saturating_add(3), path_width, 1);
    let replace_style = if editor.find_mode == Some(true) {
        base.fg(contrast(p))
            .bg(p.accent)
            .add_modifier(Modifier::BOLD)
    } else {
        base.fg(p.text).bg(p.surface0)
    };
    b.set_style(replace_area, replace_style);
    put_text(
        b,
        replace_area.x,
        replace_area.y,
        replace_area.width,
        &format!(" replace: {}", editor.replace_text),
        replace_style,
    );
    let content_area = Rect::new(
        path_x,
        body_y.saturating_add(5),
        path_width,
        body_height.saturating_sub(5),
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
        let style = syntax_line_style(base, p, &editor.path, line);
        put_text(b, content_area.x, y, content_area.width, line, style);
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
        " arrows select file · [/] switch tabs · x close tab · tab fields · ctrl+s save · esc close ",
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

fn syntax_line_style(base: Style, p: &Palette, path: &str, line: &str) -> Style {
    let Some(extension) = path.rsplit_once('.').map(|(_, extension)| extension) else {
        return base.fg(p.text).bg(p.surface0);
    };
    let supported = matches!(
        extension.to_ascii_lowercase().as_str(),
        "c" | "cpp"
            | "css"
            | "go"
            | "html"
            | "java"
            | "js"
            | "json"
            | "md"
            | "py"
            | "rs"
            | "sh"
            | "sql"
            | "toml"
            | "ts"
            | "tsx"
            | "yaml"
            | "yml"
    );
    if !supported {
        return base.fg(p.text).bg(p.surface0);
    }
    let trimmed = line.trim_start();
    let color = if trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("--")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
    {
        p.overlay1
    } else if trimmed.contains("fn ")
        || trimmed.contains("def ")
        || trimmed.contains("class ")
        || trimmed.contains("struct ")
        || trimmed.contains("function ")
        || trimmed.starts_with("import ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("return ")
    {
        p.blue
    } else if trimmed.contains('"') || trimmed.contains('\'') {
        p.green
    } else {
        p.text
    };
    base.fg(color).bg(p.surface0)
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
