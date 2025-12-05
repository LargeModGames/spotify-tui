//! Player worker that runs librespot in a background thread
//!
//! This module handles the actual audio playback using librespot.

use super::commands::PlayerCommand;
use super::events::PlayerEvent;
use anyhow::{anyhow, Result};
use librespot_core::{
  authentication::Credentials, cache::Cache, config::SessionConfig, session::Session,
  spotify_id::SpotifyId,
};
use librespot_playback::{
  audio_backend,
  config::{AudioFormat, Bitrate, PlayerConfig, VolumeCtrl},
  mixer::{self, MixerConfig},
  player::{Player, PlayerEvent as LibrespotPlayerEvent, SinkStatus},
};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

/// Configuration for the player worker
#[derive(Debug, Clone)]
pub struct PlayerWorkerConfig {
  /// Audio backend to use (None for default)
  pub audio_backend: Option<String>,
  /// Specific audio device (None for default)
  pub audio_device: Option<String>,
  /// Audio bitrate
  pub bitrate: Bitrate,
  /// Enable volume normalization
  pub normalize_volume: bool,
  /// Cache directory for audio files
  pub cache_path: Option<PathBuf>,
  /// Maximum cache size in bytes
  pub cache_size: Option<u64>,
}

impl Default for PlayerWorkerConfig {
  fn default() -> Self {
    Self {
      audio_backend: None,
      audio_device: None,
      bitrate: Bitrate::Bitrate320,
      normalize_volume: true,
      cache_path: None,
      cache_size: Some(1024 * 1024 * 1024), // 1GB default
    }
  }
}

/// The player worker that manages librespot playback
pub struct PlayerWorker {
  /// Channel to receive commands from the main thread
  command_rx: mpsc::Receiver<PlayerCommand>,
  /// Channel to send events to the main thread
  event_tx: mpsc::Sender<PlayerEvent>,
  /// Librespot session
  session: Option<Session>,
  /// Librespot player
  player: Option<Arc<Player>>,
  /// Configuration
  config: PlayerWorkerConfig,
  /// Current track URI
  current_track_uri: Option<String>,
  /// Current volume (0-65535)
  current_volume: u16,
}

impl PlayerWorker {
  /// Create a new player worker
  pub fn new(
    command_rx: mpsc::Receiver<PlayerCommand>,
    event_tx: mpsc::Sender<PlayerEvent>,
    config: PlayerWorkerConfig,
  ) -> Self {
    Self {
      command_rx,
      event_tx,
      session: None,
      player: None,
      config,
      current_track_uri: None,
      current_volume: u16::MAX / 2, // 50% default
    }
  }

