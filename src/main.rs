#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chip8;
use chip8::{renderer::Renderer, Chip8};
fn main() {
    Chip8::new(&mut Renderer::new("Chip8/SuperChip Interpreter", 1024, 720).unwrap()).run();
}
