use glam::Vec2;
use macroquad::prelude as mq;
use tabula_presentation::{InputEvent, Key, PointerButton, PointerPhase};

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
    last_position: Vec2,
}

#[derive(Debug, Default)]
pub(crate) struct InputState {
    active_touch: Option<ActiveTouch>,
    previous_mouse_position: Option<Vec2>,
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
        let mouse_position = Vec2::new(mouse_position.0, mouse_position.1);
        let mut mouse = Vec::new();
        for (button, macroquad_button) in [
            (PointerButton::Primary, mq::MouseButton::Left),
            (PointerButton::Secondary, mq::MouseButton::Right),
            (PointerButton::Middle, mq::MouseButton::Middle),
        ] {
            if mq::is_mouse_button_pressed(macroquad_button) {
                mouse.push(RawMouse {
                    position: mouse_position,
                    button,
                    phase: PointerPhase::Down,
                });
            }
            if mq::is_mouse_button_released(macroquad_button) {
                mouse.push(RawMouse {
                    position: mouse_position,
                    button,
                    phase: PointerPhase::Up,
                });
            }
        }
        if self.previous_mouse_position != Some(mouse_position) {
            mouse.push(RawMouse {
                position: mouse_position,
                button: PointerButton::Primary,
                phase: PointerPhase::Move,
            });
        }
        self.previous_mouse_position = Some(mouse_position);

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
                .find(|touch| touch.phase == RawTouchPhase::Started)
                .map(|touch| {
                    self.active_touch = Some(ActiveTouch {
                        id: touch.id,
                        last_position: touch.position,
                    });
                    pointer_event(touch.position, PointerPhase::Down)
                }),
        };

        let mut events = touch_event.into_iter().collect::<Vec<_>>();
        if !touch_contact {
            events.extend(mouse.into_iter().map(|event| InputEvent::Pointer {
                position: event.position,
                button: event.button,
                phase: event.phase,
            }));
        }
        events.extend(
            keys.into_iter()
                .map(|(key, pressed)| InputEvent::Key { key, pressed }),
        );
        events
    }

    fn active_touch_event(&mut self, active: ActiveTouch, touch: RawTouch) -> Option<InputEvent> {
        match touch.phase {
            RawTouchPhase::Started | RawTouchPhase::Stationary => None,
            RawTouchPhase::Moved => {
                self.active_touch = Some(ActiveTouch {
                    id: active.id,
                    last_position: touch.position,
                });
                Some(pointer_event(touch.position, PointerPhase::Move))
            }
            RawTouchPhase::Ended => {
                self.active_touch = None;
                Some(pointer_event(touch.position, PointerPhase::Up))
            }
            RawTouchPhase::Cancelled => {
                self.active_touch = None;
                Some(pointer_event(touch.position, PointerPhase::Cancel))
            }
        }
    }
}

fn pointer_event(position: Vec2, phase: PointerPhase) -> InputEvent {
    InputEvent::Pointer {
        position,
        button: PointerButton::Primary,
        phase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            [InputEvent::Pointer {
                position: Vec2::new(1.0, 1.0),
                button: PointerButton::Primary,
                phase: PointerPhase::Down,
            }]
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
            [pointer_event(Vec2::new(9.0, 9.0), PointerPhase::Down)]
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
            [pointer_event(Vec2::new(10.0, 10.0), PointerPhase::Move)]
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
            [pointer_event(Vec2::new(11.0, 11.0), PointerPhase::Up)]
        );

        assert_eq!(
            state.normalize(vec![], mouse(), []),
            [pointer_event(Vec2::ZERO, PointerPhase::Move)]
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
            [pointer_event(Vec2::new(4.0, 4.0), PointerPhase::Cancel)]
        );
        assert_eq!(
            state.normalize(vec![], mouse, []),
            [pointer_event(Vec2::ZERO, PointerPhase::Move)]
        );
    }
}
