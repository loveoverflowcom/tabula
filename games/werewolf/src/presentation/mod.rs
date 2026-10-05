//! Authorized View → renderer-neutral local Werewolf simulator. (doc 04 §5)
//!
//! Privacy is fail-closed across view changes: a revealed card is bound to one
//! seat, phase, round and lifecycle. Concealment removes every role-dependent
//! draw/control/accessibility label and clears private selection (I-5, I-10).

#![allow(clippy::float_arithmetic, clippy::doc_markdown)]

pub mod assets;
mod render;
#[cfg(test)]
mod tests;

use tabula_core::{SeatId, SpectatorTier, Viewer};
use tabula_design::Theme;
use tabula_game_api::{A11yAction, A11yDescription, A11yItem, A11yRegion, ActionId};
use tabula_presentation::{
    ActionButton, AssetPackRef, AudioCues, ButtonInteraction, FocusGraph, FocusId, FocusNode,
    FocusState, FrameCtx, GamePresentation, InputEvent, Intent, Key, NavigationAction, Rect,
    RenderList, Vec2, Viewport,
};

use crate::rules::{Perspective, PrivateKnowledge, RoleKnowledge, View, ViewEvent};
use crate::{Alignment, Ballot, Command, NightChoice, Phase, PlayerStatus, Role, WerewolfRules};

/// One client-side view tab, not an authoritative phase or command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Panel {
    #[default]
    Card,
    Table,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PotionMode {
    #[default]
    Heal,
    Poison,
}

/// The authorization/lifecycle facts to which a deliberate reveal is bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RevealScope {
    seat: SeatId,
    role: Role,
    phase: Phase,
    round: u32,
    alive: bool,
    can_act: bool,
    status: PlayerStatus,
}

#[derive(Clone, Copy, Debug, Default)]
struct ActivationKeys {
    enter: bool,
    space: bool,
}

/// Ephemeral controls and reveal state, never saved or sent upstream. (I-10)
#[derive(Clone, Debug)]
pub struct WerewolfLocal {
    viewport: Viewport,
    theme: Theme,
    reveal: Option<RevealScope>,
    selected: Option<SeatId>,
    selection_scope: Option<RevealScope>,
    potion: PotionMode,
    panel: Panel,
    page: usize,
    focus: FocusState,
    interaction: ButtonInteraction,
    activation_keys: ActivationKeys,
    viewer_request: Option<Viewer>,
    advance_request: bool,
    restart_request: bool,
}

impl Default for WerewolfLocal {
    fn default() -> Self {
        Self {
            viewport: Viewport::new(Vec2::new(1024.0, 768.0)).expect("literal viewport is valid"),
            theme: Theme::by_kind(tabula_design::ThemeKind::Dark),
            reveal: None,
            selected: None,
            selection_scope: None,
            potion: PotionMode::Heal,
            panel: Panel::Card,
            page: 0,
            focus: FocusState::default(),
            interaction: ButtonInteraction::default(),
            activation_keys: ActivationKeys::default(),
            viewer_request: None,
            advance_request: false,
            restart_request: false,
        }
    }
}

impl WerewolfLocal {
    /// Keeps input and rendering geometry identical for the current native/WASM frame.
    pub fn set_frame_context(&mut self, frame: &FrameCtx) {
        self.viewport = frame.viewport();
        self.theme = frame.theme();
    }

    /// Immediately clears private-local selection, card reveal and pending presses.
    /// The host must call this before replacing a projection or restarting.
    pub fn conceal(&mut self) {
        self.reveal = None;
        self.selected = None;
        self.selection_scope = None;
        self.potion = PotionMode::Heal;
        self.interaction = ButtonInteraction::default();
        if self.activation_keys.enter {
            self.interaction
                .suppress_activation_until_release(Key::Enter);
        }
        if self.activation_keys.space {
            self.interaction
                .suppress_activation_until_release(Key::Space);
        }
        self.focus = FocusState::new(
            Some(FocusId::new(6)),
            tabula_presentation::FocusModality::Keyboard,
            self.focus.is_window_focused(),
        );
    }

    /// Prevents a held activation key crossing a restart or perspective change.
    /// The host should seed this from currently held keys in every new session.
    pub fn suppress_activation_until_release(&mut self, key: Key) {
        match key {
            Key::Enter => self.activation_keys.enter = true,
            Key::Space => self.activation_keys.space = true,
            _ => {}
        }
        self.interaction.suppress_activation_until_release(key);
    }

