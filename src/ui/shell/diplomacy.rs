//! Diplomacy table geometry shared by rendering and input. Uses the existing
//! active-game shell at logical battlefield resolution, including UI zoom.
use super::geom::RectPx;
use super::in_game_shell::InGameShellLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiplomacyButton {
    House(usize),
    Previous,
    Next,
    Resume,
}
#[derive(Debug, Clone)]
pub struct DiplomacyLayout {
    pub title: RectPx,
    pub footer: RectPx,
    pub rows: Vec<(RectPx, RectPx)>,
    pub previous: RectPx,
    pub next: RectPx,
    pub resume: RectPx,
    pub page_size: usize,
}
pub fn diplomacy_layout(
    width: i32,
    height: i32,
    shell: InGameShellLayout,
    button: [i32; 2],
) -> DiplomacyLayout {
    let page_size = ((height - 145) / 40).max(1) as usize;
    let action_x = width - 184 - button[0];
    DiplomacyLayout {
        title: RectPx::new(24, 30, width - 215, 24),
        footer: RectPx::new(24, height - 68, width - 215, 44),
        rows: (0..page_size)
            .map(|row| {
                let y = 72 + row as i32 * 40;
                (
                    RectPx::new(24, y, (action_x - 32).max(0), 38),
                    RectPx::new(action_x, y + 3, button[0], button[1]),
                )
            })
            .collect(),
        previous: RectPx::new(
            width - 147,
            shell.side3.y - button[1] - 90,
            button[0],
            button[1],
        ),
        next: RectPx::new(
            width - 147,
            shell.side3.y - button[1] - 50,
            button[0],
            button[1],
        ),
        resume: RectPx::new(width - 147, shell.side3.y - button[1], button[0], button[1]),
        page_size,
    }
}
