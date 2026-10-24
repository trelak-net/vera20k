//! Retained semantic model for the retail launcher Options dialog `0xD5`.
//!
//! The parent owns bounded control state and emits ordered effects. It owns no
//! profile, filesystem, audio device, window, or child-dialog callback.
//! The `shell` child supplies retail resource geometry and physical-pixel input;
//! the app shell renderer paints that model through the shared retail compositor.

use crate::ui::shell::trackbar::{
    TrackbarHold, TrackbarPress, trackbar_position_from_x, trackbar_press,
};

pub(crate) mod shell;

/// gamemd-derived: launcher owner `OptionsClass__ShowLauncherDialog @
/// 0x0055FC80` and its primary-proc slice `0x0055FDB0..0x0056047A` bind the
/// RT_DIALOG `0xD5` controls to these launcher-local CSF keys/fallbacks. The
/// template's slider captions (`GUI:HigherDetail`, `GUI:Harder`,
/// `GUI:Faster`) never show: the init renames them by position. The combo's
/// own caption, `GUI:SetResolution`, left with the resolution list: the combo
/// is VERA's [`Zoom`] list now.
pub(crate) const LAUNCHER_LABEL_SPECS: [(&str, &str); 30] = [
    ("GUI:OptionsMenu", "Options"),
    ("GUI:MainMenu", "Main Menu"),
    ("GUI:Keyboard", "Keyboard"),
    ("GUI:Network", "Network"),
    ("GUI:DisplayOptions", "Display Options"),
    ("GUI:GameOptions", "Game Options"),
    ("GUI:UIOptions", "UI Options"),
    ("GUI:AudioOptions", "Audio Options"),
    ("GUI:VisualDetails", "Visual Details"),
    ("GUI:Difficulty", "Difficulty"),
    ("GUI:Tooltips", "Tooltips"),
    ("GUI:ScrollRate", "Scroll Rate"),
    ("GUI:TargetLines", "Target Lines"),
    ("GUI:ShowHidden", "See Hidden Objects"),
    ("GUI:MusicVolume", "Music Volume"),
    ("GUI:SoundVolume", "Sound Volume"),
    ("GUI:VoiceVolume", "Voice Volume"),
    ("GUI:Blank", ""),
    ("TXT_LOW", "Low"),
    ("TXT_HIGH", "High"),
    ("TXT_EASY", "Easy"),
    ("TXT_NORMAL", "Normal"),
    ("TXT_HARD", "Hard"),
    ("TXT_SLOWEST", "Slowest"),
    ("TXT_SLOWER", "Slower"),
    ("TXT_SLOW", "Slow"),
    ("TXT_MEDIUM", "Medium"),
    ("TXT_FAST", "Fast"),
    ("TXT_FASTER", "Faster"),
    ("TXT_FASTEST", "Fastest"),
];

/// VERA's own captions and status help on this page. The retail string table
/// has nothing for full screen or zoom, so these are English everywhere.
pub(crate) const RESOLUTIONS: [(u32, u32, &str); 7] = [
    (800, 600, "800 x 600"),
    (1024, 768, "1024 x 768"),
    (1280, 960, "1280 x 960"),
    (1600, 900, "1600 x 900"),
    (1920, 1080, "1920 x 1080"),
    (1920, 1200, "1920 x 1200"),
    (2560, 1440, "2560 x 1440"),
];
pub(crate) const ZOOM_LABEL: &str = "Zoom";
pub(crate) const ZOOM_HELP: &str =
    "How big battles look. Auto picks the best size for your screen.";
pub(crate) const FULL_SCREEN_LABEL: &str = "Full Screen";
pub(crate) const FULL_SCREEN_HELP: &str = "Fill the whole screen. Untick it to play in a window.";

/// VERA's battle zoom: the Options combo that took the place of gamemd's
/// resolution list (`0x005601A0..0x00560270`). The window keeps the size the
/// player gives it, and a zoomed match draws its whole frame (battlefield,
/// sidebar, cursor) at the window size divided by the factor, then scales it
/// up by whole pixels. The menus fit the window on their own. Saved as
/// `[VERA] Zoom` in RA2MD.INI, a section gamemd never reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Zoom {
    /// The largest factor that still shows the original 800x600 view.
    #[default]
    Auto,
    Normal,
    Large,
    Huge,
}