    /// Consumes an explicit local seat/public perspective request; there is no Audit switch.
    pub fn take_viewer_request(&mut self) -> Option<Viewer> {
        self.viewer_request.take()
    }

    /// Consumes the local simulator's request to fire the real projected phase deadline.
    pub fn take_advance_phase_request(&mut self) -> bool {
        core::mem::take(&mut self.advance_request)
    }

    /// Consumes an explicit fresh local-match restart request.
    pub fn take_restart_request(&mut self) -> bool {
        core::mem::take(&mut self.restart_request)
    }

    fn is_revealed(&self, view: &View) -> bool {
        self.reveal.is_some() && self.reveal == reveal_scope(view)
    }

    fn active_selected(&self, view: &View) -> Option<SeatId> {
        (self.selection_scope == reveal_scope(view))
            .then_some(self.selected)
            .flatten()
    }

    fn reconcile(&mut self, view: &View) {
        if self.reveal.is_some() && self.reveal != reveal_scope(view) {
            self.conceal();
        }
        if self
            .selected
            .is_some_and(|seat| !view.roster.iter().any(|s| s.seat == seat && s.alive))
        {
            self.selected = None;
        }
        self.page = self
            .page
            .min(page_count(view, self.viewport).saturating_sub(1));
    }
}

/// Renderer-independent presentation of the maintained ClassicV1 rules.
#[derive(Debug)]
pub struct WerewolfPresentation;

impl GamePresentation for WerewolfPresentation {
    type Rules = WerewolfRules;
    type Local = WerewolfLocal;

    fn asset_pack() -> AssetPackRef {
        assets::asset_pack()
    }

    fn present(view: &View, local: &Self::Local, frame: &FrameCtx) -> RenderList {
        render::present(view, local, frame).expect("validated viewport and bounded game geometry")
    }

    fn on_view_event(_event: &ViewEvent, local: &mut Self::Local, frame: &FrameCtx) -> AudioCues {
        // A projected event may replace authorization, phase or lifecycle. Reset
        // conservatively even for private acknowledgements, never animate secrets.
        local.set_frame_context(frame);
        local.conceal();
        AudioCues::default()
    }

    #[allow(clippy::too_many_lines)] // Closed action dispatch keeps every privacy reset beside its trigger.
    fn on_input(
        input: &InputEvent,
        view: &View,
        local: &mut Self::Local,
    ) -> Option<Intent<Command>> {
        local.reconcile(view);
        if let InputEvent::Key { key, pressed } = input {
            match key {
                Key::Enter => local.activation_keys.enter = *pressed,
                Key::Space => local.activation_keys.space = *pressed,
                _ => {}
            }
        }
        if matches!(
            input,
            InputEvent::Focus(false)
                | InputEvent::Key {
                    key: Key::Escape,
                    pressed: true
                }
        ) {
            local.conceal();
            if matches!(input, InputEvent::Focus(false)) {
                local.focus.set_window_focused(false);
            }
            return None;
        }
        let controls = controls(view, local, local.viewport);
        let buttons: Vec<_> = controls
            .iter()
            .map(|control| control.button(&local.theme))
            .collect();
        let nodes = buttons
            .iter()
            .enumerate()
            .map(|(i, button)| {
                let previous = i.checked_sub(1).map(|j| buttons[j].id());
                let next = buttons.get(i + 1).map(|b| b.id());
                FocusNode::with_neighbors(
                    button.id(),
                    button.rect(),
                    previous,
                    next,
                    previous,
                    next,
                )
            })
            .collect();
        let graph = FocusGraph::new(nodes).expect("unique local control ids");
        let action = local
            .interaction
            .on_input(input, &buttons, &graph, &mut local.focus);
        let NavigationAction::Activate(id) = action else {
            return None;
        };
        let control = controls.iter().find(|control| control.id == id)?;
        if !control.enabled {
            return None;
        }
        match control.action {
            Action::Reveal => {
                if local.is_revealed(view) {
                    local.conceal();
                } else {
                    local.reveal = reveal_scope(view);
                }
            }
            Action::PreviousSeat | Action::NextSeat => {
                let current = viewer_seat(view)
                    .and_then(|seat| view.roster.iter().position(|s| s.seat == seat));
                let index = match (control.action, current) {
                    (Action::PreviousSeat, Some(i)) => i
                        .checked_sub(1)
                        .unwrap_or(view.roster.len().saturating_sub(1)),
                    (Action::NextSeat, Some(i)) => (i + 1) % view.roster.len().max(1),
                    _ => 0,
                };
                if let Some(seat) = view.roster.get(index).map(|s| s.seat) {
                    local.conceal();
                    local.page = 0;
                    local.viewer_request = Some(Viewer::Seat(seat));
                }
            }
            Action::Public => {
                local.conceal();
                local.viewer_request = Some(Viewer::Spectator(SpectatorTier::Live));
            }
            Action::Advance => {
                local.conceal();
                local.advance_request = true;
            }
            Action::Restart => {
                local.conceal();
                local.restart_request = true;
            }
            Action::Card => {
                local.panel = Panel::Card;
            }
            Action::Table => {
                local.panel = Panel::Table;
            }
            Action::Page => {
                local.page = (local.page + 1) % page_count(view, local.viewport).max(1);
            }
            Action::Target(seat) => {
                if target_command(view, local, seat).is_some() {
                    local.selected = Some(seat);
                    local.selection_scope = reveal_scope(view);
                }
            }
            Action::Heal => {
                local.potion = PotionMode::Heal;
                local.selected = None;
            }
            Action::Poison => {
                local.potion = PotionMode::Poison;
                local.selected = None;
            }
            Action::Submit => return selected_command(view, local).map(Intent::new),
            Action::Pass => return pass_command(view).map(Intent::new),
            Action::Unvote => {
                return view
                    .legal_commands
                    .contains(&Command::Unvote)
                    .then(|| Intent::new(Command::Unvote))
            }
        }
        None
    }

