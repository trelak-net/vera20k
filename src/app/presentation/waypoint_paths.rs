//! Tactical waypoint overlay. Geometry is clipped before rasterizing lines;
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
const ROUTE_TINT: [f32; 3] = [0.88; 3];

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
    // Use the shared one-pixel rasterizer, including contiguous diagonal
    // spans. Sampling separate 2x2 dots left holes and changed phase on pan.
    crate::render::surface_line::solid_line(
        [a.0.round() as i32, a.1.round() as i32],
        [b.0.round() as i32, b.1.round() as i32],
        |r| rect(output, r.map(|v| v as f32), tint),
    );
}
fn marker(output: &mut GroupLabelInstances, (x, y): (f32, f32), tint: [f32; 3]) {
    for r in [
        [x - 3.0, y - 3.0, 7.0, 1.0],
        [x - 3.0, y + 3.0, 7.0, 1.0],
        [x - 3.0, y - 2.0, 1.0, 5.0],
        [x + 3.0, y - 2.0, 1.0, 5.0],
    ] {
        rect(output, r, tint);
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
    let tint = ROUTE_TINT;
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
            // A small, unnumbered node remains clickable for patrols/Delete.
            // Control-group digits belong on units, never on waypoint paths.
            let color = if planning.selected_node == Some((path.id, i)) {
                [1.0; 3]
            } else {
                tint
            };
            marker(&mut out, (x, y), color);
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

#[cfg(test)]
mod fidelity_tests {
    use super::*;
    #[test]
    fn waypoint_route_is_neutral_unnumbered_and_continuous_at_large_coordinates() {
        let mut out = GroupLabelInstances::default();
        line(
            &mut out,
            (-20000.0, 12000.0),
            (20000.0, 12000.0),
            [-500.0, 11900.0, 500.0, 12500.0],
            ROUTE_TINT,
        );
        marker(&mut out, (320.0, 12000.0), ROUTE_TINT);
        assert!(out.text.is_empty());
        assert!(!out.boxes.is_empty());
        assert!(
            out.boxes
                .iter()
                .all(|r| r.tint[0] == r.tint[1] && r.tint[1] == r.tint[2] && r.tint[0] > 0.8)
        );
        // Every visible line pixel covered; generation is bounded by the view.
        for x in -500..=500 {
            assert!(out.boxes.iter().any(|r| x as f32 >= r.position[0]
                && (x as f32) < r.position[0] + r.size[0]
                && r.position[1] == 12000.0));
        }
        assert!(out.boxes.len() < 1100);
    }
}
