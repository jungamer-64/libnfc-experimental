use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct DriverCaps(u8);

impl DriverCaps {
    pub const NONE: Self = Self(0);
    pub const SCAN: Self = Self(1 << 0);
    pub const OPEN: Self = Self(1 << 1);

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl BitOr for DriverCaps {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for DriverCaps {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for DriverCaps {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for DriverCaps {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct DeviceCaps(u64);

impl DeviceCaps {
    pub const NONE: Self = Self(0);
    pub const INFO: Self = Self(1 << 0);
    pub const SET_PROPERTY_BOOL: Self = Self(1 << 1);
    pub const SET_PROPERTY_INT: Self = Self(1 << 2);
    pub const SUPPORTED_MODULATIONS: Self = Self(1 << 3);
    pub const SUPPORTED_BAUD_RATES: Self = Self(1 << 4);
    pub const INITIATOR_INIT: Self = Self(1 << 5);
    pub const INITIATOR_INIT_SECURE_ELEMENT: Self = Self(1 << 6);
    pub const SELECT_PASSIVE_TARGET: Self = Self(1 << 7);
    pub const POLL_TARGET: Self = Self(1 << 8);
    pub const SELECT_DEP_TARGET: Self = Self(1 << 9);
    pub const DESELECT_TARGET: Self = Self(1 << 10);
    pub const TARGET_IS_PRESENT: Self = Self(1 << 11);
    pub const TARGET_INIT: Self = Self(1 << 12);
    pub const TRANSCEIVE_BYTES: Self = Self(1 << 13);
    pub const TRANSCEIVE_BITS: Self = Self(1 << 14);
    pub const TRANSCEIVE_BYTES_TIMED: Self = Self(1 << 15);
    pub const TRANSCEIVE_BITS_TIMED: Self = Self(1 << 16);
    pub const TARGET_SEND_BYTES: Self = Self(1 << 17);
    pub const TARGET_RECEIVE_BYTES: Self = Self(1 << 18);
    pub const TARGET_SEND_BITS: Self = Self(1 << 19);
    pub const TARGET_RECEIVE_BITS: Self = Self(1 << 20);
    pub const ABORT_COMMAND: Self = Self(1 << 21);
    pub const IDLE: Self = Self(1 << 22);
    pub const POWERDOWN: Self = Self(1 << 23);
    pub const PN53X_TRANSCEIVE: Self = Self(1 << 24);
    pub const PN53X_READ_REGISTER: Self = Self(1 << 25);
    pub const PN53X_WRITE_REGISTER: Self = Self(1 << 26);
    pub const PN532_SAM_CONFIGURATION: Self = Self(1 << 27);

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl BitOr for DeviceCaps {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for DeviceCaps {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for DeviceCaps {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAndAssign for DeviceCaps {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}
