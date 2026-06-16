#[cfg(feature = "pcsc_helper")]
use crate::pcsc as platform_pcsc;

use super::{
    PcscAttribute, PcscBackend, PcscCard, PcscCardStatus, PcscDisposition, PcscProtocol,
    PcscProtocols, PcscShareMode,
};

pub(super) fn pcsc_error_message(code: i32) -> Option<&'static str> {
    #[cfg(feature = "pcsc_helper")]
    {
        platform_pcsc::error_message(code)
    }

    #[cfg(not(feature = "pcsc_helper"))]
    {
        let _ = code;
        None
    }
}

pub(super) fn stringify_pcsc_error(code: i32) -> String {
    pcsc_error_message(code)
        .map(str::to_string)
        .unwrap_or_else(|| format!("Unknown error: 0x{:08X}", code as u32))
}

#[cfg(feature = "pcsc_helper")]
fn map_share_mode(value: PcscShareMode) -> platform_pcsc::ShareMode {
    match value {
        PcscShareMode::Exclusive => platform_pcsc::ShareMode::Exclusive,
        PcscShareMode::Shared => platform_pcsc::ShareMode::Shared,
        PcscShareMode::Direct => platform_pcsc::ShareMode::Direct,
    }
}

#[cfg(feature = "pcsc_helper")]
fn map_protocols(value: PcscProtocols) -> platform_pcsc::Protocols {
    let mut protocols = platform_pcsc::Protocols::UNDEFINED;
    if value.contains(PcscProtocol::T0) {
        protocols = platform_pcsc::Protocols(protocols.0 | platform_pcsc::Protocols::T0.0);
    }
    if value.contains(PcscProtocol::T1) {
        protocols = platform_pcsc::Protocols(protocols.0 | platform_pcsc::Protocols::T1.0);
    }
    if value.contains(PcscProtocol::Raw) {
        protocols = platform_pcsc::Protocols(protocols.0 | platform_pcsc::Protocols::RAW.0);
    }
    protocols
}

#[cfg(feature = "pcsc_helper")]
fn map_disposition(value: PcscDisposition) -> platform_pcsc::Disposition {
    match value {
        PcscDisposition::LeaveCard => platform_pcsc::Disposition::LeaveCard,
        PcscDisposition::ResetCard => platform_pcsc::Disposition::ResetCard,
        PcscDisposition::UnpowerCard => platform_pcsc::Disposition::UnpowerCard,
        PcscDisposition::EjectCard => platform_pcsc::Disposition::EjectCard,
    }
}

#[cfg(feature = "pcsc_helper")]
fn map_protocol(value: platform_pcsc::Protocol) -> PcscProtocol {
    match value {
        platform_pcsc::Protocol::T0 => PcscProtocol::T0,
        platform_pcsc::Protocol::T1 => PcscProtocol::T1,
        platform_pcsc::Protocol::Raw => PcscProtocol::Raw,
    }
}

#[cfg(feature = "pcsc_helper")]
fn map_attribute(value: PcscAttribute) -> platform_pcsc::Attribute {
    match value {
        PcscAttribute::VendorName => platform_pcsc::Attribute::VendorName,
        PcscAttribute::VendorIfdType => platform_pcsc::Attribute::VendorIfdType,
        PcscAttribute::VendorIfdVersion => platform_pcsc::Attribute::VendorIfdVersion,
        PcscAttribute::VendorIfdSerialNo => platform_pcsc::Attribute::VendorIfdSerialNo,
        PcscAttribute::IccTypePerAtr => platform_pcsc::Attribute::IccTypePerAtr,
    }
}

#[cfg(feature = "pcsc_helper")]
pub(super) struct SystemPcscBackend;

#[cfg(feature = "pcsc_helper")]
struct SystemPcscCard {
    inner: Box<dyn platform_pcsc::Card>,
}

#[cfg(feature = "pcsc_helper")]
impl PcscCard for SystemPcscCard {
    fn reconnect(
        &mut self,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
        disposition: PcscDisposition,
    ) -> Result<(), i32> {
        self.inner.reconnect(
            map_share_mode(share_mode),
            map_protocols(preferred_protocols),
            map_disposition(disposition),
        )
    }

    fn status2_owned(&self) -> Result<PcscCardStatus, i32> {
        self.inner.status2_owned().map(|status| PcscCardStatus {
            present: status.present,
            atr: status.atr,
            protocol: status.protocol.map(map_protocol),
        })
    }

    fn get_attribute_owned(&self, attribute: PcscAttribute) -> Result<Vec<u8>, i32> {
        self.inner.get_attribute_owned(map_attribute(attribute))
    }

    fn transmit(&self, send_buffer: &[u8], receive_capacity: usize) -> Result<Vec<u8>, i32> {
        self.inner.transmit(send_buffer, receive_capacity)
    }

    fn control(
        &self,
        control_code: u64,
        send_buffer: &[u8],
        receive_capacity: usize,
    ) -> Result<Vec<u8>, i32> {
        self.inner
            .control(control_code, send_buffer, receive_capacity)
    }
}

#[cfg(feature = "pcsc_helper")]
impl PcscBackend for SystemPcscBackend {
    fn list_readers_owned(&self) -> Result<Vec<String>, i32> {
        platform_pcsc::Backend::list_readers_owned(&platform_pcsc::SystemBackend)
    }

    fn connect(
        &self,
        reader: &str,
        share_mode: PcscShareMode,
        preferred_protocols: PcscProtocols,
    ) -> Result<Box<dyn PcscCard>, i32> {
        let card = platform_pcsc::Backend::connect(
            &platform_pcsc::SystemBackend,
            reader,
            map_share_mode(share_mode),
            map_protocols(preferred_protocols),
        )?;
        Ok(Box::new(SystemPcscCard { inner: card }))
    }
}
