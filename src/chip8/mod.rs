use std::{
    fs, mem,
    path::{Path, PathBuf},
    time::Instant,
    usize,
};

use rand::{thread_rng, Rng};

use sdl2::{keyboard::Scancode, video::SwapInterval};

use imgui::{im_str, ColorEdit, Direction, EditableColor, ImString, Slider};

mod beeper;
mod framebuffer;
pub mod renderer;
mod utils;

use self::{beeper::Beeper, framebuffer::FrameBuffer, renderer::Renderer, utils::Color};

const WIDTH: usize = 64; // Chip8 width
const HEIGHT: usize = 32; // Chip8 height

const S_WIDTH: usize = 128; // SuperChip width
const S_HEIGHT: usize = 64; // SuperChip height

const MEMORY_SIZE: usize = 0x1000; // 4 KB

const KEY_MAP: [Scancode; 16] = [
    Scancode::X,
    Scancode::Num1,
    Scancode::Num2,
    Scancode::Num3,
    Scancode::Q,
    Scancode::W,
    Scancode::E,
    Scancode::A,
    Scancode::S,
    Scancode::D,
    Scancode::Z,
    Scancode::C,
    Scancode::Num4,
    Scancode::R,
    Scancode::F,
    Scancode::V,
];

const SMALL_FONT_SIZE: usize = 5 * 16;

const FONT_DATA: [u8; SMALL_FONT_SIZE + 10 * 10] = [
    // Chip8 hex font data
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
    // SuperChip font data (no hex chars)
    0x3C, 0x7E, 0xE7, 0xC3, 0xC3, 0xC3, 0xC3, 0xE7, 0x7E, 0x3C, // 0
    0x18, 0x38, 0x58, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x3C, // 1
    0x3E, 0x7F, 0xC3, 0x06, 0x0C, 0x18, 0x30, 0x60, 0xFF, 0xFF, // 2
    0x3C, 0x7E, 0xC3, 0x03, 0x0E, 0x0E, 0x03, 0xC3, 0x7E, 0x3C, // 3
    0x06, 0x0E, 0x1E, 0x36, 0x66, 0xC6, 0xFF, 0xFF, 0x06, 0x06, // 4
    0xFF, 0xFF, 0xC0, 0xC0, 0xFC, 0xFE, 0x03, 0xC3, 0x7E, 0x3C, // 5
    0x3E, 0x7C, 0xE0, 0xC0, 0xFC, 0xFE, 0xC3, 0xC3, 0x7E, 0x3C, // 6
    0xFF, 0xFF, 0x03, 0x06, 0x0C, 0x18, 0x30, 0x60, 0x60, 0x60, // 7
    0x3C, 0x7E, 0xC3, 0xC3, 0x7E, 0x7E, 0xC3, 0xC3, 0x7E, 0x3C, // 8
    0x3C, 0x7E, 0xC3, 0xC3, 0x7F, 0x3F, 0x03, 0x03, 0x3E, 0x7C, // 9
];

struct Config {
    shift_behaviour: bool, // in 8xy6-8xyE use Vy if this is true otherwise use Vx
    draw_behaviour: bool,  // wrap screen when drawing sprites if this is true
    store_behaviour: bool, // in FX55-FX65 increment I after copying if this is true
}

