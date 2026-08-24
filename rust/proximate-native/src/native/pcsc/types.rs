#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PcscShareMode {
    #[cfg(feature = "driver-acr122-pcsc")]
    Exclusive,
    #[cfg(feature = "driver-pcsc")]
    Shared,
    Direct,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PcscProtocol {
    T0,
    T1,
    Raw,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PcscProtocols(pub(crate) u8);

impl PcscProtocols {
    #[cfg(feature = "driver-acr122-pcsc")]
    pub(crate) const UNDEFINED: Self = Self(0);
    pub(crate) const T0: Self = Self(1 << 0);
    pub(crate) const T1: Self = Self(1 << 1);
    pub(crate) const RAW: Self = Self(1 << 2);
    pub(crate) const ANY: Self = Self(Self::T0.0 | Self::T1.0);

    pub(crate) const fn contains(self, protocol: PcscProtocol) -> bool {
        let mask = match protocol {
            PcscProtocol::T0 => Self::T0.0,
            PcscProtocol::T1 => Self::T1.0,
            PcscProtocol::Raw => Self::RAW.0,
        };
        self.0 & mask != 0
    }
}

#[cfg(feature = "driver-pcsc")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PcscAttribute {
    VendorName,
    VendorIfdType,
    VendorIfdVersion,
    VendorIfdSerialNo,
    IccTypePerAtr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PcscCardStatus {
    pub present: bool,
    pub atr: Vec<u8>,
    pub protocol: Option<PcscProtocol>,
}

pub(crate) trait PcscCard: Send {
    #[cfg(feature = "driver-pcsc")]
    fn reconnect(
        &mut self,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
    ) -> Result<(), i32>;

    fn status2_owned(&self) -> Result<PcscCardStatus, i32>;

    #[cfg(feature = "driver-pcsc")]
    fn get_attribute_owned(&self, attribute: PcscAttribute) -> Result<Vec<u8>, i32>;

    fn transmit(&self, send_buffer: &[u8], receive_capacity: usize) -> Result<Vec<u8>, i32>;

    #[cfg(feature = "driver-acr122-pcsc")]
    fn control(
        &self,
        control_code: u64,
        send_buffer: &[u8],
        receive_capacity: usize,
    ) -> Result<Vec<u8>, i32>;
}

pub(crate) trait PcscBackend: Send + Sync {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32>;

    fn connect(
        &self,
        reader: &str,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
    ) -> Result<Box<dyn PcscCard>, i32>;
}
