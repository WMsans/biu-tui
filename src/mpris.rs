use anyhow::{Context, Result};
use async_channel::{Receiver, Sender, unbounded};
use mpris_server::{Metadata, PlaybackStatus, Player, Time, TrackId, Volume};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::audio::PlayerState;
use crate::playing_list::PlaylistItem;

#[derive(Debug, Clone, PartialEq)]
pub enum MprisCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Seek(Duration),
    SetVolume(f64),
}

#[derive(Debug, Clone)]
pub enum MprisUpdate {
    SetTrack(PlaylistItem),
    SetState(PlayerState),
    SetPosition(Duration),
    SetVolume(f32),
}

pub struct MprisManager {
    command_receiver: Receiver<MprisCommand>,
    update_sender: Sender<MprisUpdate>,
    _thread: JoinHandle<()>,
}

impl MprisManager {
    pub fn new() -> Result<Self> {
        let (command_sender, command_receiver): (Sender<MprisCommand>, Receiver<MprisCommand>) =
            unbounded();
        let (update_sender, update_receiver): (Sender<MprisUpdate>, Receiver<MprisUpdate>) =
            unbounded();

        let thread = std::thread::Builder::new()
            .name("mpris-server".into())
            .spawn(move || {
                if let Err(e) = Self::run_mpris_server(command_sender, update_receiver) {
                    eprintln!("MPRIS server thread error: {}", e);
                }
            })
            .context("Failed to spawn MPRIS server thread")?;

        Ok(Self {
            command_receiver,
            update_sender,
            _thread: thread,
        })
    }

    fn run_mpris_server(
        command_sender: Sender<MprisCommand>,
        update_receiver: Receiver<MprisUpdate>,
    ) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Failed to create MPRIS runtime")?;

        let local_set = tokio::task::LocalSet::new();

        let player = runtime.block_on(local_set.run_until(async {
            Player::builder("com.github.biu-tui")
                .identity("Biu TUI")
                .can_play(true)
                .can_pause(true)
                .can_go_next(true)
                .can_go_previous(true)
                .can_seek(true)
                .can_control(true)
                .build()
                .await
                .context("Failed to create MPRIS player")
        }))?;

        let cmd_sender = command_sender.clone();
        player.connect_play_pause(move |_| {
            let _ = cmd_sender.try_send(MprisCommand::Play);
        });

        let cmd_sender = command_sender.clone();
        player.connect_play(move |_| {
            let _ = cmd_sender.try_send(MprisCommand::Play);
        });

        let cmd_sender = command_sender.clone();
        player.connect_pause(move |_| {
            let _ = cmd_sender.try_send(MprisCommand::Pause);
        });

        let cmd_sender = command_sender.clone();
        player.connect_stop(move |_| {
            let _ = cmd_sender.try_send(MprisCommand::Stop);
        });

        let cmd_sender = command_sender.clone();
        player.connect_next(move |_| {
            let _ = cmd_sender.try_send(MprisCommand::Next);
        });

        let cmd_sender = command_sender.clone();
        player.connect_previous(move |_| {
            let _ = cmd_sender.try_send(MprisCommand::Previous);
        });

        let cmd_sender = command_sender.clone();
        player.connect_seek(move |_, offset: Time| {
            let duration = Duration::from_micros(offset.as_micros() as u64);
            let _ = cmd_sender.try_send(MprisCommand::Seek(duration));
        });

        let cmd_sender = command_sender.clone();
        player.connect_set_position(move |_, _track_id: &TrackId, position: Time| {
            let duration = Duration::from_micros(position.as_micros() as u64);
            let _ = cmd_sender.try_send(MprisCommand::Seek(duration));
        });

        let cmd_sender = command_sender;
        player.connect_set_volume(move |_, volume: Volume| {
            let _ = cmd_sender.try_send(MprisCommand::SetVolume(volume));
        });

        local_set.spawn_local(async move {
            let mut run_task = std::pin::pin!(player.run());

            loop {
                tokio::select! {
                    update = update_receiver.recv() => {
                        match update {
                            Ok(MprisUpdate::SetTrack(item)) => {
                                let metadata = Metadata::builder()
                                    .title(item.title)
                                    .artist([item.artist])
                                    .length(Time::from_micros(item.duration as i64 * 1_000_000))
                                    .build();
                                if let Err(e) = player.set_metadata(metadata).await {
                                    eprintln!("Failed to set MPRIS metadata: {}", e);
                                }
                            }
                            Ok(MprisUpdate::SetState(state)) => {
                                let status = match state {
                                    PlayerState::Playing => PlaybackStatus::Playing,
                                    PlayerState::Paused => PlaybackStatus::Paused,
                                    PlayerState::Stopped => PlaybackStatus::Stopped,
                                };
                                if let Err(e) = player.set_playback_status(status).await {
                                    eprintln!("Failed to set MPRIS playback status: {}", e);
                                }
                            }
                            Ok(MprisUpdate::SetPosition(position)) => {
                                let time = Time::from_micros(position.as_micros() as i64);
                                player.set_position(time);
                            }
                            Ok(MprisUpdate::SetVolume(volume)) => {
                                if let Err(e) = player.set_volume(volume as f64).await {
                                    eprintln!("Failed to set MPRIS volume: {}", e);
                                }
                            }
                            Err(_) => {}
                        }
                    }
                    _ = run_task.as_mut() => break,
                }
            }
        });

        runtime.block_on(local_set);
        Ok(())
    }

    pub fn set_track(&self, item: &PlaylistItem) {
        let _ = self.update_sender.try_send(MprisUpdate::SetTrack(item.clone()));
    }

    pub fn set_state(&self, state: PlayerState) {
        let _ = self.update_sender.try_send(MprisUpdate::SetState(state));
    }

    pub fn set_position(&self, position: Duration) {
        let _ = self.update_sender.try_send(MprisUpdate::SetPosition(position));
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.update_sender.try_send(MprisUpdate::SetVolume(volume));
    }

    pub fn poll_commands(&self) -> Vec<MprisCommand> {
        let mut commands = Vec::new();
        while let Ok(cmd) = self.command_receiver.try_recv() {
            commands.push(cmd);
        }
        commands
    }
}
