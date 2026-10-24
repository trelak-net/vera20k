//! Held keyboard / latched command-bar input for player waypoint routes.
//! Drafts and progress belong to Simulation and use its normal command queue.
use crate::app::AppState;
use crate::app::input::{
    commands::{preferred_local_owner_name, schedule_command},
    hotkeys::{HotkeyCommand, HotkeyResolution},
};
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::planning::PlannedOrder;
use winit::{event::ElementState, keyboard::KeyCode};

#[derive(Default)]
pub(crate) struct PlanningInput {
    held: Option<KeyCode>,
    latched: bool,
    pub selected_node: Option<(u32, usize)>,
}
impl PlanningInput {
    pub(crate) fn active(&self) -> bool {
        self.held.is_some() || self.latched
    }
    fn press(&mut self, key: KeyCode, repeat: bool) {
        if !repeat {
            self.held = Some(key);
        }
    }
    fn release(&mut self, key: KeyCode) -> bool {
        if self.held == Some(key) {
            self.held = None;
            true
        } else {
            false
        }
    }
}
fn sound(state: &mut AppState, index: usize) {
    if let Some(sound_id) = state
        .rules()
        .and_then(|r| r.general.planning_sounds[index].clone())
    {
        state
            .match_state
            .match_audio
            .sound_events
            .push(crate::audio::events::GameSoundEvent::UiSound { sound_id });
    }
}
fn commit(state: &mut AppState) {
    if let Some(owner) = preferred_local_owner_name(state) {
        schedule_command(state, &owner, Command::PlanCommit);
    }
    state.match_state.input.planning.selected_node = None;
    sound(state, 1);
    log::debug!("Waypoint mode committed");
}
pub(crate) fn key_edge(
    state: &mut AppState,
    resolution: HotkeyResolution,
    key: KeyCode,
    edge: ElementState,
    repeat: bool,
    admitted: bool,
) -> bool {
    let before = state.match_state.input.planning.active();
    if edge.is_pressed() {
        if resolution != HotkeyResolution::Command(HotkeyCommand::PlanningMode) || !admitted {
            return false;
        }
        state.match_state.input.planning.press(key, repeat);
        if !before {
            sound(state, 0);
            log::debug!("Waypoint mode started");
        }
        true
    } else if state.match_state.input.planning.release(key) {
        if before && !state.match_state.input.planning.active() {
            commit(state);
        }
        true
    } else {
        false
    }
}
pub(crate) fn toggle(state: &mut AppState) {
    if state.match_state.input.planning.active() {
        finish(state);
    } else {
        state.match_state.input.planning.latched = true;
        sound(state, 0);
    }
}
pub(crate) fn finish(state: &mut AppState) {
    if state.match_state.input.planning.active() {
        state.match_state.input.planning.held = None;
        state.match_state.input.planning.latched = false;
        commit(state);
    }
}
pub(crate) fn capture_orders(state: &mut AppState, queued: Vec<CommandEnvelope>) -> bool {
    let Some(runtime) = state.match_state.sim_runtime.as_ref() else {
        return true;
    };
    let orders = queued
        .iter()
        .filter_map(|e| PlannedOrder::from_command(&e.payload, &runtime.simulation))
        .collect::<Vec<_>>();
    if orders.is_empty() {
        return true;
    }
    let owner = runtime
        .simulation
        .interner
        .resolve(queued[0].owner)
        .to_owned();
    schedule_command(state, &owner, Command::PlanConnect { orders });
    state.match_state.input.planning.selected_node = None;
    sound(state, 2);
    true
}

