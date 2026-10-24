//! Control-group badges from DrawPipScalePips (gamemd 0x0070A6FD..0x0070A97B).
//! Group slots here already use their displayed digit (0..9), unlike retail's
//! internal 0..9 -> displayed 1..9,0. Membership remains owned by input state.

use crate::app::AppState;
use crate::app::input::commands::preferred_local_owner_name;
use crate::app::presentation::instances::in_view;
use crate::app::presentation::ui_overlays::{health_bar_hover_target, status_entity_visible_plain};
use crate::map::entities::EntityCategory;
use crate::render::batch::SpriteInstance;
use crate::rules::house_colors::{NO_REMAP, house_text_rgb};

#[derive(Default)]
pub(crate) struct GroupLabelInstances {
    pub boxes: Vec<SpriteInstance>,
    pub text: Vec<SpriteInstance>,
}

/// DrawHealthBar passes (-10,+10) to the pip routine for mobile objects
/// (0x006F6A8E). The number then adds (-4,-39) for Infantry, (-4,-36)
/// otherwise (0x0070A71C..0x0070A75B). PixelSelectionBracketDelta affects
/// the health strip, but neither of these original number coordinates.
fn text_position(category: EntityCategory, anchor: (f32, f32)) -> (f32, f32) {
    let y = if category == EntityCategory::Infantry {
        -39.0
    } else {
        -36.0
    };
    (anchor.0 - 4.0, anchor.1 + y)
}

fn rectangle(rect: [f32; 4], tint: [f32; 3]) -> SpriteInstance {
    SpriteInstance {
        position: [rect[0], rect[1]],
        size: [rect[2], rect[3]],
        uv_origin: [0.0; 2],
        uv_size: [1.0; 2],
        depth: 0.0004,
        tint,
        alpha: 1.0,
        ..Default::default()
    }
}

pub(crate) fn build(state: &AppState, sw: f32, sh: f32) -> GroupLabelInstances {
    let mut output = GroupLabelInstances::default();
    let groups = &state.match_state.input.control_groups;
    if groups.iter().all(Vec::is_empty) {
        return output;
    }
    let Some(sim) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
    else {
        return output;
    };
    let local_owner = preferred_local_owner_name(state);
    let owner_id = local_owner
        .as_deref()
        .and_then(|name| sim.interner.get(name));
    let hovered = health_bar_hover_target(state, local_owner.as_deref()).map(|(id, _)| id);
    let font = &state.renderer.bit_font;
    let rules = state.rules();
    let camera = [
        state.match_state.input.camera_x,
        state.match_state.input.camera_y,
    ];
    // Walk only grouped IDs, rather than scanning A20's entire entity store.
    for (digit, members) in groups.iter().take(10).enumerate() {
        let text = digit.to_string();
        for &id in members {
            let Some(entity) = sim.entities().get(id) else {
                continue;
            };
            if !entity.is_active()
                || !entity.is_alive()
                || entity.sinking.is_active()
                || entity.passenger_role.is_inside_transport()
                || (!entity.selected && hovered != Some(id))
                || !status_entity_visible_plain(
                    owner_id,
                    &sim.fog,
                    &entity.position,
                    entity.owner(),
                    state.match_state.sandbox_full_visibility,
                )
            {
                continue;
            }
            let (sx, sy) =
                crate::app::presentation::instances::interpolated_screen_position_entity(entity);
            let anchor = if entity.category == EntityCategory::Structure {
                let Some(obj) =
                    rules.and_then(|r| r.object(sim.interner.resolve(entity.type_ref())))
                else {
                    continue;
                };
                // Building DrawHealthBar admits the pip tail through
                // ShowOccupantPips; storage buildings return before the label.
                if !obj.show_occupant_pips
                    || obj.pip_scale == crate::rules::object_type::PipScale::Tiberium
                {
                    continue;
                }
                let (_, fh) = crate::rules::foundation::foundation_dimensions(&obj.foundation);
                (sx - f32::from(fh) * 30.0, sy + f32::from(fh) * 15.0 - 15.0)
            } else {
                (sx - 10.0, sy + 10.0)
            };
            let (x, y) = text_position(entity.category, anchor);
            // GetTextDimensions(0x49, xpad=2, ypad=-2), then widen left
            // by one pixel (0x0070A8AF). Black fill and house-color outline.
            let rect = [
                x - 3.0,
                y + 2.0,
                font.text_width(&text) as f32 + 5.0,
                font.cell_height() - 4.0,
            ];
            if !in_view(
                rect[0], rect[1], rect[2], rect[3], camera[0], camera[1], sw, sh, 0.0,
            ) {
                continue;
            }
            let color = state
                .match_state
                .match_presentation
                .house_color_map
                .get(sim.interner.resolve(entity.owner()))
                .copied()
                .unwrap_or(NO_REMAP);
            let rgb = house_text_rgb(rules.map_or(&[], |r| r.color_schemes.as_slice()), color);
            let tint = rgb.map(|c| f32::from(c) / 255.0);
            output.boxes.push(rectangle(rect, [0.0; 3]));
            let [bx, by, w, h] = rect;
            for edge in [
                [bx, by, w, 1.0],
                [bx, by + h - 1.0, w, 1.0],
                [bx, by + 1.0, 1.0, h - 2.0],
                [bx + w - 1.0, by + 1.0, 1.0, h - 2.0],
            ] {
                output.boxes.push(rectangle(edge, tint));
            }
            output
                .text
                .extend(font.build_text(&text, x, y, 1.0, 0.0004, tint, [0.0; 2]));
        }
    }
    output
}