impl Zoom {
    /// The combo's rows, top to bottom.
    pub(crate) const ALL: [Self; 4] = [Self::Auto, Self::Normal, Self::Large, Self::Huge];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Normal => "100% (1x)",
            Self::Large => "200% (2x)",
            Self::Huge => "300% (3x)",
        }
    }

    pub(crate) fn row(self) -> usize {
        Self::ALL.iter().position(|zoom| *zoom == self).unwrap_or(0)
    }

    /// The `[VERA] Zoom` value: 0 for Auto, otherwise the fixed factor.
    pub(crate) const fn ini_value(self) -> i32 {
        match self {
            Self::Auto => 0,
            Self::Normal => 1,
            Self::Large => 2,
            Self::Huge => 3,
        }
    }

    /// Any value this build doesn't know reads as Auto.
    pub(crate) const fn from_ini_value(value: i32) -> Self {
        match value {
            1 => Self::Normal,
            2 => Self::Large,
            3 => Self::Huge,
            _ => Self::Auto,
        }
    }

    /// The whole-pixel factor for a window of `width` x `height`. No choice
    /// shrinks the view below gamemd's smallest mode, 640x480.
    pub(crate) fn factor(self, width: u32, height: u32) -> u32 {
        let wanted = match self {
            Self::Auto => (width / 800).min(height / 600),
            Self::Normal => 1,
            Self::Large => 2,
            Self::Huge => 3,
        };
        wanted.min((width / 640).min(height / 480)).max(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LauncherOptionsLabels {
    pub(crate) options: String,
    pub(crate) main_menu: String,
    pub(crate) keyboard: String,
    pub(crate) network: String,
    pub(crate) display_options: String,
    pub(crate) game_options: String,
    pub(crate) ui_options: String,
    pub(crate) audio_options: String,
    pub(crate) visual_details: String,
    pub(crate) difficulty: String,
    pub(crate) tooltips: String,
    pub(crate) scroll_rate: String,
    pub(crate) target_lines: String,
    pub(crate) show_hidden: String,
    pub(crate) music_volume: String,
    pub(crate) sound_volume: String,
    pub(crate) voice_volume: String,
    pub(crate) blank: String,
    low: String,
    high: String,
    easy: String,
    normal: String,
    hard: String,
    scroll_tokens: [String; 7],
}

impl LauncherOptionsLabels {
    pub(crate) fn resolve(csf: &dyn Fn(&str) -> Option<String>) -> Self {
        let label = |index: usize| {
            let (key, fallback) = LAUNCHER_LABEL_SPECS[index];
            csf(key).unwrap_or_else(|| fallback.to_string())
        };
        Self {
            options: label(0),
            main_menu: label(1),
            keyboard: label(2),
            network: label(3),
            display_options: label(4),
            game_options: label(5),
            ui_options: label(6),
            audio_options: label(7),
            visual_details: label(8),
            difficulty: label(9),
            tooltips: label(10),
            scroll_rate: label(11),
            target_lines: label(12),
            show_hidden: label(13),
            music_volume: label(14),
            sound_volume: label(15),
            voice_volume: label(16),
            blank: label(17),
            low: label(18),
            high: label(19),
            easy: label(20),
            normal: label(21),
            hard: label(22),
            scroll_tokens: [
                label(23),
                label(24),
                label(25),
                label(26),
                label(27),
                label(28),
                label(29),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LauncherTrackbarId {
    Detail,
    Difficulty,
    Scroll,
    Score,
    Sound,
    Voice,
}

impl LauncherTrackbarId {
    const fn maximum(self) -> u8 {
        match self {
            Self::Detail => 1,
            Self::Difficulty => 2,
            Self::Scroll => 6,
            Self::Score | Self::Sound | Self::Voice => 10,
        }
    }

    /// The value plaque's reserve; Detail, Difficulty and Scroll turn the
    /// plaque off (`0x4AC` with 0).
    pub(crate) const fn plaque_reserve(self) -> i32 {
        match self {
            Self::Detail | Self::Difficulty | Self::Scroll => 0,
            Self::Score | Self::Sound | Self::Voice => 50,
        }
    }

    const fn is_audio(self) -> bool {
        matches!(self, Self::Score | Self::Sound | Self::Voice)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LauncherCheckboxId {
    Tooltips,
    TargetLines,
    ShowHidden,
    /// VERA's own box: the window fills the screen. It changes the window at
    /// once instead of waiting for the parent's accept.
    FullScreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LauncherCue {
    MainButton,
    GenericClick,
    Checkbox,
    ComboOpen,
    ComboClose,
}

#[derive(Debug, Clone, PartialEq)]
/// gamemd-derived: notification order in launcher primary-proc slice
/// `0x0055FDB0..0x0056047A`; owner `OptionsClass__ShowLauncherDialog @
/// 0x0055FC80` consumes these before any parent-result teardown.
pub(crate) enum LauncherOptionsEvent {
    Cue(LauncherCue),
    ZoomSelected(Zoom),
    ResolutionSelected(u32, u32),
    FullScreenChanged(bool),
    ScorePreview(f32),
    SoundPreview(f32),
    VoicePreview(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// gamemd-derived: the distinct exits owned by
/// `OptionsClass__ShowLauncherDialog @ 0x0055FC80`; every accepted result first
/// projects through `OptionsClass__ApplyFromLauncherDialog @ 0x0055FAA0`.
pub(crate) enum LauncherParentResult {
    Back,
    Network,
    Keyboard,
    Terminal,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LauncherOptionsFrameOutput {
    pub(crate) events: Vec<LauncherOptionsEvent>,
    pub(crate) result: Option<LauncherParentResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LauncherOptionsValues {
    pub(crate) detail_position: u8,
    pub(crate) difficulty_position: u8,
    pub(crate) scroll_position: u8,
    pub(crate) tooltips: bool,
    pub(crate) target_lines: bool,
    pub(crate) show_hidden: bool,
    /// The window's state when the page opened, then each toggle.
    pub(crate) full_screen: bool,
    pub(crate) score_position: u8,
    pub(crate) sound_position: u8,
    pub(crate) voice_position: u8,
}

impl Default for LauncherOptionsValues {
    fn default() -> Self {
        Self {
            detail_position: 1,
            difficulty_position: 1,
            scroll_position: 3,
            tooltips: true,
            target_lines: true,
            show_hidden: false,
            full_screen: false,
            score_position: 4,
            sound_position: 7,
            voice_position: 7,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LauncherOptionsPacked {
    pub(crate) detail_level: i32,
    pub(crate) difficulty: i32,
    pub(crate) unit_action_lines: bool,
    pub(crate) show_hidden: bool,
    pub(crate) tooltips: bool,
    pub(crate) scroll_rate: i32,
    pub(crate) score_volume: f32,
    pub(crate) sound_volume: f32,
    pub(crate) voice_volume: f32,
}

/// One integer-pixel input frame from the retail control rectangle used by
/// the shell renderer and input admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PhysicalControlFrame {
    pub(crate) local_x: i32,
    pub(crate) local_y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

/// gamemd-derived: the fresh launcher trackbars created by primary-proc slice
/// `0x0055FDB0..0x0056047A` reject an out-of-range set-position request and
/// retain constructor position zero rather than clamping it.
pub(crate) fn admitted_initial_position(requested: i64, maximum: u8) -> u8 {
    u8::try_from(requested)
        .ok()
        .filter(|value| *value <= maximum)
        .unwrap_or(0)
}

/// gamemd-derived: launcher volume setup in primary-proc slice
/// `0x0055FDB0..0x0056047A`, fed by setters `0x005FA4A0/0x005FA510/0x005FA590`,
/// forms the x87 request `trunc(volume * 10 + 0.5)` before range admission.
pub(crate) fn admitted_volume_position(volume: f32) -> u8 {
    let request = f64::from(volume) * 10.0 + 0.5;
    if !request.is_finite() {
        return 0;
    }
    admitted_initial_position(request.trunc() as i64, 10)
}

/// Detail caption by position: `TXT_LOW`, else `TXT_HIGH` (table
/// `0x0082A248` entries 0 and 2).
fn detail_caption(labels: &LauncherOptionsLabels, position: u8) -> String {
    if position == 0 {
        labels.low.clone()
    } else {
        labels.high.clone()
    }
}

/// Difficulty caption by position (table `0x0082A254`).
fn difficulty_caption(labels: &LauncherOptionsLabels, position: u8) -> String {
    match position {
        0 => labels.easy.clone(),
        1 => labels.normal.clone(),
        _ => labels.hard.clone(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OptionsDialogState {
    labels: LauncherOptionsLabels,
    values: LauncherOptionsValues,
    zoom: Zoom,
    zoom_popup_open: bool,
    resolution_popup_open: bool,
    resolution: (u32, u32),
    launcher_audio_available: bool,
    detail_caption: String,
    difficulty_caption: String,
    scroll_caption: String,
    /// The slider holding the mouse since a press on it.
    capture: Option<TrackbarHold<LauncherTrackbarId>>,
    pending_events: Vec<LauncherOptionsEvent>,
    pending_result: Option<LauncherParentResult>,
    shell_interaction: shell::ShellInteraction,
}

impl Default for OptionsDialogState {
    fn default() -> Self {
        let labels = LauncherOptionsLabels::resolve(&|_| None);
        Self::new(labels, LauncherOptionsValues::default(), Zoom::Auto, false)
    }
}

impl OptionsDialogState {
    /// gamemd-derived: `OptionsClass__ShowLauncherDialog @ 0x0055FC80`
    /// creates each fresh `0xD5` parent, while primary-proc slice
    /// `0x0055FDB0..0x0056047A` admits initial positions and preserves the
    /// resource Detail/Difficulty/Scroll captions until a changed notification.
    pub(crate) fn new(
        labels: LauncherOptionsLabels,
        mut values: LauncherOptionsValues,
        zoom: Zoom,
        launcher_audio_available: bool,
    ) -> Self {
        values.detail_position = admitted_initial_position(i64::from(values.detail_position), 1);
        values.difficulty_position =
            admitted_initial_position(i64::from(values.difficulty_position), 2);
        values.scroll_position = admitted_initial_position(i64::from(values.scroll_position), 6);
        values.score_position = admitted_initial_position(i64::from(values.score_position), 10);
        values.sound_position = admitted_initial_position(i64::from(values.sound_position), 10);
        values.voice_position = admitted_initial_position(i64::from(values.voice_position), 10);
        // The 0x497 init sets each trackbar's range and position; every change
        // sends WM_HSCROLL (0x0061E609..0x0061E6AF), whose handler
        // (0x0055FF68) names the position (tables 0x0082A248 / 0x0082A254 /
        // 0x0082A260), so the resource captions never show.
        let detail_caption = detail_caption(&labels, values.detail_position);
        let difficulty_caption = difficulty_caption(&labels, values.difficulty_position);
        let scroll_caption = labels.scroll_tokens[usize::from(values.scroll_position)].clone();
        Self {
            detail_caption,
            difficulty_caption,
            scroll_caption,
            labels,
            values,
            zoom,
            zoom_popup_open: false,
            resolution_popup_open: false,
            resolution: (800, 600),
            launcher_audio_available,
            capture: None,
            pending_events: Vec::new(),
            pending_result: None,
            shell_interaction: shell::ShellInteraction::default(),
        }
    }

    pub(crate) const fn launcher_audio_available(&self) -> bool {
        self.launcher_audio_available
    }

    /// gamemd-derived: the parent snapshot consumed by
    /// `OptionsClass__ApplyFromLauncherDialog @ 0x0055FAA0` maps the six
    /// positions and three normalized checkboxes exactly as below.
    pub(crate) fn pack(&self) -> LauncherOptionsPacked {
        LauncherOptionsPacked {
            detail_level: if self.values.detail_position == 0 {
                0
            } else {
                2
            },
            difficulty: i32::from(self.values.difficulty_position),
            unit_action_lines: self.values.target_lines,
            show_hidden: self.values.show_hidden,
            tooltips: self.values.tooltips,
            scroll_rate: 6 - i32::from(self.values.scroll_position),
            score_volume: f32::from(self.values.score_position) * 0.1,
            sound_volume: f32::from(self.values.sound_position) * 0.1,
            voice_volume: f32::from(self.values.voice_position) * 0.1,
        }
    }

    pub(crate) fn trackbar_position(&self, id: LauncherTrackbarId) -> u8 {
        match id {
            LauncherTrackbarId::Detail => self.values.detail_position,
            LauncherTrackbarId::Difficulty => self.values.difficulty_position,
            LauncherTrackbarId::Scroll => self.values.scroll_position,
            LauncherTrackbarId::Score => self.values.score_position,
            LauncherTrackbarId::Sound => self.values.sound_position,
            LauncherTrackbarId::Voice => self.values.voice_position,
        }
    }

    /// gamemd-derived: primary-proc slice `0x0055FDB0..0x0056047A` changes
    /// captions and queues preview/cue work only after the integer position
    /// actually changes; the accepted projection remains owned by `0x0055FAA0`.
    fn set_trackbar_position(&mut self, id: LauncherTrackbarId, position: u8) {
        if position > id.maximum() || (id.is_audio() && !self.launcher_audio_available) {
            return;
        }
        let slot = match id {
            LauncherTrackbarId::Detail => &mut self.values.detail_position,
            LauncherTrackbarId::Difficulty => &mut self.values.difficulty_position,
            LauncherTrackbarId::Scroll => &mut self.values.scroll_position,
            LauncherTrackbarId::Score => &mut self.values.score_position,
            LauncherTrackbarId::Sound => &mut self.values.sound_position,
            LauncherTrackbarId::Voice => &mut self.values.voice_position,
        };
        if *slot == position {
            return;
        }
        *slot = position;
        match id {
            LauncherTrackbarId::Detail => {
                self.detail_caption = detail_caption(&self.labels, position);
                self.pending_events
                    .push(LauncherOptionsEvent::Cue(LauncherCue::GenericClick));
            }
            LauncherTrackbarId::Difficulty => {
                self.difficulty_caption = difficulty_caption(&self.labels, position);
                self.pending_events
                    .push(LauncherOptionsEvent::Cue(LauncherCue::GenericClick));
            }
            LauncherTrackbarId::Scroll => {
                self.scroll_caption = self.labels.scroll_tokens[usize::from(position)].clone();
                self.pending_events
                    .push(LauncherOptionsEvent::Cue(LauncherCue::GenericClick));
            }
            LauncherTrackbarId::Score => self.pending_events.push(
                LauncherOptionsEvent::ScorePreview(f32::from(position) * 0.1),
            ),
            LauncherTrackbarId::Sound => self.pending_events.push(
                LauncherOptionsEvent::SoundPreview(f32::from(position) * 0.1),
            ),
            LauncherTrackbarId::Voice => self.pending_events.push(
                LauncherOptionsEvent::VoicePreview(f32::from(position) * 0.1),
            ),
        }
    }

    /// A press on one trackbar ([`trackbar_press`]).
    pub(crate) fn trackbar_mouse_down(
        &mut self,
        id: LauncherTrackbarId,
        frame: PhysicalControlFrame,
    ) {
        if id.is_audio() && !self.launcher_audio_available {
            return;
        }
        let press = trackbar_press(
            i32::from(self.trackbar_position(id)),
            frame.local_x,
            frame.local_y,
            frame.width,
            frame.height,
            id.plaque_reserve(),
            i32::from(id.maximum()),
        );
        self.capture = press.hold(id);
        if let TrackbarPress::Jump(position) = press {
            // The position never exceeds the byte-sized maximum.
            self.set_trackbar_position(id, position as u8);
        }
    }

    pub(crate) fn trackbar_mouse_move(
        &mut self,
        id: LauncherTrackbarId,
        frame: PhysicalControlFrame,
    ) {
        if self.capture != Some(TrackbarHold { id, dragging: true }) {
            return;
        }
        let position = trackbar_position_from_x(
            frame.local_x,
            frame.width,
            id.plaque_reserve(),
            i32::from(id.maximum()),
        );
        self.set_trackbar_position(id, position as u8);
    }

    pub(crate) fn trackbar_mouse_up(&mut self, id: LauncherTrackbarId) {
        if self.capture.is_some_and(|hold| hold.id == id) {
            self.capture = None;
        }
    }

    /// gamemd-derived: checkbox owner `0x006163A0` admits only the unsigned
    /// 18x18 icon square, normalizes the toggle, then emits the checkbox cue.
    pub(crate) fn checkbox_mouse_down(
        &mut self,
        id: LauncherCheckboxId,
        frame: PhysicalControlFrame,
    ) {
        if !(0..18).contains(&frame.local_x) || !(0..18).contains(&frame.local_y) {
            return;
        }
        let slot = match id {
            LauncherCheckboxId::Tooltips => &mut self.values.tooltips,
            LauncherCheckboxId::TargetLines => &mut self.values.target_lines,
            LauncherCheckboxId::ShowHidden => &mut self.values.show_hidden,
            LauncherCheckboxId::FullScreen => &mut self.values.full_screen,
        };
        *slot = !*slot;
        let checked = *slot;
        self.pending_events
            .push(LauncherOptionsEvent::Cue(LauncherCue::Checkbox));
        if id == LauncherCheckboxId::FullScreen {
            self.pending_events
                .push(LauncherOptionsEvent::FullScreenChanged(checked));
        }
    }

    /// Follow a full-screen change the page didn't make (the window's own
    /// title-bar button), so the box shows what the window is.
    pub(crate) fn sync_full_screen(&mut self, full_screen: bool) {
        self.values.full_screen = full_screen;
    }

    /// Like gamemd's combo owner `0x00617250`, a press emits GUIComboOpen
    /// first. gamemd then opens the list only from the 20-pixel arrow
    /// (`x > client_width - 20`); VERA's own Zoom box opens from anywhere on it.
    pub(crate) fn combo_mouse_down(&mut self, frame: PhysicalControlFrame) {
        if frame.local_x < 0
            || frame.local_x >= frame.width
            || frame.local_y < 0
            || frame.local_y >= frame.height
        {
            return;
        }
        self.pending_events
            .push(LauncherOptionsEvent::Cue(LauncherCue::ComboOpen));
        self.zoom_popup_open = !self.zoom_popup_open;
    }

    /// gamemd-derived: ordinary `0xD5` owner-draw buttons emit
    /// `[AudioVisual] GUIMainButtonSound` on admitted primary mouse-down;
    /// `OptionsClass__ShowLauncherDialog @ 0x0055FC80` observes the distinct
    /// Back/Network/Keyboard result only on the later activation release.
    pub(crate) fn main_button_mouse_down(&mut self) {
        self.pending_events
            .push(LauncherOptionsEvent::Cue(LauncherCue::MainButton));
    }

    /// Like gamemd's resolution row (primary-proc slice
    /// `0x0055FDB0..0x0056047A`), a valid zoom row is published at once; the
    /// accepted projection `0x0055FAA0` never owns it.
    pub(crate) fn sync_resolution(&mut self, width: u32, height: u32) {
        self.resolution = (width, height);
    }

    pub(crate) fn select_resolution(&mut self, index: usize) {
        if let Some(&(width, height, _)) = RESOLUTIONS.get(index) {
            self.resolution = (width, height);
            self.resolution_popup_open = false;
            self.pending_events
                .push(LauncherOptionsEvent::ResolutionSelected(width, height));
        }
    }

    pub(crate) fn select_zoom(&mut self, index: usize) {
        let Some(zoom) = Zoom::ALL.get(index).copied() else {
            return;
        };
        self.zoom = zoom;
        self.zoom_popup_open = false;
        self.pending_events
            .push(LauncherOptionsEvent::ZoomSelected(zoom));
    }

    /// gamemd-derived: `OptionsClass__ShowLauncherDialog @ 0x0055FC80` gives
    /// Back, Network, Keyboard, and terminal completion distinct parent
    /// results; the first admitted result owns the frame's teardown path.
    pub(crate) fn request_result(&mut self, result: LauncherParentResult) {
        if self.pending_result.is_none() {
            self.pending_result = Some(result);
        }
    }

    /// gamemd-derived: primary-proc slice `0x0055FDB0..0x0056047A` observes
    /// control notifications in order before owner `0x0055FC80` dispatches a
    /// parent result through accepted projection `0x0055FAA0`.
    pub(crate) fn drain_output(&mut self) -> LauncherOptionsFrameOutput {
        LauncherOptionsFrameOutput {
            events: std::mem::take(&mut self.pending_events),
            result: self.pending_result.take(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::csf_file::CsfFile;
    use crate::ui::shell::trackbar::thumb_left;

    fn labels() -> LauncherOptionsLabels {
        LauncherOptionsLabels::resolve(&|_| None)
    }

    fn state(audio: bool) -> OptionsDialogState {
        OptionsDialogState::new(
            labels(),
            LauncherOptionsValues::default(),
            Zoom::Large,
            audio,
        )
    }

    fn frame(x: i32, y: i32, width: i32, height: i32) -> PhysicalControlFrame {
        PhysicalControlFrame {
            local_x: x,
            local_y: y,
            width,
            height,
        }
    }

    fn build_single_label_csf(label: &str, value: &str) -> Vec<u8> {
        let encoded_value: Vec<u8> = value
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .map(|byte| !byte)
            .collect();
        let mut data = Vec::new();
        data.extend_from_slice(&0x4353_4620_u32.to_le_bytes());
        data.extend_from_slice(&3_u32.to_le_bytes());
        data.extend_from_slice(&1_u32.to_le_bytes());
        data.extend_from_slice(&1_u32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0x4C42_4C20_u32.to_le_bytes());
        data.extend_from_slice(&1_u32.to_le_bytes());
        data.extend_from_slice(&(label.len() as u32).to_le_bytes());
        data.extend_from_slice(label.as_bytes());
        data.extend_from_slice(&0x5354_5220_u32.to_le_bytes());
        data.extend_from_slice(&(value.encode_utf16().count() as u32).to_le_bytes());
        data.extend_from_slice(&encoded_value);
        data
    }

    #[test]
    fn exact_launcher_label_fallbacks_are_local_and_complete() {
        const EXPECTED: [(&str, &str); 30] = [
            ("GUI:OptionsMenu", "Options"),
            ("GUI:MainMenu", "Main Menu"),
            ("GUI:Keyboard", "Keyboard"),
            ("GUI:Network", "Network"),
            ("GUI:DisplayOptions", "Display Options"),
            ("GUI:GameOptions", "Game Options"),
            ("GUI:UIOptions", "UI Options"),
            ("GUI:AudioOptions", "Audio Options"),
            ("GUI:VisualDetails", "Visual Details"),
            ("GUI:Difficulty", "Difficulty"),
            ("GUI:Tooltips", "Tooltips"),
            ("GUI:ScrollRate", "Scroll Rate"),
            ("GUI:TargetLines", "Target Lines"),
            ("GUI:ShowHidden", "See Hidden Objects"),
            ("GUI:MusicVolume", "Music Volume"),
            ("GUI:SoundVolume", "Sound Volume"),
            ("GUI:VoiceVolume", "Voice Volume"),
            ("GUI:Blank", ""),
            ("TXT_LOW", "Low"),
            ("TXT_HIGH", "High"),
            ("TXT_EASY", "Easy"),
            ("TXT_NORMAL", "Normal"),
            ("TXT_HARD", "Hard"),
            ("TXT_SLOWEST", "Slowest"),
            ("TXT_SLOWER", "Slower"),
            ("TXT_SLOW", "Slow"),
            ("TXT_MEDIUM", "Medium"),
            ("TXT_FAST", "Fast"),
            ("TXT_FASTER", "Faster"),
            ("TXT_FASTEST", "Fastest"),
        ];
        assert_eq!(LAUNCHER_LABEL_SPECS, EXPECTED);
        let seen = std::cell::RefCell::new(Vec::new());
        let labels = LauncherOptionsLabels::resolve(&|key| {
            seen.borrow_mut().push(key.to_string());
            None
        });
        assert_eq!(seen.into_inner(), EXPECTED.map(|(key, _)| key.to_string()));
        let actual_values = [
            labels.options.as_str(),
            labels.main_menu.as_str(),
            labels.keyboard.as_str(),
            labels.network.as_str(),
            labels.display_options.as_str(),
            labels.game_options.as_str(),
            labels.ui_options.as_str(),
            labels.audio_options.as_str(),
            labels.visual_details.as_str(),
            labels.difficulty.as_str(),
            labels.tooltips.as_str(),
            labels.scroll_rate.as_str(),
            labels.target_lines.as_str(),
            labels.show_hidden.as_str(),
            labels.music_volume.as_str(),
            labels.sound_volume.as_str(),
            labels.voice_volume.as_str(),
            labels.blank.as_str(),
            labels.low.as_str(),
            labels.high.as_str(),
            labels.easy.as_str(),
            labels.normal.as_str(),
            labels.hard.as_str(),
            labels.scroll_tokens[0].as_str(),
            labels.scroll_tokens[1].as_str(),
            labels.scroll_tokens[2].as_str(),
            labels.scroll_tokens[3].as_str(),
            labels.scroll_tokens[4].as_str(),
            labels.scroll_tokens[5].as_str(),
            labels.scroll_tokens[6].as_str(),
        ];
        assert_eq!(actual_values, EXPECTED.map(|(_, fallback)| fallback));
    }

    #[test]
    fn loaded_nonempty_csf_missing_launcher_key_uses_local_fallback() {
        let csf = CsfFile::from_bytes(&build_single_label_csf(
            "GUI:OptionsMenu",
            "Localized Options",
        ))
        .unwrap();
        assert!(!csf.is_empty());
        let labels = LauncherOptionsLabels::resolve(&|key| csf.get(key).map(str::to_owned));
        assert_eq!(labels.options, "Localized Options");
        assert_eq!(labels.main_menu, "Main Menu");
        assert_eq!(csf.text("GUI:MainMenu"), "MISSING:'GUI:MainMenu'");
    }

    #[test]
    fn captions_name_the_position_from_the_start_and_only_changes_click() {
        let mut state = OptionsDialogState::new(
            labels(),
            LauncherOptionsValues {
                detail_position: 0,
                difficulty_position: 2,
                scroll_position: 0,
                ..Default::default()
            },
            Zoom::Auto,
            true,
        );
        // Retail opens with the positions named (TXT_LOW/HARD/SLOWEST here).
        assert_eq!(state.detail_caption, "Low");
        assert_eq!(state.difficulty_caption, "Hard");
        assert_eq!(state.scroll_caption, "Slowest");

        state.set_trackbar_position(LauncherTrackbarId::Detail, 1);
        state.set_trackbar_position(LauncherTrackbarId::Difficulty, 2);
        state.set_trackbar_position(LauncherTrackbarId::Scroll, 6);
        assert_eq!(state.detail_caption, "High");
        assert_eq!(state.difficulty_caption, "Hard", "an unchanged move");
        assert_eq!(state.scroll_caption, "Fastest");
        assert_eq!(
            state.drain_output().events,
            [
                LauncherOptionsEvent::Cue(LauncherCue::GenericClick),
                LauncherOptionsEvent::Cue(LauncherCue::GenericClick),
            ]
        );
    }

    #[test]
    fn initial_control_requests_reject_out_of_range_and_nonfinite_volume() {
        assert_eq!(admitted_initial_position(-1, 6), 0);
        assert_eq!(admitted_initial_position(7, 6), 0);
        assert_eq!(admitted_initial_position(6, 6), 6);
        assert_eq!(admitted_volume_position(0.74), 7);
        assert_eq!(admitted_volume_position(0.75), 8);
        assert_eq!(admitted_volume_position(-0.2), 0);
        assert_eq!(admitted_volume_position(1.1), 0);
        assert_eq!(admitted_volume_position(f32::NAN), 0);
        assert_eq!(admitted_volume_position(f32::INFINITY), 0);
    }

    #[test]
    fn thumb_down_captures_without_jump_and_captured_motion_is_x_only() {
        let mut state = state(true);
        let before = state.trackbar_position(LauncherTrackbarId::Difficulty);
        let left = thumb_left(i32::from(before), 180, 0, 2);
        state.trackbar_mouse_down(LauncherTrackbarId::Difficulty, frame(left + 5, 23, 180, 24));
        assert_eq!(
            state.trackbar_position(LauncherTrackbarId::Difficulty),
            before
        );
        assert_eq!(
            state.capture,
            Some(TrackbarHold {
                id: LauncherTrackbarId::Difficulty,
                dragging: true,
            })
        );
        state.trackbar_mouse_move(LauncherTrackbarId::Difficulty, frame(179, -400, 180, 24));
        assert_eq!(state.trackbar_position(LauncherTrackbarId::Difficulty), 2);
        state.trackbar_mouse_up(LauncherTrackbarId::Difficulty);
        assert_eq!(state.capture, None);
    }

    #[test]
    fn rail_jump_has_lower_strip_gate_and_holds_without_drag() {
        let mut state = state(true);
        let held = Some(TrackbarHold {
            id: LauncherTrackbarId::Scroll,
            dragging: false,
        });
        state.trackbar_mouse_down(LauncherTrackbarId::Scroll, frame(151, 6, 180, 24));
        assert_eq!(
            state.trackbar_position(LauncherTrackbarId::Scroll),
            3,
            "strict lower edge rejects"
        );
        assert_eq!(state.capture, held);
        state.trackbar_mouse_up(LauncherTrackbarId::Scroll);
        state.trackbar_mouse_down(LauncherTrackbarId::Scroll, frame(151, 7, 180, 24));
        assert_eq!(state.trackbar_position(LauncherTrackbarId::Scroll), 6);
        assert_eq!(state.capture, held);
        state.trackbar_mouse_move(LauncherTrackbarId::Scroll, frame(0, 7, 180, 24));
        assert_eq!(state.trackbar_position(LauncherTrackbarId::Scroll), 6);
    }

    #[test]
    fn common_audio_gate_rejects_every_audio_input_without_mutation() {
        let mut state = state(false);
        let before = state.values;
        for id in [
            LauncherTrackbarId::Score,
            LauncherTrackbarId::Sound,
            LauncherTrackbarId::Voice,
        ] {
            state.trackbar_mouse_down(id, frame(67, 23, 128, 24));
            state.trackbar_mouse_move(id, frame(67, 23, 128, 24));
        }
        assert_eq!(state.values, before);
        assert!(state.drain_output().events.is_empty());
    }

    #[test]
    fn checkbox_hit_is_strict_icon_square_and_cue_follows_toggle() {
        let mut state = state(true);
        assert!(state.values.tooltips);
        state.checkbox_mouse_down(LauncherCheckboxId::Tooltips, frame(17, 17, 220, 20));
        assert!(!state.values.tooltips);
        assert_eq!(
            state.drain_output().events,
            [LauncherOptionsEvent::Cue(LauncherCue::Checkbox)]
        );
        state.checkbox_mouse_down(LauncherCheckboxId::Tooltips, frame(18, 5, 220, 20));
        state.checkbox_mouse_down(LauncherCheckboxId::Tooltips, frame(80, 5, 220, 20));
        assert!(!state.values.tooltips);
        assert!(state.drain_output().events.is_empty());
    }

    #[test]
    fn combo_cues_then_toggles_from_anywhere_on_its_face() {
        let mut state = state(true);
        state.combo_mouse_down(frame(4, 5, 180, 24));
        assert!(
            state.zoom_popup_open,
            "the face opens the list, not only the arrow"
        );
        assert_eq!(
            state.drain_output().events,
            [LauncherOptionsEvent::Cue(LauncherCue::ComboOpen)]
        );
        state.combo_mouse_down(frame(161, 5, 180, 24));
        assert!(!state.zoom_popup_open, "a second press closes it");
        state.combo_mouse_down(frame(180, 5, 180, 24));
        assert!(!state.zoom_popup_open, "outside the face does nothing");
        state.combo_mouse_down(frame(161, 5, 180, 24));
        assert!(state.zoom_popup_open);
        state.select_zoom(99);
        assert!(
            state
                .drain_output()
                .events
                .iter()
                .all(|event| !matches!(event, LauncherOptionsEvent::ZoomSelected(_)))
        );
        state.select_zoom(0);
        assert_eq!(state.zoom, Zoom::Auto);
        assert_eq!(
            state.drain_output().events,
            [LauncherOptionsEvent::ZoomSelected(Zoom::Auto)]
        );
    }

    #[test]
    fn full_screen_box_reports_each_toggle_after_its_cue() {
        let mut state = state(true);
        state.checkbox_mouse_down(LauncherCheckboxId::FullScreen, frame(5, 5, 180, 16));
        assert_eq!(
            state.drain_output().events,
            [
                LauncherOptionsEvent::Cue(LauncherCue::Checkbox),
                LauncherOptionsEvent::FullScreenChanged(true),
            ]
        );
        // The window's own button changes it with no event; the next click
        // turns it off again.
        state.sync_full_screen(false);
        state.checkbox_mouse_down(LauncherCheckboxId::FullScreen, frame(5, 5, 180, 16));
        assert_eq!(
            state.drain_output().events,
            [
                LauncherOptionsEvent::Cue(LauncherCue::Checkbox),
                LauncherOptionsEvent::FullScreenChanged(true),
            ]
        );
        // The full-screen state is the window's, not part of the accepted pack.
        assert_eq!(state.pack(), self::state(true).pack());
    }

    #[test]
    fn zoom_factor_is_whole_and_never_shows_less_than_640x480() {
        let factor = |zoom: Zoom, w, h| zoom.factor(w, h);
        // Auto keeps at least the original 800x600 view.
        assert_eq!(factor(Zoom::Auto, 800, 600), 1);
        assert_eq!(factor(Zoom::Auto, 1920, 1080), 1);
        assert_eq!(factor(Zoom::Auto, 1600, 1200), 2);
        assert_eq!(factor(Zoom::Auto, 3440, 1440), 2);
        assert_eq!(factor(Zoom::Auto, 3840, 2160), 3);
        // A fixed choice gives way only where it would cut below 640x480.
        assert_eq!(factor(Zoom::Normal, 3840, 2160), 1);
        assert_eq!(factor(Zoom::Large, 1920, 1080), 2);
        assert_eq!(factor(Zoom::Large, 1024, 768), 1);
        assert_eq!(factor(Zoom::Huge, 3440, 1440), 3);
        assert_eq!(factor(Zoom::Huge, 1920, 1080), 2);
        // A minimised or tiny window still gets a usable factor.
        assert_eq!(factor(Zoom::Huge, 0, 0), 1);
        for zoom in Zoom::ALL {
            assert_eq!(Zoom::from_ini_value(zoom.ini_value()), zoom);
            assert_eq!(Zoom::ALL[zoom.row()], zoom);
        }
        assert_eq!(Zoom::from_ini_value(7), Zoom::Auto);
    }

    #[test]
    fn audio_changes_emit_preview_without_generic_click_and_pack_exact_values() {
        let mut state = state(true);
        state.set_trackbar_position(LauncherTrackbarId::Score, 0);
        state.set_trackbar_position(LauncherTrackbarId::Sound, 5);
        state.set_trackbar_position(LauncherTrackbarId::Voice, 6);
        assert_eq!(
            state.drain_output().events,
            [
                LauncherOptionsEvent::ScorePreview(0.0),
                LauncherOptionsEvent::SoundPreview(0.5),
                LauncherOptionsEvent::VoicePreview(0.6),
            ]
        );
        let packed = state.pack();
        assert_eq!(packed.detail_level, 2);
        assert_eq!(packed.difficulty, 1);
        assert_eq!(packed.scroll_rate, 3);
        assert_eq!(packed.score_volume.to_bits(), 0.0_f32.to_bits());
        assert_eq!(packed.sound_volume.to_bits(), 0.5_f32.to_bits());
        assert_eq!(packed.voice_volume.to_bits(), 0.6_f32.to_bits());
    }

    #[test]
    fn main_button_press_cues_before_release_result() {
        let mut state = state(true);
        state.main_button_mouse_down();
        assert_eq!(
            state.drain_output(),
            LauncherOptionsFrameOutput {
                events: vec![LauncherOptionsEvent::Cue(LauncherCue::MainButton)],
                result: None,
            }
        );

        state.request_result(LauncherParentResult::Network);
        assert_eq!(
            state.drain_output(),
            LauncherOptionsFrameOutput {
                events: Vec::new(),
                result: Some(LauncherParentResult::Network),
            }
        );
    }

    #[test]
    fn escape_is_no_event_and_first_parent_result_wins_one_frame() {
        let mut state = state(true);
        assert!(state.drain_output().events.is_empty());
        state.request_result(LauncherParentResult::Network);
        state.request_result(LauncherParentResult::Back);
        assert_eq!(
            state.drain_output().result,
            Some(LauncherParentResult::Network)
        );
        assert_eq!(state.drain_output().result, None);
    }
}