pub struct Chip8<'a> {
    v: [u8; 16],             // 16 8-bit registers
    flag_registers: [u8; 8], // special registers, used by FX75* and FX85*

    stack: [u16; 16], // stack for storing pc in subroutines

    memory: Box<[u8; MEMORY_SIZE]>, // 4 KB memory, 0x0..0x1FF -> chip8 interpreter, 0x200..0xFFF -> rom data

    sp: usize, // stack pointer
    pc: u16,   // program counter

    r_address: u16,    // address register aka I
    r_delay_timer: u8, // delay timer register
    r_sound_timer: u8, // sound timer register

    delay_tick: f64,
    sound_tick: f64,

    dt_interval: f64,
    st_interval: f64,
    cycles_per_frame: u32,

    elapsed_time: Instant, // time elapsed between frames

    // state bools
    running: bool,
    waiting_key_input: bool,
    rom_loaded: bool,
    vsync_open: bool,

    current_rom_path: PathBuf, // path to currently working rom

    config: Config,

    // imgui
    imgui_error_message: String, // error string to create imgui popup windows
    imgui_lock_to_pc: bool,

    color_on: Color,  // foreground color
    color_off: Color, // background color

    width: usize,  // current buffer width  -> 128 on high res otherwise 64
    height: usize, // current buffer height -> 64  on high res otherwise 32

    // buffer to store pixels
    screen_buffer: Box<[Color; S_WIDTH * S_HEIGHT]>,

    beeper: Beeper, // simple struct for generating square waves

    framebuffer: FrameBuffer,
    renderer: Option<&'a mut Renderer>,
}

impl<'a> Chip8<'a> {
    pub fn new(renderer: &'a mut Renderer) -> Self {
        let mut memory = Box::new([0; MEMORY_SIZE]);

        memory[..FONT_DATA.len()].copy_from_slice(&FONT_DATA);

        // default colors
        let color_off = Color::new(0x0C, 0x42, 0x71);
        let color_on = Color::new(0xDF, 0xF9, 0xDC);

        let delay_tick_duration = 1.0 / 60.0;
        let sound_tick_duration = 1.0 / 60.0;

        Self {
            v: [0; 16],
            flag_registers: [0; 8],

            stack: [0; 16],

            memory,

            sp: 0,
            pc: 0x200,

            r_address: 0,
            r_delay_timer: 0,
            r_sound_timer: 0,

            delay_tick: delay_tick_duration,
            sound_tick: sound_tick_duration,

            dt_interval: delay_tick_duration,
            st_interval: sound_tick_duration,
            cycles_per_frame: 60,

            elapsed_time: Instant::now(),

            running: true,
            waiting_key_input: false,
            rom_loaded: false,
            vsync_open: true,

            current_rom_path: PathBuf::new(),

            config: Config {
                shift_behaviour: true,
                draw_behaviour: true,
                store_behaviour: true,
            },

            imgui_error_message: String::new(),
            imgui_lock_to_pc: true,

            color_on,
            color_off,

            // default to low res
            width: WIDTH,
            height: HEIGHT,

            screen_buffer: Box::new([color_off; S_WIDTH * S_HEIGHT]),

            beeper: Beeper::new(&renderer.sdl).unwrap(),

            renderer: Some(renderer),
            framebuffer: FrameBuffer::new(),
        }
    }

    pub fn run(mut self) {
        while self.running {
            self.poll_events();

            for _ in 0..self.cycles_per_frame {
                if self.is_pc_valid() {
                    self.run_next_opcode();
                } else {
                    // draw random colors for fun
                    for i in 0..self.width * self.height {
                        self.screen_buffer[(i / self.width) * S_WIDTH + (i % self.width)] =
                            Color::rand();
                    }
                    break;
                }
            }

            self.draw();
        }
    }

    fn wait_key_input(&mut self) -> usize {
        let mut result = None;
        self.waiting_key_input = true;

        while self.running && self.waiting_key_input && !result.is_some() {
            self.poll_events();

            for (index, key) in KEY_MAP.iter().enumerate() {
                if self.is_key_pressed(*key) {
                    result = Some(index);
                    break;
                }
            }

            self.draw();
        }
        self.waiting_key_input = false;

        result.unwrap_or(0)
    }

    fn poll_events(&mut self) {
        if self.renderer.as_mut().unwrap().poll_events() {
            self.running = false;
            return;
        }

        let elapsed = self.elapsed_time.elapsed().as_secs_f64();
        self.elapsed_time = Instant::now();

        self.delay_tick -= elapsed;
        self.sound_tick -= elapsed;

        if self.delay_tick <= 0.0 {
            if self.r_delay_timer > 0 {
                self.r_delay_timer -= 1;
            }

            self.delay_tick = self.dt_interval;
        }

        if self.sound_tick <= 0.0 {
            if self.r_sound_timer > 0 {
                self.r_sound_timer -= 1;

                if self.r_sound_timer == 0 {
                    self.beeper.device.pause();
                }
            }

            self.sound_tick = self.st_interval;
        }
    }

