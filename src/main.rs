
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chip8;
use chip8::{Chip8, renderer::Renderer};
fn main()
{

    Chip8::new(&mut Renderer::new("Chip8/SuperChip Interpreter", 1024, 720).unwrap()).run();
}