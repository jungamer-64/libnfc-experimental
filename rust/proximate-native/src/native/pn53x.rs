#![allow(dead_code)]

use proximate_driver::{
    BaudRate, ConnectionString, DepInfo, DepMode, DeviceCaps, DeviceMeta, Error, InfoBackend,
    InitiatorBackend, Mode, Modulation, ModulationType, Pn53xBackend, Property, PropertyBackend,
    Target, TargetBackend, TargetInfo,
};
use std::thread;
use std::time::Duration;

mod core;
mod crc_bits;
mod device;
mod frame;
mod target_decode;
#[cfg(test)]
mod tests;
mod transport;
mod types;

const NFC_EIO: i32 = -1;
const NFC_EINVARG: i32 = -2;
const NFC_EDEVNOTSUPP: i32 = -3;
const NFC_ENOTSUCHDEV: i32 = -4;
const NFC_EOVFLOW: i32 = -5;
const NFC_ETIMEOUT: i32 = -6;
const NFC_EOPABORTED: i32 = -7;
const NFC_ENOTIMPL: i32 = -8;
const NFC_ETGRELEASED: i32 = -10;
const NFC_ERFTRANS: i32 = -20;

const HOST_TO_PN53X_TFI: u8 = 0xD4;
const PN53X_TO_HOST_TFI: u8 = 0xD5;
const PN53X_GET_FIRMWARE_VERSION: u8 = 0x02;
const PN53X_READ_REGISTER: u8 = 0x06;
const PN53X_WRITE_REGISTER: u8 = 0x08;
const PN532_SAM_CONFIGURATION: u8 = 0x14;
const PN53X_IN_DATA_EXCHANGE: u8 = 0x40;
const PN53X_IN_COMMUNICATE_THRU: u8 = 0x42;
const PN53X_IN_DESELECT: u8 = 0x44;
const PN53X_IN_LIST_PASSIVE_TARGET: u8 = 0x4A;
const PN53X_IN_JUMP_FOR_DEP: u8 = 0x56;
const PN53X_TG_GET_DATA: u8 = 0x86;
const PN53X_TG_INIT_AS_TARGET: u8 = 0x8C;
const PN53X_TG_SET_DATA: u8 = 0x8E;
const PN53X_TG_GET_INITIATOR_COMMAND: u8 = 0x88;
const PN53X_TG_RESPONSE_TO_INITIATOR: u8 = 0x90;

const PN53X_STATUS_TIMEOUT: u8 = 0x01;
const PN53X_STATUS_CRC: u8 = 0x02;
const PN53X_STATUS_PARITY: u8 = 0x03;
const PN53X_STATUS_BITCOUNT: u8 = 0x04;
const PN53X_STATUS_FRAMING: u8 = 0x05;
const PN53X_STATUS_BITCOLL: u8 = 0x06;
const PN53X_STATUS_SMALLBUF: u8 = 0x07;
const PN53X_STATUS_BUFOVF: u8 = 0x09;
const PN53X_STATUS_RFTIMEOUT: u8 = 0x0a;
const PN53X_STATUS_RFPROTO: u8 = 0x0b;
const PN53X_STATUS_OVHEAT: u8 = 0x0d;
const PN53X_STATUS_INBUFOVF: u8 = 0x0e;
const PN53X_STATUS_INVPARAM: u8 = 0x10;
const PN53X_STATUS_DEPUNKCMD: u8 = 0x12;
const PN53X_STATUS_INVRXFRAM: u8 = 0x13;
const PN53X_STATUS_MFAUTH: u8 = 0x14;
const PN53X_STATUS_SECNOTSUPP: u8 = 0x18;
const PN53X_STATUS_BCC: u8 = 0x23;
const PN53X_STATUS_DEPINVSTATE: u8 = 0x25;
const PN53X_STATUS_OPNOTALL: u8 = 0x26;
const PN53X_STATUS_CMD: u8 = 0x27;
const PN53X_STATUS_TGREL: u8 = 0x29;
const PN53X_STATUS_CID: u8 = 0x2a;
const PN53X_STATUS_CDISCARDED: u8 = 0x2b;
const PN53X_STATUS_NFCID3: u8 = 0x2c;
const PN53X_STATUS_OVCURRENT: u8 = 0x2d;
const PN53X_STATUS_NAD: u8 = 0x2e;

const PN53X_TARGET_MODE_NORMAL: u8 = 0x00;
const PN53X_TARGET_MODE_PASSIVE_ONLY: u8 = 0x01;
const PN53X_TARGET_MODE_DEP_ONLY: u8 = 0x02;
const PN53X_TARGET_MODE_ISO14443_4_PICC_ONLY: u8 = 0x04;
const SAK_ISO14443_4_COMPLIANT: u8 = 0x20;
const SAK_MIFARE_CLASSIC_MASK: u8 = 0x08;

