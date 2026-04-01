use crate::c_api_impl::NFC_BUFSIZE_CONNSTRING;
use crate::ffi_support::{
    as_mut, as_ref, bounded_strlen, c_string_ptr_to_string, copy_bytes_to_c_buffer,
    fixed_c_buffer_to_string,
};
use crate::ffi_types::{
    nfc_barcode_info, nfc_baud_rate, nfc_dep_info, nfc_dep_mode, nfc_felica_info,
    nfc_iso14443a_info, nfc_iso14443b_info, nfc_iso14443b2ct_info, nfc_iso14443b2sr_info,
    nfc_iso14443bi_info, nfc_iso14443biclass_info, nfc_jewel_info, nfc_mode, nfc_modulation,
    nfc_modulation_type, nfc_property, nfc_target, nfc_target_info,
};
use crate::lifecycle::{
    DEVICE_NAME_LENGTH, MAX_USER_DEFINED_DEVICES, NFC_DRIVER_NAME_MAX, nfc_context, nfc_device,
    nfc_driver, nfc_user_defined_device, scan_type_enum,
};
use crate::release_allocated_ptr;
use libc::{c_char, c_int};
use proximate::rust_api as rt;
use std::ffi::CString;
use std::{ptr, slice};

const NFC_EINVARG: c_int = -2;
const NFC_EDEVNOTSUPP: c_int = -3;
const NFC_ENOTSUCHDEV: c_int = -4;
const NFC_EOVFLOW: c_int = -5;
const NFC_ENOTIMPL: c_int = -8;
const NFC_ESOFT: c_int = -80;

pub(crate) fn error_to_status(error: &rt::Error) -> c_int {
    match error {
        rt::Error::InvalidArgument(_) => NFC_EINVARG,
        rt::Error::InvalidEncoding(_) => NFC_EINVARG,
        rt::Error::BufferTooSmall { .. } => NFC_EOVFLOW,
        rt::Error::InvalidConnectionString(_) => NFC_EINVARG,
        rt::Error::DriverNotFound(_) => NFC_ENOTSUCHDEV,
        rt::Error::DriverOpenFailed(_) => NFC_ESOFT,
        rt::Error::UnsupportedOperation(_) => NFC_ENOTIMPL,
        rt::Error::DeviceOperationFailed { code, .. } => *code,
    }
}

struct RustDeviceState {
    handle: Box<dyn rt::OpenedDevice>,
    strerror: CString,
    supported_modulations: Vec<nfc_modulation_type>,
    supported_baud_rates: Vec<nfc_baud_rate>,
}

unsafe fn rust_device_state<'a>(device: *mut nfc_device) -> Option<&'a mut RustDeviceState> {
    let device = unsafe { as_mut(device) }?;
    unsafe { (device.driver_data as *mut RustDeviceState).as_mut() }
}

pub(crate) fn set_device_last_error(device: *mut nfc_device, value: c_int) {
    if let Some(device) = unsafe { as_mut(device) } {
        device.last_error = value;
    }
}

fn sync_bool_property(device: *mut nfc_device, property: rt::Property, value: bool) {
    let Some(device) = (unsafe { as_mut(device) }) else {
        return;
    };

    match property {
        rt::Property::HandleCrc => device.bCrc = value,
        rt::Property::HandleParity => device.bPar = value,
        rt::Property::EasyFraming => device.bEasyFraming = value,
        rt::Property::InfiniteSelect => device.bInfiniteSelect = value,
        rt::Property::AutoIso14443_4 => device.bAutoIso14443_4 = value,
        _ => {}
    }
}

fn sync_property_mirrors(device: *mut nfc_device, handle: &dyn rt::OpenedDevice) {
    for property in [
        rt::Property::HandleCrc,
        rt::Property::HandleParity,
        rt::Property::EasyFraming,
        rt::Property::InfiniteSelect,
        rt::Property::AutoIso14443_4,
    ] {
        if let Some(value) = handle.property_bool_state(property) {
            sync_bool_property(device, property, value);
        }
    }
}

fn driver_error_status(error: &rt::Error) -> c_int {
    match error {
        rt::Error::UnsupportedOperation(_) => NFC_EDEVNOTSUPP,
        _ => error_to_status(error),
    }
}

fn unsupported_driver_status(device: *mut nfc_device) -> c_int {
    set_device_last_error(device, NFC_EDEVNOTSUPP);
    0
}

