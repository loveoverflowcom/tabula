use glam::Vec2;
use macroquad::prelude as mq;
use tabula_presentation::{InputEvent, Key, PointerButton, PointerPhase, PointerPosition};

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
    active_touch: Option<ActiveTouch>,
    active_mouse_buttons: [bool; 3],
    last_mouse_position: Option<PointerPosition>,
}

impl InputState {
    pub(crate) fn drain(&mut self) -> Vec<InputEvent> {
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
                mouse.push(RawMouse {
                    position: raw_mouse_position,
                    button,
                    phase: PointerPhase::Up,
                });
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
        self.normalize(touches, mouse, key_events)
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

fn pointer_event(position: PointerPosition, phase: PointerPhase) -> InputEvent {
    InputEvent::Pointer {
        position,
        button: PointerButton::Primary,
        phase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_pointer_event(position: Vec2, phase: PointerPhase) -> InputEvent {
        pointer_event(PointerPosition::try_from(position).unwrap(), phase)
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
