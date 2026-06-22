use std::sync::mpsc::Sender;
use std::sync::Mutex;

use rdev::{Event, EventType, Key};
use winit::event_loop::EventLoopProxy;

use crate::state::InputEvent;

/// Last known mouse position, updated from MouseMove events.
/// Needed because rdev 0.5 ButtonPress/ButtonRelease carry no coordinates.
static LAST_POS: Mutex<(f64, f64)> = Mutex::new((0.0, 0.0));

/// Start global input listener.
/// Sends InputEvent through the channel and wakes the event loop via proxy.
/// Blocks the calling thread until listener ends.
pub fn start_input_listener(tx: Sender<InputEvent>, proxy: EventLoopProxy<()>) {
    // rdev::listen blocks the current thread, so caller should spawn this
    rdev::listen(move |event: Event| {
        let input_event = match event.event_type {
            EventType::KeyPress(key) if is_modifier(key) => {
                Some(InputEvent::ModifierChanged { pressed: true })
            }
            EventType::KeyRelease(key) if is_modifier(key) => {
                Some(InputEvent::ModifierChanged { pressed: false })
            }
            EventType::MouseMove { x, y } => {
                if let Ok(mut pos) = LAST_POS.lock() {
                    *pos = (x, y);
                }
                Some(InputEvent::MouseMove { x: x as i32, y: y as i32 })
            }
            EventType::ButtonPress(rdev::Button::Left) => {
                let pos = LAST_POS.lock().map(|p| *p).unwrap_or((0.0, 0.0));
                Some(InputEvent::MouseButtonDown { x: pos.0 as i32, y: pos.1 as i32 })
            }
            EventType::ButtonRelease(rdev::Button::Left) => {
                let pos = LAST_POS.lock().map(|p| *p).unwrap_or((0.0, 0.0));
                Some(InputEvent::MouseButtonUp { x: pos.0 as i32, y: pos.1 as i32 })
            }
            _ => None,
        };
        if let Some(event) = input_event {
            let _ = tx.send(event);
            // Wake the winit event loop so it drains the channel in about_to_wait
            let _ = proxy.send_event(());
        }
    })
    .expect("Failed to start input listener");
}

fn is_modifier(key: Key) -> bool {
    matches!(key, Key::ControlLeft | Key::ControlRight)
}
