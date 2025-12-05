# Librespot Integration Plan for Spotatui

This document outlines the plan to integrate librespot directly into spotatui, enabling local audio playback without requiring an external Spotify Connect device.

## Goals

1. **Local Playback**: Play Spotify audio directly through the local machine
2. **Hybrid Mode**: Keep existing Spotify Connect device control alongside local playback
3. **Seamless UX**: User can switch between local and remote playback modes
4. **Maintain Compatibility**: Existing functionality should continue to work

## Architecture Overview

### Current Architecture
```
┌─────────────┐      ┌─────────────────┐      ┌─────────────────┐
│  UI Thread  │ ──── │  Network Thread │ ──── │  Spotify Web    │
│  (ratatui)  │      │  (IoEvent)      │      │  API (rspotify) │
└─────────────┘      └─────────────────┘      └─────────────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │ External Device │
                     │ (spotifyd/phone)│
                     └─────────────────┘
```

### Proposed Architecture
```
┌─────────────┐      ┌─────────────────┐      ┌─────────────────┐
│  UI Thread  │ ──── │  Network Thread │ ──── │  Spotify Web    │
│  (ratatui)  │      │  (IoEvent)      │      │  API (rspotify) │
└─────────────┘      └─────────────────┘      └─────────────────┘
                              │
                    ┌─────────┴─────────┐
                    ▼                   ▼
           ┌─────────────────┐  ┌─────────────────┐
           │ Librespot       │  │ External Device │
           │ Player Thread   │  │ (spotifyd/phone)│
           │ (local audio)   │  │                 │
           └─────────────────┘  └─────────────────┘
```

## Dependencies to Add

```toml
[dependencies]
# Core librespot crates
librespot-core = "0.8"        # Session, authentication
librespot-playback = "0.8"    # Player, mixer, audio backends
librespot-oauth = "0.8"       # OAuth2 authentication

[features]
default = ["librespot"]
librespot = ["librespot-core", "librespot-playback", "librespot-oauth"]
```

## Implementation Phases

### Phase 1: Foundation (Estimated: 2-3 days)

#### 1.1 Add Dependencies
- [ ] Add librespot crates to `Cargo.toml`
- [ ] Create feature flag `librespot` (enabled by default)
- [ ] Test compilation on Windows/Linux/macOS

#### 1.2 Create Player Module
- [ ] Create `src/player/mod.rs` - Module root
- [ ] Create `src/player/worker.rs` - Background worker thread
- [ ] Create `src/player/commands.rs` - Command enum for worker communication
- [ ] Create `src/player/events.rs` - Player event types

#### 1.3 Authentication Integration
- [ ] Modify `src/config.rs` to store librespot credentials
- [ ] Create OAuth2 flow that works for both rspotify and librespot
- [ ] Share authentication tokens between rspotify and librespot sessions

### Phase 2: Core Playback (Estimated: 3-4 days)

#### 2.1 Worker Thread Implementation
```rust
// src/player/worker.rs
pub struct PlayerWorker {
    session: Session,
    player: Arc<Player>,
    mixer: Arc<dyn Mixer>,
    commands: mpsc::Receiver<PlayerCommand>,
    events_tx: mpsc::Sender<PlayerEvent>,
}

pub enum PlayerCommand {
    Load { uri: String, start_playing: bool, position_ms: u32 },
    Play,
    Pause,
    Stop,
    Seek(u32),
    SetVolume(u16),
    Preload(String),
    Shutdown,
}
```

#### 2.2 Integrate with App State
- [ ] Add `LocalPlayer` field to `App` struct
- [ ] Add `PlaybackMode` enum: `Local` | `Remote(DeviceId)`
- [ ] Modify `App::dispatch()` to route playback commands appropriately

#### 2.3 IoEvent Extensions
```rust
// Add to network.rs IoEvent enum
pub enum IoEvent {
    // ... existing variants ...
    
    // Local playback
    LocalPlay,
    LocalPause,
    LocalSeek(u32),
    LocalSetVolume(u16),
    LocalLoad { uri: String, start_playing: bool },
    SwitchToLocalPlayback,
    SwitchToRemotePlayback(String), // device_id
}
```

### Phase 3: UI Integration (Estimated: 2-3 days)

#### 3.1 Device Selection Enhancement
- [ ] Add "This Device (spotatui)" as first option in device list
- [ ] Visual indicator for current playback mode (local vs remote)
- [ ] Modify `handlers/select_device.rs` to handle local device selection

#### 3.2 Playbar Updates
- [ ] Show local playback progress from librespot events
- [ ] Volume control routes to local mixer when in local mode
- [ ] Progress bar updates from player events (not API polling)

#### 3.3 Playback Handler Modifications
- [ ] `handlers/playbar.rs` - Route play/pause/seek to local player
- [ ] `handlers/track_table.rs` - Play tracks via local player
- [ ] `handlers/playlist.rs` - Queue management for local playback

### Phase 4: Queue Management (Estimated: 2-3 days)