    fn a11y(view: &View, local: &Self::Local) -> A11yDescription {
        let revealed = local.is_revealed(view);
        let role = reveal_scope(view)
            .filter(|_| revealed)
            .map(|scope| role_name(scope.role));
        let status = format!(
            "Ma Sói. Mô phỏng cục bộ điều khiển mọi ghế. Vòng {}. {}. Góc nhìn: {}. {}",
            view.round,
            phase_name(view.phase),
            perspective_label(view),
            role.map_or("Lá bài đang che".to_owned(), |r| format!(
                "Vai của bạn: {r}"
            ))
        );
        let mut regions = vec![A11yRegion {
            label: "Người chơi".into(),
            items: view
                .roster
                .iter()
                .map(|seat| {
                    let label =
                        public_seat_label(seat.seat, seat.alive, seat.role, view.phase, revealed);
                    let selected =
                        local.active_selected(view) == Some(seat.seat) && can_select(view, local);
                    A11yItem {
                        label,
                        position: format!("Ghế {}", u16::from(seat.seat.0) + 1),
                        state: if selected {
                            "Đang chọn mục tiêu".into()
                        } else if seat.alive {
                            "Còn sống".into()
                        } else {
                            "Đã chết".into()
                        },
                        activates: target_command(view, local, seat.seat)
                            .is_some()
                            .then(|| ActionId(format!("target-{}", seat.seat.0))),
                    }
                })
                .collect(),
        }];
        if let Some(scope) = reveal_scope(view).filter(|_| revealed) {
            regions.push(A11yRegion {
                label: "Lá bài đã mở".into(),
                items: vec![A11yItem {
                    label: format!("{}. {}", role_name(scope.role), role_rules(scope.role)),
                    position: "Lá bài của bạn".into(),
                    state: "Đã mở. Escape để che".into(),
                    activates: Some(ActionId("reveal-card".into())),
                }],
            });
            let knowledge = knowledge_lines(view);
            if !knowledge.is_empty() {
                regions.push(A11yRegion {
                    label: "Thông tin riêng được phép xem".into(),
                    items: knowledge
                        .into_iter()
                        .map(|label| A11yItem {
                            label,
                            position: "Thông tin riêng".into(),
                            state: "Chỉ góc nhìn hiện tại".into(),
                            activates: None,
                        })
                        .collect(),
                });
            }
        }
        let public_items = public_a11y_items(view);
        if !public_items.is_empty() {
            regions.push(A11yRegion {
                label: "Phiếu và kết quả công khai".into(),
                items: public_items,
            });
        }
        let actions = controls(view, local, local.viewport)
            .into_iter()
            .map(|control| A11yAction {
                id: ActionId(control.action_id()),
                label: control.label.replace('\n', " · "),
                enabled: control.enabled,
            })
            .collect();
        A11yDescription {
            status,
            regions,
            actions,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Reveal,
    PreviousSeat,
    NextSeat,
    Public,
    Advance,
    Restart,
    Card,
    Table,
    Page,
    Target(SeatId),
    Heal,
    Poison,
    Submit,
    Pass,
    Unvote,
}

struct Control {
    id: FocusId,
    rect: Rect,
    label: String,
    enabled: bool,
    action: Action,
    selected: bool,
}

impl Control {
    fn action_id(&self) -> String {
        match self.action {
            Action::Target(seat) => format!("target-{}", seat.0),
            Action::Reveal => "reveal-card".into(),
            _ => format!("control-{}", self.id.get()),
        }
    }
    fn button<'a>(&'a self, theme: &Theme) -> ActionButton<'a> {
        let (icon, label) = self
            .label
            .split_once('\n')
            .map_or((None, self.label.as_str()), |(icon, label)| {
                (Some(icon), label)
            });
        let button = ActionButton::new(self.id, self.rect, label, theme.density.min_target)
            .expect("all control bounds meet 44dp floor")
            .enabled(self.enabled)
            .tone(if self.selected {
                tabula_presentation::ButtonTone::Filled
            } else {
                tabula_presentation::ButtonTone::Tonal
            });
        icon.map_or(button, |icon| button.with_icon(icon))
    }
}

