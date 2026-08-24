use std::num::NonZeroU8;

use crate::Error;

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
    Passive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BaudRate {
    Br106,
    Br212,
    Br424,
    Br847,
}

impl BaudRate {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Br106 => "106 kbps",
            Self::Br212 => "212 kbps",
            Self::Br424 => "424 kbps",
            Self::Br847 => "847 kbps",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModulationType {
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
    modulation_type: ModulationType,
    baud_rate: BaudRate,
}

impl Modulation {
    /// Constructs a protocol-valid modulation and baud-rate pair.
    ///
    /// Device-specific support remains an admission decision performed by the
    /// selected driver after this protocol-level validation succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] when libnfc does not define the
    /// supplied baud rate for the modulation.
    pub fn try_new(modulation_type: ModulationType, baud_rate: BaudRate) -> Result<Self, Error> {
        let valid = match modulation_type {
            ModulationType::Iso14443A | ModulationType::Iso14443B => matches!(
                baud_rate,
                BaudRate::Br106 | BaudRate::Br212 | BaudRate::Br424 | BaudRate::Br847
            ),
            ModulationType::Jewel
            | ModulationType::Iso14443Bi
            | ModulationType::Iso14443B2Sr
            | ModulationType::Iso14443B2Ct
            | ModulationType::Barcode
            | ModulationType::Iso14443BiClass => baud_rate == BaudRate::Br106,
            ModulationType::Felica => {
                matches!(baud_rate, BaudRate::Br212 | BaudRate::Br424)
            }
            ModulationType::Dep => {
                matches!(
                    baud_rate,
                    BaudRate::Br106 | BaudRate::Br212 | BaudRate::Br424
                )
            }
        };
        if !valid {
            return Err(Error::InvalidArgument("modulation baud rate"));
        }
        Ok(Self {
            modulation_type,
            baud_rate,
        })
    }

    pub const fn modulation_type(self) -> ModulationType {
        self.modulation_type
    }

    pub const fn baud_rate(self) -> BaudRate {
        self.baud_rate
    }
}

/// Timeout semantics shared by libnfc device operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationTimeout(i32);

impl OperationTimeout {
    /// Use the operation-specific configured timeout.
    pub const DEFAULT: Self = Self(-1);
    /// Wait until completion or explicit cancellation.
    pub const INFINITE: Self = Self(0);

    /// Decodes libnfc's signed millisecond representation.
    ///
    /// # Errors
    ///
    /// Values below `-1` are rejected.
    pub fn from_libnfc_millis(value: i32) -> Result<Self, Error> {
        if value < -1 {
            Err(Error::InvalidArgument("timeout"))
        } else {
            Ok(Self(value))
        }
    }

    /// Constructs a finite timeout.
    ///
    /// # Errors
    ///
    /// Zero and values exceeding libnfc's signed ABI range are rejected.
    pub fn try_milliseconds(value: u32) -> Result<Self, Error> {
        let value = i32::try_from(value).map_err(|_| Error::InvalidArgument("timeout"))?;
        if value == 0 {
            return Err(Error::InvalidArgument("timeout"));
        }
        Ok(Self(value))
    }

    /// Returns libnfc's canonical signed millisecond representation.
    pub const fn to_libnfc_millis(self) -> i32 {
        self.0
    }

    /// Returns a finite millisecond budget, if this operation has one.
    pub const fn finite_millis(self) -> Option<u32> {
        if self.0 > 0 {
            Some(self.0 as u32)
        } else {
            None
        }
    }

    /// Resolves the default timeout for a driver boundary that still consumes
    /// libnfc's millisecond representation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if the configured default cannot be
    /// represented by libnfc's signed timeout ABI.
    pub fn resolve_libnfc_millis(self, default_millis: u32) -> Result<i32, Error> {
        if self == Self::DEFAULT {
            i32::try_from(default_millis).map_err(|_| Error::InvalidArgument("timeout"))
        } else {
            Ok(self.0)
        }
    }
}

/// Number of complete polling iterations requested by a caller.
///
/// libnfc reserves `0xff` for continuous polling and defines `0x01..=0xfe`
/// as finite counts. Zero has no protocol meaning and cannot be represented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PollIterations(NonZeroU8);

impl PollIterations {
    pub const CONTINUOUS: Self = Self(NonZeroU8::MAX);

