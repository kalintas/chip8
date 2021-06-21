
use std::{fs, path::{Path, PathBuf}};

use rand::{Rng, thread_rng};

use sdl2::keyboard::Scancode;

use imgui::{ColorEdit, Condition, Direction, EditableColor, Slider, im_str};

pub mod renderer;
mod framebuffer;
mod utils;
mod beeper;

use self::{framebuffer::FrameBuffer, renderer::Renderer, utils::Color, beeper::Beeper};

const WIDTH:  usize = 64;
const HEIGHT: usize = 32;

const MEMORY_SIZE: usize = 0x1000; // 4 KB

const KEY_MAP: [Scancode; 16] =
[
    Scancode::X   , Scancode::Num1, Scancode::Num2, Scancode::Num3, 
    Scancode::Q   , Scancode::W   , Scancode::E   , Scancode::A   , 
    Scancode::S   , Scancode::D   , Scancode::Z   , Scancode::C   , 
    Scancode::Num4, Scancode::R   , Scancode::F   , Scancode::V   ,
];

// chip8 hex font data
const FONT_DATA: [u8; 5 * 16] =
[
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];



pub struct Chip8<'a>
{
    registers: [u8; 16],
    stack: [u16; 16],
    
    r_address: u16,    // address register
    r_delay_timer: u8, // delay timer register
    r_sound_timer: u8, // sound timer register

    sp: usize,
    pc: u16,

    memory: [u8; MEMORY_SIZE],
    
    running: bool,
    
    rom_loaded: bool,
    current_rom_path: PathBuf,

    schip_mode_on: bool,

    imgui_error_message: String,

    cycles_per_frame: u32,
    
    imgui_lock_to_pc: bool,

    waiting_key_input: bool,

    color_on:  Color,
    color_off: Color,

    screen_buffer: Vec<Color>,

    beeper: Beeper,

    framebuffer: FrameBuffer,
    renderer: Option<&'a mut Renderer>,
}