    fn draw(&mut self) {
        let renderer = self.renderer.take().unwrap(); // dirty hack v1

        renderer.clear_screen();

        let width = renderer.window_width;
        let height = renderer.window_height;

        let src = (0, 0, self.width as _, self.height as _);
        let dest = (0, height as i32, (width / 2) as i32, (height / 2) as i32);

        self.framebuffer.update_buffer(
            S_WIDTH as _,
            S_HEIGHT as _,
            self.screen_buffer.as_ptr() as _,
            gl::RGBA8,
            gl::RGBA,
        );
        self.framebuffer.draw_buffer(src, dest);

        let mut run_next_opcode = false;
        let mut vsync_open = self.vsync_open;

        // ugly imgui rendering
        renderer.render(|ui| {
            let imgui_window = |name, pos, width| {
                imgui::Window::new(name)
                    .resizable(false)
                    .collapsible(false)
                    .movable(false)
                    .position(pos, imgui::Condition::Always)
                    .size(
                        [width as f32, (height / 2) as f32],
                        imgui::Condition::Always,
                    )
            };

            imgui_window(im_str!("Settings"), [(width / 2) as f32, 0.0], width / 2).build(
                ui,
                || {
                    if self.rom_loaded {
                        let file_name =
                            self.current_rom_path.file_name().unwrap().to_str().unwrap();
                        ui.text(format!("Currently Running -> {}", file_name));
                    } else {
                        ui.text("Waiting Chip8/SuperChip Rom");
                    }

                    if ui.small_button(im_str!("open rom")) {
                        if let Some(rom_path) =
                            tinyfiledialogs::open_file_dialog("Open", "./", None)
                        {
                            self.open_rom(rom_path);
                        }
                    }

                    ui.same_line(0.0);

                    if ui.small_button(im_str!("restart rom")) {
                        // another hack to prevent making a useless allocation
                        let path = mem::take(&mut self.current_rom_path);
                        self.open_rom(path);
                    }

                    ui.same_line(0.0);

                    if ui.small_button(im_str!("reset")) {
                        self.reset_state();
                    }

                    ui.same_line(0.0);
                    ui.checkbox(im_str!("vsync"), &mut vsync_open);

                    if ui.button(im_str!("*##0"), [0.0, 0.0]) {
                        self.cycles_per_frame = 60
                    }
                    ui.same_line(0.0);
                    Slider::new(im_str!("Cycles per Frame"))
                        .range(0..=1000)
                        .build(ui, &mut self.cycles_per_frame);

                    if ui.button(im_str!("*##1"), [0.0, 0.0]) {
                        self.dt_interval = 1.0 / 60.0
                    }
                    ui.same_line(0.0);
                    Slider::new(im_str!("Delay Tick Interval"))
                        .range(0.0..=1.0)
                        .build(ui, &mut self.dt_interval);

                    if ui.button(im_str!("*##2"), [0.0, 0.0]) {
                        self.st_interval = 1.0 / 60.0
                    }
                    ui.same_line(0.0);
                    Slider::new(im_str!("Sound Tick Interval"))
                        .range(0.0..=1.0)
                        .build(ui, &mut self.st_interval);

                    // pause and step over
                    ui.separator();
                    ui.radio_button(im_str!("Pause"), &mut self.cycles_per_frame, 0);

                    if self.cycles_per_frame == 0 {
                        ui.same_line(0.0);
                        run_next_opcode =
                            ui.arrow_button(im_str!("1"), Direction::Right) && self.is_pc_valid();
                        ui.same_line(0.0);
                        ui.text("Step Over");
                    }

                    // config checkboxes
                    ui.separator();
                    ui.checkbox(
                        im_str!("Shift Vy in 8XYE and 8XY6"),
                        &mut self.config.shift_behaviour,
                    );
                    ui.checkbox(
                        im_str!("Wrap around screen when drawing sprites"),
                        &mut self.config.draw_behaviour,
                    );
                    ui.checkbox(
                        im_str!("Increment I after FX55 and FX65"),
                        &mut self.config.store_behaviour,
                    );

                    // color sliders
                    let handle_color = |name, screen_buffer: &mut [Color], color: &mut Color| {
                        let mut new_color = color.as_array();
                        ColorEdit::new(name, EditableColor::Float3(&mut new_color)).build(ui);
                        let new_color = Color::from_array(new_color);

                        if *color != new_color {
                            screen_buffer.iter_mut().for_each(|c| {
                                if c == color {
                                    *c = new_color;
                                }
                            });

                            *color = new_color;
                        }
                    };

                    ui.separator();
                    handle_color(
                        im_str!("Background Color"),
                        &mut *self.screen_buffer,
                        &mut self.color_off,
                    );
                    handle_color(
                        im_str!("Foreground Color"),
                        &mut *self.screen_buffer,
                        &mut self.color_on,
                    );

                    // audio
                    ui.separator();
                    ui.text("Audio Settings:");

                    {
                        let mut callback = self.beeper.device.lock();

                        if ui.button(im_str!("*##3"), [0.0, 0.0]) {
                            callback.volume = 0.2;
                        }
                        ui.same_line(0.0);
                        Slider::new(im_str!("Volume"))
                            .range(0.0..=1.0)
                            .build(ui, &mut callback.volume);

                        if ui.button(im_str!("*##4"), [0.0, 0.0]) {
                            callback.freq = 444.1;
                        }
                        ui.same_line(0.0);
                        Slider::new(im_str!("Frequency"))
                            .range(0.0..=2000.0)
                            .build(ui, &mut callback.freq);
                    }
                },
            );

            imgui_window(im_str!("Registers"), [0.0, (height / 2) as f32], width / 3).build(
                ui,
                || {
                    let print_registers = |registers: &[u8]| {
                        for (index, reg) in registers.iter().enumerate() {
                            ui.text(format!("V{:x}: {:#04x}", index, reg));
                            if index % 2 == 0 {
                                ui.same_line(0.0);
                            }
                        }
                    };

                    ui.set_window_font_scale(1.3);

                    print_registers(&self.v);

                    ui.text(format!("I : {:#x}", self.r_address));
                    ui.text(format!("DT: {:#x}", self.r_delay_timer));
                    ui.text(format!("ST: {:#x}", self.r_sound_timer));
                    ui.text(format!("PC: {:#x}", self.pc));
                    ui.text(format!("SP: {:#x}", self.sp));

                    ui.separator();
                    ui.text("Flag Registers:");

                    print_registers(&self.flag_registers);

                    ui.set_window_font_scale(1.0);
                },
            );

            imgui_window(
                im_str!("Memory"),
                [(width / 3) as f32, (height / 2) as f32],
                width / 3,
            )
            .menu_bar(true)
            .build(ui, || {
                ui.menu_bar(|| {
                    ui.checkbox(im_str!("Lock to PC"), &mut self.imgui_lock_to_pc);
                });

                let mut iter = self.memory.iter().enumerate();

                if self.pc % 2 == 1 {
                    iter.next();
                }

                while let Some((index, first)) = iter.next() {
                    let second = if let Some(second) = iter.next() {
                        second
                    } else {
                        break;
                    };

                    let string = format!("{:#x}: {:02X} {:02X}", index, first, second.1);

                    if index as u16 == self.pc {
                        ui.text_colored([1.0, 0.0, 0.0, 1.0], string);
                        if self.imgui_lock_to_pc {
                            ui.set_scroll_here_y();
                        }
                    } else {
                        ui.text(string);
                    }
                }
            });

            imgui_window(
                im_str!("Keyboard"),
                [(width * 2 / 3) as f32, (height / 2) as f32],
                width / 3,
            )
            .build(ui, || {
                if self.waiting_key_input {
                    ui.text("Waiting key input");
                }

                for (index, value) in KEY_MAP.iter().enumerate() {
                    ui.label_text(&im_str!("{:?}", value), &im_str!("{:X} -> ", index));
                }
            });

            let mut popup_id = Default::default();

            if !self.imgui_error_message.is_empty() {
                // BAD!!
                // give every different sized error message a unique id
                // otherwise imgui doesnt render it properly
                popup_id = unsafe {
                    ImString::from_utf8_unchecked(
                        self.imgui_error_message.len().to_ne_bytes().to_vec(),
                    )
                };
                ui.open_popup(&popup_id);
            }

            ui.popup_modal(&popup_id)
                .scroll_bar(false)
                .movable(false)
                .resizable(false)
                .title_bar(false)
                .build(|| {
                    ui.text(&self.imgui_error_message);

                    if ui.button(im_str!("OK"), [0.0, 0.0]) {
                        self.imgui_error_message = String::new();

                        self.reset_state();

                        ui.close_current_popup();
                    }
                });
        });

        if self.vsync_open != vsync_open {
            renderer
                .video_subsys
                .gl_set_swap_interval(if vsync_open {
                    SwapInterval::VSync
                } else {
                    SwapInterval::Immediate
                })
                .unwrap();

            self.vsync_open = vsync_open;
        }

        self.renderer = Some(renderer); // dirty hack v2

        // this function may use self.renderer so
        // call it after moving renderer to self.renderer
        if run_next_opcode {
            self.run_next_opcode();
        }
    }

