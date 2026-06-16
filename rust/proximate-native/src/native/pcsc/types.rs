use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum PcscShareMode {
    Exclusive,
    Shared,
    Direct,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum PcscDisposition {
    LeaveCard,
    ResetCard,
    UnpowerCard,
    EjectCard,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum PcscProtocol {
    T0,
    T1,
    Raw,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct PcscProtocols(pub(super) u8);

impl PcscProtocols {
    pub(super) const UNDEFINED: Self = Self(0);
    pub(super) const T0: Self = Self(1 << 0);
    pub(super) const T1: Self = Self(1 << 1);
    pub(super) const RAW: Self = Self(1 << 2);
    pub(super) const ANY: Self = Self(Self::T0.0 | Self::T1.0);

    pub(super) const fn contains(self, protocol: PcscProtocol) -> bool {
        let mask = match protocol {
            PcscProtocol::T0 => Self::T0.0,
            PcscProtocol::T1 => Self::T1.0,
            PcscProtocol::Raw => Self::RAW.0,
        };
        self.0 & mask != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum PcscAttribute {
    VendorName,
    VendorIfdType,
    VendorIfdVersion,
    VendorIfdSerialNo,
    IccTypePerAtr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PcscCardStatus {
    pub present: bool,
    pub atr: Vec<u8>,
    pub protocol: Option<PcscProtocol>,
}

pub(super) trait PcscCard: Send {
    fn reconnect(
        &mut self,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
        disposition: PcscDisposition,
    ) -> Result<(), i32>;

    fn status2_owned(&self) -> Result<PcscCardStatus, i32>;

    fn get_attribute_owned(&self, attribute: PcscAttribute) -> Result<Vec<u8>, i32>;

    fn transmit(&self, send_buffer: &[u8], receive_capacity: usize) -> Result<Vec<u8>, i32>;

    fn control(
        &self,
        control_code: u64,
        send_buffer: &[u8],
        receive_capacity: usize,
    ) -> Result<Vec<u8>, i32>;
}

pub(super) trait PcscBackend: Send + Sync {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32>;

    fn connect(
        &self,
        reader: &str,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
    ) -> Result<Box<dyn PcscCard>, i32>;
}