  /// Initialize the librespot session
  /// First tries to use cached credentials, then falls back to OAuth flow
  pub async fn initialize(&mut self, _client_id: &str, _redirect_port: u16) -> Result<()> {
    // IMPORTANT: For librespot to stream audio, we MUST use Spotify's keymaster client ID
    // Using a custom app's client_id will authenticate but NOT grant streaming rights
    // This is the same client ID used by official Spotify apps and librespot
    // The keymaster client ID ONLY accepts redirect URI: http://127.0.0.1:8898/login
    const KEYMASTER_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
    const KEYMASTER_REDIRECT_PORT: u16 = 8898;
    const KEYMASTER_REDIRECT_PATH: &str = "/login";

    eprintln!(
      "Debug: Starting player initialization with keymaster client_id (ignoring app client_id)"
    );
    let session_config = SessionConfig::default();

    // Setup cache if configured
    let cache = if let Some(ref cache_path) = self.config.cache_path {
      eprintln!("Debug: Setting up cache at {:?}", cache_path);
      // Ensure directory exists
      if let Err(e) = std::fs::create_dir_all(cache_path) {
        eprintln!("Debug: Failed to create cache dir: {}", e);
      }
      Cache::new(
        Some(cache_path.clone()),
        Some(cache_path.join("volume")),
        Some(cache_path.join("files")),
        self.config.cache_size,
      )
      .ok()
    } else {
      eprintln!("Debug: No cache path configured");
      None
    };

    // Try to get cached credentials first
    let credentials = if let Some(ref cache) = cache {
      if let Some(creds) = cache.credentials() {
        eprintln!(
          "Debug: Using cached credentials for user: {:?}, auth_type: {:?}",
          creds.username, creds.auth_type
        );
        creds
      } else {
        eprintln!("Debug: No cached credentials, starting OAuth flow");
        // Run OAuth in a blocking thread to avoid tokio runtime conflicts
        let path = KEYMASTER_REDIRECT_PATH.to_string();
        tokio::task::spawn_blocking(move || {
          Self::get_oauth_credentials(KEYMASTER_CLIENT_ID, KEYMASTER_REDIRECT_PORT, &path)
        })
        .await
        .map_err(|e| anyhow!("OAuth task failed: {:?}", e))??
      }
    } else {
      eprintln!("Debug: No cache, starting OAuth flow");
      // Run OAuth in a blocking thread to avoid tokio runtime conflicts
      let path = KEYMASTER_REDIRECT_PATH.to_string();
      tokio::task::spawn_blocking(move || {
        Self::get_oauth_credentials(KEYMASTER_CLIENT_ID, KEYMASTER_REDIRECT_PORT, &path)
      })
      .await
      .map_err(|e| anyhow!("OAuth task failed: {:?}", e))??
    };

    eprintln!(
      "Debug: Got credentials, auth_type={:?}, username={:?}",
      credentials.auth_type, credentials.username
    );

    // Create session
    let session = Session::new(session_config, cache.clone());

    // Connect session - this will authenticate and store reusable credentials
    eprintln!("Debug: Connecting session...");
    session.connect(credentials, true).await?;
    eprintln!("Debug: Session connected successfully!");
    eprintln!("Debug: Session username: {}", session.username());

    // Check if credentials were stored
    if let Some(ref c) = cache {
      if let Some(stored_creds) = c.credentials() {
        eprintln!(
          "Debug: Stored credentials after connect: auth_type={:?}, username={:?}",
          stored_creds.auth_type, stored_creds.username
        );
      } else {
        eprintln!("Debug: No credentials stored after connect!");
      }
    }

    // Check if we have a valid session
    let user_data = session.user_data();
    eprintln!("Debug: User data: {:?}", user_data);

    self.session = Some(session.clone());

    // Create player
    self.create_player(session)?;

    Ok(())
  }

  /// Get OAuth credentials using PKCE flow with browser
  /// This function is blocking and should be called from spawn_blocking
  fn get_oauth_credentials(
    client_id: &str,
    redirect_port: u16,
    redirect_path: &str,
  ) -> Result<Credentials> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use rand::Rng;
    use sha2::{Digest, Sha256};

    let redirect_uri = format!("http://127.0.0.1:{}{}", redirect_port, redirect_path);

    // These are the OAuth scopes required for librespot streaming playback
    // Based on librespot's OAUTH_SCOPES in main.rs
    let scopes = [
      "app-remote-control",
      "playlist-modify",
      "playlist-modify-private",
      "playlist-modify-public",
      "playlist-read",
      "playlist-read-collaborative",
      "playlist-read-private",
      "streaming",
      "ugc-image-upload",
      "user-follow-modify",
      "user-follow-read",
      "user-library-modify",
      "user-library-read",
      "user-modify",
      "user-modify-playback-state",
      "user-modify-private",
      "user-personalized",
      "user-read-birthdate",
      "user-read-currently-playing",
      "user-read-email",
      "user-read-play-history",
      "user-read-playback-position",
      "user-read-playback-state",
      "user-read-private",
      "user-read-recently-played",
      "user-top-read",
    ]
    .join(" ");

    eprintln!("Debug: Starting OAuth flow with browser");
    eprintln!(
      "Debug: client_id={}, redirect_uri={}",
      client_id, redirect_uri
    );

    // Generate PKCE verifier (random 64 bytes, base64url encoded)
    let verifier_bytes: Vec<u8> = (0..64).map(|_| rand::thread_rng().gen::<u8>()).collect();
    let verifier = URL_SAFE_NO_PAD.encode(&verifier_bytes);

