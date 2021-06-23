
use rand::{Rng, thread_rng};

#[repr(C)]
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Color
{
    r: u8, g: u8, b: u8, a: u8
}

impl Color
{
    pub fn new(r: u8, g: u8, b: u8) -> Self
    {
        Self { r, g, b, a: 0 }
    }

    pub fn rand() -> Self
    {
        let rand: u32 = thread_rng().gen();

        Self
        {
            r: ( rand        & 0xFF) as u8,
            g: ((rand >> 8 ) & 0xFF) as u8,
            b: ((rand >> 16) & 0xFF) as u8,
            a: 0
        }
    }

    pub fn as_array(&self) -> [f32; 3]
    {
        [ (self.r as f32) / 255.0, (self.g as f32) / 255.0, (self.b as f32) / 255.0 ]
    }

    pub fn from_array(arr: [f32; 3]) -> Self
    {
        Self::new((arr[0] * 255.0) as u8, (arr[1] * 255.0) as u8, (arr[2] * 255.0) as u8)
    }
}

#[allow(dead_code)]
pub fn clear_gl_errors()
{
    while unsafe { gl::GetError() } != gl::NO_ERROR {}
}

#[allow(dead_code)]
pub fn check_gl_errors()
{
    loop
    {
        match unsafe { gl::GetError() }
        {
            gl::NO_ERROR => break,
            error => 
            {
                panic!("[OpenGL Error]: {}", error);
            }
        }
    }
    
}

#[macro_export]
#[cfg(debug_assertions)]
macro_rules! gl_call 
{
    ($x: expr) => 
    {
        {
            crate::chip8::utils::clear_gl_errors();

            let result = unsafe { $x };
        
            crate::chip8::utils::check_gl_errors();
            
            result
        }
    };
}

#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! gl_call 
{
    ($x: expr) => 
    {        
        unsafe { $x }
    };
}