/// Pure responsive geometry shared by input and drawing. Tiny viewports use
/// paging and a separate card/table tab instead of reducing touch targets.
#[derive(Clone, Copy)]
struct Layout {
    viewport: Vec2,
    compact: bool,
    content: Rect,
    card: Rect,
    table: Rect,
    reveal: Rect,
}

impl Layout {
    fn new(viewport: Viewport) -> Self {
        let size = viewport.size();
        let compact = size.x < 760.0 || size.y < 620.0;
        let margin = 16.0_f32.min(size.x * 0.03);
        let content_y = if compact { 178.0 } else { 128.0 };
        let content_h = (size.y - content_y - 116.0).max(1.0);
        let content = rect(
            margin,
            content_y,
            (size.x - margin * 2.0).max(1.0),
            content_h,
        );
        let card_width = if compact {
            (content.size().x - 20.0)
                .min((content_h - 52.0).max(50.0) / 1.5)
                .min(320.0)
        } else {
            ((content_h - 62.0).max(60.0) / 1.5)
                .min(320.0)
                .min(content.size().x * 0.36)
        };
        let card_width = card_width.max(20.0);
        let card_x = if compact {
            (size.x - card_width) / 2.0
        } else {
            margin + 24.0
        };
        let card = rect(card_x, content_y, card_width, card_width * 1.5);
        let reveal = rect(
            if compact { margin } else { card_x },
            card.origin().y + card.size().y + 8.0,
            if compact {
                content.size().x
            } else {
                card_width
            },
            44.0,
        );
        let table = if compact {
            content
        } else {
            rect(
                card_x + card_width + 36.0,
                content_y,
                (size.x - margin - card_x - card_width - 36.0).max(44.0),
                content_h,
            )
        };
        Self {
            viewport: size,
            compact,
            content,
            card,
            table,
            reveal,
        }
    }
}

fn index_f32(index: usize) -> f32 {
    f32::from(u16::try_from(index).expect("bounded roster and retained presentation lines"))
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::new(Vec2::new(x, y), Vec2::new(width, height)).expect("finite bounded layout")
}

fn reveal_scope(view: &View) -> Option<RevealScope> {
    let Perspective::Seat {
        seat,
        role,
        alive,
        can_act,
    } = view.perspective
    else {
        return None;
    };
    let status = view.roster.iter().find(|s| s.seat == seat)?.status;
    Some(RevealScope {
        seat,
        role,
        phase: view.phase,
        round: view.round,
        alive,
        can_act,
        status,
    })
}

