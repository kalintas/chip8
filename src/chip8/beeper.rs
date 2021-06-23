

use sdl2::audio::{AudioDevice, AudioCallback, AudioSpecDesired};


const SAMPLE_RATE: i32 = 44100;

pub struct Callback
{
    pub freq: f32,
    time: f32,
    pub volume: f32
}

impl AudioCallback for Callback
{
    type Channel = f32;

    fn callback(&mut self, buffer: &mut [Self::Channel]) 
    {
        const TIME_INC: f32 = 1.0 / SAMPLE_RATE as f32;

        for value in buffer.iter_mut()
        {
            *value = if self.time > 0.5 { self.volume } else { -self.volume };
            self.time = (self.time + TIME_INC * self.freq) % 1.0;
        }
    }
}

// simple struct for square wave generation
pub struct Beeper
{
    _audio_subsys: sdl2::AudioSubsystem,
    pub device: AudioDevice<Callback>
}

impl Beeper
{
    pub fn new(sdl: &sdl2::Sdl) -> Result<Self, String>
    {
        let _audio_subsys = sdl.audio()?;

        let desired_spec = AudioSpecDesired
        {
            freq: Some(SAMPLE_RATE),
            channels: Some(1), // mono channel
            samples: Some(512)
        };

        let device = _audio_subsys.open_playback(None, &desired_spec, |_|
        {
            Callback
            {
                freq: 441.0,
                time: 0.0,
                volume: 0.2
            }
        })?;

        Ok(Self { _audio_subsys, device })
    }
}