    // helper functions
    fn is_pc_valid(&self) -> bool {
        self.rom_loaded && self.pc < MEMORY_SIZE as u16
    }

    fn reset_state(&mut self) {
        self.v = [0; 16];
        self.stack = [0; 16];
        self.sp = 0;
        self.pc = 0x200;
        self.r_address = 0;
        self.r_delay_timer = 0;
        self.r_sound_timer = 0;
        self.delay_tick = self.dt_interval;
        self.sound_tick = self.st_interval;

        self.rom_loaded = false;
        self.waiting_key_input = false;

        self.width = WIDTH;
        self.height = HEIGHT;

        self.current_rom_path = PathBuf::new();

        self.beeper.device.pause();
    }

    fn clear_screen(&mut self) {
        self.screen_buffer.fill(self.color_off);
    }

    fn is_key_pressed(&self, key: Scancode) -> bool {
        self.renderer
            .as_ref()
            .unwrap()
            .event_pump
            .keyboard_state()
            .is_scancode_pressed(key)
    }

    fn show_error(&mut self, message: String) {
        self.imgui_error_message = message;
        self.rom_loaded = false;
    }

    fn open_rom(&mut self, path: impl AsRef<Path>) {
        let rom = match fs::read(path.as_ref()) {
            Ok(rom) => rom,
            Err(err) => {
                self.show_error(format!("{}\npath: {:?}", err, path.as_ref()));
                return;
            }
        };

        let end = 0x200 + rom.len();

        if end > MEMORY_SIZE {
            self.show_error(format!(
                "invalid rom\nrom size({}) cannot exceed {} bytes",
                rom.len(),
                MEMORY_SIZE - 0x200
            ));
            return;
        }

        self.memory[0x200..end].copy_from_slice(&rom);
        self.memory[end..].fill(0);

        self.reset_state();

        self.rom_loaded = true;

        self.clear_screen();

        self.current_rom_path = path.as_ref().to_owned();
    }

