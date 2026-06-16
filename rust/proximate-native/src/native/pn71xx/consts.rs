use proximate_driver::{BaudRate, ModulationType};
use std::time::Duration;

pub(super) const NFC_SUCCESS: i32 = 0;
pub(super) const NFC_EIO: i32 = -1;
pub(super) const NFC_EINVARG: i32 = -2;

pub(super) const PN71XX_DRIVER_NAME: &str = "pn71xx";
pub(super) const PN71XX_DEVICE_NAME: &str = "pn71xx-device";
pub(super) const PN71XX_INFO: &str = "PN71XX nfc driver using libnfc-nci userspace library";
pub(super) const DESFIRE_ATS: [u8; 4] = [0x75, 0x77, 0x81, 0x02];
pub(super) const DEFAULT_NFA_TECH_MASK: i32 = 0x07;
#[cfg(test)]
pub(super) const NFC_SETTLE_DELAY: Duration = Duration::ZERO;
#[cfg(not(test))]
pub(super) const NFC_SETTLE_DELAY: Duration = Duration::from_secs(1);
pub(super) const POLL_PERIOD_FACTOR_MICROS: u64 = 150_000;

pub(super) const TARGET_TYPE_ISO14443_3A: u32 = 0x01;
pub(super) const TARGET_TYPE_ISO14443_3B: u32 = 0x02;
pub(super) const TARGET_TYPE_FELICA: u32 = 0x03;
pub(super) const TARGET_TYPE_MIFARE_CLASSIC: u32 = 0x08;
pub(super) const TARGET_TYPE_MIFARE_UL: u32 = 0x09;
pub(super) const TARGET_TYPE_ISO14443_4: u32 = 0x20;

pub(super) const NFA_PROTOCOL_T1T: u8 = 0x01;

pub(super) const SUPPORTED_MODULATIONS: &[ModulationType] = &[
    ModulationType::Iso14443A,
    ModulationType::Felica,
    ModulationType::Iso14443B,
    ModulationType::Iso14443Bi,
    ModulationType::Iso14443B2Sr,
    ModulationType::Iso14443B2Ct,
    ModulationType::Jewel,
    ModulationType::Dep,
];

pub(super) const ISO14443A_SUPPORTED_BAUD_RATES: &[BaudRate] = &[
    BaudRate::Br847,
    BaudRate::Br424,
    BaudRate::Br212,
    BaudRate::Br106,
];
pub(super) const FELICA_SUPPORTED_BAUD_RATES: &[BaudRate] = &[BaudRate::Br424, BaudRate::Br212];
pub(super) const DEP_SUPPORTED_BAUD_RATES: &[BaudRate] =
    &[BaudRate::Br424, BaudRate::Br212, BaudRate::Br106];
pub(super) const JEWEL_SUPPORTED_BAUD_RATES: &[BaudRate] = &[
    BaudRate::Br847,
    BaudRate::Br424,
    BaudRate::Br212,
    BaudRate::Br106,
];
pub(super) const ISO14443B_SUPPORTED_BAUD_RATES: &[BaudRate] = &[
    BaudRate::Br847,
    BaudRate::Br424,
    BaudRate::Br212,
    BaudRate::Br106,
];
