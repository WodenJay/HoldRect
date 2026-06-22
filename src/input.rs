use std::sync::mpsc::Sender;
use std::sync::Mutex;

use rdev::{Event, EventType, Key};
use winit::event_loop::EventLoopProxy;

use crate::state::InputEvent;

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
#[cfg(windows)]
use windows::Win32::Foundation::POINT;

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

/// Poll mouse button state via GetAsyncKeyState.
/// Supplements rdev which misses button events when modifier keys are held.
#[cfg(windows)]
pub fn start_button_poller(tx: Sender<InputEvent>, proxy: EventLoopProxy<()>) {
    std::thread::spawn(move || {
        let mut was_pressed = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
            let pressed = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } & 0x8000u16 as i16 != 0;
            if pressed != was_pressed {
                was_pressed = pressed;
                let mut pt = POINT { x: 0, y: 0 };
                unsafe { let _ = GetCursorPos(&mut pt); }
                let event = if pressed {
                    if let Ok(mut pos) = LAST_POS.lock() {
                        *pos = (pt.x as f64, pt.y as f64);
                    }
                    InputEvent::MouseButtonDown { x: pt.x, y: pt.y }
                } else {
                    InputEvent::MouseButtonUp { x: pt.x, y: pt.y }
                };
                let _ = tx.send(event);
                let _ = proxy.send_event(());
            }
        }
    });
}
