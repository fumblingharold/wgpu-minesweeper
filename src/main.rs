use minesweeper;

/// Calls the run function from lib.rs that sets up the window and the event loop.
fn main() {
    pollster::block_on(minesweeper::run());
}
