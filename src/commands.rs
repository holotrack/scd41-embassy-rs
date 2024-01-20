pub const STOP_PERIODIC_MEASUREMENT: [u8; 2] = [0x3f, 0x86];
pub const ADDR: u8 = 0x62; // default addr
pub const WAKE_UP: [u8; 2] = [0x36, 0xf6];
pub const REINIT: [u8; 2] = [0x36, 0x46];
pub const START_PERIODIC_MESUREMENT: [u8; 2] = [0x21, 0xb1];
pub const READ_MEASUREMENT: [u8; 2] = [0xec, 0x05];