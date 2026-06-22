mod input;
mod overlay;
mod tray;

use holdrect::state;

fn main() {
    println!("HoldRect v0.1 starting...");
    let _ = state::AppState::default();
}