fn viewer_seat(view: &View) -> Option<SeatId> {
    match view.perspective {
        Perspective::Seat { seat, .. } => Some(seat),
        _ => None,
    }
}

fn page_size(viewport: Viewport) -> usize {
    if Layout::new(viewport).compact {
        if viewport.size().y < 700.0 {
            4
        } else {
            8
        }
    } else {
        10
    }
}

fn page_count(view: &View, viewport: Viewport) -> usize {
    view.roster.len().div_ceil(page_size(viewport))
}

#[allow(clippy::too_many_lines)] // Single shared control vocabulary owns both rendering and hit testing.
fn controls(view: &View, local: &WerewolfLocal, viewport: Viewport) -> Vec<Control> {
    let layout = Layout::new(viewport);
    let width = layout.viewport.x;
    let mut result = Vec::new();
    let mut add = |id, rect, label: String, enabled, action, selected| {
        result.push(Control {
            id: FocusId::new(id),
            rect,
            label,
            enabled,
            action,
            selected,
        });
    };
    let margin = layout.content.origin().x;
    let gap = 8.0;
    let seat_width = ((width - margin * 2.0 - gap * 2.0) / 3.0).clamp(44.0, 180.0);
    for (index, (action, label)) in [
        (Action::PreviousSeat, "Ghế trước"),
        (Action::NextSeat, "Ghế sau"),
        (Action::Public, "Công khai"),
    ]
    .into_iter()
    .enumerate()
    {
        add(
            1 + u32::try_from(index).expect("three navigation buttons"),
            rect(
                margin + (seat_width + gap) * index_f32(index),
                80.0,
                seat_width,
                44.0,
            ),
            label.into(),
            !view.roster.is_empty(),
            action,
            matches!(action, Action::Public) && viewer_seat(view).is_none(),
        );
    }
    if layout.compact {
        let tab_width = ((width - margin * 2.0 - gap) / 2.0).max(44.0);
        add(
            4,
            rect(margin, 130.0, tab_width, 44.0),
            "Lá bài".into(),
            true,
            Action::Card,
            local.panel == Panel::Card,
        );
        add(
            5,
            rect(margin + tab_width + gap, 130.0, tab_width, 44.0),
            "Bàn chơi".into(),
            true,
            Action::Table,
            local.panel == Panel::Table,
        );
    }
    if !layout.compact || local.panel == Panel::Card {
        add(
            6,
            layout.reveal,
            if local.is_revealed(view) {
                "Che lá bài · Esc"
            } else {
                "Xem lá bài của tôi"
            }
            .into(),
            reveal_scope(view).is_some(),
            Action::Reveal,
            local.is_revealed(view),
        );
    }
    if !layout.compact || local.panel == Panel::Table {
        let start = local.page * page_size(viewport);
        let columns = if layout.table.size().x >= 500.0 { 5 } else { 4 };
        let cell_width =
            ((layout.table.size().x - gap * index_f32(columns - 1)) / index_f32(columns)).max(44.0);
        for (i, seat) in view
            .roster
            .iter()
            .skip(start)
            .take(page_size(viewport))
            .enumerate()
        {
            let selected =
                local.active_selected(view) == Some(seat.seat) && can_select(view, local);
            let label = format!(
                "{}\n{}",
                u16::from(seat.seat.0) + 1,
                if selected {
                    "Chọn"
                } else if seat.alive {
                    "Sống"
                } else {
                    "Chết"
                }
            );
            add(
                100 + u32::from(seat.seat.0),
                rect(
                    layout.table.origin().x + (cell_width + gap) * index_f32(i % columns),
                    layout.table.origin().y + 34.0 + 56.0 * index_f32(i / columns),
                    cell_width,
                    48.0,
                ),
                label,
                seat.alive && target_command(view, local, seat.seat).is_some(),
                Action::Target(seat.seat),
                selected,
            );
        }
        let rows = view
            .roster
            .iter()
            .skip(start)
            .take(page_size(viewport))
            .count()
            .div_ceil(columns);
        let mut action_y = layout.table.origin().y + 34.0 + index_f32(rows) * 56.0 + 8.0;
        if page_count(view, viewport) > 1 {
            add(
                10,
                rect(
                    layout.table.origin().x,
                    action_y,
                    layout.table.size().x,
                    44.0,
                ),
                format!(
                    "Người chơi {}/{} · Trang sau",
                    local.page + 1,
                    page_count(view, viewport)
                ),
                true,
                Action::Page,
                false,
            );
            action_y += 52.0;
        }
        if view.phase == Phase::Night
            && local.is_revealed(view)
            && reveal_scope(view).is_some_and(|scope| scope.role == Role::Witch)
        {
            let w = ((layout.table.size().x - gap) / 2.0).max(44.0);
            let inventory = match &view.knowledge {
                PrivateKnowledge::Living { witch_potions, .. } => *witch_potions,
                _ => None,
            };
            add(
                11,
                rect(layout.table.origin().x, action_y, w, 44.0),
                "Bình cứu".into(),
                inventory.is_some_and(|p| p.heal),
                Action::Heal,
                local.potion == PotionMode::Heal,
            );
            add(
                12,
                rect(layout.table.origin().x + w + gap, action_y, w, 44.0),
                "Bình độc".into(),
                inventory.is_some_and(|p| p.poison),
                Action::Poison,
                local.potion == PotionMode::Poison,
            );
            action_y += 52.0;
        }
        if can_select(view, local) {
            let w = ((layout.table.size().x - gap) / 2.0).max(44.0);
            add(
                13,
                rect(layout.table.origin().x, action_y, w, 44.0),
                if view.phase == Phase::Vote {
                    "Bỏ phiếu"
                } else {
                    "Gửi lựa chọn"
                }
                .into(),
                selected_command(view, local).is_some(),
                Action::Submit,
                false,
            );
            add(
                14,
                rect(layout.table.origin().x + w + gap, action_y, w, 44.0),
                if view.phase == Phase::Vote {
                    "Trắng phiếu"
                } else {
                    "Bỏ qua"
                }
                .into(),
                pass_command(view).is_some(),
                Action::Pass,
                false,
            );
            if view.legal_commands.contains(&Command::Unvote) {
                add(
                    15,
                    rect(
                        layout.table.origin().x,
                        action_y + 52.0,
                        layout.table.size().x,
                        44.0,
                    ),
                    "Rút phiếu".into(),
                    true,
                    Action::Unvote,
                    false,
                );
            }
        }
    }
    let footer_y = (layout.viewport.y - 100.0).max(0.0);
    let w = ((width - margin * 2.0 - gap) / 2.0).clamp(44.0, 260.0);
    add(
        20,
        rect(margin, footer_y, w, 44.0),
        "Pha tiếp theo".into(),
        view.phase.is_playing(),
        Action::Advance,
        false,
    );
    add(
        21,
        rect(margin + w + gap, footer_y, w, 44.0),
        "Ván mới".into(),
        true,
        Action::Restart,
        false,
    );
    result
}

