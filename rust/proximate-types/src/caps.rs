use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct DriverCaps: u8 {
        const NONE = 0;
        const SCAN = 1 << 0;
        const OPEN = 1 << 1;
    }
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
    pub struct DeviceCaps: u64 {
        const NONE = 0;
        const INFO = 1 << 0;
        const SET_PROPERTY_BOOL = 1 << 1;
        const SET_PROPERTY_INT = 1 << 2;
        const SUPPORTED_MODULATIONS = 1 << 3;
        const SUPPORTED_BAUD_RATES = 1 << 4;
        const INITIATOR_INIT = 1 << 5;
        const INITIATOR_INIT_SECURE_ELEMENT = 1 << 6;
        const SELECT_PASSIVE_TARGET = 1 << 7;
        const POLL_TARGET = 1 << 8;
        const SELECT_DEP_TARGET = 1 << 9;
        const DESELECT_TARGET = 1 << 10;
        const TARGET_IS_PRESENT = 1 << 11;
        const TARGET_INIT = 1 << 12;
        const TRANSCEIVE_BYTES = 1 << 13;
        const TRANSCEIVE_BITS = 1 << 14;
        const TRANSCEIVE_BYTES_TIMED = 1 << 15;
        const TRANSCEIVE_BITS_TIMED = 1 << 16;
        const TARGET_SEND_BYTES = 1 << 17;
        const TARGET_RECEIVE_BYTES = 1 << 18;
        const TARGET_SEND_BITS = 1 << 19;
        const TARGET_RECEIVE_BITS = 1 << 20;
        const ABORT_COMMAND = 1 << 21;
        const IDLE = 1 << 22;
        const POWERDOWN = 1 << 23;
        const PN53X_TRANSCEIVE = 1 << 24;
        const PN53X_READ_REGISTER = 1 << 25;
        const PN53X_WRITE_REGISTER = 1 << 26;
        const PN532_SAM_CONFIGURATION = 1 << 27;
    }
}
