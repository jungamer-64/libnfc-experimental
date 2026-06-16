use super::{
    NFC_EDEVNOTSUPP, NFC_EINVARG, NFC_EIO, NFC_ENOTIMPL, NFC_ERFTRANS, NFC_ETGRELEASED,
    PN53X_STATUS_BCC, PN53X_STATUS_BITCOLL, PN53X_STATUS_BITCOUNT, PN53X_STATUS_BUFOVF,
    PN53X_STATUS_CDISCARDED, PN53X_STATUS_CID, PN53X_STATUS_CMD, PN53X_STATUS_CRC,
    PN53X_STATUS_DEPINVSTATE, PN53X_STATUS_DEPUNKCMD, PN53X_STATUS_FRAMING, PN53X_STATUS_INBUFOVF,
    PN53X_STATUS_INVPARAM, PN53X_STATUS_INVRXFRAM, PN53X_STATUS_MFAUTH, PN53X_STATUS_NAD,
    PN53X_STATUS_NFCID3, PN53X_STATUS_OPNOTALL, PN53X_STATUS_OVCURRENT, PN53X_STATUS_OVHEAT,
    PN53X_STATUS_PARITY, PN53X_STATUS_RFPROTO, PN53X_STATUS_RFTIMEOUT, PN53X_STATUS_SECNOTSUPP,
    PN53X_STATUS_SMALLBUF, PN53X_STATUS_TGREL, PN53X_STATUS_TIMEOUT,
};
use proximate_driver::Error;

pub(super) struct BitTransceiveRequest<'tx, 'rx, 'parity> {
    pub(super) operation: &'static str,
    pub(super) command: u8,
    pub(super) tx: &'tx [u8],
    pub(super) tx_bits_len: usize,
    pub(super) tx_parity: Option<&'parity [u8]>,
    pub(super) rx: &'rx mut [u8],
    pub(super) rx_parity: Option<&'parity mut [u8]>,
    pub(super) timeout_ms: i32,
}

pub(super) fn status_error(operation: &'static str, code: i32) -> Error {
    Error::DeviceOperationFailed { operation, code }
}

pub(super) fn status_code(error: &Error) -> i32 {
    match error {
        Error::InvalidArgument(_)
        | Error::InvalidEncoding(_)
        | Error::InvalidConnectionString(_) => -2,
        Error::BufferTooSmall { .. } => -5,
        Error::DriverNotFound(_) => -4,
        Error::DriverOpenFailed(_) => -80,
        Error::MissingCapability(_) => NFC_EDEVNOTSUPP,
        Error::UnsupportedOperation(_) => NFC_ENOTIMPL,
        Error::DeviceOperationFailed { code, .. } => *code,
    }
}

pub(crate) trait Pn53xTransport {
    fn send(&mut self, payload: &[u8], timeout_ms: i32) -> Result<(), Error>;
    fn receive(&mut self, buffer: &mut [u8], timeout_ms: i32) -> Result<usize, Error>;
    fn abort_command(&mut self) -> Result<(), Error>;

    fn wake_up(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

pub(super) fn pn53x_translate_status(status: u8) -> i32 {
    match status {
        0 => 0,
        PN53X_STATUS_TIMEOUT
        | PN53X_STATUS_CRC
        | PN53X_STATUS_PARITY
        | PN53X_STATUS_BITCOUNT
        | PN53X_STATUS_FRAMING
        | PN53X_STATUS_BITCOLL
        | PN53X_STATUS_RFPROTO
        | PN53X_STATUS_RFTIMEOUT
        | PN53X_STATUS_DEPUNKCMD
        | PN53X_STATUS_DEPINVSTATE
        | PN53X_STATUS_NAD
        | PN53X_STATUS_NFCID3
        | PN53X_STATUS_INVRXFRAM
        | PN53X_STATUS_BCC
        | PN53X_STATUS_CID => NFC_ERFTRANS,
        PN53X_STATUS_SMALLBUF
        | PN53X_STATUS_OVCURRENT
        | PN53X_STATUS_BUFOVF
        | PN53X_STATUS_OVHEAT
        | PN53X_STATUS_INBUFOVF => NFC_EIO,
        PN53X_STATUS_INVPARAM
        | PN53X_STATUS_OPNOTALL
        | PN53X_STATUS_CMD
        | PN53X_STATUS_SECNOTSUPP => NFC_EINVARG,
        PN53X_STATUS_TGREL | PN53X_STATUS_CDISCARDED => NFC_ETGRELEASED,
        PN53X_STATUS_MFAUTH => -30,
        _ => NFC_EIO,
    }
}
