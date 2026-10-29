//! Windows groups every device node of one physical product under a container
//! id. An endpoint whose container also holds a Bluetooth-enumerated node is a
//! Bluetooth device, even when the audio itself is offloaded to another driver
//! (Intel Smart Sound reports such headsets under `INTELAUDIO`).

use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_Get_DevNode_PropertyW, CM_Get_Device_ID_ListW, CM_Get_Device_ID_List_SizeW,
    CM_Locate_DevNodeW, CM_GETIDLIST_FILTER_ENUMERATOR, CM_GETIDLIST_FILTER_PRESENT,
    CM_LOCATE_DEVNODE_NORMAL, CR_SUCCESS,
};
use windows::Win32::Devices::Properties::{
    DEVPKEY_Device_ContainerId, DEVPROPTYPE, DEVPROP_TYPE_GUID,
};
use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};

/// The PC itself. Its own Bluetooth radio lives here, so it never marks an endpoint.
const LOCAL_MACHINE: GUID = GUID::from_u128(0x0000_0000_0000_0000_ffff_ffff_ffff_ffff);
/// Classic Bluetooth, LE devices, and LE services.
const BLUETOOTH_ENUMERATORS: [&str; 3] = ["BTHENUM", "BTHLE", "BTHLEDEVICE"];

pub fn default_output_is_bluetooth() -> Option<bool> {
    let _com = ComScope::enter();
    let endpoint = default_endpoint_id()?;
    let container = container_of(&format!(r"SWD\MMDEVAPI\{endpoint}"))?;
    if container == LOCAL_MACHINE {
        return Some(false);
    }
    Some(
        BLUETOOTH_ENUMERATORS
            .iter()
            .flat_map(|enumerator| present_devices(enumerator))
            .any(|id| container_of(&id) == Some(container)),
    )
}

fn default_endpoint_id() -> Option<String> {
    // SAFETY: COM is initialized on this thread by `ComScope`, and the id
    // string is freed with the allocator that produced it.
    unsafe {
        let devices: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let raw = devices
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .ok()?
            .GetId()
            .ok()?;
        let id = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0.cast()));
        id
    }
}

fn container_of(instance_id: &str) -> Option<GUID> {
    let id = wide(instance_id);
    let mut node = 0;
    let mut kind = DEVPROPTYPE::default();
    let mut container = GUID::zeroed();
    let mut size = u32::try_from(size_of::<GUID>()).ok()?;
    // SAFETY: `id` is NUL-terminated and outlives both calls; the buffer is a
    // GUID and `size` is its exact length.
    let found = unsafe {
        CM_Locate_DevNodeW(&raw mut node, PCWSTR(id.as_ptr()), CM_LOCATE_DEVNODE_NORMAL)
            == CR_SUCCESS
            && CM_Get_DevNode_PropertyW(
                node,
                &DEVPKEY_Device_ContainerId,
                &raw mut kind,
                Some((&raw mut container).cast()),
                &raw mut size,
                0,
            ) == CR_SUCCESS
    };
    (found && kind == DEVPROP_TYPE_GUID).then_some(container)
}

/// Instance ids of the present devices one enumerator owns.
fn present_devices(enumerator: &str) -> Vec<String> {
    let filter = wide(enumerator);
    let flags = CM_GETIDLIST_FILTER_ENUMERATOR | CM_GETIDLIST_FILTER_PRESENT;
    let mut len = 0;
    // SAFETY: `filter` is NUL-terminated and outlives both calls; the list is
    // written into a buffer of the length the first call reported.
    unsafe {
        if CM_Get_Device_ID_List_SizeW(&raw mut len, PCWSTR(filter.as_ptr()), flags) != CR_SUCCESS {
            return Vec::new();
        }
        let mut list = vec![0u16; len as usize];
        if CM_Get_Device_ID_ListW(PCWSTR(filter.as_ptr()), &mut list, flags) != CR_SUCCESS {
            return Vec::new();
        }
        list.split(|&c| c == 0)
            .filter(|id| !id.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Balances a successful `CoInitializeEx`. A thread already in another
/// apartment (cpal's audio threads are) keeps it and is not uninitialized.
struct ComScope(bool);

impl ComScope {
    fn enter() -> Self {
        // SAFETY: paired with `CoUninitialize` in `Drop` only when it succeeded.
        Self(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for ComScope {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful `CoInitializeEx` in `enter`.
            unsafe { CoUninitialize() };
        }
    }
}
