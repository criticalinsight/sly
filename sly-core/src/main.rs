mod state;
mod control;
mod cortex;
mod memory;
mod io;
mod parser;
mod safety;
mod error;

use crate::state::GlobalState;

fn main() {
    // Zero-Lib initialization
    match GlobalState::new_transient() {
        Ok(mut state) => {
            control::cortex_loop(&mut state);
        }
        Err(e) => {
            eprintln!("Fatal System Error: {}", e);
            std::process::exit(1);
        }
    }
}
