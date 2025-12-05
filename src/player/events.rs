//! Events emitted by the local player
//!
//! These events are sent from the player worker thread to the main thread
//! to communicate playback state changes.

/// Events emitted by the player worker
#[derive(Debug, Clone)]
pub enum PlayerEvent {
  /// Player has been initialized successfully
  Initialized,

  /// Player initialization failed
  InitializationFailed {
    /// Error message
    message: String,
  },

  /// Playback has started
  Playing {
    /// Current track URI
    track_uri: String,
    /// Current position in milliseconds
    position_ms: u32,
    /// Track duration in milliseconds
    duration_ms: u32,
  },

  /// Playback has been paused
  Paused {
    /// Current track URI
    track_uri: String,
    /// Position where playback was paused (in milliseconds)
    position_ms: u32,
  },

  /// Playback has stopped
  Stopped,

  /// Current track has ended
  TrackEnded {
    /// URI of the track that ended
    track_uri: String,
  },

  /// Position update (sent periodically during playback)
  Position {
    /// Current position in milliseconds
    position_ms: u32,
    /// Track duration in milliseconds
    duration_ms: u32,
  },

  /// Volume has changed
  VolumeChanged {
    /// New volume level (0-65535)
    volume: u16,
  },

  /// Time to preload the next track (for gapless playback)
  TimeToPreloadNextTrack,

  /// Track is being loaded
  Loading {
    /// URI of the track being loaded
    track_uri: String,
  },

  /// An error occurred
  Error {
    /// Error message
    message: String,
  },

  /// Session disconnected
  SessionDisconnected,

  /// Player worker has shut down
  Shutdown,
}

impl PlayerEvent {
  /// Returns true if this is an error event
  pub fn is_error(&self) -> bool {
    matches!(self, PlayerEvent::Error { .. })
  }

  /// Returns true if this is a playing event
  pub fn is_playing(&self) -> bool {
    matches!(self, PlayerEvent::Playing { .. })
  }

  /// Returns true if this is a paused event
  pub fn is_paused(&self) -> bool {
    matches!(self, PlayerEvent::Paused { .. })
  }
}
