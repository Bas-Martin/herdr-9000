use super::*;

pub(super) fn render_resource_library_overlay(
    b: &mut Buffer,
    library: &ClientResourceLibraryOverlay,
    p: &Palette,
) -> Option<OverlayRender> {
    let outer = popup(b.area, 110, 26)?;
    let inner = panel(b, outer, p.accent, p.panel_bg)?;
    if inner.height < 7 || inner.width < 32 {
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
        "resource library",
        base.fg(p.text).add_modifier(Modifier::BOLD),
    );
    let subtitle = if library.loading {
        "loading reusable prompts, skills, and MCP servers…"
    } else if library.resources.is_empty() {
        "no reusable resources configured"
    } else {
        "resources are scoped and persisted on the endpoint"
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
    if let Some(error) = library.error.as_deref() {
        put_text(
            b,
            body.x,
            body.y,
            body.width,
            &format!(" error: {error}"),
            base.fg(p.red),
        );
    } else if library.loading {
        put_text(
            b,
            body.x,
            body.y,
            body.width,
            " loading resources…",
            base.fg(p.overlay1),
        );
    } else {
        for (index, resource) in library
            .resources
            .iter()
            .enumerate()
            .take(body.height as usize)
        {
            let rect = Rect::new(body.x, body.y + index as u16, body.width, 1);
            rows.push((rect, index));
            let kind = match resource.kind {
                crate::resource::ResourceKind::Prompt => "prompt",
                crate::resource::ResourceKind::Skill => "skill",
                crate::resource::ResourceKind::Mcp => "mcp",
            };
            let scope = match resource.scope {
                crate::resource::ResourceScope::Global => "global",
                crate::resource::ResourceScope::Project => "project",
                crate::resource::ResourceScope::Task => "task",
            };
            let state = if resource.enabled { "on" } else { "off" };
            let assigned = if library.task_id.is_some()
                && library
                    .assigned_resource_ids
                    .contains(&resource.resource_id)
            {
                "✓"
            } else {
                " "
            };
            let label = format!(
                " {assigned} {state:>3}  {kind:<6} {scope:<7} {} · {}",
                resource.name,
                resource
                    .capability_diagnostic
                    .as_deref()
                    .unwrap_or(resource.description.as_str())
            );
            let style = if index == library.selected {
                base.fg(contrast(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD)
            } else if resource.capability_diagnostic.is_some() {
                base.fg(p.yellow)
            } else {
                base.fg(p.text)
            };
            b.set_style(rect, style);
            put_text(b, rect.x, rect.y, rect.width, &label, style);
        }
    }
    let footer = if library.task_id.is_some() {
        " j/k select · enter assign/unassign · n new · e edit · d delete · r refresh · esc close "
    } else {
        " j/k select · enter toggle · n new · e edit · d delete · r refresh · esc close "
    };
    let footer_y = inner.bottom().saturating_sub(1);
    put_text(
        b,
        inner.x,
        footer_y,
        inner.width,
        footer,
        base.fg(p.overlay0),
    );
    Some(OverlayRender {
        resource_rows: rows,
        ..OverlayRender::default()
    })
}
