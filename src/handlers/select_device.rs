use super::{
  super::app::{ActiveBlock, App},
  common_key_events,
};
use crate::event::Key;
use crate::network::IoEvent;

/// Special device ID for local playback via librespot
#[cfg(feature = "librespot")]
pub const LOCAL_DEVICE_ID: &str = "__LOCAL_DEVICE__";

/// Get the effective number of devices (including local device if librespot is enabled)
fn get_device_count(app: &App) -> usize {
  let remote_count = app.devices.as_ref().map_or(0, |d| d.devices.len());
  #[cfg(feature = "librespot")]
  {
    remote_count + 1 // +1 for local device
  }
  #[cfg(not(feature = "librespot"))]
  {
    remote_count
  }
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    Key::Esc => {
      app.set_current_route_state(Some(ActiveBlock::Library), None);
    }
    k if common_key_events::down_event(k) => {
      let device_count = get_device_count(app);
      if device_count > 0 {
        if let Some(selected_device_index) = app.selected_device_index {
          let next_index = if selected_device_index >= device_count - 1 {
            0
          } else {
            selected_device_index + 1
          };
          app.selected_device_index = Some(next_index);
        }
      }
    }
    k if common_key_events::up_event(k) => {
      let device_count = get_device_count(app);
      if device_count > 0 {
        if let Some(selected_device_index) = app.selected_device_index {
          let next_index = if selected_device_index == 0 {
            device_count - 1
          } else {
            selected_device_index - 1
          };
          app.selected_device_index = Some(next_index);
        }
      }
    }
    k if common_key_events::high_event(k) => {
      if get_device_count(app) > 0 {
        app.selected_device_index = Some(0);
      }
    }
    k if common_key_events::middle_event(k) => {
      let device_count = get_device_count(app);
      if device_count > 0 {
        app.selected_device_index = Some(device_count / 2);
      }
    }
    k if common_key_events::low_event(k) => {
      let device_count = get_device_count(app);
      if device_count > 0 {
        app.selected_device_index = Some(device_count - 1);
      }
    }
    Key::Enter => {
      if let Some(index) = app.selected_device_index {
        #[cfg(feature = "librespot")]
        {
          // Index 0 is "This Device (spotatui)" for local playback
          if index == 0 {
            // Only dispatch SwitchToLocalPlayback - it handles initialization internally
            app.dispatch(IoEvent::SwitchToLocalPlayback);
            return;
          }
          // Adjust index for remote devices (subtract 1 since local is at index 0)
          let remote_index = index - 1;
          if let Some(devices) = &app.devices {
            if let Some(device) = devices.devices.get(remote_index) {
              if let Some(device_id) = &device.id {
                app.dispatch(IoEvent::TransferPlaybackToDevice(device_id.clone()));
              }
            }
          }
        }
        #[cfg(not(feature = "librespot"))]
        {
          if let Some(devices) = &app.devices {
            if let Some(device) = devices.devices.get(index) {
              if let Some(device_id) = &device.id {
                app.dispatch(IoEvent::TransferPlaybackToDevice(device_id.clone()));
              }
            }
          }
        }
      }
    }
    _ => {}
  }
}