    // Generate PKCE challenge (SHA256 hash of verifier, base64url encoded)
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    // Generate state for CSRF protection
    let state_bytes: Vec<u8> = (0..16).map(|_| rand::thread_rng().gen::<u8>()).collect();
    let state = URL_SAFE_NO_PAD.encode(&state_bytes);

    // Build authorization URL
    let auth_url = format!(
      "https://accounts.spotify.com/authorize?\
       client_id={}&\
       response_type=code&\
       redirect_uri={}&\
       scope={}&\
       state={}&\
       code_challenge={}&\
       code_challenge_method=S256",
      client_id,
      urlencoding::encode(&redirect_uri),
      urlencoding::encode(&scopes),
      state,
      challenge
    );

    // Start local server to receive callback
    let listener = TcpListener::bind(format!("127.0.0.1:{}", redirect_port))?;
    eprintln!("Debug: Listening on port {} for callback", redirect_port);

    // Open browser
    eprintln!("Debug: Opening browser for authentication...");
    if let Err(e) = open::that(&auth_url) {
      eprintln!("Failed to open browser: {}. Please visit manually:", e);
      eprintln!("{}", auth_url);
    }

    // Wait for callback
    let (mut stream, _) = listener.accept()?;

    // Read the HTTP request
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the authorization code
    let code = request_line
      .split_whitespace()
      .nth(1)
      .and_then(|path| {
        url::Url::parse(&format!("http://localhost{}", path))
          .ok()
          .and_then(|url| {
            url
              .query_pairs()
              .find(|(key, _)| key == "code")
              .map(|(_, value)| value.to_string())
          })
      })
      .ok_or_else(|| anyhow!("Failed to extract authorization code"))?;

