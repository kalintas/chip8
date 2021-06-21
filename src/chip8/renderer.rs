use std::{error::Error, time::Instant};

use imgui_opengl_renderer::Renderer as ImguiRenderer;
use sdl2::{EventPump, event::{Event, WindowEvent}, video::{GLContext, GLProfile, SwapInterval}, keyboard::Keycode};

use imgui_sdl2::ImguiSdl2;

use crate::gl_call;

pub struct Renderer
{
    last_frame: Instant,

    pub window_width: u32,
    pub window_height: u32,

    pub imgui: imgui::Context,
    imgui_sdl: ImguiSdl2,

    imgui_renderer: ImguiRenderer,

    pub event_pump: EventPump,

    pub window: sdl2::video::Window,
    _video_subsys: sdl2::VideoSubsystem,
    pub sdl: sdl2::Sdl,

    _gl_context: GLContext
}

impl Renderer
{
    pub fn new(title: impl AsRef<str>, width: u32, height: u32) -> Result<Self, Box<dyn Error>>
    {
        let sdl = sdl2::init()?;
        let _video_subsys = sdl.video()?;

        let gl_attr = _video_subsys.gl_attr();

        gl_attr.set_context_profile(GLProfile::Core);
        gl_attr.set_context_version(3, 3);

        let window = _video_subsys.window(title.as_ref(), width, height)
            .opengl()
            .resizable()
            .position_centered()
            .build()?;
        
        let event_pump = sdl.event_pump()?;

        let _gl_context = window.gl_create_context()?;
        gl::load_with(|s| _video_subsys.gl_get_proc_address(s) as _);

        _video_subsys.gl_set_swap_interval(SwapInterval::VSync)?;

        let mut imgui = imgui::Context::create();

        imgui.set_ini_filename(None);

        let imgui_renderer = ImguiRenderer::new(&mut imgui, |s| _video_subsys.gl_get_proc_address(s) as _);

        let imgui_sdl = ImguiSdl2::new(&mut imgui, &window);

        Ok(Self 
        {
            window_width: width, window_height: height,

            last_frame: Instant::now(),

            imgui, imgui_sdl, imgui_renderer,
            event_pump, window, 
            _video_subsys, sdl, _gl_context
        })
    }

    pub fn poll_events(&mut self) -> bool
    {
        for event in self.event_pump.poll_iter()
        {
            self.imgui_sdl.handle_event(&mut self.imgui, &event);
            if self.imgui_sdl.ignore_event(&event) { continue; }

            match event 
            {
                Event::Quit{ .. } | Event::KeyDown{ keycode: Some(Keycode::Escape), .. } => return true,
                Event::Window{ win_event, .. } =>
                {
                    match win_event
                    {
                        WindowEvent::Resized(width, height) =>
                        {
                            self.window_width  = width  as u32;
                            self.window_height = height as u32;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        false
    }

    pub fn clear_screen(&mut self)
    {
        gl_call!(gl::Clear(gl::COLOR_BUFFER_BIT));
        gl_call!(gl::ClearColor(0.0, 0.0, 0.0, 1.0));
    }

    pub fn render(&mut self, func: impl FnOnce(&imgui::Ui))
    {
        // update imgui's delta time
        self.imgui.io_mut().delta_time = self.last_frame.elapsed().as_secs_f32();
        self.last_frame = Instant::now();
        
        // render imgui
        self.imgui_sdl.prepare_frame(self.imgui.io_mut(), &self.window, &self.event_pump.mouse_state());
        
        let ui = self.imgui.frame();
        
        func(&ui);
        
        self.imgui_sdl.prepare_render(&ui, &self.window);
        
        self.imgui_renderer.render(ui);
        
        // swap buffers
        self.window.gl_swap_window();
    }
}
