use std::ffi::CString;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShareMode {
    Exclusive,
    Shared,
    Direct,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Disposition {
    LeaveCard,
    ResetCard,
    UnpowerCard,
    EjectCard,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Protocol {
    T0,
    T1,
    Raw,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Protocols(pub u8);

impl Protocols {
    pub const UNDEFINED: Self = Self(0);
    pub const T0: Self = Self(1 << 0);
    pub const T1: Self = Self(1 << 1);
    pub const RAW: Self = Self(1 << 2);
    pub const ANY: Self = Self(Self::T0.0 | Self::T1.0);

    pub const fn contains(self, protocol: Protocol) -> bool {
        let mask = match protocol {
            Protocol::T0 => Self::T0.0,
            Protocol::T1 => Self::T1.0,
            Protocol::Raw => Self::RAW.0,
        };
        self.0 & mask != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Attribute {
    VendorName,
    VendorIfdType,
    VendorIfdVersion,
    VendorIfdSerialNo,
    IccTypePerAtr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardStatus {
    pub present: bool,
    pub atr: Vec<u8>,
    pub protocol: Option<Protocol>,
}

pub trait Card: Send {
    fn reconnect(
        &mut self,
        share_mode: ShareMode,
        preferred_protocols: Protocols,
        disposition: Disposition,
    ) -> Result<(), i32>;

    fn status2_owned(&self) -> Result<CardStatus, i32>;

    fn get_attribute_owned(&self, attribute: Attribute) -> Result<Vec<u8>, i32>;

    fn transmit(&self, send_buffer: &[u8], receive_capacity: usize) -> Result<Vec<u8>, i32>;

    fn control(
        &self,
        control_code: u64,
        send_buffer: &[u8],
        receive_capacity: usize,
    ) -> Result<Vec<u8>, i32>;
}

pub trait Backend: Send + Sync {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32>;

    fn connect(
        &self,
        reader: &str,
        share_mode: ShareMode,
        preferred_protocols: Protocols,
    ) -> Result<Box<dyn Card>, i32>;
}

pub const fn ctl_code(code: u32) -> u64 {
    pcsc::ctl_code(code as pcsc::ffi::DWORD) as u64
}

pub fn error_message(code: i32) -> Option<&'static str> {
    Some(match code {
        x if x == pcsc::ffi::SCARD_S_SUCCESS as u32 as i32 => "Command successful.",
        x if x == pcsc::ffi::SCARD_F_INTERNAL_ERROR as u32 as i32 => "Internal error.",
        x if x == pcsc::ffi::SCARD_E_CANCELLED as u32 as i32 => "Command cancelled.",
        x if x == pcsc::ffi::SCARD_E_INVALID_HANDLE as u32 as i32 => "Invalid handle.",
        x if x == pcsc::ffi::SCARD_E_INVALID_PARAMETER as u32 as i32 => "Invalid parameter given.",
        x if x == pcsc::ffi::SCARD_E_INVALID_TARGET as u32 as i32 => "Invalid target given.",
        x if x == pcsc::ffi::SCARD_E_NO_MEMORY as u32 as i32 => "Not enough memory.",
        x if x == pcsc::ffi::SCARD_F_WAITED_TOO_LONG as u32 as i32 => "Waited too long.",
        x if x == pcsc::ffi::SCARD_E_INSUFFICIENT_BUFFER as u32 as i32 => "Insufficient buffer.",
        x if x == pcsc::ffi::SCARD_E_UNKNOWN_READER as u32 as i32 => "Unknown reader specified.",
        x if x == pcsc::ffi::SCARD_E_TIMEOUT as u32 as i32 => "Command timeout.",
        x if x == pcsc::ffi::SCARD_E_SHARING_VIOLATION as u32 as i32 => "Sharing violation.",
        x if x == pcsc::ffi::SCARD_E_NO_SMARTCARD as u32 as i32 => "No smart card inserted.",
        x if x == pcsc::ffi::SCARD_E_UNKNOWN_CARD as u32 as i32 => "Unknown card.",
        x if x == pcsc::ffi::SCARD_E_CANT_DISPOSE as u32 as i32 => "Cannot dispose handle.",
        x if x == pcsc::ffi::SCARD_E_PROTO_MISMATCH as u32 as i32 => "Card protocol mismatch.",
        x if x == pcsc::ffi::SCARD_E_NOT_READY as u32 as i32 => "Subsystem not ready.",
        x if x == pcsc::ffi::SCARD_E_INVALID_VALUE as u32 as i32 => "Invalid value given.",
        x if x == pcsc::ffi::SCARD_E_SYSTEM_CANCELLED as u32 as i32 => "System cancelled.",
        x if x == pcsc::ffi::SCARD_F_COMM_ERROR as u32 as i32 => "RPC transport error.",
        x if x == pcsc::ffi::SCARD_F_UNKNOWN_ERROR as u32 as i32 => "Unknown error.",
        x if x == pcsc::ffi::SCARD_E_INVALID_ATR as u32 as i32 => "Invalid ATR.",
        x if x == pcsc::ffi::SCARD_E_NOT_TRANSACTED as u32 as i32 => "Transaction failed.",
        x if x == pcsc::ffi::SCARD_E_READER_UNAVAILABLE as u32 as i32 => "Reader is unavailable.",
        x if x == pcsc::ffi::SCARD_E_PCI_TOO_SMALL as u32 as i32 => "PCI struct too small.",
        x if x == pcsc::ffi::SCARD_E_READER_UNSUPPORTED as u32 as i32 => "Reader is unsupported.",
        x if x == pcsc::ffi::SCARD_E_DUPLICATE_READER as u32 as i32 => "Reader already exists.",
        x if x == pcsc::ffi::SCARD_E_CARD_UNSUPPORTED as u32 as i32 => "Card is unsupported.",
        x if x == pcsc::ffi::SCARD_E_NO_SERVICE as u32 as i32 => "Service not available.",
        x if x == pcsc::ffi::SCARD_E_SERVICE_STOPPED as u32 as i32 => "Service was stopped.",
        x if x == pcsc::ffi::SCARD_E_NO_READERS_AVAILABLE as u32 as i32 => {
            "Cannot find a smart card reader."
        }
        x if x == pcsc::ffi::SCARD_W_UNSUPPORTED_CARD as u32 as i32 => "Card is not supported.",
        x if x == pcsc::ffi::SCARD_W_UNRESPONSIVE_CARD as u32 as i32 => "Card is unresponsive.",
        x if x == pcsc::ffi::SCARD_W_UNPOWERED_CARD as u32 as i32 => "Card is unpowered.",
        x if x == pcsc::ffi::SCARD_W_RESET_CARD as u32 as i32 => "Card was reset.",
        x if x == pcsc::ffi::SCARD_W_REMOVED_CARD as u32 as i32 => "Card was removed.",
        x if x == pcsc::ffi::SCARD_E_UNSUPPORTED_FEATURE as u32 as i32 => "Feature not supported.",
        _ => return None,
    })
}

fn error_code(error: pcsc::Error) -> i32 {
    error as u32 as i32
}

fn map_share_mode(value: ShareMode) -> pcsc::ShareMode {
    match value {
        ShareMode::Exclusive => pcsc::ShareMode::Exclusive,
        ShareMode::Shared => pcsc::ShareMode::Shared,
        ShareMode::Direct => pcsc::ShareMode::Direct,
    }
}

fn map_protocols(value: Protocols) -> pcsc::Protocols {
    let mut protocols = pcsc::Protocols::UNDEFINED;
    if value.contains(Protocol::T0) {
        protocols |= pcsc::Protocols::T0;
    }
    if value.contains(Protocol::T1) {
        protocols |= pcsc::Protocols::T1;
    }
    if value.contains(Protocol::Raw) {
        protocols |= pcsc::Protocols::RAW;
    }
    protocols
}

fn map_disposition(value: Disposition) -> pcsc::Disposition {
    match value {
        Disposition::LeaveCard => pcsc::Disposition::LeaveCard,
        Disposition::ResetCard => pcsc::Disposition::ResetCard,
        Disposition::UnpowerCard => pcsc::Disposition::UnpowerCard,
        Disposition::EjectCard => pcsc::Disposition::EjectCard,
    }
}

fn map_protocol(value: pcsc::Protocol) -> Protocol {
    match value {
        pcsc::Protocol::T0 => Protocol::T0,
        pcsc::Protocol::T1 => Protocol::T1,
        pcsc::Protocol::RAW => Protocol::Raw,
    }
}

fn map_attribute(value: Attribute) -> pcsc::Attribute {
    match value {
        Attribute::VendorName => pcsc::Attribute::VendorName,
        Attribute::VendorIfdType => pcsc::Attribute::VendorIfdType,
        Attribute::VendorIfdVersion => pcsc::Attribute::VendorIfdVersion,
        Attribute::VendorIfdSerialNo => pcsc::Attribute::VendorIfdSerialNo,
        Attribute::IccTypePerAtr => pcsc::Attribute::IccTypePerAtr,
    }
}

pub struct SystemBackend;

impl SystemBackend {
    fn establish() -> Result<pcsc::Context, i32> {
        pcsc::Context::establish(pcsc::Scope::User).map_err(error_code)
    }
}

struct SystemCard {
    _context: pcsc::Context,
    card: pcsc::Card,
}

impl Card for SystemCard {
    fn reconnect(
        &mut self,
        share_mode: ShareMode,
        preferred_protocols: Protocols,
        disposition: Disposition,
    ) -> Result<(), i32> {
        self.card
            .reconnect(
                map_share_mode(share_mode),
                map_protocols(preferred_protocols),
                map_disposition(disposition),
            )
            .map_err(error_code)
    }

    fn status2_owned(&self) -> Result<CardStatus, i32> {
        self.card
            .status2_owned()
            .map(|status| CardStatus {
                present: status.status().contains(pcsc::Status::PRESENT),
                atr: status.atr().to_vec(),
                protocol: status.protocol2().map(map_protocol),
            })
            .map_err(error_code)
    }

    fn get_attribute_owned(&self, attribute: Attribute) -> Result<Vec<u8>, i32> {
        self.card
            .get_attribute_owned(map_attribute(attribute))
            .map_err(error_code)
    }

    fn transmit(&self, send_buffer: &[u8], receive_capacity: usize) -> Result<Vec<u8>, i32> {
        let mut buffer = vec![0u8; receive_capacity.max(2)];
        self.card
            .transmit(send_buffer, &mut buffer)
            .map(|payload| payload.to_vec())
            .map_err(error_code)
    }

    fn control(
        &self,
        control_code: u64,
        send_buffer: &[u8],
        receive_capacity: usize,
    ) -> Result<Vec<u8>, i32> {
        let mut buffer = vec![0u8; receive_capacity.max(2)];
        let control_code: pcsc::ffi::DWORD = control_code.try_into().map_err(|_| -2)?;
        self.card
            .control(control_code, send_buffer, &mut buffer)
            .map(|payload| payload.to_vec())
            .map_err(error_code)
    }
}

impl Backend for SystemBackend {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32> {
        let context = Self::establish()?;
        let readers = context.list_readers_owned().map_err(error_code)?;
        Ok(readers
            .into_iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect())
    }

    fn connect(
        &self,
        reader: &str,
        share_mode: ShareMode,
        preferred_protocols: Protocols,
    ) -> Result<Box<dyn Card>, i32> {
        let context = Self::establish()?;
        let reader = CString::new(reader).map_err(|_| -2)?;
        let card = context
            .connect(
                reader.as_c_str(),
                map_share_mode(share_mode),
                map_protocols(preferred_protocols),
            )
            .map_err(error_code)?;
        Ok(Box::new(SystemCard {
            _context: context,
            card,
        }))
    }
}
