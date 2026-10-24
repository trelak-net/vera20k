//! Live player-list projection and diplomacy input; all mutations use the
//! synchronized command scheduler, never a presentation-side alliance graph.
use crate::app::{App, AppState};
use crate::sim::{command::Command, intern::InternedId};
use crate::ui::{
    pause_menu::InGameMenuState,
    shell::diplomacy::{DiplomacyButton, DiplomacyLayout, diplomacy_layout},
};
use winit::event::MouseButton;

pub(crate) struct DiplomacyRow {
    pub house: InternedId,
    pub name: String,
    pub detail: String,
    pub action: &'static str,
    pub enabled: bool,
}
pub(crate) fn rows(state: &AppState) -> Vec<DiplomacyRow> {
    let (Some(runtime), Some(rules)) = (state.match_state.sim_runtime.as_ref(), state.rules())
    else {
        return Vec::new();
    };
    let sim = &runtime.simulation;
    let Some(local) = sim.session.current_house() else {
        return Vec::new();
    };
    sim.session
        .house_order
        .iter()
        .filter_map(|owner| sim.houses.get(owner).map(|house| (*owner, house)))
        .filter(|(_, house)| !house.multiplay_passive)
        .enumerate()
        .map(|(index, (owner, house))| {
            let ours = sim.house_considers_ally(local, owner);
            let theirs = sim.house_considers_ally(owner, local);
            let relation = if owner == local {
                "You"
            } else if house.is_defeated {
                "Defeated"
            } else if ours && theirs {
                "Allied"
            } else if ours {
                "Alliance offered"
            } else if theirs {
                "Offers you an alliance"
            } else {
                "Enemy"
            };
            let name = if owner == local {
                state
                    .match_state
                    .match_presentation
                    .local_player_handle
                    .clone()
                    .unwrap_or_else(|| "You".into())
            } else {
                format!(
                    "{} {}",
                    if house.is_human { "Player" } else { "Computer" },
                    index + 1
                )
            };
            let country = sim.interner.resolve(house.country.unwrap_or(owner));
            let (ui_name, plain_name) = rules.country_display_name_sources(country);
            let country = ui_name
                .and_then(|key| state.process_assets.csf.as_ref()?.get(key))
                .or(plain_name)
                .unwrap_or(country);
            let block = sim.alliance_change_block(local, owner, rules);
            let enabled = block.is_none() && !house.is_defeated;
            let action = if house.is_defeated {
                "Defeated"
            } else if owner == local {
                "You"
            } else if !house.is_human {
                "Fixed (AI)"
            } else if block == Some(crate::sim::diplomacy::AllianceChangeBlock::LastEnemy) {
                "Last opponent"
            } else if !enabled {
                "Locked"
            } else if ours && theirs {
                "Break alliance"
            } else if ours {
                "Cancel offer"
            } else if theirs {
                "Accept alliance"
            } else {
                "Offer alliance"
            };
            DiplomacyRow {
                house: owner,
                name,
                detail: format!("{country} - {relation}"),
                action,
                enabled,
            }
        })
        .collect()
}
pub(crate) fn layout(state: &AppState) -> Option<DiplomacyLayout> {
    let (shell, size) =
        crate::app::frontend::skirmish_shell_render::current_in_game_shell_layout(state)?;
    Some(diplomacy_layout(
        state.render_width() as i32,
        state.render_height() as i32,
        shell,
        size,
    ))
}
pub(crate) fn page(state: &AppState, layout: &DiplomacyLayout, count: usize) -> usize {
    state
        .match_state
        .match_presentation
        .diplomacy_page
        .min(count.saturating_sub(1) / layout.page_size)
}
fn hit(state: &AppState) -> Option<DiplomacyButton> {
    let layout = layout(state)?;
    let rows = rows(state);
    let page = page(state, &layout, rows.len());
    let (x, y) = state.cursor_position();
    let contains =
        |rect: crate::ui::shell::geom::RectPx| rect.contains(x.round() as i32, y.round() as i32);
    if contains(layout.resume) {
        return Some(DiplomacyButton::Resume);
    }
    if page > 0 && contains(layout.previous) {
        return Some(DiplomacyButton::Previous);
    }
    if (page + 1) * layout.page_size < rows.len() && contains(layout.next) {
        return Some(DiplomacyButton::Next);
    }
    layout
        .rows
        .iter()
        .enumerate()
        .find_map(|(offset, (_, rect))| {
            let index = page * layout.page_size + offset;
            (contains(*rect) && rows.get(index).is_some_and(|row| row.enabled))
                .then_some(DiplomacyButton::House(index))
        })
}
pub(crate) fn cursor_moved(state: &mut AppState) {
    state
        .match_state
        .match_presentation
        .diplomacy_interaction
        .hovered = hit(state);
}
pub(crate) fn mouse(state: &mut AppState, button: MouseButton, pressed: bool) {
    if button != MouseButton::Left {
        return;
    }
    let over = hit(state);
    if pressed {
        state
            .match_state
            .match_presentation
            .diplomacy_interaction
            .press(over);
        if over.is_some() {
            App::play_skirmish_shell_generic_click_sound(state);
        }
    } else if let Some(button) = state
        .match_state
        .match_presentation
        .diplomacy_interaction
        .release(over)
    {
        match button {
            DiplomacyButton::Resume => {
                App::enter_in_game_menu_state(state, InGameMenuState::Closed)
            }
            DiplomacyButton::Previous => {
                state.match_state.match_presentation.diplomacy_page = state
                    .match_state
                    .match_presentation
                    .diplomacy_page
                    .saturating_sub(1);
            }
            DiplomacyButton::Next => {
                state.match_state.match_presentation.diplomacy_page += 1;
            }
            DiplomacyButton::House(index) => {
                if let Some(row) = rows(state).get(index)
                    && row.enabled
                {
                    queue(state, row.house);
                }
            }
        }
    }
}
fn queue(state: &mut AppState, target: InternedId) {
    let Some(runtime) = state.match_state.sim_runtime.as_ref() else {
        return;
    };
    let Some(owner) = runtime.simulation.session.current_house() else {
        return;
    };
    let name = runtime.simulation.interner.resolve(owner).to_string();
    crate::app::input::planning::finish(state);
    App::enter_in_game_menu_state(state, InGameMenuState::Closed);
    crate::app::input::commands::schedule_command(state, &name, Command::ToggleAlliance { target });
    log::info!(
        "Alliance command queued: owner={} target={:?}",
        name,
        target
    );
}
pub(crate) fn toggle_selected(state: &mut AppState) {
    let (Some(runtime), Some(rules)) = (state.match_state.sim_runtime.as_ref(), state.rules())
    else {
        return;
    };
    let sim = &runtime.simulation;
    let selection = crate::app::input::dispatch::selected_stable_ids_in_order(
        Some(sim),
        Some(rules),
        &state.match_state.input.selection_order,
        state.match_state.input.selection_order_pending,
    );
    let Some(target) = selection
        .first()
        .and_then(|id| sim.entities().get(*id))
        .map(|entity| entity.owner())
    else {
        return;
    };
    let Some(local) = sim.session.current_house() else {
        return;
    };
    if let Some(reason) = sim.alliance_change_block(local, target, rules) {
        crate::app::input::messages::post_system_message(state, reason.message());
    } else {
        queue(state, target);
    }
}
