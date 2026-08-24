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

use super::{
    NFC_EDEVNOTSUPP, NFC_EINVARG, NFC_EIO, NFC_ENOTIMPL, NFC_EOPABORTED, NFC_ERFTRANS,
    NFC_ETGRELEASED, NFC_ETIMEOUT, PN53X_STATUS_BCC, PN53X_STATUS_BITCOLL, PN53X_STATUS_BITCOUNT,
    PN53X_STATUS_BUFOVF, PN53X_STATUS_CDISCARDED, PN53X_STATUS_CID, PN53X_STATUS_CMD,
    PN53X_STATUS_CRC, PN53X_STATUS_DEPINVSTATE, PN53X_STATUS_DEPUNKCMD, PN53X_STATUS_FRAMING,
    PN53X_STATUS_INBUFOVF, PN53X_STATUS_INVPARAM, PN53X_STATUS_INVRXFRAM, PN53X_STATUS_MFAUTH,
    PN53X_STATUS_NAD, PN53X_STATUS_NFCID3, PN53X_STATUS_OPNOTALL, PN53X_STATUS_OVCURRENT,
    PN53X_STATUS_OVHEAT, PN53X_STATUS_PARITY, PN53X_STATUS_RFPROTO, PN53X_STATUS_RFTIMEOUT,
    PN53X_STATUS_SECNOTSUPP, PN53X_STATUS_SMALLBUF, PN53X_STATUS_TGREL, PN53X_STATUS_TIMEOUT,
};
use proximate_driver::{CommandAbortHandle, Error, OperationTimeout, TimerCycles};

pub(super) struct BitTransceiveRequest<'tx, 'rx, 'parity> {
    pub(super) operation: &'static str,
    pub(super) command: u8,
    pub(super) tx: &'tx [u8],
    pub(super) tx_bits_len: usize,
    pub(super) tx_parity: Option<&'parity [u8]>,
    pub(super) rx: &'rx mut [u8],
    pub(super) rx_parity: Option<&'parity mut [u8]>,
    pub(super) timeout: OperationTimeout,
}

pub(super) struct TimedBitTransceiveRequest<'tx, 'rx, 'tx_parity, 'rx_parity> {
    pub(super) operation: &'static str,
    pub(super) tx: &'tx [u8],
    pub(super) tx_bits_len: usize,
    pub(super) tx_parity: Option<&'tx_parity [u8]>,
    pub(super) rx: &'rx mut [u8],
    pub(super) rx_parity: Option<&'rx_parity mut [u8]>,
    pub(super) max_cycles: TimerCycles,
}

pub(super) fn status_error(operation: &'static str, code: i32) -> Error {
    match code {
        NFC_EINVARG => Error::InvalidArgument(operation),
        NFC_EDEVNOTSUPP => Error::MissingCapability(operation),
        NFC_ENOTIMPL => Error::UnsupportedOperation(operation),
        NFC_ETIMEOUT => Error::Timeout(operation),
        NFC_EOPABORTED => Error::Aborted(operation),
        NFC_ETGRELEASED => Error::TargetReleased(operation),
        NFC_ERFTRANS => Error::RfTransmission(operation),
        -30 => Error::Authentication(operation),
        NFC_EIO => Error::Io(operation),
        -90 => Error::Chip(operation),
        _ => Error::DeviceOperationFailed { operation, code },
    }
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
        Error::Timeout(_) => NFC_ETIMEOUT,
        Error::Aborted(_) => NFC_EOPABORTED,
        Error::TargetReleased(_) => NFC_ETGRELEASED,
        Error::RfTransmission(_) => NFC_ERFTRANS,
        Error::Authentication(_) => -30,
        Error::Io(_) => NFC_EIO,
        Error::Chip(_) => -90,
        Error::OutcomeUnknown { .. } | Error::RecoveryFailed { .. } => -80,
        Error::DeviceOperationFailed { code, .. } => *code,
    }
}

/// Classifies whether a failed transport send left the PN53x protocol state
/// interpretable.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum TransportSendError {
    /// The frame was not sent, or transport-specific cancellation/recovery
    /// established a known protocol state.
    ProtocolStable(Error),
    /// Transmission started and the transport cannot prove whether the chip
    /// accepted the frame.
    OutcomeUnknown(Error),
}

pub(crate) trait Pn53xTransport {
    fn send(&mut self, payload: &[u8], timeout: OperationTimeout)
    -> Result<(), TransportSendError>;
    fn receive(&mut self, buffer: &mut [u8], timeout: OperationTimeout) -> Result<usize, Error>;
    fn abort_command(&mut self) -> Result<(), Error>;

    fn command_abort_handle(&self) -> Option<CommandAbortHandle> {
        None
    }

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
