#[allow(non_upper_case_globals)]
pub const Diagnose: u8 = 0x00;
pub const PN53x_EXTENDED_FRAME__DATA_MAX_LEN: usize = 264;
pub const PN53X_REG_CIU_TxMode: u16 = 0x6302;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum pn532_sam_mode {
    PSM_NORMAL = 0x01,
    PSM_VIRTUAL_CARD = 0x02,
    PSM_WIRED_CARD = 0x03,
    PSM_DUAL_CARD = 0x04,
}