fn status_from_result(device: *mut nfc_device, result: Result<c_int, rt::Error>) -> c_int {
    match result {
        Ok(status) => {
            set_device_last_error(device, 0);
            status
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

fn count_from_result(device: *mut nfc_device, result: Result<usize, rt::Error>) -> c_int {
    match result {
        Ok(count) => {
            set_device_last_error(device, 0);
            count as c_int
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

fn bool_from_result(device: *mut nfc_device, result: Result<bool, rt::Error>) -> c_int {
    match result {
        Ok(value) => {
            set_device_last_error(device, 0);
            if value { 1 } else { 0 }
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

fn option_target_from_result(
    device: *mut nfc_device,
    target: *mut nfc_target,
    result: Result<Option<rt::Target>, rt::Error>,
) -> c_int {
    match result {
        Ok(Some(runtime_target)) => {
            set_device_last_error(device, 0);
            if !target.is_null() {
                write_target_to_c(&runtime_target, target);
            }
            1
        }
        Ok(None) => {
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

fn copy_device_identity(
    device: *mut nfc_device,
    name: &str,
    connstring: &rt::ConnectionString,
) -> bool {
    let Some(device) = (unsafe { as_mut(device) }) else {
        return false;
    };

    let copied_name = unsafe {
        copy_bytes_to_c_buffer(
            device.name.as_mut_ptr(),
            DEVICE_NAME_LENGTH,
            name.as_bytes(),
        )
    };
    let copied_connstring = unsafe {
        copy_bytes_to_c_buffer(
            device.connstring.as_mut_ptr(),
            NFC_BUFSIZE_CONNSTRING,
            connstring.as_str().as_bytes(),
        )
    };
    copied_name && copied_connstring
}

pub(crate) fn context_from_c(context: *const nfc_context) -> rt::Context {
    let Some(context_ref) = (unsafe { as_ref(context) }) else {
        return rt::Context::default();
    };

    let mut user_defined_devices = Vec::new();
    let count = (context_ref.user_defined_device_count as usize).min(MAX_USER_DEFINED_DEVICES);
    for configured in &context_ref.user_defined_devices[..count] {
        let connstring = fixed_c_buffer_to_string(&configured.connstring);
        if connstring.is_empty() {
            continue;
        }
        let Ok(connstring) = rt::ConnectionString::new(connstring) else {
            continue;
        };
        user_defined_devices.push(rt::UserDefinedDevice {
            name: fixed_c_buffer_to_string(&configured.name),
            connstring,
            optional: configured.optional,
        });
    }

    rt::Context::with_config(rt::ContextConfig {
        allow_autoscan: context_ref.allow_autoscan,
        allow_intrusive_scan: context_ref.allow_intrusive_scan,
        log_level: context_ref.log_level,
        user_defined_devices,
    })
}

pub(crate) fn write_context_to_c(context: &rt::Context, destination: *mut nfc_context) {
    let Some(destination) = (unsafe { as_mut(destination) }) else {
        return;
    };

    destination.allow_autoscan = context.config.allow_autoscan;
    destination.allow_intrusive_scan = context.config.allow_intrusive_scan;
    destination.log_level = context.config.log_level;
    destination.user_defined_device_count = 0;

    for slot in &mut destination.user_defined_devices {
        unsafe { ptr::write_bytes(slot as *mut nfc_user_defined_device, 0, 1) };
    }

    for (index, configured) in context
        .config
        .user_defined_devices
        .iter()
        .take(MAX_USER_DEFINED_DEVICES)
        .enumerate()
    {
        let slot = &mut destination.user_defined_devices[index];
        let _ = unsafe {
            copy_bytes_to_c_buffer(
                slot.name.as_mut_ptr(),
                DEVICE_NAME_LENGTH,
                configured.name.as_bytes(),
            )
        };
        let _ = unsafe {
            copy_bytes_to_c_buffer(
                slot.connstring.as_mut_ptr(),
                NFC_BUFSIZE_CONNSTRING,
                configured.connstring.as_str().as_bytes(),
            )
        };
        slot.optional = configured.optional;
        destination.user_defined_device_count += 1;
    }
}

pub(crate) fn target_from_c(target: *const nfc_target) -> rt::Target {
    let Some(target_ref) = (unsafe { as_ref(target) }) else {
        return rt::Target::new(rt::Modulation {
            modulation_type: rt::ModulationType::Undefined,
            baud_rate: rt::BaudRate::Undefined,
        });
    };

    let modulation =
        modulation_from_c(unsafe { ptr::read_unaligned(ptr::addr_of!(target_ref.nm)) });
    let info_union = unsafe { ptr::read_unaligned(ptr::addr_of!(target_ref.nti)) };
    let info = match modulation.modulation_type {
        rt::ModulationType::Iso14443A => {
            let info = unsafe { info_union.nai };
            rt::TargetInfo::Iso14443A {
                atqa: info.abtAtqa,
                sak: info.btSak,
                uid: info.abtUid[..info.szUidLen.min(info.abtUid.len())].to_vec(),
                ats: info.abtAts[..info.szAtsLen.min(info.abtAts.len())].to_vec(),
            }
        }
        rt::ModulationType::Felica => {
            let info = unsafe { info_union.nfi };
            rt::TargetInfo::Felica {
                len: info.szLen,
                response_code: info.btResCode,
                id: info.abtId,
                pad: info.abtPad,
                system_code: info.abtSysCode,
            }
        }
        rt::ModulationType::Iso14443B => {
            let info = unsafe { info_union.nbi };
            rt::TargetInfo::Iso14443B {
                pupi: info.abtPupi,
                application_data: info.abtApplicationData,
                protocol_info: info.abtProtocolInfo,
                card_identifier: info.ui8CardIdentifier,
            }
        }
        rt::ModulationType::Iso14443Bi => {
            let info = unsafe { info_union.nii };
            rt::TargetInfo::Iso14443Bi {
                div: info.abtDIV,
                version_log: info.btVerLog,
                config: info.btConfig,
                atr: info.abtAtr[..info.szAtrLen.min(info.abtAtr.len())].to_vec(),
            }
        }
        rt::ModulationType::Iso14443BiClass => {
            let info = unsafe { info_union.nhi };
            rt::TargetInfo::Iso14443BiClass { uid: info.abtUID }
        }
        rt::ModulationType::Iso14443B2Sr => {
            let info = unsafe { info_union.nsi };
            rt::TargetInfo::Iso14443B2Sr { uid: info.abtUID }
        }
        rt::ModulationType::Iso14443B2Ct => {
            let info = unsafe { info_union.nci };
            rt::TargetInfo::Iso14443B2Ct {
                uid: info.abtUID,
                product_code: info.btProdCode,
                fabrication_code: info.btFabCode,
            }
        }
        rt::ModulationType::Jewel => {
            let info = unsafe { info_union.nji };
            rt::TargetInfo::Jewel {
                sens_res: info.btSensRes,
                id: info.btId,
            }
        }
        rt::ModulationType::Dep => {
            let info = unsafe { info_union.ndi };
            rt::TargetInfo::Dep(dep_info_from_c(info))
        }
        rt::ModulationType::Barcode => {
            let info = unsafe { info_union.nti };
            rt::TargetInfo::Barcode {
                data: info.abtData[..info.szDataLen.min(info.abtData.len())].to_vec(),
            }
        }
        rt::ModulationType::Undefined => rt::TargetInfo::None,
    };

    rt::Target { modulation, info }
}

pub(crate) fn write_target_to_c(target: &rt::Target, destination: *mut nfc_target) {
    let Some(destination) = (unsafe { as_mut(destination) }) else {
        return;
    };

    let mut raw_target = unsafe { std::mem::zeroed::<nfc_target>() };
    raw_target.nm = modulation_to_c(target.modulation);
    raw_target.nti = target_info_to_c(&target.info);
    unsafe { ptr::write_unaligned(destination, raw_target) };
}

pub(crate) fn property_from_c(property: nfc_property) -> rt::Property {
    match property {
        nfc_property::NP_TIMEOUT_COMMAND => rt::Property::TimeoutCommand,
        nfc_property::NP_TIMEOUT_ATR => rt::Property::TimeoutAtr,
        nfc_property::NP_TIMEOUT_COM => rt::Property::TimeoutCom,
        nfc_property::NP_HANDLE_CRC => rt::Property::HandleCrc,
        nfc_property::NP_HANDLE_PARITY => rt::Property::HandleParity,
        nfc_property::NP_ACTIVATE_FIELD => rt::Property::ActivateField,
        nfc_property::NP_ACTIVATE_CRYPTO1 => rt::Property::ActivateCrypto1,
        nfc_property::NP_INFINITE_SELECT => rt::Property::InfiniteSelect,
        nfc_property::NP_ACCEPT_INVALID_FRAMES => rt::Property::AcceptInvalidFrames,
        nfc_property::NP_ACCEPT_MULTIPLE_FRAMES => rt::Property::AcceptMultipleFrames,
        nfc_property::NP_AUTO_ISO14443_4 => rt::Property::AutoIso14443_4,
        nfc_property::NP_EASY_FRAMING => rt::Property::EasyFraming,
        nfc_property::NP_FORCE_ISO14443_A => rt::Property::ForceIso14443A,
        nfc_property::NP_FORCE_ISO14443_B => rt::Property::ForceIso14443B,
        nfc_property::NP_FORCE_SPEED_106 => rt::Property::ForceSpeed106,
    }
}

pub(crate) fn mode_to_c(mode: rt::Mode) -> nfc_mode {
    match mode {
        rt::Mode::Target => nfc_mode::N_TARGET,
        rt::Mode::Initiator => nfc_mode::N_INITIATOR,
    }
}

pub(crate) fn modulation_from_c(modulation: nfc_modulation) -> rt::Modulation {
    rt::Modulation {
        modulation_type: match unsafe { ptr::addr_of!(modulation.nmt).read_unaligned() } {
            nfc_modulation_type::NMT_UNDEFINED => rt::ModulationType::Undefined,
            nfc_modulation_type::NMT_ISO14443A => rt::ModulationType::Iso14443A,
            nfc_modulation_type::NMT_JEWEL => rt::ModulationType::Jewel,
            nfc_modulation_type::NMT_ISO14443B => rt::ModulationType::Iso14443B,
            nfc_modulation_type::NMT_ISO14443BI => rt::ModulationType::Iso14443Bi,
            nfc_modulation_type::NMT_ISO14443B2SR => rt::ModulationType::Iso14443B2Sr,
            nfc_modulation_type::NMT_ISO14443B2CT => rt::ModulationType::Iso14443B2Ct,
            nfc_modulation_type::NMT_FELICA => rt::ModulationType::Felica,
            nfc_modulation_type::NMT_DEP => rt::ModulationType::Dep,
            nfc_modulation_type::NMT_BARCODE => rt::ModulationType::Barcode,
            nfc_modulation_type::NMT_ISO14443BICLASS => rt::ModulationType::Iso14443BiClass,
        },
        baud_rate: match unsafe { ptr::addr_of!(modulation.nbr).read_unaligned() } {
            nfc_baud_rate::NBR_UNDEFINED => rt::BaudRate::Undefined,
            nfc_baud_rate::NBR_106 => rt::BaudRate::Br106,
            nfc_baud_rate::NBR_212 => rt::BaudRate::Br212,
            nfc_baud_rate::NBR_424 => rt::BaudRate::Br424,
            nfc_baud_rate::NBR_847 => rt::BaudRate::Br847,
        },
    }
}

pub(crate) fn modulation_to_c(modulation: rt::Modulation) -> nfc_modulation {
    nfc_modulation {
        nmt: match modulation.modulation_type {
            rt::ModulationType::Undefined => nfc_modulation_type::NMT_UNDEFINED,
            rt::ModulationType::Iso14443A => nfc_modulation_type::NMT_ISO14443A,
            rt::ModulationType::Jewel => nfc_modulation_type::NMT_JEWEL,
            rt::ModulationType::Iso14443B => nfc_modulation_type::NMT_ISO14443B,
            rt::ModulationType::Iso14443Bi => nfc_modulation_type::NMT_ISO14443BI,
            rt::ModulationType::Iso14443B2Sr => nfc_modulation_type::NMT_ISO14443B2SR,
            rt::ModulationType::Iso14443B2Ct => nfc_modulation_type::NMT_ISO14443B2CT,
            rt::ModulationType::Felica => nfc_modulation_type::NMT_FELICA,
            rt::ModulationType::Dep => nfc_modulation_type::NMT_DEP,
            rt::ModulationType::Barcode => nfc_modulation_type::NMT_BARCODE,
            rt::ModulationType::Iso14443BiClass => nfc_modulation_type::NMT_ISO14443BICLASS,
        },
        nbr: baud_rate_to_c(modulation.baud_rate),
    }
}

pub(crate) fn dep_mode_from_c(mode: nfc_dep_mode) -> rt::DepMode {
    match mode {
        nfc_dep_mode::NDM_UNDEFINED => rt::DepMode::Undefined,
        nfc_dep_mode::NDM_PASSIVE => rt::DepMode::Passive,
        nfc_dep_mode::NDM_ACTIVE => rt::DepMode::Active,
    }
}

pub(crate) fn dep_mode_to_c(mode: rt::DepMode) -> nfc_dep_mode {
    match mode {
        rt::DepMode::Undefined => nfc_dep_mode::NDM_UNDEFINED,
        rt::DepMode::Passive => nfc_dep_mode::NDM_PASSIVE,
        rt::DepMode::Active => nfc_dep_mode::NDM_ACTIVE,
    }
}

pub(crate) fn baud_rate_from_c(rate: nfc_baud_rate) -> rt::BaudRate {
    match rate {
        nfc_baud_rate::NBR_UNDEFINED => rt::BaudRate::Undefined,
        nfc_baud_rate::NBR_106 => rt::BaudRate::Br106,
        nfc_baud_rate::NBR_212 => rt::BaudRate::Br212,
        nfc_baud_rate::NBR_424 => rt::BaudRate::Br424,
        nfc_baud_rate::NBR_847 => rt::BaudRate::Br847,
    }
}

pub(crate) fn baud_rate_to_c(rate: rt::BaudRate) -> nfc_baud_rate {
    match rate {
        rt::BaudRate::Undefined => nfc_baud_rate::NBR_UNDEFINED,
        rt::BaudRate::Br106 => nfc_baud_rate::NBR_106,
        rt::BaudRate::Br212 => nfc_baud_rate::NBR_212,
        rt::BaudRate::Br424 => nfc_baud_rate::NBR_424,
        rt::BaudRate::Br847 => nfc_baud_rate::NBR_847,
    }
}

pub(crate) fn register_external_drivers(
    registry: &mut rt::DriverRegistry,
    snapshot: &[crate::core::DriverHandle],
) {
    for handle in snapshot {
        registry.register_driver(rt::wrap_driver_backend(Box::new(DriverAdapter::new(
            handle.0,
        ))));
    }
}

pub(crate) fn is_rust_shim_device(raw: *mut nfc_device) -> bool {
    unsafe { as_ref(raw) }
        .map(|device| ptr::eq(device.driver, ptr::addr_of!(RUST_DEVICE_SHIM_DRIVER)))
        .unwrap_or(false)
}

pub(crate) fn borrowed_device(raw: *mut nfc_device) -> Box<dyn rt::OpenedDevice> {
    if is_rust_shim_device(raw) {
        return rt::wrap_device_backend(Box::new(RustBorrowedDeviceBackend::new(raw)));
    }
    rt::wrap_device_backend(Box::new(DeviceAdapter::borrowed(raw)))
}

fn bytes_ptr(bytes: &[u8]) -> *const u8 {
    if bytes.is_empty() {
        ptr::null()
    } else {
        bytes.as_ptr()
    }
}

fn bytes_mut_ptr(bytes: &mut [u8]) -> *mut u8 {
    if bytes.is_empty() {
        ptr::null_mut()
    } else {
        bytes.as_mut_ptr()
    }
}

fn optional_bytes_ptr(bytes: Option<&[u8]>) -> *const u8 {
    match bytes {
        Some(value) if !value.is_empty() => value.as_ptr(),
        _ => ptr::null(),
    }
}

fn optional_bytes_mut_ptr(bytes: Option<&mut [u8]>) -> *mut u8 {
    match bytes {
        Some(value) if !value.is_empty() => value.as_mut_ptr(),
        _ => ptr::null_mut(),
    }
}

unsafe fn input_slice<'a>(
    device: *mut nfc_device,
    bytes: *const u8,
    len: usize,
) -> Result<&'a [u8], c_int> {
    if len == 0 {
        return Ok(&[]);
    }
    if bytes.is_null() {
        set_device_last_error(device, NFC_EINVARG);
        return Err(NFC_EINVARG);
    }
    Ok(unsafe { slice::from_raw_parts(bytes, len) })
}

unsafe fn output_slice<'a>(
    device: *mut nfc_device,
    bytes: *mut u8,
    len: usize,
) -> Result<&'a mut [u8], c_int> {
    if len == 0 {
        return Ok(&mut []);
    }
    if bytes.is_null() {
        set_device_last_error(device, NFC_EINVARG);
        return Err(NFC_EINVARG);
    }
    Ok(unsafe { slice::from_raw_parts_mut(bytes, len) })
}

unsafe fn parity_marker<'a>(bytes: *const u8) -> Option<&'a [u8]> {
    if bytes.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts(bytes, 1) })
    }
}

unsafe fn parity_marker_mut<'a>(bytes: *mut u8) -> Option<&'a mut [u8]> {
    if bytes.is_null() {
        None
    } else {
        Some(unsafe { slice::from_raw_parts_mut(bytes, 1) })
    }
}

fn refresh_cached_strerror(state: &mut RustDeviceState) -> *const c_char {
    let message = CString::new(state.handle.strerror())
        .unwrap_or_else(|_| CString::new("invalid strerror").expect("static string is valid"));
    state.strerror = message;
    state.strerror.as_ptr()
}

unsafe extern "C" fn rust_device_close(device: *mut nfc_device) {
    let Some(device_ref) = (unsafe { as_mut(device) }) else {
        return;
    };

    let state_ptr = device_ref.driver_data as *mut RustDeviceState;
    device_ref.driver_data = ptr::null_mut();
    device_ref.driver = ptr::null();
    if !state_ptr.is_null() {
        unsafe {
            drop(Box::from_raw(state_ptr));
        }
    }
    unsafe { release_allocated_ptr(device.cast()) };
}

unsafe extern "C" fn rust_device_strerror(device: *const nfc_device) -> *const c_char {
    let Some(state) = (unsafe { rust_device_state(device.cast_mut()) }) else {
        return ptr::null();
    };
    refresh_cached_strerror(state)
}

unsafe extern "C" fn rust_device_initiator_init(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.initiator_init())
}

unsafe extern "C" fn rust_device_initiator_init_secure_element(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.initiator_init_secure_element())
}

unsafe extern "C" fn rust_device_select_passive_target(
    device: *mut nfc_device,
    modulation: nfc_modulation,
    init_data: *const u8,
    init_data_len: usize,
    target: *mut nfc_target,
) -> c_int {
    let payload = match unsafe { input_slice(device, init_data, init_data_len) } {
        Ok(bytes) if bytes.is_empty() => None,
        Ok(bytes) => Some(bytes),
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    option_target_from_result(
        device,
        target,
        state
            .handle
            .select_passive_target(modulation_from_c(modulation), payload),
    )
}

unsafe extern "C" fn rust_device_poll_target(
    device: *mut nfc_device,
    modulations: *const nfc_modulation,
    modulation_count: usize,
    poll_nr: u8,
    period: u8,
    target: *mut nfc_target,
) -> c_int {
    let modulations = if modulation_count == 0 {
        &[]
    } else if modulations.is_null() {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    } else {
        unsafe { slice::from_raw_parts(modulations, modulation_count) }
    };
    let runtime_modulations: Vec<_> = modulations.iter().copied().map(modulation_from_c).collect();
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    option_target_from_result(
        device,
        target,
        state
            .handle
            .poll_target(&runtime_modulations, poll_nr, period),
    )
}

unsafe extern "C" fn rust_device_select_dep_target(
    device: *mut nfc_device,
    dep_mode: nfc_dep_mode,
    baud_rate: nfc_baud_rate,
    initiator: *const nfc_dep_info,
    target: *mut nfc_target,
    timeout: c_int,
) -> c_int {
    let initiator = if initiator.is_null() {
        None
    } else {
        Some(dep_info_from_c(unsafe { ptr::read_unaligned(initiator) }))
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    option_target_from_result(
        device,
        target,
        state.handle.select_dep_target(
            dep_mode_from_c(dep_mode),
            baud_rate_from_c(baud_rate),
            initiator.as_ref(),
            timeout,
        ),
    )
}

unsafe extern "C" fn rust_device_deselect_target(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.deselect_target().map(|()| 0))
}

unsafe extern "C" fn rust_device_transceive_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(device, state.handle.transceive_bytes(tx, rx, timeout))
}

unsafe extern "C" fn rust_device_transceive_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_bits_len.div_ceil(8)) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let rx_len = tx_bits_len.div_ceil(8).max(1);
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(
        device,
        state.handle.transceive_bits(
            tx,
            tx_bits_len,
            unsafe { parity_marker(tx_parity) },
            rx,
            unsafe { parity_marker_mut(rx_parity) },
        ),
    )
}

unsafe extern "C" fn rust_device_transceive_bytes_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    rx: *mut u8,
    rx_len: usize,
    cycles: *mut u32,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.transceive_bytes_timed(tx, rx) {
        Ok((count, measured_cycles)) => {
            if let Some(cycles) = unsafe { as_mut(cycles) } {
                *cycles = measured_cycles;
            }
            set_device_last_error(device, 0);
            count as c_int
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_transceive_bits_timed(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
    rx: *mut u8,
    rx_parity: *mut u8,
    cycles: *mut u32,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_bits_len.div_ceil(8)) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let rx_len = tx_bits_len.div_ceil(8).max(1);
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.transceive_bits_timed(
        tx,
        tx_bits_len,
        unsafe { parity_marker(tx_parity) },
        rx,
        unsafe { parity_marker_mut(rx_parity) },
    ) {
        Ok((count, measured_cycles)) => {
            if let Some(cycles) = unsafe { as_mut(cycles) } {
                *cycles = measured_cycles;
            }
            set_device_last_error(device, 0);
            count as c_int
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_target_is_present(
    device: *mut nfc_device,
    target: *const nfc_target,
) -> c_int {
    let runtime_target = (!target.is_null()).then(|| target_from_c(target));
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    bool_from_result(
        device,
        state.handle.target_is_present(runtime_target.as_ref()),
    )
}

unsafe extern "C" fn rust_device_target_init(
    device: *mut nfc_device,
    target: *mut nfc_target,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    if target.is_null() {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    }
    let mut runtime_target = target_from_c(target.cast_const());
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.target_init(&mut runtime_target, rx, timeout) {
        Ok(count) => {
            set_device_last_error(device, 0);
            write_target_to_c(&runtime_target, target);
            count as c_int
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_target_send_bytes(
    device: *mut nfc_device,
    tx: *const u8,
    tx_len: usize,
    timeout: c_int,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(device, state.handle.target_send_bytes(tx, timeout))
}

unsafe extern "C" fn rust_device_target_receive_bytes(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    timeout: c_int,
) -> c_int {
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(device, state.handle.target_receive_bytes(rx, timeout))
}

unsafe extern "C" fn rust_device_target_send_bits(
    device: *mut nfc_device,
    tx: *const u8,
    tx_bits_len: usize,
    tx_parity: *const u8,
) -> c_int {
    let tx = match unsafe { input_slice(device, tx, tx_bits_len.div_ceil(8)) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(
        device,
        state
            .handle
            .target_send_bits(tx, tx_bits_len, unsafe { parity_marker(tx_parity) }),
    )
}

unsafe extern "C" fn rust_device_target_receive_bits(
    device: *mut nfc_device,
    rx: *mut u8,
    rx_len: usize,
    rx_parity: *mut u8,
) -> c_int {
    let rx = match unsafe { output_slice(device, rx, rx_len) } {
        Ok(bytes) => bytes,
        Err(status) => return status,
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    count_from_result(
        device,
        state
            .handle
            .target_receive_bits(rx, unsafe { parity_marker_mut(rx_parity) }),
    )
}

unsafe extern "C" fn rust_device_set_property_bool(
    device: *mut nfc_device,
    property: nfc_property,
    enable: bool,
) -> c_int {
    let runtime_property = property_from_c(property);
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.set_property_bool(runtime_property, enable) {
        Ok(()) => {
            let mirrored = state
                .handle
                .property_bool_state(runtime_property)
                .unwrap_or(enable);
            sync_bool_property(device, runtime_property, mirrored);
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_set_property_int(
    device: *mut nfc_device,
    property: nfc_property,
    value: c_int,
) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(
        device,
        state
            .handle
            .set_property_int(property_from_c(property), value)
            .map(|()| 0),
    )
}

unsafe extern "C" fn rust_device_get_supported_modulation(
    device: *mut nfc_device,
    mode: nfc_mode,
    supported: *mut *const nfc_modulation_type,
) -> c_int {
    let Some(supported) = (unsafe { as_mut(supported) }) else {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.supported_modulations(match mode {
        nfc_mode::N_TARGET => rt::Mode::Target,
        nfc_mode::N_INITIATOR => rt::Mode::Initiator,
    }) {
        Ok(values) => {
            state.supported_modulations = values.into_iter().map(modulation_type_to_c).collect();
            state
                .supported_modulations
                .push(nfc_modulation_type::NMT_UNDEFINED);
            *supported = state.supported_modulations.as_ptr();
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_get_supported_baud_rate(
    device: *mut nfc_device,
    mode: nfc_mode,
    modulation_type: nfc_modulation_type,
    supported: *mut *const nfc_baud_rate,
) -> c_int {
    let Some(supported) = (unsafe { as_mut(supported) }) else {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.supported_baud_rates(
        match mode {
            nfc_mode::N_TARGET => rt::Mode::Target,
            nfc_mode::N_INITIATOR => rt::Mode::Initiator,
        },
        modulation_type_from_c(modulation_type),
    ) {
        Ok(values) => {
            state.supported_baud_rates = values.into_iter().map(baud_rate_to_c).collect();
            state
                .supported_baud_rates
                .push(nfc_baud_rate::NBR_UNDEFINED);
            *supported = state.supported_baud_rates.as_ptr();
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_get_information_about(
    device: *mut nfc_device,
    buffer: *mut *mut c_char,
) -> c_int {
    let Some(buffer) = (unsafe { as_mut(buffer) }) else {
        set_device_last_error(device, NFC_EINVARG);
        return NFC_EINVARG;
    };
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    match state.handle.information_about() {
        Ok(message) => {
            let allocation = unsafe { libc::malloc(message.len() + 1) as *mut c_char };
            if allocation.is_null() {
                set_device_last_error(device, NFC_ESOFT);
                return NFC_ESOFT;
            }
            if !unsafe { copy_bytes_to_c_buffer(allocation, message.len() + 1, message.as_bytes()) }
            {
                unsafe { release_allocated_ptr(allocation.cast()) };
                set_device_last_error(device, NFC_ESOFT);
                return NFC_ESOFT;
            }
            *buffer = allocation;
            set_device_last_error(device, 0);
            0
        }
        Err(error) => {
            let status = driver_error_status(&error);
            if status == NFC_EDEVNOTSUPP {
                unsupported_driver_status(device)
            } else {
                set_device_last_error(device, status);
                status
            }
        }
    }
}

unsafe extern "C" fn rust_device_abort_command(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.abort_command().map(|()| 0))
}

unsafe extern "C" fn rust_device_idle(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.idle().map(|()| 0))
}

unsafe extern "C" fn rust_device_powerdown(device: *mut nfc_device) -> c_int {
    let Some(state) = (unsafe { rust_device_state(device) }) else {
        return NFC_EINVARG;
    };
    status_from_result(device, state.handle.powerdown().map(|()| 0))
}

const RUST_DEVICE_DRIVER_NAME: *const c_char =
    b"proximate_rust_shim\0" as *const u8 as *const c_char;

static RUST_DEVICE_SHIM_DRIVER: nfc_driver = nfc_driver {
    name: RUST_DEVICE_DRIVER_NAME,
    scan_type: scan_type_enum::NOT_AVAILABLE,
    scan: None,
    open: None,
    close: Some(rust_device_close),
    strerror: Some(rust_device_strerror),
    initiator_init: Some(rust_device_initiator_init),
    initiator_init_secure_element: Some(rust_device_initiator_init_secure_element),
    initiator_select_passive_target: Some(rust_device_select_passive_target),
    initiator_poll_target: Some(rust_device_poll_target),
    initiator_select_dep_target: Some(rust_device_select_dep_target),
    initiator_deselect_target: Some(rust_device_deselect_target),
    initiator_transceive_bytes: Some(rust_device_transceive_bytes),
    initiator_transceive_bits: Some(rust_device_transceive_bits),
    initiator_transceive_bytes_timed: Some(rust_device_transceive_bytes_timed),
    initiator_transceive_bits_timed: Some(rust_device_transceive_bits_timed),
    initiator_target_is_present: Some(rust_device_target_is_present),
    target_init: Some(rust_device_target_init),
    target_send_bytes: Some(rust_device_target_send_bytes),
    target_receive_bytes: Some(rust_device_target_receive_bytes),
    target_send_bits: Some(rust_device_target_send_bits),
    target_receive_bits: Some(rust_device_target_receive_bits),
    device_set_property_bool: Some(rust_device_set_property_bool),
    device_set_property_int: Some(rust_device_set_property_int),
    get_supported_modulation: Some(rust_device_get_supported_modulation),
    get_supported_baud_rate: Some(rust_device_get_supported_baud_rate),
    device_get_information_about: Some(rust_device_get_information_about),
    abort_command: Some(rust_device_abort_command),
    idle: Some(rust_device_idle),
    powerdown: Some(rust_device_powerdown),
};

pub(crate) fn attach_rust_device(
    device: rt::Device,
    context: *const nfc_context,
) -> Result<*mut nfc_device, rt::Error> {
    let name = device.name().to_string();
    let connstring = device.connstring().clone();
    let handle = device.into_handle();
    let connstring_c =
        CString::new(connstring.as_str()).map_err(|_| rt::Error::InvalidEncoding("connstring"))?;
    let raw = unsafe { crate::lifecycle::nfc_device_new(context, connstring_c.as_ptr()) };
    if raw.is_null() {
        return Err(rt::Error::DriverOpenFailed(connstring.as_str().to_string()));
    }

    let state = Box::new(RustDeviceState {
        strerror: CString::new(handle.strerror())
            .unwrap_or_else(|_| CString::new("invalid strerror").expect("static string is valid")),
        handle,
        supported_modulations: Vec::new(),
        supported_baud_rates: Vec::new(),
    });
    if let Some(device_ref) = unsafe { as_mut(raw) } {
        device_ref.context = context;
        device_ref.driver = ptr::addr_of!(RUST_DEVICE_SHIM_DRIVER);
        device_ref.driver_data = Box::into_raw(state).cast();
    }

    if !copy_device_identity(raw, &name, &connstring) {
        unsafe { rust_device_close(raw) };
        return Err(rt::Error::BufferTooSmall {
            needed: name.len().max(connstring.as_str().len()) + 1,
            available: DEVICE_NAME_LENGTH.min(NFC_BUFSIZE_CONNSTRING),
        });
    }

    if let Some(state) = unsafe { rust_device_state(raw) } {
        sync_property_mirrors(raw, state.handle.as_ref());
        set_device_last_error(raw, state.handle.last_error());
    }

    Ok(raw)
}

struct RustBorrowedDeviceBackend {
    raw: *mut nfc_device,
    name: String,
    connstring: rt::ConnectionString,
}

unsafe impl Send for RustBorrowedDeviceBackend {}

impl RustBorrowedDeviceBackend {
    fn new(raw: *mut nfc_device) -> Self {
        let name = unsafe { as_ref(raw) }
            .map(|device| fixed_c_buffer_to_string(&device.name))
            .unwrap_or_default();
        let connstring_string = unsafe { as_ref(raw) }
            .map(|device| fixed_c_buffer_to_string(&device.connstring))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        let connstring = rt::ConnectionString::new(connstring_string)
            .unwrap_or_else(|_| rt::ConnectionString::new("unknown").expect("valid connstring"));
        Self {
            raw,
            name,
            connstring,
        }
    }

    fn with_handle<R>(
        &mut self,
        f: impl FnOnce(&mut dyn rt::OpenedDevice) -> Result<R, rt::Error>,
    ) -> Result<R, rt::Error> {
        let Some(state) = (unsafe { rust_device_state(self.raw) }) else {
            return Err(rt::Error::DriverNotFound("rust shim".to_string()));
        };
        let result = f(state.handle.as_mut());
        sync_property_mirrors(self.raw, state.handle.as_ref());
        result
    }
}

impl rt::DeviceBackend for RustBorrowedDeviceBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &rt::ConnectionString {
        &self.connstring
    }

    fn last_error(&self) -> i32 {
        unsafe { as_ref(self.raw) }
            .map(|device| device.last_error)
            .unwrap_or(0)
    }

    fn set_last_error(&mut self, value: i32) {
        set_device_last_error(self.raw, value);
    }

    fn unsupported_error_code(&self) -> i32 {
        NFC_EDEVNOTSUPP
    }

    fn strerror_backend(&self) -> Option<String> {
        let state = unsafe { rust_device_state(self.raw) }?;
        Some(state.handle.strerror())
    }

    fn information_about_backend(&mut self) -> Result<String, rt::Error> {
        self.with_handle(|handle| handle.information_about())
    }

    fn set_property_bool_backend(
        &mut self,
        property: rt::Property,
        enable: bool,
    ) -> Result<(), rt::Error> {
        let result = self.with_handle(|handle| handle.set_property_bool(property, enable));
        if result.is_ok() {
            let mirrored = unsafe { rust_device_state(self.raw) }
                .and_then(|state| state.handle.property_bool_state(property))
                .unwrap_or(enable);
            sync_bool_property(self.raw, property, mirrored);
        }
        result
    }

    fn set_property_int_backend(
        &mut self,
        property: rt::Property,
        value: i32,
    ) -> Result<(), rt::Error> {
        self.with_handle(|handle| handle.set_property_int(property, value))
    }

    fn supported_modulations_backend(
        &mut self,
        mode: rt::Mode,
    ) -> Result<Vec<rt::ModulationType>, rt::Error> {
        self.with_handle(|handle| handle.supported_modulations(mode))
    }

    fn supported_baud_rates_backend(
        &mut self,
        mode: rt::Mode,
        modulation_type: rt::ModulationType,
    ) -> Result<Vec<rt::BaudRate>, rt::Error> {
        self.with_handle(|handle| handle.supported_baud_rates(mode, modulation_type))
    }

    fn property_bool_state(&self, property: rt::Property) -> Option<bool> {
        let device = unsafe { as_ref(self.raw) }?;
        Some(match property {
            rt::Property::HandleCrc => device.bCrc,
            rt::Property::HandleParity => device.bPar,
            rt::Property::EasyFraming => device.bEasyFraming,
            rt::Property::InfiniteSelect => device.bInfiniteSelect,
            rt::Property::AutoIso14443_4 => device.bAutoIso14443_4,
            _ => return None,
        })
    }

    fn initiator_init_backend(&mut self) -> Result<i32, rt::Error> {
        self.with_handle(|handle| handle.initiator_init())
    }

    fn initiator_init_secure_element_backend(&mut self) -> Result<i32, rt::Error> {
        self.with_handle(|handle| handle.initiator_init_secure_element())
    }

    fn select_passive_target_backend(
        &mut self,
        nm: rt::Modulation,
        init_data: &[u8],
    ) -> Result<Option<rt::Target>, rt::Error> {
        self.with_handle(|handle| handle.select_passive_target(nm, Some(init_data)))
    }

    fn poll_target_backend(
        &mut self,
        modulations: &[rt::Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<rt::Target>, rt::Error> {
        self.with_handle(|handle| handle.poll_target(modulations, poll_nr, period))
    }

    fn select_dep_target_backend(
        &mut self,
        ndm: rt::DepMode,
        nbr: rt::BaudRate,
        initiator: Option<&rt::DepInfo>,
        timeout: i32,
    ) -> Result<Option<rt::Target>, rt::Error> {
        self.with_handle(|handle| handle.select_dep_target(ndm, nbr, initiator, timeout))
    }

    fn deselect_target_backend(&mut self) -> Result<(), rt::Error> {
        self.with_handle(|handle| handle.deselect_target())
    }

    fn target_is_present_backend(
        &mut self,
        target: Option<&rt::Target>,
    ) -> Result<bool, rt::Error> {
        self.with_handle(|handle| handle.target_is_present(target))
    }

    fn target_init_backend(
        &mut self,
        target: &mut rt::Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        self.with_handle(|handle| handle.target_init(target, rx, timeout))
    }

    fn transceive_bytes_backend(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        self.with_handle(|handle| handle.transceive_bytes(tx, rx, timeout))
    }

    fn transceive_bits_backend(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        self.with_handle(|handle| handle.transceive_bits(tx, tx_bits_len, tx_parity, rx, rx_parity))
    }

    fn transceive_bytes_timed_backend(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), rt::Error> {
        self.with_handle(|handle| handle.transceive_bytes_timed(tx, rx))
    }

    fn transceive_bits_timed_backend(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), rt::Error> {
        self.with_handle(|handle| {
            handle.transceive_bits_timed(tx, tx_bits_len, tx_parity, rx, rx_parity)
        })
    }

    fn target_send_bytes_backend(&mut self, tx: &[u8], timeout: i32) -> Result<usize, rt::Error> {
        self.with_handle(|handle| handle.target_send_bytes(tx, timeout))
    }

    fn target_receive_bytes_backend(
        &mut self,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        self.with_handle(|handle| handle.target_receive_bytes(rx, timeout))
    }

    fn target_send_bits_backend(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, rt::Error> {
        self.with_handle(|handle| handle.target_send_bits(tx, tx_bits_len, tx_parity))
    }

    fn target_receive_bits_backend(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        self.with_handle(|handle| handle.target_receive_bits(rx, rx_parity))
    }

    fn abort_command_backend(&mut self) -> Result<(), rt::Error> {
        self.with_handle(|handle| handle.abort_command())
    }

    fn idle_backend(&mut self) -> Result<(), rt::Error> {
        self.with_handle(|handle| handle.idle())
    }

    fn powerdown_backend(&mut self) -> Result<(), rt::Error> {
        self.with_handle(|handle| handle.powerdown())
    }

    fn pn53x_transceive_backend(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        self.with_handle(|handle| handle.pn53x_transceive(tx, rx, timeout))
    }

    fn pn53x_read_register_backend(&mut self, register: u16) -> Result<u8, rt::Error> {
        self.with_handle(|handle| handle.pn53x_read_register(register))
    }

    fn pn53x_write_register_backend(
        &mut self,
        register: u16,
        symbol_mask: u8,
        value: u8,
    ) -> Result<(), rt::Error> {
        self.with_handle(|handle| handle.pn53x_write_register(register, symbol_mask, value))
    }

    fn pn532_sam_configuration_backend(
        &mut self,
        mode: u8,
        timeout: i32,
    ) -> Result<i32, rt::Error> {
        self.with_handle(|handle| handle.pn532_sam_configuration(mode, timeout))
    }
}

pub(crate) struct DriverAdapter {
    raw: *const nfc_driver,
    name: String,
    scan_type: rt::ScanType,
}

unsafe impl Send for DriverAdapter {}
unsafe impl Sync for DriverAdapter {}

impl DriverAdapter {
    pub(crate) fn new(raw: *const nfc_driver) -> Self {
        let name = unsafe { as_ref(raw) }
            .map(|driver| c_string_ptr_to_string(driver.name, NFC_DRIVER_NAME_MAX))
            .unwrap_or_default();
        let scan_type = unsafe { as_ref(raw) }
            .map(|driver| match driver.scan_type {
                scan_type_enum::NOT_INTRUSIVE => rt::ScanType::NotIntrusive,
                scan_type_enum::INTRUSIVE => rt::ScanType::Intrusive,
                scan_type_enum::NOT_AVAILABLE => rt::ScanType::NotAvailable,
            })
            .unwrap_or(rt::ScanType::NotAvailable);
        Self {
            raw,
            name,
            scan_type,
        }
    }
}

impl rt::DriverBackend for DriverAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn scan_type(&self) -> rt::ScanType {
        self.scan_type
    }

    fn scan_with_capacity(
        &self,
        context: &rt::Context,
        capacity: usize,
    ) -> Result<Vec<rt::ConnectionString>, rt::Error> {
        let Some(driver) = (unsafe { as_ref(self.raw) }) else {
            return Ok(Vec::new());
        };
        let Some(scan) = driver.scan else {
            return Ok(Vec::new());
        };

        let mut raw_context = unsafe { std::mem::zeroed::<nfc_context>() };
        write_context_to_c(context, ptr::addr_of_mut!(raw_context));

        let mut buffer = vec![[0 as c_char; NFC_BUFSIZE_CONNSTRING]; capacity];
        let found = unsafe { scan(ptr::addr_of!(raw_context), buffer.as_mut_ptr(), capacity) };
        let mut result = Vec::new();
        for connstring in buffer.iter().take(found.min(capacity)) {
            let value = fixed_c_buffer_to_string(connstring);
            if value.is_empty() {
                continue;
            }
            result.push(rt::ConnectionString::new(value)?);
        }
        Ok(result)
    }

    fn open(
        &self,
        context: &rt::Context,
        connstring: &rt::ConnectionString,
    ) -> Result<Box<dyn rt::DeviceBackend>, rt::Error> {
        let Some(driver) = (unsafe { as_ref(self.raw) }) else {
            return Err(rt::Error::DriverOpenFailed(connstring.as_str().to_string()));
        };
        let Some(open) = driver.open else {
            return Err(rt::Error::DriverOpenFailed(connstring.as_str().to_string()));
        };

        let mut raw_context = unsafe { std::mem::zeroed::<nfc_context>() };
        write_context_to_c(context, ptr::addr_of_mut!(raw_context));

        let connstring_c = std::ffi::CString::new(connstring.as_str())
            .map_err(|_| rt::Error::InvalidEncoding("connstring"))?;
        let raw_device = unsafe { open(ptr::addr_of!(raw_context), connstring_c.as_ptr()) };
        if raw_device.is_null() {
            return Err(rt::Error::DriverOpenFailed(connstring.as_str().to_string()));
        }

        Ok(Box::new(DeviceAdapter::owned(raw_device)))
    }
}

pub(crate) struct DeviceAdapter {
    raw: *mut nfc_device,
    name: String,
    connstring: rt::ConnectionString,
    owned: bool,
}

unsafe impl Send for DeviceAdapter {}

impl DeviceAdapter {
    pub(crate) fn borrowed(raw: *mut nfc_device) -> Self {
        Self::new(raw, false)
    }

    pub(crate) fn owned(raw: *mut nfc_device) -> Self {
        Self::new(raw, true)
    }

    fn new(raw: *mut nfc_device, owned: bool) -> Self {
        let name = unsafe { as_ref(raw) }
            .map(|device| fixed_c_buffer_to_string(&device.name))
            .unwrap_or_default();
        let connstring_string = unsafe { as_ref(raw) }
            .map(|device| fixed_c_buffer_to_string(&device.connstring))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        let connstring = rt::ConnectionString::new(connstring_string)
            .unwrap_or_else(|_| rt::ConnectionString::new("unknown").expect("valid connstring"));
        Self {
            raw,
            name,
            connstring,
            owned,
        }
    }

    fn set_last_error(&mut self, value: c_int) {
        if let Some(device) = unsafe { as_mut(self.raw) } {
            device.last_error = value;
        }
    }

    fn driver_ref(&self) -> Option<&nfc_driver> {
        let device = unsafe { as_ref(self.raw) }?;
        unsafe { as_ref(device.driver) }
    }

    fn status_to_result(operation: &'static str, status: c_int) -> Result<c_int, rt::Error> {
        if status < 0 {
            Err(rt::Error::DeviceOperationFailed {
                operation,
                code: status,
            })
        } else {
            Ok(status)
        }
    }
}

impl Drop for DeviceAdapter {
    fn drop(&mut self) {
        if self.owned && !self.raw.is_null() {
            unsafe { crate::core::bridge_close_device(self.raw) };
        }
    }
}

impl rt::DeviceBackend for DeviceAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn connstring(&self) -> &rt::ConnectionString {
        &self.connstring
    }

    fn last_error(&self) -> i32 {
        unsafe { as_ref(self.raw) }
            .map(|device| device.last_error)
            .unwrap_or(0)
    }

    fn set_last_error(&mut self, value: i32) {
        DeviceAdapter::set_last_error(self, value);
    }

    fn unsupported_error_code(&self) -> i32 {
        NFC_EDEVNOTSUPP
    }

    fn strerror_backend(&self) -> Option<String> {
        let Some(driver) = self.driver_ref() else {
            return None;
        };
        let Some(callback) = driver.strerror else {
            return None;
        };
        let value = unsafe { callback(self.raw.cast_const()) };
        if value.is_null() {
            None
        } else {
            Some(c_string_ptr_to_string(value, bounded_strlen(value, 256)))
        }
    }

    fn information_about_backend(&mut self) -> Result<String, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation(
                "device_get_information_about",
            ));
        };
        let Some(callback) = driver.device_get_information_about else {
            return Err(rt::Error::UnsupportedOperation(
                "device_get_information_about",
            ));
        };

        let mut buffer = ptr::null_mut();
        Self::status_to_result("device_get_information_about", unsafe {
            callback(self.raw, ptr::addr_of_mut!(buffer))
        })?;
        let value = c_string_ptr_to_string(buffer, bounded_strlen(buffer, 4096));
        unsafe { release_allocated_ptr(buffer.cast()) };
        Ok(value)
    }

    fn set_property_bool_backend(
        &mut self,
        property: rt::Property,
        enable: bool,
    ) -> Result<(), rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("device_set_property_bool"));
        };
        let Some(callback) = driver.device_set_property_bool else {
            return Err(rt::Error::UnsupportedOperation("device_set_property_bool"));
        };
        Self::status_to_result("device_set_property_bool", unsafe {
            callback(self.raw, property_to_c(property), enable)
        })?;
        Ok(())
    }

    fn set_property_int_backend(
        &mut self,
        property: rt::Property,
        value: i32,
    ) -> Result<(), rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("device_set_property_int"));
        };
        let Some(callback) = driver.device_set_property_int else {
            return Err(rt::Error::UnsupportedOperation("device_set_property_int"));
        };
        Self::status_to_result("device_set_property_int", unsafe {
            callback(self.raw, property_to_c(property), value)
        })?;
        Ok(())
    }

    fn supported_modulations_backend(
        &mut self,
        mode: rt::Mode,
    ) -> Result<Vec<rt::ModulationType>, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("get_supported_modulation"));
        };
        let Some(callback) = driver.get_supported_modulation else {
            return Err(rt::Error::UnsupportedOperation("get_supported_modulation"));
        };

        let mut supported = ptr::null();
        Self::status_to_result("get_supported_modulation", unsafe {
            callback(self.raw, mode_to_c(mode), ptr::addr_of_mut!(supported))
        })?;

        let mut values = Vec::new();
        let mut index = 0usize;
        while !supported.is_null() {
            let value = unsafe { supported.add(index).read() };
            if matches!(value, nfc_modulation_type::NMT_UNDEFINED) {
                break;
            }
            values.push(modulation_type_from_c(value));
            index += 1;
        }
        Ok(values)
    }

    fn supported_baud_rates_backend(
        &mut self,
        mode: rt::Mode,
        modulation_type: rt::ModulationType,
    ) -> Result<Vec<rt::BaudRate>, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("get_supported_baud_rate"));
        };
        let Some(callback) = driver.get_supported_baud_rate else {
            return Err(rt::Error::UnsupportedOperation("get_supported_baud_rate"));
        };

        let mut supported = ptr::null();
        Self::status_to_result("get_supported_baud_rate", unsafe {
            callback(
                self.raw,
                mode_to_c(mode),
                modulation_type_to_c(modulation_type),
                ptr::addr_of_mut!(supported),
            )
        })?;

        let mut values = Vec::new();
        let mut index = 0usize;
        while !supported.is_null() {
            let value = unsafe { supported.add(index).read() };
            if matches!(value, nfc_baud_rate::NBR_UNDEFINED) {
                break;
            }
            values.push(baud_rate_from_c(value));
            index += 1;
        }
        Ok(values)
    }

    fn property_bool_state(&self, property: rt::Property) -> Option<bool> {
        let device = unsafe { as_ref(self.raw) }?;
        Some(match property {
            rt::Property::HandleCrc => device.bCrc,
            rt::Property::HandleParity => device.bPar,
            rt::Property::EasyFraming => device.bEasyFraming,
            rt::Property::InfiniteSelect => device.bInfiniteSelect,
            rt::Property::AutoIso14443_4 => device.bAutoIso14443_4,
            _ => return None,
        })
    }

    fn initiator_init_backend(&mut self) -> Result<i32, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("initiator_init"));
        };
        let Some(callback) = driver.initiator_init else {
            return Err(rt::Error::UnsupportedOperation("initiator_init"));
        };
        Self::status_to_result("initiator_init", unsafe { callback(self.raw) })
    }

    fn initiator_init_secure_element_backend(&mut self) -> Result<i32, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_init_secure_element",
            ));
        };
        let Some(callback) = driver.initiator_init_secure_element else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_init_secure_element",
            ));
        };
        Self::status_to_result("initiator_init_secure_element", unsafe {
            callback(self.raw)
        })
    }

    fn select_passive_target_backend(
        &mut self,
        nm: rt::Modulation,
        init_data: &[u8],
    ) -> Result<Option<rt::Target>, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_select_passive_target",
            ));
        };
        let Some(callback) = driver.initiator_select_passive_target else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_select_passive_target",
            ));
        };
        let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
        let status = Self::status_to_result("initiator_select_passive_target", unsafe {
            callback(
                self.raw,
                modulation_to_c(nm),
                if init_data.is_empty() {
                    ptr::null()
                } else {
                    init_data.as_ptr()
                },
                init_data.len(),
                ptr::addr_of_mut!(target),
            )
        })?;
        if status == 0 {
            return Ok(None);
        }
        Ok(Some(target_from_c(ptr::addr_of!(target))))
    }

    fn poll_target_backend(
        &mut self,
        modulations: &[rt::Modulation],
        poll_nr: u8,
        period: u8,
    ) -> Result<Option<rt::Target>, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("initiator_poll_target"));
        };
        let Some(callback) = driver.initiator_poll_target else {
            return Err(rt::Error::UnsupportedOperation("initiator_poll_target"));
        };
        let raw_modulations: Vec<nfc_modulation> =
            modulations.iter().copied().map(modulation_to_c).collect();
        let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
        let status = Self::status_to_result("initiator_poll_target", unsafe {
            callback(
                self.raw,
                raw_modulations.as_ptr(),
                raw_modulations.len(),
                poll_nr,
                period,
                ptr::addr_of_mut!(target),
            )
        })?;
        if status == 0 {
            return Ok(None);
        }
        Ok(Some(target_from_c(ptr::addr_of!(target))))
    }

    fn select_dep_target_backend(
        &mut self,
        ndm: rt::DepMode,
        nbr: rt::BaudRate,
        initiator: Option<&rt::DepInfo>,
        timeout: i32,
    ) -> Result<Option<rt::Target>, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_select_dep_target",
            ));
        };
        let Some(callback) = driver.initiator_select_dep_target else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_select_dep_target",
            ));
        };
        let raw_initiator = initiator.map(dep_info_to_c);
        let mut target = unsafe { std::mem::zeroed::<nfc_target>() };
        let status = Self::status_to_result("initiator_select_dep_target", unsafe {
            callback(
                self.raw,
                dep_mode_to_c(ndm),
                baud_rate_to_c(nbr),
                raw_initiator
                    .as_ref()
                    .map_or(ptr::null(), |value| ptr::addr_of!(*value)),
                ptr::addr_of_mut!(target),
                timeout,
            )
        })?;
        if status == 0 {
            return Ok(None);
        }
        Ok(Some(target_from_c(ptr::addr_of!(target))))
    }

    fn deselect_target_backend(&mut self) -> Result<(), rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("initiator_deselect_target"));
        };
        let Some(callback) = driver.initiator_deselect_target else {
            return Err(rt::Error::UnsupportedOperation("initiator_deselect_target"));
        };
        Self::status_to_result("initiator_deselect_target", unsafe { callback(self.raw) })?;
        Ok(())
    }

    fn target_is_present_backend(
        &mut self,
        target: Option<&rt::Target>,
    ) -> Result<bool, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_target_is_present",
            ));
        };
        let Some(callback) = driver.initiator_target_is_present else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_target_is_present",
            ));
        };
        let raw_target = target.map(target_to_c);
        let status = Self::status_to_result("initiator_target_is_present", unsafe {
            callback(
                self.raw,
                raw_target
                    .as_ref()
                    .map_or(ptr::null(), |value| ptr::addr_of!(*value)),
            )
        })?;
        Ok(status > 0)
    }

    fn target_init_backend(
        &mut self,
        target: &mut rt::Target,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("target_init"));
        };
        let Some(callback) = driver.target_init else {
            return Err(rt::Error::UnsupportedOperation("target_init"));
        };
        let mut raw_target = target_to_c(target);
        let count = Self::status_to_result("target_init", unsafe {
            callback(
                self.raw,
                ptr::addr_of_mut!(raw_target),
                bytes_mut_ptr(rx),
                rx.len(),
                timeout,
            )
        })?;
        *target = target_from_c(ptr::addr_of!(raw_target));
        Ok(count as usize)
    }

    fn transceive_bytes_backend(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_transceive_bytes",
            ));
        };
        let Some(callback) = driver.initiator_transceive_bytes else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_transceive_bytes",
            ));
        };
        let count = Self::status_to_result("initiator_transceive_bytes", unsafe {
            callback(
                self.raw,
                bytes_ptr(tx),
                tx.len(),
                bytes_mut_ptr(rx),
                rx.len(),
                timeout,
            )
        })?;
        Ok(count as usize)
    }

    fn transceive_bits_backend(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("initiator_transceive_bits"));
        };
        let Some(callback) = driver.initiator_transceive_bits else {
            return Err(rt::Error::UnsupportedOperation("initiator_transceive_bits"));
        };
        let count = Self::status_to_result("initiator_transceive_bits", unsafe {
            callback(
                self.raw,
                bytes_ptr(tx),
                tx_bits_len,
                optional_bytes_ptr(tx_parity),
                bytes_mut_ptr(rx),
                optional_bytes_mut_ptr(rx_parity),
            )
        })?;
        Ok(count as usize)
    }

    fn transceive_bytes_timed_backend(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(usize, u32), rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_transceive_bytes_timed",
            ));
        };
        let Some(callback) = driver.initiator_transceive_bytes_timed else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_transceive_bytes_timed",
            ));
        };
        let mut cycles = 0u32;
        let count = Self::status_to_result("initiator_transceive_bytes_timed", unsafe {
            callback(
                self.raw,
                bytes_ptr(tx),
                tx.len(),
                bytes_mut_ptr(rx),
                rx.len(),
                ptr::addr_of_mut!(cycles),
            )
        })?;
        Ok((count as usize, cycles))
    }

    fn transceive_bits_timed_backend(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<(usize, u32), rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_transceive_bits_timed",
            ));
        };
        let Some(callback) = driver.initiator_transceive_bits_timed else {
            return Err(rt::Error::UnsupportedOperation(
                "initiator_transceive_bits_timed",
            ));
        };
        let mut cycles = 0u32;
        let count = Self::status_to_result("initiator_transceive_bits_timed", unsafe {
            callback(
                self.raw,
                bytes_ptr(tx),
                tx_bits_len,
                optional_bytes_ptr(tx_parity),
                bytes_mut_ptr(rx),
                optional_bytes_mut_ptr(rx_parity),
                ptr::addr_of_mut!(cycles),
            )
        })?;
        Ok((count as usize, cycles))
    }

    fn target_send_bytes_backend(&mut self, tx: &[u8], timeout: i32) -> Result<usize, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("target_send_bytes"));
        };
        let Some(callback) = driver.target_send_bytes else {
            return Err(rt::Error::UnsupportedOperation("target_send_bytes"));
        };
        let count = Self::status_to_result("target_send_bytes", unsafe {
            callback(self.raw, bytes_ptr(tx), tx.len(), timeout)
        })?;
        Ok(count as usize)
    }

    fn target_receive_bytes_backend(
        &mut self,
        rx: &mut [u8],
        timeout: i32,
    ) -> Result<usize, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("target_receive_bytes"));
        };
        let Some(callback) = driver.target_receive_bytes else {
            return Err(rt::Error::UnsupportedOperation("target_receive_bytes"));
        };
        let count = Self::status_to_result("target_receive_bytes", unsafe {
            callback(self.raw, bytes_mut_ptr(rx), rx.len(), timeout)
        })?;
        Ok(count as usize)
    }

    fn target_send_bits_backend(
        &mut self,
        tx: &[u8],
        tx_bits_len: usize,
        tx_parity: Option<&[u8]>,
    ) -> Result<usize, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("target_send_bits"));
        };
        let Some(callback) = driver.target_send_bits else {
            return Err(rt::Error::UnsupportedOperation("target_send_bits"));
        };
        let count = Self::status_to_result("target_send_bits", unsafe {
            callback(
                self.raw,
                bytes_ptr(tx),
                tx_bits_len,
                optional_bytes_ptr(tx_parity),
            )
        })?;
        Ok(count as usize)
    }

    fn target_receive_bits_backend(
        &mut self,
        rx: &mut [u8],
        rx_parity: Option<&mut [u8]>,
    ) -> Result<usize, rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("target_receive_bits"));
        };
        let Some(callback) = driver.target_receive_bits else {
            return Err(rt::Error::UnsupportedOperation("target_receive_bits"));
        };
        let count = Self::status_to_result("target_receive_bits", unsafe {
            callback(
                self.raw,
                bytes_mut_ptr(rx),
                rx.len(),
                optional_bytes_mut_ptr(rx_parity),
            )
        })?;
        Ok(count as usize)
    }

    fn abort_command_backend(&mut self) -> Result<(), rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("abort_command"));
        };
        let Some(callback) = driver.abort_command else {
            return Err(rt::Error::UnsupportedOperation("abort_command"));
        };
        Self::status_to_result("abort_command", unsafe { callback(self.raw) })?;
        Ok(())
    }

    fn idle_backend(&mut self) -> Result<(), rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("idle"));
        };
        let Some(callback) = driver.idle else {
            return Err(rt::Error::UnsupportedOperation("idle"));
        };
        Self::status_to_result("idle", unsafe { callback(self.raw) })?;
        Ok(())
    }

    fn powerdown_backend(&mut self) -> Result<(), rt::Error> {
        let Some(driver) = self.driver_ref() else {
            return Err(rt::Error::UnsupportedOperation("powerdown"));
        };
        let Some(callback) = driver.powerdown else {
            return Err(rt::Error::UnsupportedOperation("powerdown"));
        };
        Self::status_to_result("powerdown", unsafe { callback(self.raw) })?;
        Ok(())
    }

    fn into_native_payload(self: Box<Self>) -> Option<Box<dyn std::any::Any + Send>> {
        None
    }
}

