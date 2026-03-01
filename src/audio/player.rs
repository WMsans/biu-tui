use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use super::decoder::AudioDecoder;

const BUFFER_SIZE: usize = 44100 * 2 * 3;

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
    audio_buffer: Arc<Mutex<VecDeque<i16>>>,
    _decoder_thread: Option<std::thread::JoinHandle<()>>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(PlayerState::Stopped)),
            position: Arc::new(Mutex::new(Duration::ZERO)),
            duration: Arc::new(Mutex::new(Duration::ZERO)),
            volume: Arc::new(Mutex::new(1.0)),
            _stream: None,
            audio_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(BUFFER_SIZE))),
            _decoder_thread: None,
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
        self.stop();

        let host = cpal::default_host();
        let device = host.default_output_device().context("No output device")?;
        let supported_config = device.default_output_config()?;
        let config = supported_config.config();

        *self.position.lock() = Duration::ZERO;
        *self.state.lock() = PlayerState::Playing;

        let audio_buffer = self.audio_buffer.clone();
        let state = self.state.clone();
        let url_owned = url.to_string();

        let decoder_thread = std::thread::spawn(move || {
            if let Ok(mut decoder) = AudioDecoder::from_url(&url_owned) {
                while *state.lock() == PlayerState::Playing {
                    match decoder.decode_next() {
                        Ok(Some(samples)) => {
                            let mut buffer = audio_buffer.lock();
                            for sample in samples {
                                if buffer.len() < BUFFER_SIZE {
                                    buffer.push_back(sample);
                                }
                            }
                        }
                        Ok(None) => {
                            *state.lock() = PlayerState::Stopped;
                            break;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        });

        let audio_buffer_clone = self.audio_buffer.clone();
        let volume_clone = self.volume.clone();
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buffer = audio_buffer_clone.lock();
                let vol = *volume_clone.lock();

                for frame in data.chunks_mut(2) {
                    let left_sample = buffer.pop_front().unwrap_or(0) as f32 / i16::MAX as f32;
                    let right_sample = buffer.pop_front().unwrap_or(0) as f32 / i16::MAX as f32;

                    frame[0] = left_sample * vol;
                    if frame.len() > 1 {
                        frame[1] = right_sample * vol;
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;

        stream.play()?;
        self._stream = Some(stream);
        self._decoder_thread = Some(decoder_thread);

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
        self.audio_buffer.lock().clear();
        self._decoder_thread = None;
    }

    pub fn seek(&self, _position: Duration) {
        *self.position.lock() = _position;
    }
}