    fn unkown_instruction(&mut self) {
        let upper = self.memory[self.pc as usize - 2];
        let lower = self.memory[self.pc as usize - 1];

        self.show_error(format!(
            "unkown instruction\nopcode = {:02X} {:02X}",
            upper, lower
        ));
    }

    fn push_pc(&mut self) {
        if self.sp >= self.stack.len() {
            self.show_error("cannot push the stack\nstack overflow".to_string());
            return;
        }

        self.stack[self.sp] = self.pc;

        self.sp += 1;
    }

    fn pop_pc(&mut self) {
        if self.sp == 0 {
            self.show_error(
                "cannot pop the stack\ntryed to pop the stack before pushing it".to_string(),
            );
            return;
        }

        self.sp -= 1;

        self.pc = self.stack[self.sp];
    }

    // xor pixel to given locations and return if any pixel is setted off
    fn set_pixel(&mut self, mut x: usize, mut y: usize, value: u8) -> bool {
        if x >= self.width {
            if self.config.draw_behaviour {
                x = x % self.width;
            }
            // wrap around
            else {
                return false;
            }
        }

        if y >= self.height {
            if self.config.draw_behaviour {
                y = y % self.height;
            }
            // wrap around
            else {
                return false;
            }
        }

        if value == 0 {
            false
        } else {
            let pixel = &mut self.screen_buffer[y * S_WIDTH + x];

            if *pixel == self.color_off {
                *pixel = self.color_on;
                false
            } else {
                *pixel = self.color_off;
                true
            }
        }
    }