pub(crate) fn dep_info_from_c(info: nfc_dep_info) -> rt::DepInfo {
    rt::DepInfo {
        nfcid3: info.abtNFCID3,
        did: info.btDID,
        bs: info.btBS,
        br: info.btBR,
        timeout: info.btTO,
        pp: info.btPP,
        general_bytes: info.abtGB[..info.szGB.min(info.abtGB.len())].to_vec(),
        mode: dep_mode_from_c(info.ndm),
    }
}

fn dep_info_to_c(info: &rt::DepInfo) -> nfc_dep_info {
    let mut raw = unsafe { std::mem::zeroed::<nfc_dep_info>() };
    raw.abtNFCID3 = info.nfcid3;
    raw.btDID = info.did;
    raw.btBS = info.bs;
    raw.btBR = info.br;
    raw.btTO = info.timeout;
    raw.btPP = info.pp;
    raw.szGB = info.general_bytes.len().min(raw.abtGB.len());
    raw.abtGB[..raw.szGB].copy_from_slice(&info.general_bytes[..raw.szGB]);
    raw.ndm = dep_mode_to_c(info.mode);
    raw
}

fn target_to_c(target: &rt::Target) -> nfc_target {
    nfc_target {
        nti: target_info_to_c(&target.info),
        nm: modulation_to_c(target.modulation),
    }
}

