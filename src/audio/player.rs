use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Sender, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use super::decoder::AudioDecoder;

#[derive(Debug, Clone, Copy)]
pub enum SeekCommand {
    Forward(Duration),
    Backward(Duration),
    To(Duration),
}

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
    sample_rate: Arc<Mutex<u32>>,
    playback_speed: Arc<Mutex<f32>>,
    stream: Arc<Mutex<Option<cpal::Stream>>>,
    audio_buffer: Arc<Mutex<VecDeque<i16>>>,
    seek_sender: Mutex<Sender<SeekCommand>>,
    _decoder_thread: Option<std::thread::JoinHandle<()>>,
}

fn buffer_size_for_sample_rate(sample_rate: u32) -> usize {
    (sample_rate as usize) * 2 * 3
}

impl AudioPlayer {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new() -> Result<Self> {
        let (seek_sender, _seek_receiver) = mpsc::channel();
        Ok(Self {
            state: Arc::new(Mutex::new(PlayerState::Stopped)),
            position: Arc::new(Mutex::new(Duration::ZERO)),
            duration: Arc::new(Mutex::new(Duration::ZERO)),
            volume: Arc::new(Mutex::new(1.0)),
            sample_rate: Arc::new(Mutex::new(44100)),
            playback_speed: Arc::new(Mutex::new(1.0)),
            stream: Arc::new(Mutex::new(None)),
            audio_buffer: Arc::new(Mutex::new(VecDeque::new())),
            seek_sender: Mutex::new(seek_sender),
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

    pub fn playback_speed(&self) -> f32 {
        *self.playback_speed.lock()
    }

    pub fn set_playback_speed(&self, speed: f32) {
        *self.playback_speed.lock() = speed.clamp(0.5, 2.0);
    }

    pub fn play(&mut self, url: &str, speed: f32) -> Result<()> {
        self.set_playback_speed(speed);
        self.stop();

        let host = cpal::default_host();
        let device = host.default_output_device().context("No output device")?;
        let supported_config = device.default_output_config()?;
        let config = supported_config.config();
        let sample_rate = config.sample_rate.0;
        *self.sample_rate.lock() = sample_rate;

        *self.position.lock() = Duration::ZERO;
        *self.state.lock() = PlayerState::Playing;

        let buffer_size = buffer_size_for_sample_rate(sample_rate);
        let audio_buffer = self.audio_buffer.clone();
        let state = self.state.clone();
        let url_owned = url.to_string();
        let duration_arc = self.duration.clone();
        let position_arc = self.position.clone();
        let speed_for_thread = *self.playback_speed.lock();
        let (seek_sender, seek_receiver) = mpsc::channel();
        *self.seek_sender.lock() = seek_sender;

        let decoder_thread = std::thread::spawn(move || {
            if let Ok(mut decoder) = AudioDecoder::from_url_with_sample_rate_and_speed(
                &url_owned,
                sample_rate,
                speed_for_thread,
            ) {
                *duration_arc.lock() = decoder.duration();

                let mut total_samples_decoded: u64 = 0;
                let channels = decoder.channels() as u64;
                let output_sample_rate = decoder.output_sample_rate() as u64;

                let min_buffer_size = buffer_size / 3;
                loop {
                    match seek_receiver.try_recv() {
                        Ok(SeekCommand::To(pos)) => {
                            audio_buffer.lock().clear();
                            if decoder.seek(pos).is_ok() {
                                *position_arc.lock() = pos;
                                total_samples_decoded = (pos.as_secs_f64()
                                    * output_sample_rate as f64
                                    * channels as f64)
                                    as u64;
                            }
                        }
                        Ok(SeekCommand::Forward(delta)) => {
                            let current = *position_arc.lock();
                            let new_pos = current + delta;
                            let duration = *duration_arc.lock();
                            let target = if new_pos > duration {
                                duration
                            } else {
                                new_pos
                            };
                            audio_buffer.lock().clear();
                            if decoder.seek(target).is_ok() {
                                *position_arc.lock() = target;
                                total_samples_decoded = (target.as_secs_f64()
                                    * output_sample_rate as f64
                                    * channels as f64)
                                    as u64;
                            }
                        }
                        Ok(SeekCommand::Backward(delta)) => {
                            let current = *position_arc.lock();
                            let target = if delta > current {
                                Duration::ZERO
                            } else {
                                current - delta
                            };
                            audio_buffer.lock().clear();
                            if decoder.seek(target).is_ok() {
                                *position_arc.lock() = target;
                                total_samples_decoded = (target.as_secs_f64()
                                    * output_sample_rate as f64
                                    * channels as f64)
                                    as u64;
                            }
                        }
                        Err(TryRecvError::Empty) => {}
                        Err(TryRecvError::Disconnected) => break,
                    }

                    let current_state = *state.lock();
                    match current_state {
                        PlayerState::Stopped => break,
                        PlayerState::Paused => {
                            std::thread::sleep(Duration::from_millis(10));
                            continue;
                        }
                        PlayerState::Playing => {}
                    }

                    match decoder.decode_next() {
                        Ok(Some(samples)) => {
                            total_samples_decoded += samples.len() as u64;

                            if channels > 0 && output_sample_rate > 0 {
                                let position_secs =
                                    total_samples_decoded / channels / output_sample_rate;
                                *position_arc.lock() = Duration::from_secs(position_secs);
                            }

                            let mut offset = 0;
                            while offset < samples.len() {
                                {
                                    let mut buffer = audio_buffer.lock();
                                    while offset < samples.len() && buffer.len() < buffer_size {
                                        buffer.push_back(samples[offset]);
                                        offset += 1;
                                    }
                                }
                                if offset < samples.len() {
                                    if *state.lock() == PlayerState::Stopped {
                                        break;
                                    }
                                    std::thread::sleep(Duration::from_millis(1));
                                }
                            }
                        }
                        Ok(None) => {
                            while !audio_buffer.lock().is_empty()
                                && *state.lock() == PlayerState::Playing
                            {
                                std::thread::sleep(Duration::from_millis(10));
                            }
                            *state.lock() = PlayerState::Stopped;
                            break;
                        }
                        Err(_) => {
                            *state.lock() = PlayerState::Stopped;
                            break;
                        }
                    }

                    let current_size = audio_buffer.lock().len();
                    if current_size < min_buffer_size {
                        std::thread::sleep(Duration::from_millis(1));
                    } else {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            } else {
                // Decoder creation failed; signal stop so the main thread
                // doesn't block forever waiting for the buffer to fill.
                *state.lock() = PlayerState::Stopped;
            }
        });

        let initial_buffer_size = buffer_size / 4;
        loop {
            if *self.state.lock() == PlayerState::Stopped {
                break;
            }
            let current_size = self.audio_buffer.lock().len();
            if current_size >= initial_buffer_size {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        let audio_buffer_clone = self.audio_buffer.clone();
        let volume_clone = self.volume.clone();
        let channels = config.channels as usize;
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let mut buffer = audio_buffer_clone.lock();
                let vol = *volume_clone.lock();

                for frame in data.chunks_mut(channels) {
                    // Our decoder outputs stereo (2 channels).
                    // Pop left and right from the buffer.
                    let left_sample = if buffer.len() >= 2 {
                        buffer.pop_front().unwrap() as f32 / i16::MAX as f32
                    } else {
                        0.0
                    };
                    let right_sample = if !buffer.is_empty() {
                        buffer.pop_front().unwrap() as f32 / i16::MAX as f32
                    } else {
                        0.0
                    };

                    // Write stereo samples to first two channels,
                    // silence any additional channels (e.g. 5.1 surround)
                    frame[0] = left_sample * vol;
                    if frame.len() > 1 {
                        frame[1] = right_sample * vol;
                    }
                    for ch in frame.iter_mut().skip(2) {
                        *ch = 0.0;
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;

        stream.play()?;
        *self.stream.lock() = Some(stream);
        self._decoder_thread = Some(decoder_thread);

        Ok(())
    }

    pub fn pause(&self) {
        *self.state.lock() = PlayerState::Paused;
        if let Some(stream) = self.stream.lock().as_ref() {
            let _ = stream.pause();
        }
    }

    pub fn resume(&self) {
        *self.state.lock() = PlayerState::Playing;
        if let Some(stream) = self.stream.lock().as_ref() {
            let _ = stream.play();
        }
    }

    pub fn stop(&mut self) {
        *self.state.lock() = PlayerState::Stopped;
        *self.stream.lock() = None;
        if let Some(thread) = self._decoder_thread.take() {
            let _ = thread.join();
        }
        self.audio_buffer.lock().clear();
    }

    pub fn seek_forward(&self, delta: Duration) {
        let _ = self.seek_sender.lock().send(SeekCommand::Forward(delta));
    }

    pub fn seek_backward(&self, delta: Duration) {
        let _ = self.seek_sender.lock().send(SeekCommand::Backward(delta));
    }

    pub fn seek_to(&self, position: Duration) {
        let _ = self.seek_sender.lock().send(SeekCommand::To(position));
    }
}