fn can_select(view: &View, local: &WerewolfLocal) -> bool {
    reveal_scope(view).is_some_and(|scope| scope.alive && scope.can_act)
        && (view.phase == Phase::Vote || (view.phase == Phase::Night && local.is_revealed(view)))
        && !view.legal_commands.is_empty()
}

fn selected_command(view: &View, local: &WerewolfLocal) -> Option<Command> {
    if !can_select(view, local) {
        return None;
    }
    let target = local.active_selected(view)?;
    target_command(view, local, target)
}

fn target_command(view: &View, local: &WerewolfLocal, target: SeatId) -> Option<Command> {
    if !can_select(view, local) {
        return None;
    }
    let command = if view.phase == Phase::Vote {
        Command::Vote(Ballot::Target(target))
    } else {
        let choice = match reveal_scope(view)?.role {
            Role::Werewolf => NightChoice::WolfTarget(Some(target)),
            Role::Seer => NightChoice::Investigate(target),
            Role::Doctor => NightChoice::Protect(Some(target)),
            Role::Hunter => NightChoice::HunterMark(Some(target)),
            Role::Witch => match local.potion {
                PotionMode::Heal => NightChoice::WitchHeal(Some(target)),
                PotionMode::Poison => NightChoice::WitchPoison(Some(target)),
            },
            Role::Villager => return None,
        };
        Command::Night(choice)
    };
    view.legal_commands.contains(&command).then_some(command)
}