fn target_info_to_c(info: &rt::TargetInfo) -> nfc_target_info {
    match info {
        rt::TargetInfo::None => unsafe { std::mem::zeroed() },
        rt::TargetInfo::Iso14443A {
            atqa,
            sak,
            uid,
            ats,
        } => {
            let mut value = unsafe { std::mem::zeroed::<nfc_iso14443a_info>() };
            value.abtAtqa = *atqa;
            value.btSak = *sak;
            value.szUidLen = uid.len().min(value.abtUid.len());
            value.abtUid[..value.szUidLen].copy_from_slice(&uid[..value.szUidLen]);
            value.szAtsLen = ats.len().min(value.abtAts.len());
            value.abtAts[..value.szAtsLen].copy_from_slice(&ats[..value.szAtsLen]);
            nfc_target_info { nai: value }
        }
        rt::TargetInfo::Felica {
            len,
            response_code,
            id,
            pad,
            system_code,
        } => nfc_target_info {
            nfi: nfc_felica_info {
                szLen: *len,
                btResCode: *response_code,
                abtId: *id,
                abtPad: *pad,
                abtSysCode: *system_code,
            },
        },
        rt::TargetInfo::Iso14443B {
            pupi,
            application_data,
            protocol_info,
            card_identifier,
        } => nfc_target_info {
            nbi: nfc_iso14443b_info {
                abtPupi: *pupi,
                abtApplicationData: *application_data,
                abtProtocolInfo: *protocol_info,
                ui8CardIdentifier: *card_identifier,
            },
        },
        rt::TargetInfo::Iso14443Bi {
            div,
            version_log,
            config,
            atr,
        } => {
            let mut value = unsafe { std::mem::zeroed::<nfc_iso14443bi_info>() };
            value.abtDIV = *div;
            value.btVerLog = *version_log;
            value.btConfig = *config;
            value.szAtrLen = atr.len().min(value.abtAtr.len());
            value.abtAtr[..value.szAtrLen].copy_from_slice(&atr[..value.szAtrLen]);
            nfc_target_info { nii: value }
        }
        rt::TargetInfo::Iso14443BiClass { uid } => nfc_target_info {
            nhi: nfc_iso14443biclass_info { abtUID: *uid },
        },
        rt::TargetInfo::Iso14443B2Sr { uid } => nfc_target_info {
            nsi: nfc_iso14443b2sr_info { abtUID: *uid },
        },
        rt::TargetInfo::Iso14443B2Ct {
            uid,
            product_code,
            fabrication_code,
        } => nfc_target_info {
            nci: nfc_iso14443b2ct_info {
                abtUID: *uid,
                btProdCode: *product_code,
                btFabCode: *fabrication_code,
            },
        },
        rt::TargetInfo::Jewel { sens_res, id } => nfc_target_info {
            nji: nfc_jewel_info {
                btSensRes: *sens_res,
                btId: *id,
            },
        },
        rt::TargetInfo::Dep(info) => nfc_target_info {
            ndi: dep_info_to_c(info),
        },
        rt::TargetInfo::Barcode { data } => {
            let mut value = unsafe { std::mem::zeroed::<nfc_barcode_info>() };
            value.szDataLen = data.len().min(value.abtData.len());
            value.abtData[..value.szDataLen].copy_from_slice(&data[..value.szDataLen]);
            nfc_target_info { nti: value }
        }
    }
}

