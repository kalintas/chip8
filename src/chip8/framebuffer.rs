
use crate::gl_call;

pub struct FrameBuffer
{
    width: i32,
    height: i32,

    id: u32,
    texture: u32
}


impl FrameBuffer
{
    pub fn new(width: i32, height: i32) -> Self
    {
        let mut id = 0;
        
        gl_call!(gl::GenFramebuffers(1, &mut id));
        gl_call!(gl::BindFramebuffer(gl::READ_FRAMEBUFFER, id));

        let mut texture = 0;

        gl_call!(gl::GenTextures(1, &mut texture));
        gl_call!(gl::BindTexture(gl::TEXTURE_2D, texture));
        

        let framebuffer = Self 
        {
            width, height,

            id, texture
        };

        gl_call!(gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as _));
        gl_call!(gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as _));
    
        gl_call!(gl::FramebufferTexture2D(gl::READ_FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::TEXTURE_2D, texture, 0));
    
        framebuffer
    }

    pub fn update_buffer(&self, ptr: *const u8, internal_format: u32, format: u32)
    {
        gl_call!(gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.id));
        gl_call!(gl::TexImage2D(gl::TEXTURE_2D, 0, internal_format as i32, self.width, self.height, 0, format, gl::UNSIGNED_BYTE, ptr as _));
    }

    pub fn draw_buffer(&self, x0: i32, y0: i32, x1: i32, y1: i32)
    {   
        gl_call!(gl::BindFramebuffer(gl::READ_FRAMEBUFFER, self.id));
        gl_call!(gl::BindFramebuffer(gl::DRAW_FRAMEBUFFER, 0));
        gl_call!(gl::BlitFramebuffer(0, 0, self.width, self.height, x0, y0, x1, y1, gl::COLOR_BUFFER_BIT, gl::NEAREST));
    }
}

impl Drop for FrameBuffer
{
    fn drop(&mut self) 
    {   
        gl_call!(gl::DeleteFramebuffers(1, &self.id));
        gl_call!(gl::DeleteTextures(1, &self.texture));
    }
}