impl<'a> Chip8<'a>
{
    pub fn new(renderer: &'a mut Renderer) -> Self
    {        
        let mut memory = [0; MEMORY_SIZE];
         
        memory[..FONT_DATA.len()].copy_from_slice(&FONT_DATA);

        let color_off = Color::new(0x0C, 0x42, 0x71);
        let color_on  = Color::new(0xDF, 0xF9, 0xDC);

        Self
        {
            registers: [0; 16],
            stack:     [0; 16],   
            
            r_address: 0,
            r_delay_timer: 0,
            r_sound_timer: 0,

            sp: 0,
            pc: 0x200,

            memory,
            
            running: true,

            rom_loaded: false,
            current_rom_path: PathBuf::new(),

            schip_mode_on: true,

            imgui_error_message: Default::default(),
            
            imgui_lock_to_pc: true,
            waiting_key_input: false,

            screen_buffer: vec![color_off; WIDTH * HEIGHT],

            cycles_per_frame: 10,
            color_on,
            color_off,

            beeper: Beeper::new(&renderer.sdl).unwrap(),

            renderer: Some(renderer),
            framebuffer: FrameBuffer::new(WIDTH as i32, HEIGHT as i32),
        }
    }
    
    pub fn run(mut self)
    {       
        while self.running
        {
            self.poll_events();

            // update 
            for _ in 0..self.cycles_per_frame
            { 
                if self.is_pc_valid()
                {
                    self.run_next_opcode(); 
                }
                else
                {
                    self.screen_buffer.fill_with(|| Color::rand());
                    break;
                }
            }

            self.draw();
        }
    }

    fn wait_key_input(&mut self) -> usize
    {
        let mut result = None;
        self.waiting_key_input = true;

        while self.running && self.waiting_key_input && !result.is_some()
        {
            self.poll_events();

            for (index, key) in KEY_MAP.iter().enumerate() 
            { 
                if self.is_key_pressed(*key) { result = Some(index); break; } 
            } 

            self.draw();
        }
        self.waiting_key_input = false;

        result.unwrap_or(0)
    }
    
    fn poll_events(&mut self)
    {
        if self.renderer.as_mut().unwrap().poll_events() { self.running = false; return; }
        
        // count down timers every frame
        if self.r_delay_timer > 0 { self.r_delay_timer -= 1; }
        if self.r_sound_timer > 0 
        { 
            self.r_sound_timer -= 1; 
            
            if self.r_sound_timer == 0
            {
                self.beeper.device.pause();
            }
        }
    }

    fn draw(&mut self)
    {
        let renderer = self.renderer.take().unwrap(); // dirty hack v1

        renderer.clear_screen();

        let width  = renderer.window_width;
        let height = renderer.window_height;

        self.framebuffer.update_buffer(self.screen_buffer.as_ptr() as *const u8, gl::RGBA8, gl::RGBA);
        self.framebuffer.draw_buffer(0, height as i32, (width / 2) as i32, (height / 2) as i32);

        let mut rom_path = None;
        let mut restart_rom = false;

        let mut run_next_opcode = false;

        renderer.render(|ui| 
        {
            let imgui_window = |name, pos, width|
            {
                imgui::Window::new(name).resizable(false).collapsible(false).movable(false)
                    .position(pos, imgui::Condition::Always)
                    .size([width as f32, (height / 2) as f32], Condition::Always)
            };
    
            imgui_window(im_str!("settings"), [(width / 2) as f32, 0.0], width / 2)
            .build(ui, || 
                {
                    if self.rom_loaded
                    {
                        ui.text(format!("currently running -> {}",  self.current_rom_path.file_name().unwrap().to_str().unwrap()));
                    }
                    else
                    {
                        ui.text("waiting chip8 rom");
                    }


                    if ui.small_button(im_str!("open rom")) 
                    {                        
                        rom_path = tinyfiledialogs::open_file_dialog("Open", "./", None);
                    }
                    
                    ui.same_line(0.0);

                    restart_rom = ui.small_button(im_str!("restart rom"));

                    ui.same_line(0.0);

                    if ui.small_button(im_str!("unload rom"))
                    {
                        self.reset_state();
                    }
                    
                    ui.checkbox(im_str!("schip mode on"), &mut self.schip_mode_on);

                    Slider::new(im_str!("cycles per frame")).range(0..=1000).build(ui, &mut self.cycles_per_frame);

                    ui.radio_button(im_str!("pause"), &mut self.cycles_per_frame, 0);
                    
                    if self.cycles_per_frame == 0
                    {
                        ui.same_line(0.0);
                        run_next_opcode = ui.arrow_button(im_str!("1"), Direction::Right) && self.is_pc_valid(); 
                        ui.same_line(0.0); ui.text("step over"); 
                    }

                    let handle_color = |name, screen_buffer: &mut [Color], color: &mut Color|
                    {
                        let mut new_color = color.as_array();
                        ColorEdit::new(name, EditableColor::Float3(&mut new_color)).build(ui);
                        let new_color = Color::from_array(new_color);
    
                        if *color != new_color
                        {
                            screen_buffer.iter_mut().for_each(|c| if c == color { *c = new_color; });   
    
                            *color = new_color;
                        }
                    };
    
                    handle_color(im_str!("background color"), &mut self.screen_buffer, &mut self.color_off);
                    handle_color(im_str!("foreground color"), &mut self.screen_buffer, &mut self.color_on );
                    
                    ui.text("audio settings:");

                    {
                        let mut callback = self.beeper.device.lock();

                        Slider::new(im_str!("volume"   )).range(0.0..=1.0   ).build(ui, &mut callback.volume);                        
                        Slider::new(im_str!("frequency")).range(0.0..=2000.0).build(ui, &mut callback.freq  );
                    }

                });
    
        imgui_window(im_str!("registers"), [0.0, (height / 2) as f32], width / 3)
            .build(ui, || 
            {
                ui.set_window_font_scale(1.3);

                for (index, reg) in self.registers.iter().enumerate()
                {
                    ui.text(format!("V{:x}: {:#04x}", index, reg));
                    if index % 2 == 0 { ui.same_line(0.0); }
                }
    
                ui.text(format!("I : {:#x}", self.r_address));
                ui.text(format!("DT: {:#x}", self.r_delay_timer));
                ui.text(format!("ST: {:#x}", self.r_sound_timer));
                ui.text(format!("PC: {:#x}", self.pc));
                ui.text(format!("SP: {:#x}", self.sp));

                ui.set_window_font_scale(1.0);
            });
                    
            imgui_window(im_str!("memory"), [(width / 3) as f32, (height / 2) as f32], width / 3)
            .menu_bar(true)
            .build(ui, ||
            {
                ui.menu_bar(||
                {
                    ui.checkbox(im_str!("lock to program counter"), &mut self.imgui_lock_to_pc);            
                });

                let mut iter = self.memory.iter().enumerate();
    
                if self.pc % 2 == 1 { iter.next(); }

                while let Some((index, first)) = iter.next()
                {
                    let second = if let Some(second) = iter.next() { second } else { break; };
    
                    let string = format!("{:#x}: {:02X} {:02X}", index, first, second.1);
    
                    if index as u16 == self.pc
                    {   
                        ui.text_colored([1.0, 0.0, 0.0, 1.0], string);
                        if self.imgui_lock_to_pc { ui.set_scroll_here_y(); }
                    }
                    else { ui.text(string); }
                }
                
            });

            imgui_window(im_str!("keyboard"), [(width * 2 / 3) as f32, (height / 2) as f32], width / 3)
            .build(ui, ||
            {
                if self.waiting_key_input 
                {  
                    ui.text("waiting key input");
                }

                for (index, value) in KEY_MAP.iter().enumerate()
                {                                     
                    ui.label_text(&im_str!("{:?}", value), &im_str!("{:X} -> ", index));
                }
            }); 
            
            if !self.imgui_error_message.is_empty()
            {   
                ui.open_popup(im_str!("0"));
            }
            
            ui.popup_modal(im_str!("0")).scroll_bar(false).resizable(false).title_bar(false).build(|| 
            {
                ui.text(&self.imgui_error_message);

                if ui.button(im_str!("OK"), [0.0, 0.0]) 
                {
                    self.imgui_error_message = String::new();

                    ui.close_current_popup();
                }
            });
        });
        
        self.renderer = Some(renderer); // dirty hack v2

        if let Some(path) = rom_path
        {
            self.open_rom(path);
        }
        else if restart_rom
        {   
            let mut path = PathBuf::new();
            std::mem::swap(&mut path, &mut self.current_rom_path);

            self.open_rom(path);
        }
        
        if run_next_opcode
        {
            self.run_next_opcode();
        }
    }

    fn is_pc_valid(&self) -> bool
    {
        self.rom_loaded && self.pc < MEMORY_SIZE as u16
    }

    fn reset_state(&mut self)
    {
        self.registers = [0; 16];
        self.stack     = [0; 16];   
        self.r_address = 0;
        self.r_delay_timer = 0;
        self.r_sound_timer = 0;
        self.sp = 0;
        self.pc = 0x200;
        self.rom_loaded = false;

        self.current_rom_path = PathBuf::new();
        
        self.waiting_key_input = false;
    }

    fn open_rom(&mut self, path: impl AsRef<Path>)
    {
        let rom = match fs::read(path.as_ref())
        {
            Ok(rom) => rom,
            Err(err) =>
            {
                self.imgui_error_message = format!("{}\npath: {:?}", err, path.as_ref());
                return; 
            }
        };

        let end = 0x200 + rom.len();

        if end > MEMORY_SIZE 
        { 
            self.imgui_error_message = format!("invalid rom\nrom size cannot exceed {} bytes", MEMORY_SIZE);
            return; 
        }

        self.memory[0x200..end].copy_from_slice(&rom);
        self.memory[end..].fill(0);
        
        self.reset_state();

        self.rom_loaded = true;

        self.screen_buffer.fill(self.color_off);

        self.current_rom_path = path.as_ref().to_owned();
    }

    fn unkown_instruction(&mut self)
    {
        let upper = self.memory[self.pc as usize - 2];
        let lower = self.memory[self.pc as usize - 1];

        self.imgui_error_message = format!("unkown instruction at {:#x}\nopcode = {:02X} {:02X}", self.pc - 2, upper, lower);

        self.reset_state();
    }

    fn push_pc(&mut self)
    {
        self.stack[self.sp] = self.pc;

        self.sp += 1;
    }

    fn pop_pc(&mut self)
    {
        self.sp -= 1;

        self.pc = self.stack[self.sp];
    }
    
    // xor pixel to given locations and return if any pixel is setted off
    fn set_pixel(&mut self, x: usize, y: usize, value: u8) -> bool
    {
        if value == 0 || x >= WIDTH || y >= HEIGHT { false }
        else
        {
            let pixel = &mut self.screen_buffer[y * WIDTH + x];

            if *pixel == self.color_off { *pixel = self.color_on;  false }
            else                        { *pixel = self.color_off; true  }
        }
    }

    // draws sprite to screen_buffer and updates Vf flag
    fn draw_sprite(&mut self, x: usize, y: usize, height: usize)
    {
        if self.r_address as usize + height > MEMORY_SIZE
        {
            self.imgui_error_message = format!("cannot draw sprite\ninvalid address register = {:#x}", self.r_address);
            self.reset_state();
            return;
        }

        let mut collision = false;

        for i in 0..height
        {
            let row = self.memory[self.r_address as usize + i];

            for t in 0..8
            {
                let color = (row >> (7 - t)) & 0x1;

                collision |= self.set_pixel(x + t, y + i, color);
            }
        }

        self.registers[0xF] = collision as u8;
    }

    fn is_key_pressed(&self, key: Scancode) -> bool
    {
        self.renderer.as_ref().unwrap().event_pump.keyboard_state().is_scancode_pressed(key)
    }


    fn run_next_opcode(&mut self)
    {
        let upper = self.memory[self.pc as usize    ];
        let lower = self.memory[self.pc as usize + 1];

        self.pc += 2;

        let nibbles = 
        [
            ((upper >> 4) & 0xF) as usize, 
            ( upper       & 0xF) as usize, 
            ((lower >> 4) & 0xF) as usize, 
            ( lower       & 0xF) as usize
        ];

        let vx = self.registers[nibbles[1]];
        let vy = self.registers[nibbles[2]];

        let addr = (((upper & 0xF) as u16) << 8) | lower as u16;

        match nibbles[0]
        {
            0x0 =>
            {
                match lower
                {
                    0xE0 => self.screen_buffer.fill(self.color_off), // 00E0 -> CLS
                    0xEE => self.pop_pc(),                           // 00EE -> RET
                    _    => self.unkown_instruction()
                }
            }
            0x1 => self.pc = addr,                            // 1NNN -> JP addr
            0xB => self.pc = addr + self.registers[0] as u16, // BNNN -> JP V0, addr
            
            0x2 => { self.push_pc(); self.pc = addr; } // 2NNN -> CALL addr
            
            0x3 => if vx == lower { self.pc += 2 } // 3XNN -> SE  Vx, byte
            0x4 => if vx != lower { self.pc += 2 } // 4XNN -> SNE Vx, byte
            0x5 => if vx == vy    { self.pc += 2 } // 5XY0 -> SE  Vx, Vy
            0x9 => if vx != vy    { self.pc += 2 } // 9XY0 -> SNE Vx, Vy      
            
            0x6 => self.registers[nibbles[1]] = lower, // 6XNN -> LD Vx, byte
            0x7 => self.registers[nibbles[1]] = ((vx as u16 + lower as u16) % 256) as u8, // 7XNN -> ADD Vx, byte
            
            0x8 =>
            {
                match nibbles[3]
                {
                    0x0 => self.registers[nibbles[1]]  = vy, // 8XY0 -> LD Vx, Vy
                    0x1 => self.registers[nibbles[1]] |= vy, // 8XY1 -> OR Vx, Vy
                    0x2 => self.registers[nibbles[1]] &= vy, // 8XY2 -> AND Vx, Vy
                    0x3 => self.registers[nibbles[1]] ^= vy, // 8XY3 -> XOR Vx, Vy
                    0x4 => // 8XY4 -> ADD Vx, Vy
                    {
                        let result = vx as u16 + vy as u16;
                        self.registers[0xF] = (result > 0xFF) as u8;
                        self.registers[nibbles[1]] = (result % 256) as u8;
                    }
                    0x5 => { self.registers[nibbles[1]] = vx.wrapping_sub(vy); self.registers[0xF] = (vx > vy) as u8 }, // 8XY5 -> SUB  Vx, Vy
                    0x7 => { self.registers[nibbles[1]] = vy.wrapping_sub(vx); self.registers[0xF] = (vy > vx) as u8 }, // 8XY7 -> SUBN Vx, Vy   
                    0x6 => // 8XY6 -> SHR Vx {, Vy}
                    {
                        if self.schip_mode_on
                        {
                            self.registers[0xF] = vx & 0x1;
                            self.registers[nibbles[1]] >>= 1;    
                        }
                        else
                        {
                            self.registers[0xF] = vy & 0x1;
                            self.registers[nibbles[1]] = vy >> 1;
                        }
                    }                    
                    0xE => // 8XYE -> SHL Vx {, Vy}
                    {
                        if self.schip_mode_on
                        {
                            self.registers[0xF] = (vx >> 7) & 0x1;
                            self.registers[nibbles[1]] <<= 1;     
                        }
                        else
                        {
                            self.registers[0xF] = (vy >> 7) & 0x1;
                            self.registers[nibbles[1]] = vy << 1; 
                        }
                    }
                    _ => self.unkown_instruction()
                }
            }
            0xA => self.r_address = addr, // ANNN -> LD I, addr
            0xC => self.registers[nibbles[1]] = thread_rng().gen::<u8>() & lower, // CXNN -> RND Vx, byte
            0xD => self.draw_sprite(vx as usize, vy as usize, nibbles[3]), // DXYN -> DRW Vx, Vy, nibble
            0xE => 
            {
                match lower
                {
                    0x9E => if  self.is_key_pressed(KEY_MAP[vx as usize]) { self.pc += 2 } // EX9E -> SKP Vx
                    0xA1 => if !self.is_key_pressed(KEY_MAP[vx as usize]) { self.pc += 2 } // EXA1 -> SKNP Vx
                    _ => self.unkown_instruction()
                }
            }
            0xF =>
            {
                match lower
                {
                    0x07 => self.registers[nibbles[1]] = self.r_delay_timer,          // FX07 -> LD Vx, DT
                    0x15 => self.r_delay_timer = vx,                                  // FX15 -> LD DT, Vx
                    0x18 => { self.r_sound_timer = vx; self.beeper.device.resume() }, // FX18 -> LD ST, Vx
                    0x0A => self.registers[nibbles[1]] = self.wait_key_input() as u8, // FX0A -> LD Vx, K
                    0x1E => self.r_address += vx as u16,                              // FX1E -> ADD I, Vx
                    0x29 => self.r_address = vx as u16 * 5,       // FX29 -> LD F, Vx
                    0x33 => // FX33 -> LD B, Vx
                    {
                        self.memory[self.r_address as usize    ] = (vx / 100) % 10;
                        self.memory[self.r_address as usize + 1] = (vx / 10)  % 10;
                        self.memory[self.r_address as usize + 2] =  vx        % 10;
                    }
                    0x55 => // FX55 -> LD [I], Vx
                    { 
                        let len = nibbles[1] + 1;
                        self.memory[self.r_address as usize..self.r_address as usize + len].copy_from_slice(&self.registers[..len]);
                        if !self.schip_mode_on { self.r_address += len as u16; }
                    }
                    0x65 => // FX65 -> LD Vx, [I]
                    {
                        let len = nibbles[1] + 1;
                        self.registers[..len].copy_from_slice(&self.memory[self.r_address as usize..self.r_address as usize + len]);
                        if !self.schip_mode_on { self.r_address += len as u16; }
                    }
                    _ => self.unkown_instruction()
                }
            }
            _ => self.unkown_instruction()
        }        
    }
}
