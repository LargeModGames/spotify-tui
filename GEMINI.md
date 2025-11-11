# Spotify-TUI Modernization Project

## Project Overview

**spotify-tui** is a Spotify client for the terminal written in Rust. It provides a full-featured TUI (Terminal User Interface) for controlling Spotify playback, browsing libraries, searching music, and managing playlists.

### Current Status
- **Original Version**: 0.25.0 (Last updated ~2 years ago)
- **Main Issue**: Outdated dependencies causing backend API failures
- **Goal**: Update dependencies and fix breaking API changes for personal use

### Tech Stack
- **Language**: Rust (Edition 2018)
- **UI Library**: Originally `tui-rs`, migrating to `ratatui`
- **Spotify API**: Originally `rspotify 0.10.0`, migrating to `0.12.x`
- **Terminal**: `crossterm`
- **Async Runtime**: `tokio`

### Key Features
- Browse and play Spotify playlists
- Search for tracks, albums, artists, and podcasts
- Control playback (play/pause, skip, seek, volume)
- View saved tracks, albums, and followed artists
- Audio analysis visualization
- Device selection
- CLI interface alongside TUI

---

## Migration Strategy

### Dependency Updates Required

| Dependency   | Original | Target           | Reason                                       |
| ------------ | -------- | ---------------- | -------------------------------------------- |
| `rspotify`   | 0.10.0   | 0.12.x           | Spotify API wrapper (major breaking changes) |
| `tui`        | 0.16.0   | N/A (deprecated) | Renamed to `ratatui`                         |
| `ratatui`    | N/A      | 0.26.x           | Successor to `tui-rs`                        |
| `tokio`      | 0.2      | 1.40.x           | Async runtime (major version upgrade)        |
| `crossterm`  | 0.20     | 0.27.x           | Terminal manipulation                        |
| `arboard`    | 1.2.0    | 3.4.x            | Clipboard support                            |
| `dirs`       | 3.0.2    | 5.0.x            | Directory utilities                          |
| `serde_yaml` | 0.8      | 0.9.x            | YAML parsing                                 |

### Breaking Changes in rspotify 0.10 → 0.12

#### Module Structure
- `rspotify::client::Spotify` → `rspotify::AuthCodeSpotify`
- `rspotify::oauth2` → `rspotify::OAuth` + `rspotify::Credentials`
- `rspotify::senum` → `rspotify::model::enums`

#### Type Renames
- `CurrentlyPlaybackContext` → `CurrentPlaybackContext`
- `PlayingItem` → `PlayableItem`
- `PlaylistTrack` → `PlaylistItem`
- `TokenInfo` → `Token`
- `SpotifyOAuth` → `OAuth`
- `SpotifyClientCredentials` → (integrated into client)

#### API Changes
- `for_position(u32)` → `Offset::Position(u32)`
- Track/Artist/Album IDs changed from `String` to typed IDs (`TrackId`, `ArtistId`, etc.)
- OAuth flow completely redesigned
- `util::get_token()`, `util::process_token()`, `util::request_token()` removed
- Many API methods have new signatures

#### Tokio Changes
- `tokio::time::delay_for()` → `tokio::time::sleep()`

---

## Changes Completed ✅

### Dependency Updates
- ✅ Updated `Cargo.toml` with modern dependency versions
- ✅ Changed `tui` to `ratatui` in dependencies
- ✅ Updated `rspotify` to 0.12 with required features (`cli`, `env-file`, `client-reqwest`)
- ✅ Updated `tokio` to 1.40
- ✅ Updated `crossterm` to 0.27
- ✅ Updated `arboard` to 3.4
- ✅ Updated `dirs` to 5.0
- ✅ Updated `serde_yaml` to 0.9

### Global Type Renames (All `.rs` files)
- ✅ Replaced all `use tui::` → `use ratatui::` imports
- ✅ Renamed `CurrentlyPlaybackContext` → `CurrentPlaybackContext`
- ✅ Renamed `PlayingItem` → `PlayableItem`
- ✅ Renamed `PlaylistTrack` → `PlaylistItem`
- ✅ Renamed `senum::` → `model::enums::`

### Import Updates
- ✅ Updated `src/network.rs` imports to use new rspotify structure
  - Added `prelude::*`, `AuthCodeSpotify`, `Token`, `OAuth`, `Credentials`, `Config`
  - Replaced leftover `for_position()` usages with `Offset::Position()`
  - Updated enum imports to use `model::enums::`

