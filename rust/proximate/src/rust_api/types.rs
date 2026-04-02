#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidArgument(&'static str),
    InvalidEncoding(&'static str),
    BufferTooSmall { needed: usize, available: usize },
    InvalidConnectionString(String),
    DriverNotFound(String),
    DriverOpenFailed(String),
    UnsupportedOperation(&'static str),
    DeviceOperationFailed { operation: &'static str, code: i32 },
}

impl Error {
    pub fn device_code(&self) -> Option<i32> {
        match self {
            Self::DeviceOperationFailed { code, .. } => Some(*code),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanType {
    NotIntrusive,
    Intrusive,
    NotAvailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Property {
    TimeoutCommand,
    TimeoutAtr,
    TimeoutCom,
    HandleCrc,
    HandleParity,
    ActivateField,
    ActivateCrypto1,
    InfiniteSelect,
    AcceptInvalidFrames,
    AcceptMultipleFrames,
    AutoIso14443_4,
    EasyFraming,
    ForceIso14443A,
    ForceIso14443B,
    ForceSpeed106,
}

impl Property {
    #[doc(hidden)]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TimeoutCommand => "NP_TIMEOUT_COMMAND",
            Self::TimeoutAtr => "NP_TIMEOUT_ATR",
            Self::TimeoutCom => "NP_TIMEOUT_COM",
            Self::HandleCrc => "NP_HANDLE_CRC",
            Self::HandleParity => "NP_HANDLE_PARITY",
            Self::ActivateField => "NP_ACTIVATE_FIELD",
            Self::ActivateCrypto1 => "NP_ACTIVATE_CRYPTO1",
            Self::InfiniteSelect => "NP_INFINITE_SELECT",
            Self::AcceptInvalidFrames => "NP_ACCEPT_INVALID_FRAMES",
            Self::AcceptMultipleFrames => "NP_ACCEPT_MULTIPLE_FRAMES",
            Self::AutoIso14443_4 => "NP_AUTO_ISO14443_4",
            Self::EasyFraming => "NP_EASY_FRAMING",
            Self::ForceIso14443A => "NP_FORCE_ISO14443_A",
            Self::ForceIso14443B => "NP_FORCE_ISO14443_B",
            Self::ForceSpeed106 => "NP_FORCE_SPEED_106",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DepMode {
    Undefined,
    Passive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaudRate {
    Undefined,
    Br106,
    Br212,
    Br424,
    Br847,
}

impl BaudRate {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Undefined => "undefined baud rate",
            Self::Br106 => "106 kbps",
            Self::Br212 => "212 kbps",
            Self::Br424 => "424 kbps",
            Self::Br847 => "847 kbps",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModulationType {
    Undefined,
    Iso14443A,
    Jewel,
    Iso14443B,
    Iso14443Bi,
    Iso14443B2Sr,
    Iso14443B2Ct,
    Felica,
    Dep,
    Barcode,
    Iso14443BiClass,
}

impl ModulationType {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Undefined => "???",
            Self::Iso14443A => "ISO/IEC 14443A",
            Self::Jewel => "Innovision Jewel",
            Self::Iso14443B => "ISO/IEC 14443-4B",
            Self::Iso14443Bi => "ISO/IEC 14443-4B'",
            Self::Iso14443B2Sr => "ISO/IEC 14443-2B ST SRx",
            Self::Iso14443B2Ct => "ISO/IEC 14443-2B ASK CTx",
            Self::Felica => "FeliCa",
            Self::Dep => "D.E.P.",
            Self::Barcode => "Thinfilm NFC Barcode",
            Self::Iso14443BiClass => "ISO/IEC 14443-2B-3B iClass (Picopass)",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Target,
    Initiator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Modulation {
    pub modulation_type: ModulationType,
    pub baud_rate: BaudRate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepInfo {
    pub nfcid3: [u8; 10],
    pub did: u8,
    pub bs: u8,
    pub br: u8,
    pub timeout: u8,
    pub pp: u8,
    pub general_bytes: Vec<u8>,
    pub mode: DepMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TargetInfo {
    None,
    Iso14443A {
        atqa: [u8; 2],
        sak: u8,
        uid: Vec<u8>,
        ats: Vec<u8>,
    },
    Felica {
        len: usize,
        response_code: u8,
        id: [u8; 8],
        pad: [u8; 8],
        system_code: [u8; 2],
    },
    Iso14443B {
        pupi: [u8; 4],
        application_data: [u8; 4],
        protocol_info: [u8; 3],
        card_identifier: u8,
    },
    Iso14443Bi {
        div: [u8; 4],
        version_log: u8,
        config: u8,
        atr: Vec<u8>,
    },
    Iso14443BiClass {
        uid: [u8; 8],
    },
    Iso14443B2Sr {
        uid: [u8; 8],
    },
    Iso14443B2Ct {
        uid: [u8; 4],
        product_code: u8,
        fabrication_code: u8,
    },
    Jewel {
        sens_res: [u8; 2],
        id: [u8; 4],
    },
    Dep(DepInfo),
    Barcode {
        data: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    pub modulation: Modulation,
    pub info: TargetInfo,
}

impl Target {
    pub fn new(modulation: Modulation) -> Self {
        Self {
            modulation,
            info: TargetInfo::None,
        }
    }
}