fn property_to_c(property: rt::Property) -> nfc_property {
    match property {
        rt::Property::TimeoutCommand => nfc_property::NP_TIMEOUT_COMMAND,
        rt::Property::TimeoutAtr => nfc_property::NP_TIMEOUT_ATR,
        rt::Property::TimeoutCom => nfc_property::NP_TIMEOUT_COM,
        rt::Property::HandleCrc => nfc_property::NP_HANDLE_CRC,
        rt::Property::HandleParity => nfc_property::NP_HANDLE_PARITY,
        rt::Property::ActivateField => nfc_property::NP_ACTIVATE_FIELD,
        rt::Property::ActivateCrypto1 => nfc_property::NP_ACTIVATE_CRYPTO1,
        rt::Property::InfiniteSelect => nfc_property::NP_INFINITE_SELECT,
        rt::Property::AcceptInvalidFrames => nfc_property::NP_ACCEPT_INVALID_FRAMES,
        rt::Property::AcceptMultipleFrames => nfc_property::NP_ACCEPT_MULTIPLE_FRAMES,
        rt::Property::AutoIso14443_4 => nfc_property::NP_AUTO_ISO14443_4,
        rt::Property::EasyFraming => nfc_property::NP_EASY_FRAMING,
        rt::Property::ForceIso14443A => nfc_property::NP_FORCE_ISO14443_A,
        rt::Property::ForceIso14443B => nfc_property::NP_FORCE_ISO14443_B,
        rt::Property::ForceSpeed106 => nfc_property::NP_FORCE_SPEED_106,
    }
}