#### 4.1 Local Queue
- [ ] Create `src/player/queue.rs` - Local playback queue
- [ ] Track preloading (librespot's `TimeToPreloadNextTrack` event)
- [ ] Shuffle/repeat mode for local playback

#### 4.2 Sync with Remote
- [ ] Option to transfer queue when switching modes
- [ ] Maintain position when switching playback modes

### Phase 5: Polish & Edge Cases (Estimated: 2-3 days)

#### 5.1 Error Handling
- [ ] Handle session disconnection/reconnection
- [ ] Audio device unavailable errors
- [ ] Network interruption during playback

#### 5.2 Configuration
```yaml
# config.yml additions
playback:
  mode: local          # local | remote | auto
  audio_backend: rodio # rodio | alsa | pulseaudio | etc.
  audio_device: null   # specific device name or null for default
  bitrate: 320         # 96 | 160 | 320
  normalize_volume: true
  cache_audio: true
  cache_size_mb: 1024
```

#### 5.3 Testing
- [ ] Test on Windows (Rodio backend)
- [ ] Test on Linux (ALSA/PulseAudio)
- [ ] Test on macOS (CoreAudio/Rodio)
- [ ] Test mode switching during playback
- [ ] Test with poor network conditions

## File Structure

```
src/
├── player/
│   ├── mod.rs           # Module exports, LocalPlayer struct
│   ├── worker.rs        # Background thread running librespot
│   ├── commands.rs      # PlayerCommand enum
│   ├── events.rs        # PlayerEvent enum  
│   ├── queue.rs         # Local playback queue
│   └── audio_backend.rs # Backend selection logic
├── app.rs               # Add LocalPlayer, PlaybackMode
├── network.rs           # Add local playback IoEvents
├── config.rs            # Add playback config section
└── handlers/
    ├── playbar.rs       # Route to local/remote
    └── select_device.rs # Add local device option
```

## Key Code Snippets

### Session Creation (from ncspot)
```rust
use librespot_core::session::Session;
use librespot_core::authentication::Credentials;
use librespot_core::cache::Cache;

async fn create_session(credentials: Credentials) -> Result<Session, Error> {
    let session_config = SessionConfig {
        client_id: SPOTIFY_CLIENT_ID.to_string(),
        ..Default::default()
    };
    
    let cache = Cache::new(
        Some(cache_path("librespot")),
        Some(cache_path("librespot/volume")),
        Some(cache_path("librespot/files")),
        Some(1024 * 1024 * 1024), // 1GB
    )?;
    
    let session = Session::new(session_config, Some(cache));
    session.connect(credentials, true).await?;
    Ok(session)
}
```

### Player Creation
```rust
use librespot_playback::player::Player;
use librespot_playback::audio_backend;
use librespot_playback::mixer::{softmixer::SoftMixer, MixerConfig};

fn create_player(session: Session) -> (Arc<Player>, Arc<dyn Mixer>) {
    let backend = audio_backend::find(None).expect("No audio backend");
    
    let mixer = SoftMixer::open(MixerConfig::default());
    
    let player_config = PlayerConfig {
        bitrate: Bitrate::Bitrate320,
        normalisation: true,
        ..Default::default()
    };
    
    let player = Player::new(
        player_config,
        session,
        mixer.get_soft_volume(),
        move || (backend)(None, AudioFormat::default()),
    );
    
    (Arc::new(player), Arc::new(mixer))
}
```

### Event Loop
```rust
async fn player_event_loop(
    player: Arc<Player>,
    events_tx: mpsc::Sender<PlayerEvent>,
) {
    let mut channel = player.get_player_event_channel();
    
    while let Some(event) = channel.recv().await {
        match event {
            LibrespotPlayerEvent::Playing { position_ms, .. } => {
                events_tx.send(PlayerEvent::Playing(position_ms)).await;
            }
            LibrespotPlayerEvent::Paused { position_ms, .. } => {
                events_tx.send(PlayerEvent::Paused(position_ms)).await;
            }
            LibrespotPlayerEvent::EndOfTrack { .. } => {
                events_tx.send(PlayerEvent::TrackEnded).await;
            }
            LibrespotPlayerEvent::TimeToPreloadNextTrack { .. } => {
                events_tx.send(PlayerEvent::PreloadNext).await;
            }
            _ => {}
        }
    }
}
```

## Risks & Mitigations

| Risk                           | Impact | Mitigation                                    |
| ------------------------------ | ------ | --------------------------------------------- |
| Binary size increase           | Medium | Feature flag to exclude librespot             |
| Platform-specific audio issues | High   | Test on all platforms, provide backend config |
| Authentication complexity      | Medium | Share OAuth tokens, clear error messages      |
| Librespot breaking changes     | Low    | Pin version, monitor releases                 |
| Spotify ToS concerns           | Medium | Same risk as spotifyd (widely used)           |

## Success Criteria

1. ✅ Can play tracks locally without external device
2. ✅ Seamless switching between local and remote playback
3. ✅ Volume, seek, pause/play work in local mode
4. ✅ Queue/playlist playback works in local mode
5. ✅ Audio caching reduces bandwidth usage
6. ✅ Works on Windows, Linux, macOS
7. ✅ Existing remote playback functionality unchanged

## Timeline Estimate

| Phase                     | Duration | Dependencies |
| ------------------------- | -------- | ------------ |
| Phase 1: Foundation       | 2-3 days | None         |
| Phase 2: Core Playback    | 3-4 days | Phase 1      |
| Phase 3: UI Integration   | 2-3 days | Phase 2      |
| Phase 4: Queue Management | 2-3 days | Phase 3      |
| Phase 5: Polish           | 2-3 days | Phase 4      |

**Total: ~12-16 days of development**

## Next Steps

1. Start with Phase 1.1 - Add dependencies and verify compilation
2. Create the player module skeleton
3. Implement basic session creation and test connection
4. Build incrementally from there

---

*Last updated: December 5, 2025*