    // draw sprite to screen_buffer and if there is a collision set vf to 1 otherwise set vf to 0
    fn draw_sprite(&mut self, x: usize, y: usize, width: usize, height: usize) {
        if self.r_address as usize + height > MEMORY_SIZE {
            self.show_error(format!(
                "cannot draw sprite\ninvalid address register = {:#x}",
                self.r_address
            ));
            return;
        }

        let mut collision = false;

        for j in 0..height * width {
            let i = j / width;
            let t = j % width;

            let byte = self.memory[self.r_address as usize + i * (width / 8) + (t / 8)];

            // sprites are stored in big endian format
            let color = (byte >> (7 - (t % 8))) & 0x1;

            collision |= self.set_pixel(x + t, y + i, color);
        }

        self.v[0xF] = collision as u8;
    }

    // run next instruction
    fn run_next_opcode(&mut self) {
        let upper = self.memory[self.pc as usize];
        let lower = self.memory[self.pc as usize + 1];

        self.pc += 2;

        let nibbles = [
            ((upper >> 4) & 0xF) as usize,
            (upper & 0xF) as usize,
            ((lower >> 4) & 0xF) as usize,
            (lower & 0xF) as usize,
        ];

        let x = nibbles[1];
        let y = nibbles[2];

        let addr = (((upper & 0xF) as u16) << 8) | lower as u16;

        // opcodes marked with * are new SuperChip instructions

        match nibbles[0] {
            0x0 => {
                match lower {
                    0xE0 => self.clear_screen(), // 00E0 -> CLS
                    0xEE => self.pop_pc(),       // 00EE -> RET
                    0xFE =>
                    // 00FE* -> LOW
                    {
                        // switch to low resolution mode (64x32)
                        self.clear_screen();

                        self.width = WIDTH;
                        self.height = HEIGHT;
                    }
                    0xFF =>
                    // 00FF* -> HIGH
                    {
                        // switch to high resolution mode (128x64)
                        self.clear_screen();

                        self.width = S_WIDTH;
                        self.height = S_HEIGHT;
                    }
                    0xFD => self.reset_state(), // 00FD* -> EXIT
                    0xFB =>
                    // 00FB* -> SCR
                    {
                        // scroll right 4 pixels
                        for i in 0..self.height {
                            for t in (0..self.width).rev() {
                                self.screen_buffer[i * S_WIDTH + t] = if t < 4 {
                                    self.color_off
                                } else {
                                    self.screen_buffer[i * S_WIDTH + t - 4]
                                };
                            }
                        }
                    }
                    0xFC =>
                    // 00FC* -> SCL
                    {
                        // scroll left 4 pixels
                        for i in 0..self.height {
                            for t in 0..self.width {
                                self.screen_buffer[i * S_WIDTH + t] = if t >= self.width - 4 {
                                    self.color_off
                                } else {
                                    self.screen_buffer[i * S_WIDTH + t + 4]
                                };
                            }
                        }
                    }
                    n if n == 0xC0 | nibbles[3] as u8 => {
                        // scroll down 0 to 15 pixels
                        for t in 0..self.width {
                            for i in (0..self.height).rev() {
                                self.screen_buffer[i * S_WIDTH + t] = if i < nibbles[3] {
                                    self.color_off
                                } else {
                                    self.screen_buffer[(i - nibbles[3]) * S_WIDTH + t]
                                };
                            }
                        }
                    }
                    _ => self.unkown_instruction(),
                }
            }
            0x1 => self.pc = addr,                    // 1NNN -> JP addr
            0xB => self.pc = addr + self.v[0] as u16, // BNNN -> JP V0, addr

            0x2 => {
                self.push_pc();
                self.pc = addr;
            } // 2NNN -> CALL addr

            0x3 => {
                if self.v[x] == lower {
                    self.pc += 2
                }
            } // 3XNN -> SE  Vx, byte
            0x4 => {
                if self.v[x] != lower {
                    self.pc += 2
                }
            } // 4XNN -> SNE Vx, byte
            0x5 => {
                if self.v[x] == self.v[y] {
                    self.pc += 2
                }
            } // 5XY0 -> SE  Vx, Vy
            0x9 => {
                if self.v[x] != self.v[y] {
                    self.pc += 2
                }
            } // 9XY0 -> SNE Vx, Vy

            0x6 => self.v[x] = lower, // 6XNN -> LD Vx, byte
            0x7 => self.v[x] = self.v[x].wrapping_add(lower), // 7XNN -> ADD Vx, byte

            0x8 => {
                match nibbles[3] {
                    0x0 => self.v[x] = self.v[y],  // 8XY0 -> LD Vx, Vy
                    0x1 => self.v[x] |= self.v[y], // 8XY1 -> OR Vx, Vy
                    0x2 => self.v[x] &= self.v[y], // 8XY2 -> AND Vx, Vy
                    0x3 => self.v[x] ^= self.v[y], // 8XY3 -> XOR Vx, Vy
                    0x4 =>
                    // 8XY4 -> ADD Vx, Vy
                    {
                        let result = self.v[x] as u16 + self.v[y] as u16;
                        self.v[x] = (result % 256) as u8;
                        self.v[0xF] = (result > 0xFF) as u8;
                    }
                    0x5 =>
                    // 8XY5 -> SUB  Vx, Vy
                    {
                        let vf = (self.v[x] >= self.v[y]) as u8;
                        self.v[x] = self.v[x].wrapping_sub(self.v[y]);
                        self.v[0xF] = vf;
                    }
                    0x7 =>
                    // 8XY7 -> SUBN Vx, Vy
                    {
                        let vf = (self.v[y] >= self.v[x]) as u8;
                        self.v[x] = self.v[y].wrapping_sub(self.v[x]);
                        self.v[0xF] = vf;
                    }
                    0x6 =>
                    // 8XY6 -> SHR Vx {, Vy}
                    {
                        if self.config.shift_behaviour {
                            self.v[0xF] = self.v[y] & 0x1;
                            self.v[x] = self.v[y] >> 1;
                        } else {
                            self.v[0xF] = self.v[x] & 0x1;
                            self.v[x] >>= 1;
                        }
                    }
                    0xE =>
                    // 8XYE -> SHL Vx {, Vy}
                    {
                        if self.config.shift_behaviour {
                            self.v[0xF] = (self.v[y] >> 7) & 0x1;
                            self.v[x] = self.v[y] << 1;
                        } else {
                            self.v[0xF] = (self.v[x] >> 7) & 0x1;
                            self.v[x] <<= 1;
                        }
                    }
                    _ => self.unkown_instruction(),
                }
            }
            0xA => self.r_address = addr, // ANNN -> LD I, addr
            0xC => self.v[x] = thread_rng().gen::<u8>() & lower, // CXNN -> RND Vx, byte
            0xD =>
            // DXYN - DXY0*
            {
                match nibbles[3] {
                    0 => self.draw_sprite(self.v[x] as usize, self.v[y] as usize, 16, 16), // DXY0* -> DRW Vx, Vy, 0
                    height => self.draw_sprite(self.v[x] as usize, self.v[y] as usize, 8, height), // DXYN -> DRW Vx, Vy, nibble
                }
            }
            0xE => {
                match lower {
                    0x9E => {
                        if self.is_key_pressed(KEY_MAP[self.v[x] as usize % KEY_MAP.len()]) {
                            self.pc += 2
                        }
                    } // EX9E -> SKP Vx
                    0xA1 => {
                        if !self.is_key_pressed(KEY_MAP[self.v[x] as usize % KEY_MAP.len()]) {
                            self.pc += 2
                        }
                    } // EXA1 -> SKNP Vx
                    _ => self.unkown_instruction(),
                }
            }
            0xF => {
                match lower {
                    0x07 => self.v[x] = self.r_delay_timer, // FX07 -> LD Vx, DT
                    0x15 =>
                    // FX15 -> LD DT, Vx
                    {
                        self.r_delay_timer = self.v[x];
                        self.delay_tick = self.dt_interval;
                    }
                    0x18 =>
                    // FX18 -> LD ST, Vx
                    {
                        self.r_sound_timer = self.v[x];
                        self.sound_tick = self.st_interval;

                        self.beeper.device.resume()
                    }
                    0x0A => self.v[x] = self.wait_key_input() as u8, // FX0A -> LD Vx, K
                    0x1E => self.r_address += self.v[x] as u16,      // FX1E -> ADD I, Vx
                    0x29 => self.r_address = self.v[x].min(0xF) as u16 * 5, // FX29 -> LD F, Vx
                    0x30 =>
                    // FX30* -> LD HF, Vx
                    {
                        self.r_address = SMALL_FONT_SIZE as u16 + self.v[x].min(9) as u16 * 10;
                    }
                    0x33 =>
                    // FX33 -> LD B, Vx
                    {
                        self.memory[self.r_address as usize] = (self.v[x] / 100) % 10;
                        self.memory[self.r_address as usize + 1] = (self.v[x] / 10) % 10;
                        self.memory[self.r_address as usize + 2] = self.v[x] % 10;
                    }
                    0x55 =>
                    // FX55 -> LD [I], Vx
                    {
                        // store v0..vx to memory starting at I (address register)

                        let len = x + 1;
                        let dest = self.r_address as usize;

                        self.memory[dest..dest + len].copy_from_slice(&self.v[..len]);
                        if self.config.store_behaviour {
                            self.r_address += len as u16;
                        }
                    }
                    0x65 =>
                    // FX65 -> LD Vx, [I]
                    {
                        // read v0..vx from memory starting at I (address register)

                        let len = x + 1;
                        let src = self.r_address as usize;

                        self.v[..len].copy_from_slice(&self.memory[src..src + len]);
                        if self.config.store_behaviour {
                            self.r_address += len as u16;
                        }
                    }
                    0x85 =>
                    // FX85* -> LD Vx, R
                    {
                        let vx = self.v[x].min(7) as usize;

                        // restore the registers v0..vx
                        self.v[..vx].copy_from_slice(&self.flag_registers[..vx]);
                    }
                    0x75 =>
                    // FX75* -> LD R, Vx
                    {
                        let vx = self.v[x].min(7) as usize;

                        // save v0..vx registers to flag registers
                        self.flag_registers[..vx].copy_from_slice(&self.v[..vx]);
                    }
                    _ => self.unkown_instruction(),
                }
            }
            _ => self.unkown_instruction(),
        }
    }
}
