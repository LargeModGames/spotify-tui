//! Commands for controlling the local player
//!
//! These commands are sent from the main thread to the player worker thread.

/// Commands that can be sent to the player worker
#[derive(Debug, Clone)]
pub enum PlayerCommand {
  /// Initialize the player with OAuth configuration
  Initialize {
    /// Client ID for OAuth
    client_id: String,
    /// Redirect port for OAuth callback
    redirect_port: u16,
  },

  /// Load a track and optionally start playing
  Load {
    /// Spotify URI (e.g., "spotify:track:xxx")
    uri: String,
    /// Whether to start playing immediately
    start_playing: bool,
    /// Position to start at (in milliseconds)
    position_ms: u32,
  },

  /// Start/resume playback
  Play,

  /// Pause playback
  Pause,

  /// Stop playback completely
  Stop,

  /// Seek to a position (in milliseconds)
  Seek(u32),

  /// Set volume (0-65535, where 65535 is 100%)
  SetVolume(u16),

  /// Preload a track for gapless playback
  Preload(String),

  /// Shutdown the player worker
  Shutdown,
}

impl PlayerCommand {
  /// Create a load command with defaults
  pub fn load(uri: impl Into<String>) -> Self {
    Self::Load {
      uri: uri.into(),
      start_playing: true,
      position_ms: 0,
    }
  }

  /// Create a load command starting at a specific position
  pub fn load_at(uri: impl Into<String>, position_ms: u32) -> Self {
    Self::Load {
      uri: uri.into(),
      start_playing: true,
      position_ms,
    }
  }

  /// Create a load command without auto-play
  pub fn load_paused(uri: impl Into<String>) -> Self {
    Self::Load {
      uri: uri.into(),
      start_playing: false,
      position_ms: 0,
    }
  }
}
