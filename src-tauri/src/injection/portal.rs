//! Typing through xdg-desktop-portal, the sanctioned Wayland input path.
//!
//! Needs no helper binary and no `/dev/uinput` access — the compositor types on
//! our behalf once the user approves. The session is held open for the life of
//! the process, so approval is asked for once rather than per dictation.

use std::sync::Arc;

use ashpd::desktop::remote_desktop::{
    DeviceType, KeyState, NotifyKeyboardKeysymOptions, RemoteDesktop, SelectDevicesOptions,
    StartOptions,
};
use ashpd::desktop::{PersistMode, Session};
use ashpd::enumflags2::BitFlags;
use tokio::sync::{Mutex, OnceCell};

struct PortalSession {
    proxy: RemoteDesktop,
    session: Session<RemoteDesktop>,
}

static SESSION: OnceCell<Option<Arc<PortalSession>>> = OnceCell::const_new();

/// The portal session is not safe to drive concurrently.
static TYPING: Mutex<()> = Mutex::const_new(());

async fn connect() -> Option<Arc<PortalSession>> {
    let proxy = RemoteDesktop::new()
        .await
        .map_err(|e| log::debug!("portal unavailable: {e}"))
        .ok()?;

    #[allow(clippy::default_trait_access)]
    let session = proxy
        .create_session(Default::default())
        .await
        .map_err(|e| log::debug!("portal session failed: {e}"))
        .ok()?;

    proxy
        .select_devices(
            &session,
            SelectDevicesOptions::default()
                .set_devices(BitFlags::from(DeviceType::Keyboard))
                .set_persist_mode(PersistMode::ExplicitlyRevoked),
        )
        .await
        .map_err(|e| log::debug!("portal device selection failed: {e}"))
        .ok()?;

    proxy
        .start(&session, None, StartOptions::default())
        .await
        .map_err(|e| log::debug!("portal start failed: {e}"))
        .ok()?
        .response()
        .map_err(|e| log::debug!("portal permission denied: {e}"))
        .ok()?;

    log::info!("using xdg-desktop-portal for text injection");
    Some(Arc::new(PortalSession { proxy, session }))
}

pub async fn available() -> bool {
    SESSION.get_or_init(connect).await.is_some()
}

/// Sends Ctrl+V. Pairs with a clipboard write, which is far quicker than
/// typing a long transcript one keysym at a time.
///
/// # Errors
/// Returns an error when the portal is unavailable or the chord is rejected.
pub async fn send_paste_chord() -> Result<(), String> {
    const CTRL: i32 = 0xffe3;
    const V: i32 = 0x0076;

    let session = SESSION
        .get_or_init(connect)
        .await
        .clone()
        .ok_or_else(|| "desktop portal unavailable".to_string())?;

    let _guard = TYPING.lock().await;
    for (keysym, state) in [
        (CTRL, KeyState::Pressed),
        (V, KeyState::Pressed),
        (V, KeyState::Released),
        (CTRL, KeyState::Released),
    ] {
        session
            .proxy
            .notify_keyboard_keysym(
                &session.session,
                keysym,
                state,
                NotifyKeyboardKeysymOptions::default(),
            )
            .await
            .map_err(|e| format!("portal paste chord failed: {e}"))?;
    }
    Ok(())
}

/// # Errors
/// Returns an error when the portal is unavailable or a keystroke is rejected.
pub async fn type_text(text: &str) -> Result<(), String> {
    let session = SESSION
        .get_or_init(connect)
        .await
        .clone()
        .ok_or_else(|| "desktop portal unavailable".to_string())?;

    let _guard = TYPING.lock().await;
    for ch in text.chars() {
        let keysym = keysym_for(ch);
        for state in [KeyState::Pressed, KeyState::Released] {
            session
                .proxy
                .notify_keyboard_keysym(
                    &session.session,
                    keysym,
                    state,
                    NotifyKeyboardKeysymOptions::default(),
                )
                .await
                .map_err(|e| format!("portal typing failed: {e}"))?;
        }
    }
    Ok(())
}

/// Latin-1 maps to its own codepoint; above that, the Unicode offset X11 and
/// the portal both understand.
#[allow(clippy::cast_possible_wrap)]
fn keysym_for(ch: char) -> i32 {
    let code = ch as u32;
    if code < 0x100 {
        code as i32
    } else {
        (0x0100_0000 + code) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::keysym_for;

    #[test]
    fn latin1_maps_to_its_codepoint() {
        assert_eq!(keysym_for('A'), 0x41);
        assert_eq!(keysym_for(' '), 0x20);
    }

    #[test]
    fn beyond_latin1_uses_the_unicode_offset() {
        assert_eq!(keysym_for('€'), 0x0100_20AC);
        assert_eq!(keysym_for('क'), 0x0100_0915);
    }
}