pub(crate) fn node_position(sim: &crate::sim::world::Simulation, cell: (u16, u16)) -> (f32, f32) {
    let z = sim
        .resolved_terrain
        .as_ref()
        .and_then(|g| g.cell(cell.0, cell.1))
        .map_or(0, |c| c.bridge_deck_level_if_any().unwrap_or(c.level));
    crate::util::lepton::lepton_to_screen(
        cell.0,
        cell.1,
        crate::util::fixed_math::SimFixed::from_num(128),
        crate::util::fixed_math::SimFixed::from_num(128),
        z,
    )
}
pub(crate) fn node_visible(
    state: &AppState,
    cell: (u16, u16),
    owner: crate::sim::intern::InternedId,
) -> bool {
    let Some(rt) = state.match_state.sim_runtime.as_ref() else {
        return false;
    };
    if state.match_state.sandbox_full_visibility {
        return true;
    }
    let Some(terrain) = rt.simulation.resolved_terrain.as_ref() else {
        return false;
    };
    !crate::sim::vision::cell_is_shrouded(
        &rt.simulation.fog,
        &crate::map::resolved_terrain::NativeCellQuery::canonical(terrain),
        owner,
        cell,
    )
}
pub(crate) fn hit_node(state: &AppState, sx: f32, sy: f32) -> Option<(u32, usize)> {
    let sim = &state.match_state.sim_runtime.as_ref()?.simulation;
    let owner = preferred_local_owner_name(state).and_then(|o| sim.interner.get(&o))?;
    let (wx, wy) = crate::app::match_runtime::sim_tick::screen_point_to_world(state, sx, sy);
    let paths = sim.waypoint_plans.houses.get(&owner)?;
    for path in paths.iter().rev() {
        let first = if path.loop_to.is_some() {
            0
        } else {
            path.members.values().map(|m| m.node).min().unwrap_or(0)
        };
        for (i, node) in path.nodes.iter().enumerate().skip(first) {
            let cell = node.first()?.cell;
            if !node_visible(state, cell, owner) {
                continue;
            }
            let (x, y) = node_position(sim, cell);
            if (wx - x).abs() <= 9.0 && (wy - y).abs() <= 9.0 {
                return Some((path.id, i));
            }
        }
    }
    None
}
pub(crate) fn click_node(state: &mut AppState, sx: f32, sy: f32) -> bool {
    if !state.match_state.input.planning.active() {
        return false;
    }
    let Some((path_id, node)) = hit_node(state, sx, sy) else {
        return false;
    };
    state.match_state.input.planning.selected_node = Some((path_id, node));
    let Some(owner) = preferred_local_owner_name(state) else {
        return true;
    };
    let Some(rt) = state.match_state.sim_runtime.as_ref() else {
        return true;
    };
    let sim = &rt.simulation;
    let selected = crate::app::input::dispatch::selected_stable_ids_in_order(
        Some(sim),
        state.rules(),
        &state.match_state.input.selection_order,
        state.match_state.input.selection_order_pending,
    );
    let owns_path = sim
        .interner
        .get(&owner)
        .and_then(|id| sim.waypoint_plans.houses.get(&id))
        .and_then(|p| p.iter().find(|p| p.id == path_id))
        .is_some_and(|p| {
            !selected.is_empty() && selected.iter().all(|id| p.members.contains_key(id))
        });
    if owns_path {
        schedule_command(state, &owner, Command::PlanLoop { path_id, node });
    }
    true
}
pub(crate) fn delete_node(state: &mut AppState) {
    if !state.match_state.input.planning.active() {
        return;
    }
    let selected = state.match_state.input.planning.selected_node.or_else(|| {
        hit_node(
            state,
            state.match_state.input.cursor_x,
            state.match_state.input.cursor_y,
        )
    });
    if let (Some((path_id, node)), Some(owner)) = (selected, preferred_local_owner_name(state)) {
        schedule_command(state, &owner, Command::PlanDelete { path_id, node });
        state.match_state.input.planning.selected_node = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waypoint_held_key_release_tracks_physical_key_and_ignores_repeat() {
        let mut p = PlanningInput::default();
        p.press(KeyCode::KeyZ, true);
        assert!(!p.active());
        p.press(KeyCode::KeyZ, false);
        assert!(p.active());
        p.press(KeyCode::KeyZ, true);
        assert!(p.active());
        assert!(!p.release(KeyCode::ShiftLeft));
        assert!(p.active());
        assert!(p.release(KeyCode::KeyZ));
        assert!(!p.active());
    }
    #[test]
    fn waypoint_toolbar_latch_survives_keyboard_release() {
        let mut p = PlanningInput {
            latched: true,
            ..Default::default()
        };
        p.press(KeyCode::KeyZ, false);
        p.release(KeyCode::KeyZ);
        assert!(p.active());
    }
}

/// The original manual permits planning from the radar as well as the map.
/// Reuse the radar's canonical target and ordinary tactical context resolver.
pub(crate) fn radar_click(state: &mut AppState) -> bool {
    if !state.match_state.input.planning.active() {
        return false;
    }
    let Some(runtime) = state.match_state.sim_runtime.as_ref() else {
        return true;
    };
    let view = runtime.view();
    let (Some(terrain), Some(map_size), Some(minimap)) = (
        view.resolved_terrain(),
        view.playfield_map_size(),
        state.match_state.match_presentation.minimap.as_ref(),
    ) else {
        return true;
    };
    let rect = crate::app::presentation::sidebar_render::active_minimap_screen_rect(state);
    let size = crate::app::input::camera::tactical_viewport_size_px(
        state.render_width(),
        state.render_height(),
    );
    let Some(target) = minimap.native_click_target_in_rect(
        state.match_state.input.cursor_x,
        state.match_state.input.cursor_y,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        view.entities(),
        terrain,
        map_size,
        (size.0 as i32, size.1 as i32),
    ) else {
        return true;
    };
    crate::app::input::context_order::try_queue_context_order_at_cell(state, target.cell);
    true
}