    /// Decodes the public libnfc polling-count representation.
    ///
    /// # Errors
    ///
    /// Zero is rejected because libnfc defines neither a finite nor a
    /// continuous polling operation for it.
    pub fn from_libnfc(value: u8) -> Result<Self, Error> {
        NonZeroU8::new(value)
            .map(Self)
            .ok_or(Error::InvalidArgument("poll iterations"))
    }

    pub const fn to_libnfc(self) -> u8 {
        self.0.get()
    }

    pub const fn is_continuous(self) -> bool {
        self.0.get() == u8::MAX
    }
}

/// PN532 polling period in units defined by the public libnfc API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PollPeriod(u8);

impl PollPeriod {
    pub const MAX: u8 = 15;

    /// Validates a PN532 polling-period field.
    ///
    /// # Errors
    ///
    /// Values outside libnfc's documented `0x01..=0x0f` range are rejected.
    pub fn try_new(value: u8) -> Result<Self, Error> {
        if value == 0 || value > Self::MAX {
            return Err(Error::InvalidArgument("poll period"));
        }
        Ok(Self(value))
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// Validated bit-oriented transmit frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitFrame<'a> {
    bytes: &'a [u8],
    bit_len: usize,
    parity: Option<&'a [u8]>,
}

impl<'a> BitFrame<'a> {
    /// Validates the data, bit length, and optional one-byte-per-octet parity
    /// representation used by libnfc.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] when the bit count exceeds the data
    /// buffer or the parity buffer cannot cover every complete data octet.
    pub fn try_new(
        bytes: &'a [u8],
        bit_len: usize,
        parity: Option<&'a [u8]>,
    ) -> Result<Self, Error> {
        let capacity_bits = bytes
            .len()
            .checked_mul(u8::BITS as usize)
            .ok_or(Error::InvalidArgument("bit length"))?;
        if bit_len > capacity_bits {
            return Err(Error::InvalidArgument("bit length"));
        }
        if parity.is_some_and(|values| values.len() < bit_len / u8::BITS as usize) {
            return Err(Error::InvalidArgument("parity length"));
        }
        Ok(Self {
            bytes,
            bit_len,
            parity,
        })
    }

    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    pub const fn bit_len(self) -> usize {
        self.bit_len
    }

