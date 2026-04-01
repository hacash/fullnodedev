pub const P2P_HAND_SHAKE_MAGIC_NUMBER: u32 = 3148609527;
pub const P2P_MSG_DATA_MAX_SIZE: u32 = 1012 * 1024 * 32;

pub const MSG_REPORT_PEER: u8 = 1;
pub const MSG_ANSWER_PEER: u8 = 2;
pub const MSG_PING: u8 = 3;
pub const MSG_PONG: u8 = 4;
pub const MSG_REQUEST_NODE_KEY_FOR_PUBLIC_CHECK: u8 = 201;
pub const MSG_REQUEST_NEAREST_PUBLIC_NODES: u8 = 202;
pub const MSG_REMIND_ME_IS_PUBLIC: u8 = 151;
pub const MSG_CLOSE: u8 = 254;
pub const MSG_CUSTOMER: u8 = 255;
