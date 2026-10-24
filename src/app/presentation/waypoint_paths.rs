//! Tactical waypoint overlay. Geometry is clipped before generating dashes;
//! only local planned actors are visited, even on large custom maps.
use crate::app::presentation::control_group_labels::GroupLabelInstances;
use crate::app::{
    AppState,
    input::{
        commands::preferred_local_owner_name,
        planning::{node_position, node_visible},
    },
};
use crate::render::batch::SpriteInstance;
use crate::rules::house_colors::{NO_REMAP, house_text_rgb};

fn rect(output: &mut GroupLabelInstances, r: [f32; 4], tint: [f32; 3]) {
    output.boxes.push(SpriteInstance {
        position: [r[0], r[1]],
        size: [r[2], r[3]],
        uv_origin: [0.0; 2],
        uv_size: [1.0; 2],
        depth: 0.0004,
        tint,
        alpha: 1.0,
        ..Default::default()
    });
}
// Liang–Barsky clipping bounds both the generated vertices and CPU work.
fn clip(a: (f32, f32), b: (f32, f32), view: [f32; 4]) -> Option<((f32, f32), (f32, f32))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-dx, a.0 - view[0]),
        (dx, view[2] - a.0),
        (-dy, a.1 - view[1]),
        (dy, view[3] - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                lo = lo.max(t)
            } else {
                hi = hi.min(t)
            }
            if lo > hi {
                return None;
            }
        }
    }
    Some((
        (a.0 + dx * lo, a.1 + dy * lo),
        (a.0 + dx * hi, a.1 + dy * hi),
    ))
}
fn line(
    output: &mut GroupLabelInstances,
    a: (f32, f32),
    b: (f32, f32),
    view: [f32; 4],
    tint: [f32; 3],
) {
    let Some((a, b)) = clip(a, b, view) else {
        return;
    };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx.hypot(dy);
    if length < 1.0 {
        return;
    }
    for i in (0..length.ceil() as usize).step_by(3) {
        if i % 12 >= 6 {
            continue;
        }
        let t = i as f32 / length;
        rect(output, [a.0 + dx * t, a.1 + dy * t, 2.0, 2.0], tint);
    }
}
pub(crate) fn build(state: &AppState, sw: f32, sh: f32) -> GroupLabelInstances {
    let mut out = GroupLabelInstances::default();
    let Some(rt) = state.match_state.sim_runtime.as_ref() else {
        return out;
    };
    let sim = &rt.simulation;
    let Some(owner) = preferred_local_owner_name(state).and_then(|o| sim.interner.get(&o)) else {
        return out;
    };
    let Some(paths) = sim.waypoint_plans.houses.get(&owner) else {
        return out;
    };
    let planning = &state.match_state.input.planning;
    let x = state.match_state.input.camera_x;
    let y = state.match_state.input.camera_y;
    let view = [x - 10.0, y - 10.0, x + sw + 10.0, y + sh + 10.0];
    let color = state
        .match_state
        .match_presentation
        .house_color_map
        .get(sim.interner.resolve(owner))
        .copied()
        .unwrap_or(NO_REMAP);
    let tint = house_text_rgb(
        state.rules().map_or(&[], |r| r.color_schemes.as_slice()),
        color,
    )
    .map(|c| f32::from(c) / 255.0);
    let font = &state.renderer.bit_font;
    for path in paths {
        if !planning.active()
            && !path
                .members
                .keys()
                .any(|id| sim.entities().get(*id).is_some_and(|e| e.selected))
        {
            continue;
        }
        let first = if path.loop_to.is_some() {
            0
        } else {
            path.members.values().map(|m| m.node).min().unwrap_or(0)
        };
        let point = |index: usize| {
            path.nodes
                .get(index)
                .and_then(|n| n.first())
                .filter(|o| node_visible(state, o.cell, owner))
                .map(|o| node_position(sim, o.cell))
        };
        for i in first..path.nodes.len() {
            if let (Some(a), Some(b)) = (point(i), point(i + 1)) {
                line(&mut out, a, b, view, tint);
            }
        }
        if let Some(loop_to) = path.loop_to {
            if let (Some(a), Some(b)) = (point(path.nodes.len() - 1), point(loop_to)) {
                line(&mut out, a, b, view, tint);
            }
        }
        // The first leader's current position makes a one-node route visible.
        if let Some((&id, p)) = path.members.first_key_value() {
            if let (Some(e), Some(b)) = (sim.entities().get(id), point(p.node)) {
                if node_visible(state, (e.position.rx, e.position.ry), owner) {
                    let a =
                        crate::app::presentation::instances::interpolated_screen_position_entity(e);
                    line(&mut out, a, b, view, tint);
                }
            }
        }
        for i in first..path.nodes.len() {
            let Some((x, y)) = point(i) else { continue };
            if x < view[0] || x > view[2] || y < view[1] || y > view[3] {
                continue;
            }
            let text = (i + 1).to_string();
            let w = font.text_width(&text) as f32 + 6.0;
            let h = font.cell_height() - 2.0;
            let (bx, by) = (x - w / 2.0, y - h / 2.0);
            let color = if planning.selected_node == Some((path.id, i)) {
                [1.0; 3]
            } else {
                tint
            };
            rect(&mut out, [bx, by, w, h], [0.0; 3]);
            for edge in [
                [bx, by, w, 1.0],
                [bx, by + h - 1.0, w, 1.0],
                [bx, by, 1.0, h],
                [bx + w - 1.0, by, 1.0, h],
            ] {
                rect(&mut out, edge, color);
            }
            out.text.extend(font.build_text(
                &text,
                bx + 3.0,
                by - 1.0,
                1.0,
                0.0004,
                color,
                [0.0; 2],
            ));
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waypoint_line_clipping_bounds_work_on_large_maps() {
        assert_eq!(
            clip(
                (-1_000_000.0, 10.0),
                (1_000_000.0, 10.0),
                [0.0, 0.0, 800.0, 600.0]
            ),
            Some(((0.0, 10.0), (800.0, 10.0)))
        );
        assert!(clip((-5.0, -5.0), (-1.0, -1.0), [0.0, 0.0, 800.0, 600.0]).is_none());
    }
}
