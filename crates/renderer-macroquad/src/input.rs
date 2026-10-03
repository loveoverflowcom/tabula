use glam::Vec2;
use macroquad::prelude as mq;
use tabula_presentation::{InputEvent, Key, PointerButton, PointerPhase, PointerPosition};

/// Observed Miniquad lifecycle callbacks; replay never calls back into Macroquad.
///
/// These callbacks conflate minimize, blur, and (on macOS) window movement. They signal an
/// input interruption rather than an exact OS focus query. A later restored callback or fresh
/// press resumes interaction; hover and synthetic releases cannot do so.
#[derive(Debug, Default)]
struct FocusEvents {
    changes: Vec<bool>,
    fresh_press: bool,
    native_mouse_down: [bool; 3],
    replayed_mouse_up: [bool; 3],
}

impl macroquad::miniquad::EventHandler for FocusEvents {
    fn update(&mut self) {}

    fn draw(&mut self) {}

    fn window_minimized_event(&mut self) {
        self.changes.push(false);
    }

    fn window_restored_event(&mut self) {
        self.changes.push(true);
    }

    fn key_down_event(
        &mut self,
        _key: macroquad::miniquad::KeyCode,
        _modifiers: macroquad::miniquad::KeyMods,
        repeat: bool,
    ) {
        self.fresh_press |= !repeat;
    }

    fn mouse_button_down_event(
        &mut self,
        button: macroquad::miniquad::MouseButton,
        x: f32,
        y: f32,
    ) {
        let button = match button {
            macroquad::miniquad::MouseButton::Left => PointerButton::Primary,
            macroquad::miniquad::MouseButton::Right => PointerButton::Secondary,
            macroquad::miniquad::MouseButton::Middle => PointerButton::Middle,
            macroquad::miniquad::MouseButton::Unknown => return,
        };
        if x.is_finite() && y.is_finite() {
            self.fresh_press = true;
            self.native_mouse_down[mouse_button_index(button)] = true;
        }
    }

    fn touch_event(&mut self, phase: macroquad::miniquad::TouchPhase, _id: u64, x: f32, y: f32) {
        self.fresh_press |=
            phase == macroquad::miniquad::TouchPhase::Started && x.is_finite() && y.is_finite();
    }

