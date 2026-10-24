//! Diplomacy uses the loaded side artwork and font. Geometry/hit testing and
//! row eligibility share the same read-only projection with the input owner.
use super::{
    SHELL_CONTROL_TEXT_DEPTH, SHELL_LABEL_TEXT_RGB,
    in_game_shell::{self, InGameShellFrame},
};
use crate::app::{AppState, input::diplomacy};
use crate::render::shell_text::{self, ShellAlign, TextRect};
use crate::ui::shell::{diplomacy::DiplomacyButton, geom::RectPx};

pub(crate) fn render_diplomacy_shell(
    state: &mut AppState,
    encoder: &mut wgpu::CommandEncoder,
    destination: &wgpu::Texture,
) -> anyhow::Result<()> {
    let width = state.render_width() as i32;
    let height = state.render_height() as i32;
    let (shell, _) =
        in_game_shell::current_in_game_shell_layout(state).expect("active shell geometry");
    let layout = diplomacy::layout(state).expect("active diplomacy geometry");
    let rows = diplomacy::rows(state);
    let page = diplomacy::page(state, &layout, rows.len());
    let interaction = state.match_state.match_presentation.diplomacy_interaction;
    let atlas = crate::app::presentation::sidebar_render::current_sidebar_chrome(state)
        .expect("active side art");
    let mut art = in_game_shell::background_instances(atlas, shell, width, height);
    let mut texts = Vec::new();
    let mut text = |label: &str, rect: RectPx, enabled: bool, center: bool| {
        texts.push(shell_text::draw_in_rect(
            &state.renderer.bit_font,
            label,
            TextRect {
                x: rect.x,
                y: rect.y,
                w: rect.w.max(0) as u32,
                h: rect.h.max(0) as u32,
            },
            if enabled {
                SHELL_LABEL_TEXT_RGB
            } else {
                [0.6, 0.6, 0.6]
            },
            if center {
                ShellAlign::H_CENTER | ShellAlign::V_CENTER
            } else {
                ShellAlign::NONE
            },
            [0.0; 2],
            SHELL_CONTROL_TEXT_DEPTH,
        ));
    };
    text("Diplomacy", layout.title, true, false);
    let mut buttons = vec![(DiplomacyButton::Resume, layout.resume, "Resume", true)];
    if rows.len() > layout.page_size {
        buttons.push((
            DiplomacyButton::Previous,
            layout.previous,
            "Previous",
            page > 0,
        ));
        buttons.push((
            DiplomacyButton::Next,
            layout.next,
            "Next",
            (page + 1) * layout.page_size < rows.len(),
        ));
    }
    for (offset, (label, button)) in layout.rows.iter().enumerate() {
        let index = page * layout.page_size + offset;
        let Some(row) = rows.get(index) else {
            break;
        };
        text(
            &row.name,
            RectPx::new(label.x, label.y, label.w, 18),
            true,
            false,
        );
        text(
            &row.detail,
            RectPx::new(label.x, label.y + 18, label.w, 18),
            true,
            false,
        );
        buttons.push((
            DiplomacyButton::House(index),
            *button,
            row.action,
            row.enabled,
        ));
    }
    let footer = "AI teams are fixed at match start.
Esc, Tab or Enter resumes the game.";
    for (line, value) in footer.lines().enumerate() {
        text(
            value,
            RectPx::new(
                layout.footer.x,
                layout.footer.y + line as i32 * 18,
                layout.footer.w,
                18,
            ),
            true,
            false,
        );
    }
    for (id, rect, label, enabled) in buttons {
        let pressed = enabled && interaction.is_pressed(id);
        if let Some(entry) =
            atlas.in_game_shell.buttons[usize::from(pressed)].or(atlas.in_game_shell.buttons[0])
        {
            in_game_shell::push_art(&mut art, entry, rect, RectPx::new(0, 0, width, height));
        }
        text(
            label,
            super::pause_menu::button_text_rect(rect, pressed),
            enabled,
            true,
        );
    }
    in_game_shell::render_in_game_shell_frame(
        state,
        encoder,
        destination,
        InGameShellFrame {
            art,
            controls: Vec::new(),
            texts,
            label: "Diplomacy",
        },
    )
}
