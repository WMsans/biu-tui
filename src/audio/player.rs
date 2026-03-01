use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
}

pub struct AudioPlayer {
    state: Arc<Mutex<PlayerState>>,
    position: Arc<Mutex<Duration>>,
    duration: Arc<Mutex<Duration>>,
    volume: Arc<Mutex<f32>>,
    _stream: Option<cpal::Stream>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(PlayerState::Stopped)),
            position: Arc::new(Mutex::new(Duration::ZERO)),
            duration: Arc::new(Mutex::new(Duration::ZERO)),
            volume: Arc::new(Mutex::new(1.0)),
            _stream: None,
        })
    }

    pub fn state(&self) -> PlayerState {
        *self.state.lock()
    }

    pub fn position(&self) -> Duration {
        *self.position.lock()
    }

    pub fn duration(&self) -> Duration {
        *self.duration.lock()
    }

    pub fn volume(&self) -> f32 {
        *self.volume.lock()
    }

    pub fn set_volume(&self, vol: f32) {
        *self.volume.lock() = vol.clamp(0.0, 1.0);
    }

    pub fn play(&mut self, url: &str) -> Result<()> {
        let host = cpal::default_host();
        let device = host.default_output_device().context("No output device")?;
        let supported_config = device.default_output_config()?;
        let config = supported_config.config();

        *self.state.lock() = PlayerState::Playing;
        *self.position.lock() = Duration::ZERO;

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;

        stream.play()?;
        self._stream = Some(stream);

        Ok(())
    }

    pub fn pause(&self) {
        *self.state.lock() = PlayerState::Paused;
    }

    pub fn resume(&self) {
        *self.state.lock() = PlayerState::Playing;
    }

    pub fn stop(&mut self) {
        *self.state.lock() = PlayerState::Stopped;
        self._stream = None;
    }

    pub fn seek(&self, _position: Duration) {
        *self.position.lock() = _position;
    }
}