    fn mouse_button_up_event(
        &mut self,
        button: macroquad::miniquad::MouseButton,
        _x: f32,
        _y: f32,
    ) {
        let button = match button {
            macroquad::miniquad::MouseButton::Left => PointerButton::Primary,
            macroquad::miniquad::MouseButton::Right => PointerButton::Secondary,
            macroquad::miniquad::MouseButton::Middle => PointerButton::Middle,
            macroquad::miniquad::MouseButton::Unknown => return,
        };
        self.replayed_mouse_up[mouse_button_index(button)] = true;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawTouchPhase {
    Started,
    Moved,
    Stationary,
    Ended,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RawTouch {
    id: u64,
    position: Vec2,
    phase: RawTouchPhase,
}

#[derive(Clone, Debug, PartialEq)]
struct RawMouse {
    position: Vec2,
    button: PointerButton,
    phase: PointerPhase,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActiveTouch {
    id: u64,
    last_position: PointerPosition,
}

#[derive(Debug, Default)]
pub(crate) struct InputState {
    // Registration is lazy so constructing the renderer remains independent of a GPU context.
    focus_subscriber: Option<usize>,
    suspended: bool,
    active_touch: Option<ActiveTouch>,
    active_mouse_buttons: [bool; 3],
    last_mouse_position: Option<PointerPosition>,
}

impl InputState {
    pub(crate) fn drain(&mut self) -> Vec<InputEvent> {
        let subscriber = *self
            .focus_subscriber
            .get_or_insert_with(macroquad::input::utils::register_input_subscriber);
        let mut focus = FocusEvents::default();
        macroquad::input::utils::repeat_all_miniquad_input(&mut focus, subscriber);

        let mut touches = mq::touches()
            .into_iter()
            .map(|touch| {
                let phase = match touch.phase {
                    mq::TouchPhase::Started => RawTouchPhase::Started,
                    mq::TouchPhase::Moved => RawTouchPhase::Moved,
                    mq::TouchPhase::Ended => RawTouchPhase::Ended,
                    mq::TouchPhase::Cancelled => RawTouchPhase::Cancelled,
                    mq::TouchPhase::Stationary => RawTouchPhase::Stationary,
                };
                RawTouch {
                    id: touch.id,
                    position: Vec2::new(touch.position.x, touch.position.y),
                    phase,
                }
            })
            .collect::<Vec<_>>();
        touches.sort_by_key(|touch| touch.id);

        let mouse_position = mq::mouse_position();
        let raw_mouse_position = Vec2::new(mouse_position.0, mouse_position.1);
        let mouse_position = PointerPosition::try_from(raw_mouse_position).ok();
        let mut mouse = Vec::new();
        for (button, macroquad_button) in [
            (PointerButton::Primary, mq::MouseButton::Left),
            (PointerButton::Secondary, mq::MouseButton::Right),
            (PointerButton::Middle, mq::MouseButton::Middle),
        ] {
            if mq::is_mouse_button_pressed(macroquad_button) {
                mouse.push(RawMouse {
                    position: raw_mouse_position,
                    button,
                    phase: PointerPhase::Down,
                });
            }
            if mq::is_mouse_button_released(macroquad_button) {
                mouse.push(raw_mouse_release(
                    raw_mouse_position,
                    button,
                    mq::is_mouse_button_down(macroquad_button),
                ));
            }
        }
        if self.last_mouse_position != mouse_position {
            if let Some(position) = mouse_position {
                mouse.push(RawMouse {
                    position: position.get(),
                    button: PointerButton::Primary,
                    phase: PointerPhase::Move,
                });
            }
        }
        if mouse_position.is_some() {
            self.last_mouse_position = mouse_position;
        }

        let keys = [
            (Key::ArrowUp, mq::KeyCode::Up),
            (Key::ArrowDown, mq::KeyCode::Down),
            (Key::ArrowLeft, mq::KeyCode::Left),
            (Key::ArrowRight, mq::KeyCode::Right),
            (Key::Enter, mq::KeyCode::Enter),
            (Key::Space, mq::KeyCode::Space),
            (Key::Escape, mq::KeyCode::Escape),
            (Key::Tab, mq::KeyCode::Tab),
        ];

        let mut key_events = Vec::new();
        for (key, code) in keys {
            if mq::is_key_pressed(code) {
                key_events.push((key, true));
            }
            if mq::is_key_released(code) {
                key_events.push((key, false));
            }
        }
        self.normalize_frame(focus, touches, mouse, key_events)
    }

    fn normalize_frame(
        &mut self,
        focus: FocusEvents,
        touches: Vec<RawTouch>,
        mouse: Vec<RawMouse>,
        keys: impl IntoIterator<Item = (Key, bool)>,
    ) -> Vec<InputEvent> {
        let was_suspended = self.suspended;
        let keys = keys.into_iter().collect::<Vec<_>>();
        let mut events = Vec::new();
        let mut interrupted = false;
        for focused in focus.changes {
            events.push(InputEvent::Focus(focused));
            self.suspended = !focused;
            if !focused {
                interrupted = true;
                self.cancel_contacts(&mut events);
            }
        }
        if interrupted {
            // Macroquad turns held mouse/touch/keys into releases on interruption. None of
            // those frame edges may activate a control, even if restoration is also queued.
            return events;
        }
        if was_suspended && !focus.fresh_press {
            // A restored callback re-enables future input, but cannot authenticate polling
            // Down/Up edges from a held mouse re-entering the window in this frame.
            return events;
        }
        if self.suspended {
            // Polling edges alone are insufficient: Macroquad also marks mouse-enter with a
            // held button as pressed. Only a replayed, non-repeat native press can restore.
            if !focus.fresh_press {
                return events;
            }
            self.suspended = false;
            events.push(InputEvent::Focus(true));
        }
        // Miniquad mouse-enter can become a polling Down even on a later restored frame.
        // Authenticate each button independently; another fresh key/button cannot arm it.
        let mut unverified_mouse_down = [false; 3];
        for event in &mouse {
            let index = mouse_button_index(event.button);
            if event.phase == PointerPhase::Down && !focus.native_mouse_down[index] {
                unverified_mouse_down[index] = true;
            }
        }
        let mouse = mouse
            .into_iter()
            .map(|mut event| {
                let index = mouse_button_index(event.button);
                if event.phase == PointerPhase::Up
                    && (!focus.replayed_mouse_up[index] || unverified_mouse_down[index])
                {
                    // Mouse-leave synthesizes releases; held re-entry can even replay a fake
                    // Up. An unverified polling Down makes that release ambiguous, so cancel.
                    event.phase = PointerPhase::Cancel;
                }
                event
            })
            .filter(|event| {
                event.phase != PointerPhase::Down
                    || focus.native_mouse_down[mouse_button_index(event.button)]
            })
            .collect();
        events.extend(self.normalize(touches, mouse, keys));
        events
    }

    fn cancel_contacts(&mut self, events: &mut Vec<InputEvent>) {
        if let Some(touch) = self.active_touch.take() {
            events.push(pointer_event(touch.last_position, PointerPhase::Cancel));
        }
        if let Some(position) = self.last_mouse_position.take() {
            for button in [
                PointerButton::Primary,
                PointerButton::Secondary,
                PointerButton::Middle,
            ] {
                if self.active_mouse_buttons[mouse_button_index(button)] {
                    events.push(InputEvent::Pointer {
                        position,
                        button,
                        phase: PointerPhase::Cancel,
                    });
                }
            }
        }
        self.active_mouse_buttons = [false; 3];
    }

    fn normalize(
        &mut self,
        mut touches: Vec<RawTouch>,
        mouse: Vec<RawMouse>,
        keys: impl IntoIterator<Item = (Key, bool)>,
    ) -> Vec<InputEvent> {
        touches.sort_by_key(|touch| touch.id);
        let touch_contact = self.active_touch.is_some() || !touches.is_empty();
        let touch_event = match self.active_touch {
            Some(active) => {
                if let Some(touch) = touches.iter().copied().find(|touch| touch.id == active.id) {
                    self.active_touch_event(active, touch)
                } else {
                    self.active_touch = None;
                    Some(pointer_event(active.last_position, PointerPhase::Cancel))
                }
            }
            None => touches
                .into_iter()
                .filter(|touch| touch.phase == RawTouchPhase::Started)
                .find_map(|touch| {
                    let position = PointerPosition::try_from(touch.position).ok()?;
                    self.active_touch = Some(ActiveTouch {
                        id: touch.id,
                        last_position: position,
                    });
                    Some(pointer_event(position, PointerPhase::Down))
                }),
        };

        let mut events = touch_event.into_iter().collect::<Vec<_>>();
        if !touch_contact {
            events.extend(
                mouse
                    .into_iter()
                    .filter_map(|event| self.normalize_mouse_event(&event)),
            );
        }
        events.extend(
            keys.into_iter()
                .map(|(key, pressed)| InputEvent::Key { key, pressed }),
        );
        events
    }

    fn normalize_mouse_event(&mut self, event: &RawMouse) -> Option<InputEvent> {
        let button_index = mouse_button_index(event.button);
        if matches!(event.phase, PointerPhase::Up | PointerPhase::Cancel)
            && !self.active_mouse_buttons[button_index]
        {
            // Focus interruption cancels ownership; a later physical release cannot re-arm it.
            return None;
        }
        match PointerPosition::try_from(event.position) {
            Ok(position) => {
                self.last_mouse_position = Some(position);
                match event.phase {
                    PointerPhase::Down => self.active_mouse_buttons[button_index] = true,
                    PointerPhase::Up | PointerPhase::Cancel => {
                        self.active_mouse_buttons[button_index] = false;
                    }
                    PointerPhase::Move => {}
                }
                Some(InputEvent::Pointer {
                    position,
                    button: event.button,
                    phase: event.phase,
                })
            }
            Err(_)
                if matches!(event.phase, PointerPhase::Up | PointerPhase::Cancel)
                    && self.active_mouse_buttons[button_index] =>
            {
                self.active_mouse_buttons[button_index] = false;
                self.last_mouse_position
                    .map(|position| InputEvent::Pointer {
                        position,
                        button: event.button,
                        phase: PointerPhase::Cancel,
                    })
            }
            Err(_) => None,
        }
    }

    fn active_touch_event(&mut self, active: ActiveTouch, touch: RawTouch) -> Option<InputEvent> {
        match touch.phase {
            RawTouchPhase::Started | RawTouchPhase::Stationary => {
                if PointerPosition::try_from(touch.position).is_ok() {
                    None
                } else {
                    self.active_touch = None;
                    Some(pointer_event(active.last_position, PointerPhase::Cancel))
                }
            }
            RawTouchPhase::Moved => {
                if let Ok(position) = PointerPosition::try_from(touch.position) {
                    self.active_touch = Some(ActiveTouch {
                        id: active.id,
                        last_position: position,
                    });
                    Some(pointer_event(position, PointerPhase::Move))
                } else {
                    self.active_touch = None;
                    Some(pointer_event(active.last_position, PointerPhase::Cancel))
                }
            }
            RawTouchPhase::Ended => {
                self.active_touch = None;
                let position =
                    PointerPosition::try_from(touch.position).unwrap_or(active.last_position);
                let phase = if position == active.last_position && !touch.position.is_finite() {
                    PointerPhase::Cancel
                } else {
                    PointerPhase::Up
                };
                Some(pointer_event(position, phase))
            }
            RawTouchPhase::Cancelled => {
                self.active_touch = None;
                let position =
                    PointerPosition::try_from(touch.position).unwrap_or(active.last_position);
                Some(pointer_event(position, PointerPhase::Cancel))
            }
        }
    }
}

const fn mouse_button_index(button: PointerButton) -> usize {
    match button {
        PointerButton::Primary => 0,
        PointerButton::Secondary => 1,
        PointerButton::Middle => 2,
    }
}

fn raw_mouse_release(position: Vec2, button: PointerButton, still_down: bool) -> RawMouse {
    RawMouse {
        position,
        button,
        // A held button can re-enter after a synthetic leave release. Coalesced snapshots
        // cannot establish a completed click while it remains down, so cancel conservatively.
        phase: if still_down {
            PointerPhase::Cancel
        } else {
            PointerPhase::Up
        },
    }
}

fn pointer_event(position: PointerPosition, phase: PointerPhase) -> InputEvent {
    InputEvent::Pointer {
        position,
        button: PointerButton::Primary,
        phase,
    }
}

#[cfg(test)]
mod tests {
    use macroquad::miniquad::EventHandler;

    use super::*;

    fn valid_pointer_event(position: Vec2, phase: PointerPhase) -> InputEvent {
        pointer_event(PointerPosition::try_from(position).unwrap(), phase)
    }

    #[test]
    fn default_input_state_registers_no_graphics_subscriber() {
        let state = InputState::default();
        assert_eq!(state.focus_subscriber, None);
        assert!(!state.suspended);
    }

    #[test]
    fn focus_loss_cancels_contacts_before_discarding_synthetic_releases() {
        let mut state = InputState::default();
        let position = Vec2::new(10.0, 20.0);
        state.normalize(
            vec![],
            vec![RawMouse {
                position,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            }],
            [(Key::Enter, true)],
        );
        let mut focus = FocusEvents::default();
        focus.window_minimized_event();
        assert_eq!(
            state.normalize_frame(
                focus,
                vec![],
                vec![RawMouse {
                    position,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Up,
                }],
                [(Key::Enter, false)],
            ),
            [
                InputEvent::Focus(false),
                valid_pointer_event(position, PointerPhase::Cancel),
            ]
        );
        assert!(state.suspended);
        assert_eq!(state.active_mouse_buttons, [false; 3]);
        assert_eq!(state.last_mouse_position, None);
    }

    #[test]
    fn focus_loss_cancels_touch_and_restoration_cannot_replay_a_stale_release() {
        let mut state = InputState::default();
        let position = Vec2::new(3.0, 4.0);
        state.normalize(
            vec![RawTouch {
                id: 7,
                position,
                phase: RawTouchPhase::Started,
            }],
            vec![],
            [],
        );
        let mut focus = FocusEvents::default();
        focus.window_minimized_event();
        focus.window_restored_event();
        assert_eq!(
            state.normalize_frame(
                focus,
                vec![RawTouch {
                    id: 7,
                    position,
                    phase: RawTouchPhase::Ended,
                }],
                vec![],
                [(Key::Space, true), (Key::Space, false)],
            ),
            [
                InputEvent::Focus(false),
                valid_pointer_event(position, PointerPhase::Cancel),
                InputEvent::Focus(true),
            ]
        );
        assert_eq!(state.active_touch, None);
        assert!(!state.suspended);
    }

    #[test]
    fn move_only_interruption_resumes_before_a_fresh_space_press() {
        let mut state = InputState::default();
        let mut interrupted = FocusEvents::default();
        interrupted.window_minimized_event();
        assert_eq!(
            state.normalize_frame(interrupted, vec![], vec![], []),
            [InputEvent::Focus(false)]
        );
        let mut fresh = FocusEvents::default();
        fresh.key_down_event(
            mq::KeyCode::Space,
            macroquad::miniquad::KeyMods::default(),
            false,
        );
        assert_eq!(
            state.normalize_frame(fresh, vec![], vec![], [(Key::Space, true)]),
            [
                InputEvent::Focus(true),
                InputEvent::Key {
                    key: Key::Space,
                    pressed: true
                }
            ]
        );
        assert!(!state.suspended);
    }

    #[test]
    fn hover_release_and_held_key_repeat_cannot_restore_interaction() {
        let mut state = InputState::default();
        let mut interrupted = FocusEvents::default();
        interrupted.window_minimized_event();
        state.normalize_frame(interrupted, vec![], vec![], []);
        let mut unfocused_callbacks = FocusEvents::default();
        unfocused_callbacks.mouse_motion_event(10.0, 20.0);
        unfocused_callbacks.mouse_enter_event(mq::MouseButton::Left, 10.0, 20.0);
        unfocused_callbacks.key_down_event(
            mq::KeyCode::Space,
            macroquad::miniquad::KeyMods::default(),
            true,
        );
        unfocused_callbacks
            .key_up_event(mq::KeyCode::Space, macroquad::miniquad::KeyMods::default());
        assert!(!unfocused_callbacks.fresh_press);
        assert!(state
            .normalize_frame(
                unfocused_callbacks,
                vec![],
                vec![RawMouse {
                    position: Vec2::new(10.0, 20.0),
                    button: PointerButton::Primary,
                    phase: PointerPhase::Down,
                }],
                [(Key::Space, false)],
            )
            .is_empty());
        assert!(state.suspended);
    }

    #[test]
    fn restored_callback_and_fresh_pointer_press_precede_normalized_input() {
        for restored in [false, true] {
            let mut state = InputState {
                suspended: true,
                ..InputState::default()
            };
            let mut focus = FocusEvents::default();
            if restored {
                focus.window_restored_event();
            }
            focus.mouse_button_down_event(mq::MouseButton::Left, 5.0, 6.0);
            assert_eq!(
                state.normalize_frame(
                    focus,
                    vec![],
                    vec![RawMouse {
                        position: Vec2::new(5.0, 6.0),
                        button: PointerButton::Primary,
                        phase: PointerPhase::Down,
                    }],
                    [],
                ),
                [
                    InputEvent::Focus(true),
                    valid_pointer_event(Vec2::new(5.0, 6.0), PointerPhase::Down)
                ]
            );
        }
    }

    #[test]
    fn restoration_without_a_fresh_press_cannot_rearm_a_held_mouse() {
        let mut state = InputState {
            suspended: true,
            ..InputState::default()
        };
        let position = Vec2::new(5.0, 6.0);
        let mut focus = FocusEvents::default();
        focus.window_restored_event();
        focus.mouse_enter_event(mq::MouseButton::Left, position.x, position.y);
        assert_eq!(
            state.normalize_frame(
                focus,
                vec![],
                vec![RawMouse {
                    position,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Down
                }],
                [],
            ),
            [InputEvent::Focus(true)]
        );
        assert!(state
            .normalize_frame(
                FocusEvents::default(),
                vec![],
                vec![RawMouse {
                    position,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Up
                }],
                [],
            )
            .is_empty());
        assert_eq!(state.active_mouse_buttons, [false; 3]);
    }

    #[test]
    fn a_held_mouse_enter_after_the_restoration_frame_cannot_activate() {
        let mut state = InputState {
            suspended: true,
            ..InputState::default()
        };
        let mut restored = FocusEvents::default();
        restored.window_restored_event();
        assert_eq!(
            state.normalize_frame(restored, vec![], vec![], []),
            [InputEvent::Focus(true)]
        );
        let position = Vec2::new(5.0, 6.0);
        let mut entered = FocusEvents::default();
        entered.mouse_enter_event(mq::MouseButton::Left, position.x, position.y);
        assert!(state
            .normalize_frame(
                entered,
                vec![],
                vec![RawMouse {
                    position,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Down
                }],
                [],
            )
            .is_empty());
        assert!(state
            .normalize_frame(
                FocusEvents::default(),
                vec![],
                vec![RawMouse {
                    position,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Up
                }],
                [],
            )
            .is_empty());
    }

    #[test]
    fn a_fresh_key_cannot_authenticate_an_unrelated_mouse_press() {
        let mut state = InputState {
            suspended: true,
            ..InputState::default()
        };
        let mut focus = FocusEvents::default();
        focus.key_down_event(
            mq::KeyCode::Space,
            macroquad::miniquad::KeyMods::default(),
            false,
        );
        assert_eq!(
            state.normalize_frame(
                focus,
                vec![],
                vec![RawMouse {
                    position: Vec2::new(5.0, 6.0),
                    button: PointerButton::Primary,
                    phase: PointerPhase::Down,
                }],
                [(Key::Space, true)],
            ),
            [
                InputEvent::Focus(true),
                InputEvent::Key {
                    key: Key::Space,
                    pressed: true
                }
            ]
        );
        assert_eq!(state.active_mouse_buttons, [false; 3]);
    }

    #[test]
    fn synthetic_mouse_leave_release_cancels_instead_of_activating() {
        let mut state = InputState::default();
        let position = Vec2::new(5.0, 6.0);
        state.normalize(
            vec![],
            vec![RawMouse {
                position,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            }],
            [],
        );
        assert_eq!(
            state.normalize_frame(
                FocusEvents::default(),
                vec![],
                vec![RawMouse {
                    position,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Up
                }],
                [],
            ),
            [valid_pointer_event(position, PointerPhase::Cancel)]
        );
        assert_eq!(state.active_mouse_buttons, [false; 3]);
    }

    #[test]
    fn a_replayed_mouse_release_keeps_the_normal_up_phase() {
        let mut state = InputState::default();
        let position = Vec2::new(5.0, 6.0);
        state.normalize(
            vec![],
            vec![RawMouse {
                position,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            }],
            [],
        );
        let mut focus = FocusEvents::default();
        focus.mouse_button_up_event(mq::MouseButton::Left, position.x, position.y);
        assert_eq!(
            state.normalize_frame(
                focus,
                vec![],
                vec![raw_mouse_release(position, PointerButton::Primary, false)],
                [],
            ),
            [valid_pointer_event(position, PointerPhase::Up)]
        );
    }

    #[test]
    fn mouse_leave_and_held_reentry_cannot_release_an_armed_control() {
        let mut state = InputState::default();
        let position = Vec2::new(5.0, 6.0);
        state.normalize(
            vec![],
            vec![RawMouse {
                position,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            }],
            [],
        );
        // Macroquad mouse-enter records MouseButtonUp but marks polling Down while held.
        let mut focus = FocusEvents::default();
        focus.mouse_button_up_event(mq::MouseButton::Left, position.x, position.y);
        assert_eq!(
            state.normalize_frame(
                focus,
                vec![],
                vec![
                    RawMouse {
                        position,
                        button: PointerButton::Primary,
                        phase: PointerPhase::Down
                    },
                    RawMouse {
                        position,
                        button: PointerButton::Primary,
                        phase: PointerPhase::Up
                    },
                ],
                [],
            ),
            [valid_pointer_event(position, PointerPhase::Cancel)]
        );
        assert_eq!(state.active_mouse_buttons, [false; 3]);
    }

    #[test]
    fn a_coalesced_release_with_the_button_still_down_cancels_a_new_press() {
        let mut state = InputState::default();
        let position = Vec2::new(5.0, 6.0);
        let mut focus = FocusEvents::default();
        focus.mouse_button_down_event(mq::MouseButton::Left, position.x, position.y);
        focus.mouse_button_up_event(mq::MouseButton::Left, position.x, position.y);
        assert_eq!(
            state.normalize_frame(
                focus,
                vec![],
                vec![
                    RawMouse {
                        position,
                        button: PointerButton::Primary,
                        phase: PointerPhase::Down
                    },
                    raw_mouse_release(position, PointerButton::Primary, true),
                ],
                [],
            ),
            [
                valid_pointer_event(position, PointerPhase::Down),
                valid_pointer_event(position, PointerPhase::Cancel)
            ]
        );
        assert_eq!(state.active_mouse_buttons, [false; 3]);
    }

    #[test]
    fn first_started_touch_becomes_the_primary_pointer() {
        let events = InputState::default().normalize(
            vec![
                RawTouch {
                    id: 9,
                    position: Vec2::new(9.0, 9.0),
                    phase: RawTouchPhase::Started,
                },
                RawTouch {
                    id: 1,
                    position: Vec2::new(1.0, 1.0),
                    phase: RawTouchPhase::Started,
                },
            ],
            vec![RawMouse {
                position: Vec2::ZERO,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            }],
            [],
        );
        assert_eq!(
            events,
            [valid_pointer_event(Vec2::new(1.0, 1.0), PointerPhase::Down)]
        );
    }

    #[test]
    fn touch_suppresses_duplicate_mouse_events_and_keys_follow_pointer_events() {
        let events = InputState::default().normalize(
            vec![RawTouch {
                id: 1,
                position: Vec2::ONE,
                phase: RawTouchPhase::Started,
            }],
            vec![RawMouse {
                position: Vec2::ZERO,
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            }],
            [(Key::Escape, true)],
        );
        assert!(matches!(
            events[0],
            InputEvent::Pointer {
                phase: PointerPhase::Down,
                ..
            }
        ));
        assert_eq!(
            events[1],
            InputEvent::Key {
                key: Key::Escape,
                pressed: true
            }
        );
    }

    #[test]
    fn primary_touch_keeps_ownership_across_frames() {
        let mut state = InputState::default();
        let mouse = || {
            vec![RawMouse {
                position: Vec2::ZERO,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            }]
        };

        assert_eq!(
            state.normalize(
                vec![RawTouch {
                    id: 9,
                    position: Vec2::new(9.0, 9.0),
                    phase: RawTouchPhase::Started,
                }],
                mouse(),
                []
            ),
            [valid_pointer_event(Vec2::new(9.0, 9.0), PointerPhase::Down)]
        );

        assert!(state
            .normalize(
                vec![
                    RawTouch {
                        id: 1,
                        position: Vec2::ONE,
                        phase: RawTouchPhase::Started,
                    },
                    RawTouch {
                        id: 9,
                        position: Vec2::new(9.0, 9.0),
                        phase: RawTouchPhase::Stationary,
                    },
                ],
                mouse(),
                []
            )
            .is_empty());

        assert_eq!(
            state.normalize(
                vec![
                    RawTouch {
                        id: 1,
                        position: Vec2::new(2.0, 2.0),
                        phase: RawTouchPhase::Moved,
                    },
                    RawTouch {
                        id: 9,
                        position: Vec2::new(10.0, 10.0),
                        phase: RawTouchPhase::Moved,
                    },
                ],
                mouse(),
                []
            ),
            [valid_pointer_event(
                Vec2::new(10.0, 10.0),
                PointerPhase::Move
            )]
        );

        assert_eq!(
            state.normalize(
                vec![RawTouch {
                    id: 9,
                    position: Vec2::new(11.0, 11.0),
                    phase: RawTouchPhase::Ended,
                }],
                mouse(),
                []
            ),
            [valid_pointer_event(Vec2::new(11.0, 11.0), PointerPhase::Up)]
        );

        assert_eq!(
            state.normalize(vec![], mouse(), []),
            [valid_pointer_event(Vec2::ZERO, PointerPhase::Move)]
        );
    }

    #[test]
    fn disappearing_primary_touch_cancels_before_mouse_fallback() {
        let mut state = InputState::default();
        let start = RawTouch {
            id: 4,
            position: Vec2::new(4.0, 4.0),
            phase: RawTouchPhase::Started,
        };
        let mouse = vec![RawMouse {
            position: Vec2::ZERO,
            button: PointerButton::Primary,
            phase: PointerPhase::Move,
        }];
        let _ = state.normalize(vec![start], mouse.clone(), []);

        assert_eq!(
            state.normalize(vec![], mouse.clone(), []),
            [valid_pointer_event(
                Vec2::new(4.0, 4.0),
                PointerPhase::Cancel
            )]
        );
        assert_eq!(
            state.normalize(vec![], mouse, []),
            [valid_pointer_event(Vec2::ZERO, PointerPhase::Move)]
        );
    }

    #[test]
    fn invalid_platform_pointer_cannot_wedge_touch_ownership() {
        let mut state = InputState::default();
        assert_eq!(
            state.normalize(
                vec![
                    RawTouch {
                        id: 1,
                        position: Vec2::new(f32::NAN, 0.0),
                        phase: RawTouchPhase::Started,
                    },
                    RawTouch {
                        id: 2,
                        position: Vec2::new(2.0, 2.0),
                        phase: RawTouchPhase::Started,
                    },
                ],
                vec![],
                [],
            ),
            [valid_pointer_event(Vec2::new(2.0, 2.0), PointerPhase::Down)]
        );

        assert_eq!(
            state.normalize(
                vec![RawTouch {
                    id: 2,
                    position: Vec2::new(f32::INFINITY, 2.0),
                    phase: RawTouchPhase::Moved,
                }],
                vec![],
                [],
            ),
            [valid_pointer_event(
                Vec2::new(2.0, 2.0),
                PointerPhase::Cancel
            )]
        );

        assert_eq!(
            state.normalize(
                vec![RawTouch {
                    id: 3,
                    position: Vec2::new(3.0, 3.0),
                    phase: RawTouchPhase::Started,
                }],
                vec![],
                [],
            ),
            [valid_pointer_event(Vec2::new(3.0, 3.0), PointerPhase::Down)]
        );
    }

    #[test]
    fn normalized_input_never_emits_non_finite_pointer_coordinates() {
        let mut state = InputState::default();
        let events = state.normalize(
            vec![],
            vec![RawMouse {
                position: Vec2::new(0.0, f32::NEG_INFINITY),
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            }],
            [],
        );
        assert!(events.is_empty());
    }

    #[test]
    fn invalid_mouse_release_cancels_at_last_finite_position() {
        let mut state = InputState::default();
        let position = Vec2::new(10.0, 20.0);

        assert_eq!(
            state.normalize(
                vec![],
                vec![RawMouse {
                    position,
                    button: PointerButton::Primary,
                    phase: PointerPhase::Down,
                }],
                [],
            ),
            [valid_pointer_event(position, PointerPhase::Down)]
        );

        assert_eq!(
            state.normalize(
                vec![],
                vec![RawMouse {
                    position: Vec2::new(f32::NAN, f32::INFINITY),
                    button: PointerButton::Primary,
                    phase: PointerPhase::Up,
                }],
                [],
            ),
            [valid_pointer_event(position, PointerPhase::Cancel)]
        );

        assert!(state.normalize(vec![], vec![], []).is_empty());
    }
}