### Core Functionality
- ✅ **src/main.rs**: Async bootstrap + OAuth flow fully modernized for rspotify 0.12.
  - ✅ Token cache now handled via `spotify.token.lock().await`, with graceful fallback when the cache file is missing.
  - ✅ `start_tokio` runs inside `tokio::spawn`, so queued `IoEvent`s can `.await` network calls without lifetime hacks.
  - ✅ Manual and web-based auth paths both work, and CLI/UI entry now happens even when no cached token exists.
- ✅ **src/network.rs**: Cleaned up authentication helpers.
  - ✅ Added `use anyhow::anyhow;` to fix macro usage.
  - ✅ `Network` now owns an `Arc<Mutex<App>>`, eliminating the old `'a` lifetime bound.
  - ✅ Corrected `refresh_authentication` to be a proper no-op.
- ✅ **src/ui/**: Updated UI components for `ratatui` 0.26.
  - ✅ Replaced all `Spans` usages with `Line` in `src/ui/audio_analysis.rs` and `src/ui/mod.rs`.
  - ✅ Fixed `Text` value double-move in `src/ui/mod.rs`.
  - ✅ Corrected invalid `millis_to_minutes` call in `src/ui/mod.rs`.
  - ✅ Fixed `segment.start` and `section.start` field access in `src/ui/audio_analysis.rs`.

---

## Work Remaining ❌

### High Priority - Core Functionality

#### Typed Spotify IDs (Network + App)
- ❌ `IoEvent` payloads and most `Network` methods still accept raw `String` IDs (`TrackId`, `AlbumId`, `ArtistId`, `ShowId`, `PlayableId`), causing the E0412 spam seen in `cargo check`.
- ❌ `App` continues to store IDs as `String`s, so comparisons like queue lookups fail to compile against the typed IDs exposed by rspotify 0.12.

#### Playback & Queue helpers
- ❌ `start_playback`, queue additions, and recommendation helpers still build `String` URIs and offsets. They must switch to `PlayableId::from_uri`, `Offset::Position`, and propagate typed IDs through the `IoEvent` variants.

#### UI Typed-ID + Duration conversions
- ❌ `src/ui/mod.rs` expects `String` IDs and `std::time::Duration`. Need `.to_string()` conversions (or typed storage) for album/playlist/episode IDs, and convert `chrono::TimeDelta` via `.num_milliseconds()`/`.num_seconds()`.
- ❌ `ResumePoint` now exposes `resume_position`; the UI still references `resume_position_ms`.

#### Tokio Updates
- ✅ `tokio::time::delay_for()` has been fully removed; remaining async waits use `tokio::time::sleep`.

### Medium Priority - Type Conversions

#### ID Type Conversions
- ❌ Fix `TrackId<'_>` to `String` conversions throughout codebase
- ❌ Fix `ArtistId<'_>` to `String` conversions
- ❌ Fix `AlbumId<'_>` to `String` conversions
- ❌ Update all code that stores/compares IDs as Strings
- ❌ Handle lifetime parameters in ID types

#### Model Field Access
- ❌ Update `PlaylistItem` field access (fields changed from `track` to different structure)
- ❌ Review and fix `PlayableItem` enum matching
- ❌ Update any code accessing changed model fields

### Low Priority - Additional Updates

#### CLI Module
- ❌ **src/cli/*.rs**: Review and test CLI functionality with new API
- ❌ Verify command-line interface still works correctly

#### Error Handling
- ❌ Update error handling for new rspotify error types
- ❌ Test error scenarios and ensure proper user feedback

#### Testing & Validation
- ❌ Test OAuth flow end-to-end
- ❌ Test playback controls
- ❌ Test library browsing
- ❌ Test search functionality
- ❌ Test device selection
- ❌ Test CLI commands
- ❌ Verify audio analysis feature
- ❌ Test with actual Spotify account

---

## Known Issues & Blockers

### Compilation Errors (Current)
- `src/network.rs` still emits hundreds of E0412/E0308 errors because the `IoEvent` APIs accept `String` IDs while rspotify now requires typed IDs (`TrackId`, `AlbumId`, `ArtistId`, `ShowId`, `PlayableId`). Imports + conversions are missing.
- `src/ui/mod.rs` expects `String` IDs and `std::time::Duration`, so any access to playlist/album/episode IDs or `ResumePoint::resume_position_ms` fails to compile.
- Recommendation/start-playback helpers still mix `String` URIs with `PlayableId::from_uri`, leaving the queue + autoplay pipeline broken.

### Design Decisions Needed
1. Do we store typed IDs (`TrackId`, `AlbumId`, …) inside `App`/UI state, or do we continue storing Strings and convert at the rspotify call sites?
2. How strict should we be about propagating typed IDs through every `IoEvent` vs. introducing helper conversion functions?
3. Are we keeping the `redirect_uri_web_server` helper even though it only needs the port (current signature still warns about unused `spotify`)?

---

## File-by-File Status

### Core Files
| File                  | Status          | Notes                                |
| --------------------- | --------------- | ------------------------------------ |
| `Cargo.toml`          | ✅ Updated       | Dependencies modernized              |
| `src/main.rs`         | ✅ Updated       | Async bootstrap, token cache handling, and UI/CLI dispatch now compile + run. |
| `src/network.rs`      | 🔶 Partial       | Owns `Arc<Mutex<App>>`, but IoEvents still use `String` IDs and old playback helpers. |
| `src/redirect_uri.rs` | ✅ Updated       | Callback helper converted; unused `spotify` arg is the only warning. |
| `src/config.rs`       | ⚠️ Unknown       | May need updates for new OAuth       |
| `src/app.rs`          | ✅ Types updated | Model types renamed                  |

### Handler Files
| File                | Status          | Notes                        |
| ------------------- | --------------- | ---------------------------- |
| `src/handlers/*.rs` | ✅ Types updated | Model types renamed globally |

### UI Files
| File          | Status          | Notes                            |
| ------------- | --------------- | -------------------------------- |
| `src/ui/*.rs` | 🔶 Partial       | `ratatui` updates landed; still need typed-ID + `TimeDelta` conversions (`ResumePoint`, durations). |

### CLI Files
| File           | Status          | Notes                      |
| -------------- | --------------- | -------------------------- |
| `src/cli/*.rs` | ✅ Types updated | Needs testing with new API |

---

## Next Steps

### Immediate Actions (to get it compiling)
1. Convert playlist/track/episode IDs stored in `App`/UI state from `String` to rspotify’s typed IDs (or call `.to_string()` at render boundaries) so comparisons compile.
2. Finish swapping queue/start-playback helpers over to typed IDs in `src/network.rs` and update the affected `IoEvent` variants.
3. Fix the UI duration + `ResumePoint` fields to use `chrono::TimeDelta` (`.num_milliseconds()/.num_seconds()`) and the renamed `resume_position`.

### Short Term (to get it working)
1. Re-test every `Network` API method once it compiles; replace remaining `String` URIs with typed IDs and add proper error propagation/logging.
2. Retest CLI commands now that they share the async client/runtime.
3. Verify token refresh behavior in practice (currently relying on rspotify auto-refresh, `IoEvent::RefreshAuthentication` is effectively redundant).
4. Fill in the new OAuth instructions in documentation/config templates (`client.yml`, README snippets).

### Long Term (for stability)
1. Comprehensive manual testing with a Spotify account.
2. Improve error handling and surface actionable messages to the TUI/CLI.
3. Consider migrating further to rspotify 0.13+ once 0.12 is stable.
4. Keep docs (`AGENTS.md`, `GEMINI.md`, `MIGRATION_NOTES.md`) updated as new fixes land.

---

## Resources

- [rspotify 0.12 Documentation](https://docs.rs/rspotify/0.12)
- [rspotify Migration Guide](https://github.com/ramsayleung/rspotify/blob/master/CHANGELOG.md)
- [ratatui Documentation](https://docs.rs/ratatui)
- [Tokio 1.x Migration Guide](https://tokio.rs/tokio/topics/bridging)

---

## Notes for Future Developers

- This is a **personal use** fork, not intended for upstream contribution
- Focus on getting it working rather than perfect code
- The original project is unmaintained, so we own the maintenance burden
- Consider switching to an actively maintained alternative if this becomes too difficult
- Main complexity is in the Spotify OAuth flow - once that works, the rest should follow
- Keep `AGENTS.md` and `GEMINI.md` in sync—if you mark work complete or add context in one, update the other in the same change

---

*Last Updated: 2025-11-11 by Codex*
*Status: Migration In Progress - Compilation Failing*