    pub const fn parity(self) -> Option<&'a [u8]> {
        self.parity
    }
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

impl TargetInfo {
    pub const fn modulation_type(&self) -> ModulationType {
        match self {
            Self::Iso14443A { .. } => ModulationType::Iso14443A,
            Self::Felica { .. } => ModulationType::Felica,
            Self::Iso14443B { .. } => ModulationType::Iso14443B,
            Self::Iso14443Bi { .. } => ModulationType::Iso14443Bi,
            Self::Iso14443BiClass { .. } => ModulationType::Iso14443BiClass,
            Self::Iso14443B2Sr { .. } => ModulationType::Iso14443B2Sr,
            Self::Iso14443B2Ct { .. } => ModulationType::Iso14443B2Ct,
            Self::Jewel { .. } => ModulationType::Jewel,
            Self::Dep(_) => ModulationType::Dep,
            Self::Barcode { .. } => ModulationType::Barcode,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    modulation: Modulation,
    info: TargetInfo,
}

impl Target {
    /// Constructs a target whose modulation and protocol information agree.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if the information variant belongs
    /// to a different modulation type.
    pub fn try_new(modulation: Modulation, info: TargetInfo) -> Result<Self, Error> {
        if modulation.modulation_type() != info.modulation_type() {
            return Err(Error::InvalidArgument("target modulation"));
        }
        Ok(Self { modulation, info })
    }

    pub const fn modulation(&self) -> Modulation {
        self.modulation
    }

    pub const fn info(&self) -> &TargetInfo {
        &self.info
    }

    /// Applies target-mode activation without allowing the target information
    /// variant to diverge from the activated modulation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] when the modulation does not match
    /// the configured target kind or DEP mode is missing or unexpected.
    pub fn apply_activation(
        &mut self,
        modulation: Modulation,
        dep_mode: Option<DepMode>,
    ) -> Result<(), Error> {
        if modulation.modulation_type() != self.info.modulation_type() {
            return Err(Error::InvalidArgument("target activation modulation"));
        }
        match (&mut self.info, dep_mode) {
            (TargetInfo::Dep(info), Some(mode)) => info.mode = mode,
            (TargetInfo::Dep(_), None) | (_, Some(_)) => {
                return Err(Error::InvalidArgument("target activation DEP mode"));
            }
            (_, None) => {}
        }
        self.modulation = modulation;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modulation_rejects_rates_not_defined_for_protocol() {
        for (modulation_type, valid_rates) in [
            (
                ModulationType::Iso14443A,
                &[
                    BaudRate::Br106,
                    BaudRate::Br212,
                    BaudRate::Br424,
                    BaudRate::Br847,
                ][..],
            ),
            (ModulationType::Jewel, &[BaudRate::Br106][..]),
            (
                ModulationType::Felica,
                &[BaudRate::Br212, BaudRate::Br424][..],
            ),
            (
                ModulationType::Dep,
                &[BaudRate::Br106, BaudRate::Br212, BaudRate::Br424][..],
            ),
            (ModulationType::Barcode, &[BaudRate::Br106][..]),
        ] {
            for baud_rate in [
                BaudRate::Br106,
                BaudRate::Br212,
                BaudRate::Br424,
                BaudRate::Br847,
            ] {
                assert_eq!(
                    Modulation::try_new(modulation_type, baud_rate).is_ok(),
                    valid_rates.contains(&baud_rate),
                    "{modulation_type:?} at {baud_rate:?}",
                );
            }
        }
    }

    #[test]
    fn target_requires_matching_information_variant() {
        let modulation = Modulation::try_new(ModulationType::Iso14443A, BaudRate::Br106).unwrap();
        let result = Target::try_new(
            modulation,
            TargetInfo::Jewel {
                sens_res: [0; 2],
                id: [0; 4],
            },
        );
        assert_eq!(result, Err(Error::InvalidArgument("target modulation")));
    }

    #[test]
    fn operation_timeout_round_trips_only_libnfc_values() {
        assert_eq!(
            OperationTimeout::from_libnfc_millis(-2),
            Err(Error::InvalidArgument("timeout")),
        );
        for value in [-1, 0, 1, i32::MAX] {
            let timeout = OperationTimeout::from_libnfc_millis(value).unwrap();
            assert_eq!(timeout.to_libnfc_millis(), value);
        }
        assert_eq!(
            OperationTimeout::try_milliseconds(0),
            Err(Error::InvalidArgument("timeout")),
        );
        assert_eq!(
            OperationTimeout::try_milliseconds(i32::MAX as u32 + 1),
            Err(Error::InvalidArgument("timeout")),
        );
        assert_eq!(
            OperationTimeout::DEFAULT.resolve_libnfc_millis(750),
            Ok(750),
        );
        assert_eq!(OperationTimeout::INFINITE.resolve_libnfc_millis(750), Ok(0),);
    }

    #[test]
    fn poll_values_preserve_libnfc_sentinels_and_ranges() {
        assert_eq!(
            PollIterations::from_libnfc(0),
            Err(Error::InvalidArgument("poll iterations")),
        );
        assert_eq!(PollIterations::from_libnfc(1).unwrap().to_libnfc(), 1);
        assert!(!PollIterations::from_libnfc(254).unwrap().is_continuous());
        assert!(PollIterations::from_libnfc(255).unwrap().is_continuous());

        assert_eq!(
            PollPeriod::try_new(0),
            Err(Error::InvalidArgument("poll period")),
        );
        assert_eq!(PollPeriod::try_new(1).unwrap().get(), 1);
        assert_eq!(PollPeriod::try_new(15).unwrap().get(), 15);
        assert_eq!(
            PollPeriod::try_new(16),
            Err(Error::InvalidArgument("poll period")),
        );
    }

    #[test]
    fn bit_frame_validates_bit_and_parity_capacity() {
        let bytes = [0xaa, 0x55];
        assert!(BitFrame::try_new(&bytes, 16, Some(&[1, 0])).is_ok());
        assert!(BitFrame::try_new(&bytes, 15, Some(&[1])).is_ok());
        assert_eq!(
            BitFrame::try_new(&bytes, 17, None),
            Err(Error::InvalidArgument("bit length")),
        );
        assert_eq!(
            BitFrame::try_new(&bytes, 16, Some(&[1])),
            Err(Error::InvalidArgument("parity length")),
        );
    }
}
