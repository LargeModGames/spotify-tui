//! Local playback module using librespot
//!
//! This module provides local audio playback capabilities using librespot,
//! allowing spotatui to play Spotify audio directly without requiring an
//! external Spotify Connect device.

#[cfg(feature = "librespot")]
mod commands;
#[cfg(feature = "librespot")]
mod events;
#[cfg(feature = "librespot")]
mod worker;

#[cfg(feature = "librespot")]
pub use commands::PlayerCommand;
#[cfg(feature = "librespot")]
pub use events::PlayerEvent;
#[cfg(feature = "librespot")]
pub use worker::{spawn_player_worker, PlayerWorker, PlayerWorkerConfig};

use std::sync::mpsc;

/// Represents the current playback mode
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackMode {
  /// Playback through local librespot player
  #[cfg(feature = "librespot")]
  Local,
  /// Playback through a remote Spotify Connect device
  Remote(Option<String>), // device_id
}

impl Default for PlaybackMode {
  fn default() -> Self {
    PlaybackMode::Remote(None)
  }
}

/// Handle for controlling the local player from the main application
#[cfg(feature = "librespot")]
pub struct LocalPlayer {
  /// Sender to send commands to the player worker thread
  pub command_tx: mpsc::Sender<PlayerCommand>,
  /// Receiver to receive events from the player worker thread
  pub event_rx: mpsc::Receiver<PlayerEvent>,
  /// Current playback state
  pub state: LocalPlayerState,
}

#[cfg(feature = "librespot")]
impl LocalPlayer {
  /// Create a new local player handle
  pub fn new(
    command_tx: mpsc::Sender<PlayerCommand>,
    event_rx: mpsc::Receiver<PlayerEvent>,
  ) -> Self {
    Self {
      command_tx,
      event_rx,
      state: LocalPlayerState::default(),
    }
  }

  /// Send a command to the player worker
  pub fn send_command(&self, cmd: PlayerCommand) -> Result<(), mpsc::SendError<PlayerCommand>> {
    self.command_tx.send(cmd)
  }

  /// Try to receive an event from the player worker (non-blocking)
  pub fn try_recv_event(&self) -> Option<PlayerEvent> {
    self.event_rx.try_recv().ok()
  }
}

/// Current state of the local player
#[cfg(feature = "librespot")]
#[derive(Debug, Clone, Default)]
pub struct LocalPlayerState {
  /// Whether the player has been initialized
  pub is_initialized: bool,
  /// Whether playback is currently active
  pub is_playing: bool,
  /// Current track URI (if any)
  pub current_track_uri: Option<String>,
  /// Current position in milliseconds
  pub position_ms: u32,
  /// Duration of current track in milliseconds
  pub duration_ms: u32,
  /// Current volume (0-65535)
  pub volume: u16,
}