pub(crate) fn modulation_type_from_c(value: nfc_modulation_type) -> rt::ModulationType {
    match value {
        nfc_modulation_type::NMT_UNDEFINED => rt::ModulationType::Undefined,
        nfc_modulation_type::NMT_ISO14443A => rt::ModulationType::Iso14443A,
        nfc_modulation_type::NMT_JEWEL => rt::ModulationType::Jewel,
        nfc_modulation_type::NMT_ISO14443B => rt::ModulationType::Iso14443B,
        nfc_modulation_type::NMT_ISO14443BI => rt::ModulationType::Iso14443Bi,
        nfc_modulation_type::NMT_ISO14443B2SR => rt::ModulationType::Iso14443B2Sr,
        nfc_modulation_type::NMT_ISO14443B2CT => rt::ModulationType::Iso14443B2Ct,
        nfc_modulation_type::NMT_FELICA => rt::ModulationType::Felica,
        nfc_modulation_type::NMT_DEP => rt::ModulationType::Dep,
        nfc_modulation_type::NMT_BARCODE => rt::ModulationType::Barcode,
        nfc_modulation_type::NMT_ISO14443BICLASS => rt::ModulationType::Iso14443BiClass,
    }
}

fn modulation_type_to_c(value: rt::ModulationType) -> nfc_modulation_type {
    match value {
        rt::ModulationType::Undefined => nfc_modulation_type::NMT_UNDEFINED,
        rt::ModulationType::Iso14443A => nfc_modulation_type::NMT_ISO14443A,
        rt::ModulationType::Jewel => nfc_modulation_type::NMT_JEWEL,
        rt::ModulationType::Iso14443B => nfc_modulation_type::NMT_ISO14443B,
        rt::ModulationType::Iso14443Bi => nfc_modulation_type::NMT_ISO14443BI,
        rt::ModulationType::Iso14443B2Sr => nfc_modulation_type::NMT_ISO14443B2SR,
        rt::ModulationType::Iso14443B2Ct => nfc_modulation_type::NMT_ISO14443B2CT,
        rt::ModulationType::Felica => nfc_modulation_type::NMT_FELICA,
        rt::ModulationType::Dep => nfc_modulation_type::NMT_DEP,
        rt::ModulationType::Barcode => nfc_modulation_type::NMT_BARCODE,
        rt::ModulationType::Iso14443BiClass => nfc_modulation_type::NMT_ISO14443BICLASS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    struct ShimTestDevice {
        name: String,
        connstring: rt::ConnectionString,
        bool_properties: Vec<(rt::Property, bool)>,
        registers: Vec<(u16, u8)>,
    }

    impl ShimTestDevice {
        fn new(connstring: &str) -> Self {
            Self {
                name: "shim-device".to_string(),
                connstring: rt::ConnectionString::new(connstring).unwrap(),
                bool_properties: Vec::new(),
                registers: vec![(0x6302, 0x55)],
            }
        }
    }

    impl rt::OpenedDevice for ShimTestDevice {
        fn name(&self) -> &str {
            &self.name
        }

        fn connstring(&self) -> &rt::ConnectionString {
            &self.connstring
        }

        fn information_about(&mut self) -> Result<String, rt::Error> {
            Ok("shim info".to_string())
        }

        fn set_property_bool(
            &mut self,
            property: rt::Property,
            enable: bool,
        ) -> Result<(), rt::Error> {
            match self
                .bool_properties
                .iter_mut()
                .find(|entry| entry.0 == property)
            {
                Some(entry) => entry.1 = enable,
                None => self.bool_properties.push((property, enable)),
            }
            Ok(())
        }

        fn set_property_int(
            &mut self,
            _property: rt::Property,
            _value: i32,
        ) -> Result<(), rt::Error> {
            Ok(())
        }

        fn supported_modulations(
            &mut self,
            _mode: rt::Mode,
        ) -> Result<Vec<rt::ModulationType>, rt::Error> {
            Ok(vec![
                rt::ModulationType::Iso14443A,
                rt::ModulationType::Felica,
            ])
        }

        fn supported_baud_rates(
            &mut self,
            _mode: rt::Mode,
            _modulation_type: rt::ModulationType,
        ) -> Result<Vec<rt::BaudRate>, rt::Error> {
            Ok(vec![rt::BaudRate::Br106, rt::BaudRate::Br212])
        }

        fn property_bool_state(&self, property: rt::Property) -> Option<bool> {
            self.bool_properties
                .iter()
                .find(|entry| entry.0 == property)
                .map(|entry| entry.1)
        }

        fn initiator_init_driver(&mut self) -> Result<i32, rt::Error> {
            Ok(7)
        }

        fn transceive_bytes_driver(
            &mut self,
            _tx: &[u8],
            rx: &mut [u8],
            _timeout: i32,
        ) -> Result<usize, rt::Error> {
            rx[0] = 0xA5;
            Ok(1)
        }

        fn transceive_bits_timed_driver(
            &mut self,
            _tx: &[u8],
            _tx_bits_len: usize,
            _tx_parity: Option<&[u8]>,
            rx: &mut [u8],
            rx_parity: Option<&mut [u8]>,
        ) -> Result<(usize, u32), rt::Error> {
            rx[0] = 0x5A;
            if let Some(parity) = rx_parity {
                parity[0] = 0x01;
            }
            Ok((5, 4321))
        }

        fn target_is_present_driver(
            &mut self,
            _target: Option<&rt::Target>,
        ) -> Result<bool, rt::Error> {
            Ok(true)
        }

        fn target_send_bits_driver(
            &mut self,
            _tx: &[u8],
            tx_bits_len: usize,
            _tx_parity: Option<&[u8]>,
        ) -> Result<usize, rt::Error> {
            Ok(tx_bits_len)
        }

        fn target_receive_bits_driver(
            &mut self,
            rx: &mut [u8],
            rx_parity: Option<&mut [u8]>,
        ) -> Result<usize, rt::Error> {
            rx[0] = 0x3C;
            if let Some(parity) = rx_parity {
                parity[0] = 0x01;
            }
            Ok(7)
        }

        fn pn53x_transceive_driver(
            &mut self,
            tx: &[u8],
            rx: &mut [u8],
            _timeout: i32,
        ) -> Result<usize, rt::Error> {
            let len = tx.len().min(rx.len());
            rx[..len].copy_from_slice(&tx[..len]);
            Ok(len)
        }

        fn pn53x_read_register_driver(&mut self, register: u16) -> Result<u8, rt::Error> {
            self.registers
                .iter()
                .find(|entry| entry.0 == register)
                .map(|entry| entry.1)
                .ok_or(rt::Error::UnsupportedOperation("pn53x_read_register"))
        }

        fn pn53x_write_register_driver(
            &mut self,
            register: u16,
            symbol_mask: u8,
            value: u8,
        ) -> Result<(), rt::Error> {
            let slot = self
                .registers
                .iter_mut()
                .find(|entry| entry.0 == register)
                .map(|entry| &mut entry.1);
            match slot {
                Some(current) => {
                    *current = (*current & !symbol_mask) | (value & symbol_mask);
                }
                None => self
                    .registers
                    .push((register, value & symbol_mask)),
            }
            Ok(())
        }

        fn pn532_sam_configuration_driver(
            &mut self,
            mode: u8,
            _timeout: i32,
        ) -> Result<i32, rt::Error> {
            Ok(i32::from(mode))
        }
    }

    struct ShimTestDriver;

    impl rt::Driver for ShimTestDriver {
        fn name(&self) -> &str {
            "shim"
        }

        fn scan_type(&self) -> rt::ScanType {
            rt::ScanType::NotAvailable
        }

        fn scan(&self, _context: &rt::Context) -> Result<Vec<rt::ConnectionString>, rt::Error> {
            Ok(Vec::new())
        }

        fn open(
            &self,
            _context: &rt::Context,
            connstring: &rt::ConnectionString,
        ) -> Result<Box<dyn rt::OpenedDevice>, rt::Error> {
            Ok(Box::new(ShimTestDevice::new(connstring.as_str())))
        }
    }

    #[test]
    fn rust_device_shim_routes_public_calls_back_to_runtime_handle() {
        let mut registry = rt::DriverRegistry::new();
        registry.register_driver(Box::new(ShimTestDriver));

        let context = rt::Context::default();
        let connstring = rt::ConnectionString::new("shim:device").unwrap();
        let device = registry.open(&context, Some(&connstring)).unwrap();
        let raw = attach_rust_device(device, ptr::null()).unwrap();

        let name = unsafe { CStr::from_ptr(crate::initiator::nfc_device_get_name(raw)) }
            .to_str()
            .unwrap();
        assert_eq!(name, "shim-device");

        assert_eq!(
            unsafe {
                crate::initiator::nfc_device_set_property_bool(
                    raw,
                    nfc_property::NP_INFINITE_SELECT,
                    true,
                )
            },
            0
        );
        assert!(unsafe { (*raw).bInfiniteSelect });

        let mut supported = ptr::null();
        assert_eq!(
            unsafe {
                crate::initiator::nfc_device_get_supported_modulation(
                    raw,
                    nfc_mode::N_INITIATOR,
                    &mut supported,
                )
            },
            0
        );
        assert_eq!(unsafe { *supported }, nfc_modulation_type::NMT_ISO14443A);
        assert_eq!(
            unsafe { *supported.add(1) },
            nfc_modulation_type::NMT_FELICA
        );

        let tx = [0x01u8];
        let mut rx = [0u8; 4];
        assert_eq!(
            unsafe {
                crate::initiator::nfc_initiator_transceive_bytes(
                    raw,
                    tx.as_ptr(),
                    tx.len(),
                    rx.as_mut_ptr(),
                    rx.len(),
                    25,
                )
            },
            1
        );
        assert_eq!(rx[0], 0xA5);

        let tx_parity = [0x01u8];
        let mut rx_bits = [0u8; 2];
        let mut rx_parity = [0u8; 2];
        let mut cycles = 0u32;
        assert_eq!(
            unsafe {
                crate::initiator::nfc_initiator_transceive_bits_timed(
                    raw,
                    tx.as_ptr(),
                    5,
                    tx_parity.as_ptr(),
                    rx_bits.as_mut_ptr(),
                    rx_bits.len(),
                    rx_parity.as_mut_ptr(),
                    &mut cycles,
                )
            },
            5
        );
        assert_eq!(rx_bits[0], 0x5A);
        assert_eq!(rx_parity[0], 0x01);
        assert_eq!(cycles, 4321);

        assert_eq!(
            unsafe { crate::initiator::nfc_initiator_target_is_present(raw, ptr::null()) },
            1
        );
        assert_eq!(
            unsafe { crate::initiator::nfc_target_send_bits(raw, tx.as_ptr(), 5, tx_parity.as_ptr()) },
            5
        );
        assert_eq!(
            unsafe {
                crate::initiator::nfc_target_receive_bits(
                    raw,
                    rx_bits.as_mut_ptr(),
                    rx_bits.len(),
                    rx_parity.as_mut_ptr(),
                )
            },
            7
        );
        assert_eq!(rx_bits[0], 0x3C);
        assert_eq!(rx_parity[0], 0x01);

        let mut register = 0u8;
        assert_eq!(
            unsafe { crate::pn53x_read_register(raw, 0x6302, &mut register) },
            0
        );
        assert_eq!(register, 0x55);

        assert_eq!(
            unsafe { crate::pn53x_write_register(raw, 0x6302, 0x0f, 0x0a) },
            0
        );
        assert_eq!(
            unsafe { crate::pn53x_read_register(raw, 0x6302, &mut register) },
            0
        );
        assert_eq!(register, 0x5a);

        let tx_cmd = [0x40u8, 0xaa, 0xbb];
        let mut rx_cmd = [0u8; 8];
        assert_eq!(
            unsafe {
                crate::pn53x_transceive(
                    raw,
                    tx_cmd.as_ptr(),
                    tx_cmd.len(),
                    rx_cmd.as_mut_ptr(),
                    rx_cmd.len(),
                    50,
                )
            },
            tx_cmd.len() as c_int
        );
        assert_eq!(&rx_cmd[..tx_cmd.len()], &tx_cmd);

        assert_eq!(unsafe { crate::pn532_SAMConfiguration(raw, 0x03, 10) }, 3);

        unsafe { crate::compat::nfc_close(raw) };
    }
}