const PN53X_REG_CIU_TX_MODE: u16 = 0x6302;
const PN53X_REG_CIU_TMODE: u16 = 0x631a;
const PN53X_REG_CIU_TPRESCALER: u16 = 0x631b;
const PN53X_REG_CIU_TRELOAD_VAL_HI: u16 = 0x631c;
const PN53X_REG_CIU_TRELOAD_VAL_LO: u16 = 0x631d;
const PN53X_REG_CIU_TCOUNTER_VAL_HI: u16 = 0x631e;
const PN53X_REG_CIU_TCOUNTER_VAL_LO: u16 = 0x631f;
const PN53X_REG_CIU_COMMAND: u16 = 0x6331;
const PN53X_REG_CIU_FIFO_DATA: u16 = 0x6339;
const PN53X_REG_CIU_FIFO_LEVEL: u16 = 0x633a;
const PN53X_REG_CIU_CONTROL: u16 = 0x633c;
const PN53X_REG_CIU_BIT_FRAMING: u16 = 0x633d;
const SYMBOL_TX_CRC_ENABLE: u8 = 0x80;
const SYMBOL_TX_FRAMING: u8 = 0x03;
const SYMBOL_TAUTO: u8 = 0x80;
const SYMBOL_TPRESCALERHI: u8 = 0x0f;
const SYMBOL_TPRESCALERLO: u8 = 0xff;
const SYMBOL_COMMAND: u8 = 0x0f;
const SYMBOL_COMMAND_TRANSCEIVE: u8 = 0x0c;
const SYMBOL_FLUSH_BUFFER: u8 = 0x80;
const SYMBOL_FIFO_LEVEL: u8 = 0x7f;
const SYMBOL_START_SEND: u8 = 0x80;
const SYMBOL_RX_LAST_BITS: u8 = 0x07;
const SYMBOL_TX_LAST_BITS: u8 = 0x07;

pub(crate) const PN53X_ACK_FRAME: [u8; 6] = [0x00, 0x00, 0xff, 0x00, 0xff, 0x00];
const PN53X_EXTENDED_FRAME_DATA_MAX_LEN: usize = 264;
const PN53X_EXTENDED_FRAME_OVERHEAD: usize = 11;
const PN532_BUFFER_LEN: usize = PN53X_EXTENDED_FRAME_DATA_MAX_LEN + PN53X_EXTENDED_FRAME_OVERHEAD;

pub(crate) fn scan_caps(profile: Pn53xProfile) -> DeviceCaps {
    let mut caps = DeviceCaps::INFO
        | DeviceCaps::SET_PROPERTY_BOOL
        | DeviceCaps::SET_PROPERTY_INT
        | DeviceCaps::SUPPORTED_MODULATIONS
        | DeviceCaps::SUPPORTED_BAUD_RATES
        | DeviceCaps::INITIATOR_INIT
        | DeviceCaps::SELECT_PASSIVE_TARGET
        | DeviceCaps::POLL_TARGET
        | DeviceCaps::SELECT_DEP_TARGET
        | DeviceCaps::DESELECT_TARGET
        | DeviceCaps::TARGET_IS_PRESENT
        | DeviceCaps::TARGET_INIT
        | DeviceCaps::TRANSCEIVE_BYTES
        | DeviceCaps::TRANSCEIVE_BITS
        | DeviceCaps::TRANSCEIVE_BYTES_TIMED
        | DeviceCaps::TRANSCEIVE_BITS_TIMED
        | DeviceCaps::TARGET_SEND_BYTES
        | DeviceCaps::TARGET_RECEIVE_BYTES
        | DeviceCaps::TARGET_SEND_BITS
        | DeviceCaps::TARGET_RECEIVE_BITS
        | DeviceCaps::ABORT_COMMAND
        | DeviceCaps::IDLE
        | DeviceCaps::POWERDOWN
        | DeviceCaps::PN53X_TRANSCEIVE
        | DeviceCaps::PN53X_READ_REGISTER
        | DeviceCaps::PN53X_WRITE_REGISTER
        | DeviceCaps::PN532_SAM_CONFIGURATION;
    if profile.secure_element_mode.is_some() {
        caps |= DeviceCaps::INITIATOR_INIT_SECURE_ELEMENT;
    }
    caps
}

use self::core::Pn53xCore;
use self::crc_bits::{
    bits_to_bytes_len, even_parity_bit, pn53x_unwrap_frame, pn53x_wrap_frame, raw_frame_bits_len,
    timer_last_command_byte,
};
#[allow(unused_imports)]
pub(crate) use self::device::Pn53xDevice;
#[allow(unused_imports)]
pub(crate) use self::frame::{
    build_frame, build_response_frame, command_from_host_frame, is_ack_frame,
    payload_from_host_frame,
};
use self::frame::{parse_response_frame, split_status_response};
use self::target_decode::{
    build_injump_for_dep_command, build_target_init_command, cascade_iso14443a_uid,
    decode_activation_mode, decode_target_data, default_initiator_payload, is_iso14443_4_target,
    nm_to_pm, parse_dep_target,
};
pub(crate) use self::transport::Pn53xTransport;
use self::transport::{BitTransceiveRequest, pn53x_translate_status, status_code, status_error};
use self::types::{Pn53xFirmwareVersion, Pn53xPowerMode, Pn53xType, Pn532SamMode, PropertyState};
#[allow(unused_imports)]
pub(crate) use self::types::{Pn53xProfile, Pn53xUsbModel};