    // Verify state
    let returned_state = request_line.split_whitespace().nth(1).and_then(|path| {
      url::Url::parse(&format!("http://localhost{}", path))
        .ok()
        .and_then(|url| {
          url
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.to_string())
        })
    });

    if returned_state.as_deref() != Some(&state) {
      return Err(anyhow!("CSRF state mismatch"));
    }

    // Send success response
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
      <html><body><h1>Authentication successful!</h1>\
      <p>You can close this window and return to spotatui.</p></body></html>";
    stream.write_all(response.as_bytes())?;
    drop(stream);

    eprintln!("Debug: Got authorization code, exchanging for token...");

    // Exchange code for token
    let client = reqwest::blocking::Client::new();
    let token_response = client
      .post("https://accounts.spotify.com/api/token")
      .form(&[
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &redirect_uri),
        ("client_id", client_id),
        ("code_verifier", &verifier),
      ])
      .send()
      .map_err(|e| anyhow!("Token request failed: {}", e))?;

    if !token_response.status().is_success() {
      let error_text = token_response.text().unwrap_or_default();
      return Err(anyhow!("Token exchange failed: {}", error_text));
    }

    let token_data: serde_json::Value = token_response
      .json()
      .map_err(|e| anyhow!("Failed to parse token response: {}", e))?;

    let access_token = token_data["access_token"]
      .as_str()
      .ok_or_else(|| anyhow!("No access_token in response"))?;

    eprintln!("Debug: OAuth successful!");

    Ok(Credentials::with_access_token(access_token))
  }

  fn create_player(&mut self, session: Session) -> Result<()> {
    // Find audio backend
    let backend_name = self.config.audio_backend.clone();
    eprintln!("Debug: Looking for audio backend: {:?}", backend_name);
    let backend = audio_backend::find(backend_name.clone()).ok_or_else(|| {
      anyhow!(
        "Audio backend '{}' not found",
        backend_name.as_deref().unwrap_or("default")
      )
    })?;
    eprintln!("Debug: Audio backend found");

    // Create mixer for volume control
    // Use None to get the default mixer (softvol)
    let mixer_config = MixerConfig {
      volume_ctrl: VolumeCtrl::Linear,
      ..Default::default()
    };
    let mixer_fn = mixer::find(None).ok_or_else(|| anyhow!("No mixer available"))?;
    let mixer = mixer_fn(mixer_config);
    eprintln!("Debug: Mixer created");

    // Configure player
    let player_config = PlayerConfig {
      bitrate: self.config.bitrate,
      normalisation: self.config.normalize_volume,
      ..Default::default()
    };
    eprintln!(
      "Debug: Player config: bitrate={:?}, normalisation={}",
      player_config.bitrate, player_config.normalisation
    );

    // Create player - Player::new returns Arc<Player>
    let audio_device = self.config.audio_device.clone();
    let audio_format = AudioFormat::default();
    eprintln!(
      "Debug: Creating player with device: {:?}, format: {:?}",
      audio_device, audio_format
    );
    let player = Player::new(player_config, session, mixer.get_soft_volume(), move || {
      eprintln!("Debug: Audio backend factory called");
      backend(audio_device.clone(), audio_format)
    });
    player.set_sink_event_callback(Some(Box::new(|status| {
      eprintln!("Debug: Sink status changed: {:?}", status);
      if let SinkStatus::Closed | SinkStatus::TemporarilyClosed = status {
        eprintln!("Debug: Sink closed - check if the output device is available and not in use");
      }
    })));

    self.player = Some(player);
    eprintln!("Debug: Player created successfully");

    Ok(())
  }

  /// Run the player worker event loop
  pub async fn run(&mut self) -> Result<()> {
    eprintln!("Debug: Player worker run loop started");
    // Player event channel - will be set after initialization
    let mut player_event_channel: Option<
      tokio::sync::mpsc::UnboundedReceiver<LibrespotPlayerEvent>,
    > = None;

    loop {
      // Check for commands from main thread (non-blocking)
      match self.command_rx.try_recv() {
        Ok(cmd) => {
          eprintln!("Debug: Received command: {:?}", cmd);
          let was_uninitialized = self.player.is_none();
          if self.handle_command(cmd).await? {
            break; // Shutdown requested
          }
          // If player was just initialized, get the event channel
          if was_uninitialized && self.player.is_some() {
            eprintln!("Debug: Player just initialized, getting event channel");
            player_event_channel = self.player.as_ref().map(|p| p.get_player_event_channel());
            eprintln!(
              "Debug: Event channel acquired: {}",
              player_event_channel.is_some()
            );
          }
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
          eprintln!("Debug: Command channel disconnected, exiting");
          break;
        }
      }

      // Handle player events if available
      if let Some(ref mut events) = player_event_channel {
        while let Ok(event) = events.try_recv() {
          eprintln!("Debug: Received player event: {:?}", event);
          self.handle_player_event(event).await;
        }
      }

      // Small sleep to prevent busy-waiting
      tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Send shutdown event
    let _ = self.event_tx.send(PlayerEvent::Shutdown);

    Ok(())
  }

  async fn handle_command(&mut self, cmd: PlayerCommand) -> Result<bool> {
    match cmd {
      PlayerCommand::Initialize {
        client_id,
        redirect_port,
      } => match self.initialize(&client_id, redirect_port).await {
        Ok(()) => {
          let _ = self.event_tx.send(PlayerEvent::Initialized);
        }
        Err(e) => {
          let _ = self.event_tx.send(PlayerEvent::InitializationFailed {
            message: e.to_string(),
          });
        }
      },
      PlayerCommand::Load {
        uri,
        start_playing,
        position_ms,
      } => {
        eprintln!("Debug: Loading track: {}", uri);
        if let Err(e) = self.load_track(&uri, start_playing, position_ms).await {
          eprintln!("Debug: Failed to load track: {}", e);
          let _ = self.event_tx.send(PlayerEvent::Error {
            message: format!("Failed to load track: {}", e),
          });
        }
      }
      PlayerCommand::Play => {
        if let Some(ref player) = self.player {
          player.play();
        }
      }
      PlayerCommand::Pause => {
        if let Some(ref player) = self.player {
          player.pause();
        }
      }
      PlayerCommand::Stop => {
        if let Some(ref player) = self.player {
          player.stop();
        }
        self.current_track_uri = None;
      }
      PlayerCommand::Seek(position_ms) => {
        if let Some(ref player) = self.player {
          player.seek(position_ms);
        }
      }
      PlayerCommand::SetVolume(volume) => {
        // Volume control would be handled through mixer
        self.current_volume = volume;
        let _ = self.event_tx.send(PlayerEvent::VolumeChanged { volume });
      }
      PlayerCommand::Preload(uri) => {
        self.preload_track(&uri).await?;
      }
      PlayerCommand::Shutdown => {
        if let Some(ref player) = self.player {
          player.stop();
        }
        return Ok(true);
      }
    }
    Ok(false)
  }

  async fn handle_player_event(&self, event: LibrespotPlayerEvent) {
    let player_event = match event {
      LibrespotPlayerEvent::Playing { position_ms, .. } => Some(PlayerEvent::Playing {
        track_uri: self.current_track_uri.clone().unwrap_or_default(),
        position_ms,
        duration_ms: 0, // Duration will be updated separately
      }),
      LibrespotPlayerEvent::Paused { position_ms, .. } => Some(PlayerEvent::Paused {
        track_uri: self.current_track_uri.clone().unwrap_or_default(),
        position_ms,
      }),
      LibrespotPlayerEvent::Stopped { .. } => Some(PlayerEvent::Stopped),
      LibrespotPlayerEvent::EndOfTrack { .. } => Some(PlayerEvent::TrackEnded {
        track_uri: self.current_track_uri.clone().unwrap_or_default(),
      }),
      LibrespotPlayerEvent::TimeToPreloadNextTrack { .. } => {
        Some(PlayerEvent::TimeToPreloadNextTrack)
      }
      LibrespotPlayerEvent::Loading { track_id, .. } => {
        eprintln!("Debug: Loading track_id: {:?}", track_id);
        Some(PlayerEvent::Loading {
          track_uri: self.current_track_uri.clone().unwrap_or_default(),
        })
      }
      LibrespotPlayerEvent::Unavailable { track_id, .. } => {
        eprintln!(
          "Debug: Track unavailable from librespot: {:?}. This usually means the account lacks playback rights for this track/region or the audio key request failed.",
          track_id
        );
        Some(PlayerEvent::Error {
          message: format!("Track unavailable: {:?}", track_id),
        })
      }
      other => {
        eprintln!("Debug: Unhandled player event: {:?}", other);
        None
      }
    };

    if let Some(event) = player_event {
      let _ = self.event_tx.send(event);
    }
  }

  async fn load_track(&mut self, uri: &str, start_playing: bool, position_ms: u32) -> Result<()> {
    eprintln!("Debug: Parsing SpotifyId from URI: {}", uri);
    let track_id = SpotifyId::from_uri(uri).map_err(|e| {
      eprintln!("Debug: SpotifyId parse error: {:?}", e);
      anyhow!("Invalid Spotify URI '{}': {:?}", uri, e)
    })?;
    eprintln!("Debug: SpotifyId parsed successfully: {:?}", track_id);

    if let Some(ref player) = self.player {
      self.current_track_uri = Some(uri.to_string());
      eprintln!(
        "Debug: Calling player.load(track_id={:?}, start={}, pos={})",
        track_id, start_playing, position_ms
      );
      player.load(track_id, start_playing, position_ms);
      eprintln!("Debug: player.load() called successfully");
    } else {
      eprintln!("Debug: Player is None, cannot load track");
      return Err(anyhow!("Player not initialized"));
    }

    Ok(())
  }

  async fn preload_track(&mut self, uri: &str) -> Result<()> {
    let track_id = SpotifyId::from_uri(uri)?;

    if let Some(ref player) = self.player {
      player.preload(track_id);
    }

    Ok(())
  }
}

/// Spawn the player worker in a new task
/// Returns channels for communication with the worker
pub fn spawn_player_worker(
  config: PlayerWorkerConfig,
) -> (mpsc::Sender<PlayerCommand>, mpsc::Receiver<PlayerEvent>) {
  let (cmd_tx, cmd_rx) = mpsc::channel();
  let (event_tx, event_rx) = mpsc::channel();

  let mut worker = PlayerWorker::new(cmd_rx, event_tx, config);

  std::thread::spawn(move || {
    let rt = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Failed to create tokio runtime for player worker");

    rt.block_on(async move {
      if let Err(e) = worker.run().await {
        eprintln!("Player worker error: {}", e);
      }
    });
  });

  (cmd_tx, event_rx)
}
