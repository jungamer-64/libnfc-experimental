use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TagInfo {
    pub technology: u32,
    pub handle: u32,
    pub uid: [u8; 32],
    pub uid_length: u32,
    pub protocol: u8,
}

pub trait Backend: Send + Sync {
    fn initialize(&self) -> i32;
    fn deinitialize(&self);
    fn register_callbacks(&self);
    fn deregister_callbacks(&self);
    fn enable_discovery(
        &self,
        technologies_mask: i32,
        reader_mode: i32,
        enable_host_routing: i32,
        restart: i32,
    );
    fn disable_discovery(&self);
    fn transceive(&self, handle: u32, tx: &[u8], rx: &mut [u8], timeout: i32) -> i32;
    fn current_tag_snapshot(&self) -> Option<TagInfo>;
}

fn current_tag_state() -> &'static Mutex<Option<TagInfo>> {
    static CURRENT_TAG: OnceLock<Mutex<Option<TagInfo>>> = OnceLock::new();
    CURRENT_TAG.get_or_init(|| Mutex::new(None))
}

fn clear_current_tag() {
    *current_tag_state().lock().unwrap() = None;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
struct RawTagInfo {
    technology: core::ffi::c_uint,
    handle: core::ffi::c_uint,
    uid: [u8; 32],
    uid_length: core::ffi::c_uint,
    protocol: core::ffi::c_uchar,
}

impl From<RawTagInfo> for TagInfo {
    fn from(value: RawTagInfo) -> Self {
        Self {
            technology: value.technology,
            handle: value.handle,
            uid: value.uid,
            uid_length: value.uid_length,
            protocol: value.protocol,
        }
    }
}

#[repr(C)]
struct TagCallbacks {
    on_tag_arrival: Option<unsafe extern "C" fn(*mut RawTagInfo)>,
    on_tag_departure: Option<unsafe extern "C" fn()>,
}

unsafe extern "C" fn on_tag_arrival(tag: *mut RawTagInfo) {
    if tag.is_null() {
        return;
    }
    let tag = unsafe { tag.read() };
    *current_tag_state().lock().unwrap() = Some(tag.into());
}

unsafe extern "C" fn on_tag_departure() {
    clear_current_tag();
}

static TAG_CALLBACKS: TagCallbacks = TagCallbacks {
    on_tag_arrival: Some(on_tag_arrival),
    on_tag_departure: Some(on_tag_departure),
};

#[cfg(not(test))]
unsafe extern "C" {
    fn nfcManager_doInitialize() -> core::ffi::c_int;
    fn nfcManager_doDeinitialize();
    fn nfcManager_registerTagCallback(callback: *mut TagCallbacks);
    fn nfcManager_deregisterTagCallback();
    fn nfcManager_enableDiscovery(
        technologies_mask: core::ffi::c_int,
        reader_mode: core::ffi::c_int,
        enable_host_routing: core::ffi::c_int,
        restart: core::ffi::c_int,
    );
    fn nfcManager_disableDiscovery();
    fn nfcTag_transceive(
        handle: core::ffi::c_uint,
        tx_buffer: *mut core::ffi::c_uchar,
        tx_len: core::ffi::c_int,
        rx_buffer: *mut core::ffi::c_uchar,
        rx_len: core::ffi::c_int,
        timeout: core::ffi::c_int,
    ) -> core::ffi::c_int;
}

#[cfg(not(test))]
pub struct SystemBackend;

#[cfg(not(test))]
impl Backend for SystemBackend {
    fn initialize(&self) -> i32 {
        unsafe { nfcManager_doInitialize() }
    }

    fn deinitialize(&self) {
        clear_current_tag();
        unsafe { nfcManager_doDeinitialize() };
    }

    fn register_callbacks(&self) {
        clear_current_tag();
        unsafe {
            nfcManager_registerTagCallback(std::ptr::addr_of!(TAG_CALLBACKS).cast_mut());
        }
    }

    fn deregister_callbacks(&self) {
        unsafe { nfcManager_deregisterTagCallback() };
        clear_current_tag();
    }

    fn enable_discovery(
        &self,
        technologies_mask: i32,
        reader_mode: i32,
        enable_host_routing: i32,
        restart: i32,
    ) {
        unsafe {
            nfcManager_enableDiscovery(
                technologies_mask,
                reader_mode,
                enable_host_routing,
                restart,
            );
        }
    }

    fn disable_discovery(&self) {
        unsafe { nfcManager_disableDiscovery() };
    }

    fn transceive(&self, handle: u32, tx: &[u8], rx: &mut [u8], timeout: i32) -> i32 {
        unsafe {
            nfcTag_transceive(
                handle,
                tx.as_ptr().cast_mut(),
                tx.len() as core::ffi::c_int,
                rx.as_mut_ptr(),
                rx.len() as core::ffi::c_int,
                timeout,
            )
        }
    }

    fn current_tag_snapshot(&self) -> Option<TagInfo> {
        *current_tag_state().lock().unwrap()
    }
}