fn pass_command(view: &View) -> Option<Command> {
    let command = if view.phase == Phase::Vote {
        Command::Vote(Ballot::Abstain)
    } else {
        Command::Night(NightChoice::Pass)
    };
    view.legal_commands.contains(&command).then_some(command)
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Night => "Ban đêm",
        Phase::Dawn => "Bình minh",
        Phase::Day => "Thảo luận",
        Phase::Vote => "Bỏ phiếu",
        Phase::Dusk => "Hoàng hôn",
        Phase::Ended => "Kết thúc",
    }
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Villager => "Dân làng",
        Role::Werewolf => "Ma sói",
        Role::Seer => "Tiên tri",
        Role::Doctor => "Bác sĩ",
        Role::Hunter => "Thợ săn",
        Role::Witch => "Phù thủy",
    }
}

fn role_rules(role: Role) -> &'static str {
    match role {
        Role::Villager=>"Không có hành động riêng ban đêm. Thảo luận và bỏ phiếu ban ngày.",
        Role::Werewolf=>"Mỗi đêm, chọn một người sống ngoài phe Sói hoặc bỏ qua. Biết đồng đội; không thấy lựa chọn đang gửi của họ.",
        Role::Seer=>"Soi một người sống khác mỗi đêm. Bình minh nhận kết quả Sói / Không phải Sói, không phải vai cụ thể.",
        Role::Doctor=>"Bảo vệ một người, kể cả mình. Không chọn cùng người hai đêm liền. Chỉ chặn Sói, không chặn độc.",
        Role::Hunter=>"Chọn trước một người sống khác trong đêm. Nếu bạn chết, mục tiêu còn sống chết theo; một lần. Không chọn lại sau khi chết.",
        Role::Witch=>"Một bình cứu và một bình độc cho cả ván; tối đa một bình mỗi đêm. Cứu mù: không biết Sói chọn ai.",
    }
}

fn perspective_label(view: &View) -> String {
    match view.perspective {
        Perspective::Seat { seat, alive, .. } => format!(
            "Người {}{}",
            u16::from(seat.0) + 1,
            if alive { "" } else { " · Đã chết" }
        ),
        Perspective::Outside => "Công khai".into(),
        Perspective::Audit => "Kiểm tra".into(),
    }
}

fn public_seat_label(
    seat: SeatId,
    alive: bool,
    role: RoleKnowledge,
    phase: Phase,
    revealed: bool,
) -> String {
    let role = match role {
        RoleKnowledge::Known(role) if revealed || !alive || phase == Phase::Ended => {
            format!(" · {}", role_name(role))
        }
        _ => String::new(),
    };
    format!(
        "Người {} · {}{role}",
        u16::from(seat.0) + 1,
        if alive { "Còn sống" } else { "Đã chết" }
    )
}

