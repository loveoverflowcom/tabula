use glam::Vec2;
use macroquad::prelude as mq;
use tabula_presentation::{InputEvent, Key, PointerButton, PointerPhase};

#[derive(Clone, Debug, PartialEq)]
struct RawTouch {
    id: u64,
    position: Vec2,
    phase: PointerPhase,
}

#[derive(Clone, Debug, PartialEq)]
struct RawMouse {
    position: Vec2,
    button: PointerButton,
    phase: PointerPhase,
}

#[derive(Debug, Default)]
pub(crate) struct InputState {
    previous_mouse_position: Option<Vec2>,
}

impl InputState {
    pub(crate) fn drain(&mut self) -> Vec<InputEvent> {
        let mut touches = mq::touches()
            .into_iter()
            .filter_map(|touch| {
                let phase = match touch.phase {
                    mq::TouchPhase::Started => PointerPhase::Down,
                    mq::TouchPhase::Moved => PointerPhase::Move,
                    mq::TouchPhase::Ended => PointerPhase::Up,
                    mq::TouchPhase::Cancelled => PointerPhase::Cancel,
                    mq::TouchPhase::Stationary => return None,
                };
                Some(RawTouch {
                    id: touch.id,
                    position: Vec2::new(touch.position.x, touch.position.y),
                    phase,
                })
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
        normalize(touches, mouse, key_events)
    }
}

fn normalize(
    touches: Vec<RawTouch>,
    mouse: Vec<RawMouse>,
    keys: impl IntoIterator<Item = (Key, bool)>,
) -> Vec<InputEvent> {
    let mut events = Vec::new();
    let mut touches = touches;
    touches.sort_by_key(|touch| touch.id);
    if let Some(touch) = touches.into_iter().next() {
        events.push(InputEvent::Pointer {
            position: touch.position,
            button: PointerButton::Primary,
            phase: touch.phase,
        });
    } else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowest_touch_id_is_the_single_normalized_primary_pointer() {
        let events = normalize(
            vec![
                RawTouch {
                    id: 9,
                    position: Vec2::new(9.0, 9.0),
                    phase: PointerPhase::Down,
                },
                RawTouch {
                    id: 1,
                    position: Vec2::new(1.0, 1.0),
                    phase: PointerPhase::Move,
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
                phase: PointerPhase::Move,
            }]
        );
    }

    #[test]
    fn touch_suppresses_duplicate_mouse_events_and_keys_follow_pointer_events() {
        let events = normalize(
            vec![RawTouch {
                id: 1,
                position: Vec2::ONE,
                phase: PointerPhase::Up,
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
                phase: PointerPhase::Up,
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
}
