/*-
 * Free/Libre Near Field Communication (NFC) library
 *
 * Libnfc historical contributors:
 * Copyright (C) 2009      Roel Verdult
 * Copyright (C) 2009-2013 Romuald Conty
 * Copyright (C) 2010-2012 Romain Tartière
 * Copyright (C) 2010-2013 Philippe Teuwen
 * Copyright (C) 2012-2013 Ludovic Rousseau
 * See AUTHORS file for a more comprehensive list of contributors.
 * Additional contributors of this file:
 * Copyright (C) 2020      Adam Laurie
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by the
 * Free Software Foundation, either version 3 of the License, or (at your
 * option) any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU Lesser General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

#![allow(dead_code)]

use proximate_driver::{
    BaudRate, BitFrame, ConnectionString, DepInfo, DepMode, DeviceMeta, Error, InfoBackend,
    InitiatorBackend, Mode, Modulation, ModulationType, OperationTimeout, Pn53xBackend,
    PollIterations, PollPeriod, Property, PropertyBackend, Target, TargetBackend, TargetInfo,
    TimeoutProperty, TimerCycles,
};

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

pub(crate) fn probe_timeout() -> OperationTimeout {
    OperationTimeout::try_milliseconds(250).expect("PN53x probe timeout is representable")
}

const HOST_TO_PN53X_TFI: u8 = 0xD4;
const PN53X_TO_HOST_TFI: u8 = 0xD5;
const PN53X_GET_FIRMWARE_VERSION: u8 = 0x02;
const PN53X_READ_REGISTER: u8 = 0x06;
const PN53X_WRITE_REGISTER: u8 = 0x08;
const PN53X_SET_PARAMETERS: u8 = 0x12;
const PN532_SAM_CONFIGURATION: u8 = 0x14;
const PN53X_POWER_DOWN: u8 = 0x16;
const PN53X_RF_CONFIGURATION: u8 = 0x32;
const PN53X_IN_DATA_EXCHANGE: u8 = 0x40;
const PN53X_IN_COMMUNICATE_THRU: u8 = 0x42;
const PN53X_IN_DESELECT: u8 = 0x44;
const PN53X_IN_LIST_PASSIVE_TARGET: u8 = 0x4A;
const PN53X_IN_PSL: u8 = 0x4e;
const PN53X_IN_RELEASE: u8 = 0x52;
const PN53X_IN_JUMP_FOR_DEP: u8 = 0x56;
const PN532_IN_AUTO_POLL: u8 = 0x60;
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
const PN53X_REG_CIU_RX_MODE: u16 = 0x6303;
const PN53X_REG_CIU_TX_AUTO: u16 = 0x6305;
const PN53X_REG_CIU_MANUAL_RCV: u16 = 0x630d;
const PN53X_REG_CIU_TMODE: u16 = 0x631a;
const PN53X_REG_CIU_TPRESCALER: u16 = 0x631b;
const PN53X_REG_CIU_TRELOAD_VAL_HI: u16 = 0x631c;
const PN53X_REG_CIU_TRELOAD_VAL_LO: u16 = 0x631d;
const PN53X_REG_CIU_TCOUNTER_VAL_HI: u16 = 0x631e;
const PN53X_REG_CIU_TCOUNTER_VAL_LO: u16 = 0x631f;
const PN53X_REG_CIU_COMMAND: u16 = 0x6331;
const PN53X_REG_CIU_STATUS2: u16 = 0x6338;
const PN53X_REG_CIU_FIFO_DATA: u16 = 0x6339;
const PN53X_REG_CIU_FIFO_LEVEL: u16 = 0x633a;
const PN53X_REG_CIU_CONTROL: u16 = 0x633c;
const PN53X_REG_CIU_BIT_FRAMING: u16 = 0x633d;
const PN53X_REG_CIU_GS_N_OFF: u16 = 0x6323;
const PN53X_REG_CIU_RF_CFG: u16 = 0x6326;
const PN53X_REG_CIU_GS_N_ON: u16 = 0x6327;
const PN53X_REG_CIU_CW_GS_P: u16 = 0x6328;
const PN53X_REG_CIU_MOD_GS_P: u16 = 0x6329;
const SYMBOL_TX_CRC_ENABLE: u8 = 0x80;
const SYMBOL_RX_CRC_ENABLE: u8 = 0x80;
const SYMBOL_TX_SPEED: u8 = 0x70;
const SYMBOL_RX_SPEED: u8 = 0x70;
const SYMBOL_TX_FRAMING: u8 = 0x03;
const SYMBOL_RX_FRAMING: u8 = 0x03;
const SYMBOL_RX_NO_ERROR: u8 = 0x08;
const SYMBOL_RX_MULTIPLE: u8 = 0x04;
const SYMBOL_FORCE_100_ASK: u8 = 0x40;
const SYMBOL_PARITY_DISABLE: u8 = 0x10;
const SYMBOL_MF_CRYPTO1_ON: u8 = 0x08;
const SYMBOL_INITIAL_RF_ON: u8 = 0x04;
const SYMBOL_INITIATOR: u8 = 0x10;
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

const SUPPORT_ISO14443A: u8 = 0x01;
const SUPPORT_ISO14443B: u8 = 0x02;
const SUPPORT_ISO18092: u8 = 0x04;
const PARAM_AUTO_ATR_RES: u8 = 0x04;
const PARAM_AUTO_RATS: u8 = 0x10;
const RFCI_FIELD: u8 = 0x01;
const RFCI_TIMING: u8 = 0x02;
const RFCI_RETRY_SELECT: u8 = 0x05;

pub(crate) const PN53X_ACK_FRAME: [u8; 6] = [0x00, 0x00, 0xff, 0x00, 0xff, 0x00];
const PN53X_EXTENDED_FRAME_DATA_MAX_LEN: usize = 264;
const PN53X_EXTENDED_FRAME_OVERHEAD: usize = 11;
const PN532_BUFFER_LEN: usize = PN53X_EXTENDED_FRAME_DATA_MAX_LEN + PN53X_EXTENDED_FRAME_OVERHEAD;

use self::core::Pn53xCore;
use self::crc_bits::{
    bits_to_bytes_len, even_parity_bit, iso14443a_crc_append, pn53x_unwrap_frame, pn53x_wrap_frame,
    raw_frame_bits_len, timer_last_command_byte,
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
    nm_to_pm, nm_to_ptt, parse_dep_target, ptt_to_nm,
};
pub(crate) use self::transport::Pn53xTransport;
use self::transport::{
    BitTransceiveRequest, TimedBitTransceiveRequest, pn53x_translate_status, status_code,
    status_error,
};
use self::types::{
    ChipCapabilities, Pn53xFirmwareVersion, Pn53xOperatingMode, Pn53xPowerMode, Pn53xProtocolState,
    Pn53xType, Pn532SamMode, PropertyState,
};
#[allow(unused_imports)]
pub(crate) use self::types::{Pn53xProfile, Pn53xUsbModel};