#[allow(clippy::too_many_lines)] // Exhaustive authorized knowledge variants, never a canonical-state reader.
fn knowledge_lines(view: &View) -> Vec<String> {
    match &view.knowledge {
        PrivateKnowledge::Living {
            choice,
            wolf_team,
            seer_reports,
            doctor_previous,
            hunter_mark,
            witch_potions,
        } => {
            let mut lines = Vec::new();
            if let Some(choice) = choice {
                lines.push(format!(
                    "Đã gửi: {}. Cố định đến hết đêm",
                    choice_label(*choice)
                ));
            }
            if !wolf_team.is_empty() {
                lines.push(format!(
                    "Phe Sói: {}",
                    wolf_team
                        .iter()
                        .map(|seat| (u16::from(seat.0) + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            for (seat, alignment) in seer_reports {
                lines.push(format!(
                    "Người {}: {}",
                    u16::from(seat.0) + 1,
                    if *alignment == Alignment::Wolf {
                        "Sói"
                    } else {
                        "Không phải Sói"
                    }
                ));
            }
            if let Some(seat) = doctor_previous {
                lines.push(format!("Bảo vệ đêm trước: Người {}", u16::from(seat.0) + 1));
            }
            if let Some(seat) = hunter_mark {
                lines.push(format!(
                    "Mục tiêu đặt trước: Người {}",
                    u16::from(seat.0) + 1
                ));
            }
            if let Some(p) = witch_potions {
                lines.push(format!(
                    "Bình cứu: {} · Bình độc: {}",
                    if p.heal { "Còn" } else { "Hết" },
                    if p.poison { "Còn" } else { "Hết" }
                ));
            }
            lines
        }
        PrivateKnowledge::Full {
            night_choices,
            seer_reports,
            witch_potions,
            doctor_previous,
            hunter_mark,
            hunter_fired,
            history,
        } => {
            let mut lines = vec!["Góc nhìn người chết: thấy mọi vai; không thể hành động".into()];
            for (seat, choice) in night_choices {
                lines.push(format!(
                    "Người {}: {}",
                    u16::from(seat.0) + 1,
                    choice_label(*choice)
                ));
            }
            for (seat, alignment) in seer_reports {
                lines.push(format!(
                    "Soi Người {}: {}",
                    u16::from(seat.0) + 1,
                    if *alignment == Alignment::Wolf {
                        "Sói"
                    } else {
                        "Không phải Sói"
                    }
                ));
            }
            if let Some(seat) = doctor_previous {
                lines.push(format!("Bác sĩ đêm trước: Người {}", u16::from(seat.0) + 1));
            }
            if let Some(seat) = hunter_mark {
                lines.push(format!(
                    "Thợ săn đặt trước: Người {}",
                    u16::from(seat.0) + 1
                ));
            }
            lines.push(format!(
                "Trả đũa đã dùng: {}",
                if *hunter_fired { "Có" } else { "Chưa" }
            ));
            if let Some(p) = witch_potions {
                lines.push(format!(
                    "Bình cứu: {} · Bình độc: {}",
                    if p.heal { "Còn" } else { "Hết" },
                    if p.poison { "Còn" } else { "Hết" }
                ));
            }
            for event in history.iter().rev().take(8) {
                if let ViewEvent::NightActionSubmitted {
                    seat,
                    choice,
                    round,
                } = event
                {
                    lines.push(format!(
                        "Đêm {round} · Người {}: {}",
                        u16::from(seat.0) + 1,
                        choice_label(*choice)
                    ));
                }
            }
            lines
        }
        PrivateKnowledge::None => Vec::new(),
    }
}

fn choice_label(choice: NightChoice) -> String {
    match choice {
        NightChoice::WolfTarget(Some(s)) => format!("Sói chọn Người {}", u16::from(s.0) + 1),
        NightChoice::Investigate(s) => format!("Soi Người {}", u16::from(s.0) + 1),
        NightChoice::Protect(Some(s)) => format!("Bảo vệ Người {}", u16::from(s.0) + 1),
        NightChoice::WitchHeal(Some(s)) => format!("Cứu Người {}", u16::from(s.0) + 1),
        NightChoice::WitchPoison(Some(s)) => format!("Độc Người {}", u16::from(s.0) + 1),
        NightChoice::HunterMark(Some(s)) => format!("Đặt trước Người {}", u16::from(s.0) + 1),
        _ => "Bỏ qua".into(),
    }
}

fn outcome_label(view: &View) -> Option<&'static str> {
    let outcome = view.outcome.as_ref()?;
    Some(match outcome.kind() {
        tabula_core::OutcomeKind::Draw => "Ván kết thúc · Hòa",
        tabula_core::OutcomeKind::Aborted { .. } => "Ván đã hủy",
        tabula_core::OutcomeKind::Decisive => match outcome.summary() {
            "werewolf.village_wins" => "Dân làng chiến thắng",
            "werewolf.wolves_win" => "Phe Ma sói chiến thắng",
            _ => "Ván kết thúc",
        },
    })
}

fn public_a11y_items(view: &View) -> Vec<A11yItem> {
    let mut public_items = Vec::new();
    if let Some(label) = outcome_label(view) {
        public_items.push(A11yItem {
            label: label.into(),
            position: "Kết quả".into(),
            state: "Công khai".into(),
            activates: None,
        });
    }
    for (seat, ballot) in &view.votes {
        public_items.push(A11yItem {
            label: match ballot {
                Ballot::Target(target) => format!(
                    "Người {} bỏ phiếu Người {}",
                    u16::from(seat.0) + 1,
                    u16::from(target.0) + 1
                ),
                Ballot::Abstain => format!("Người {} trắng phiếu", u16::from(seat.0) + 1),
            },
            position: "Phiếu".into(),
            state: "Công khai".into(),
            activates: None,
        });
    }

    public_items
}
