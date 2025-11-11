# Migration Notes for spotify-tui to rspotify 0.13

## Key Changes in rspotify 0.13

### Module Structure Changes
- `rspotify::client::Spotify` → `rspotify::ClientCredsSpotify` or `rspotify::AuthCodeSpotify`
- `rspotify::oauth2` → `rspotify::OAuth` (and related structs renamed)
- `rspotify::senum` → `rspotify::model::enums`

### Type Renames
- `CurrentlyPlaybackContext` → `CurrentPlaybackContext`
- `PlayingItem` → `PlayableItem`
- `PlaylistTrack` → `PlaylistItem`
- `for_position()` → Use `Offset::Position(u32)` instead

### OAuth Changes
- `SpotifyOAuth` → `OAuth`
- `SpotifyClientCredentials` → Use `AuthCodeSpotify` with token
- `TokenInfo` → `Token`
- `process_token()` and `request_token()` are no longer available
- OAuth is now more integrated with the client

### API Method Changes  
- Many API methods now return `Result<T, ClientError>` instead of `Result<T, Error>`
- Some method signatures changed (e.g., parameters order, types)
- Need to use `.auto_reauth()` builder pattern for automatic token refresh

## Implementation Strategy

Since this is for personal use and the changes are extensive:

1. Use rspotify 0.11-0.12 as intermediate version (less breaking changes)
2. Or do a full rewrite using rspotify 0.13 with async/await throughout
3. Key files to update:
   - src/network.rs (main API wrapper)
   - src/main.rs (OAuth flow)
   - src/redirect_uri.rs (OAuth callback)
   - All files with model imports

## Recommended Approach

For quickest fix to get it working:
- Update to rspotify 0.12 which has fewer breaking changes
- Or use a fork that already updated to newer rspotify versions
