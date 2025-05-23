pub mod status;

use bincode::{Decode, Encode};
use defmt::Format;

use serde::{Deserialize, Serialize};
use status::Status;

use core::option::Option;

use crate::commands::CommandPacket;

#[derive(Debug, Clone, Copy, Encode, Decode, Format, Serialize, Deserialize)]
pub enum ApplicationPacket {
    Command(CommandPacket),
    Status(Status),
    PowerData {
        name: u16,
        time_stamp: u64,
        power: Option<u16>,
    },
    CurrentData {
        name: u16,
        time_stamp: u64,
        current: Option<(i8, u32)>,
    },
    VoltageData {
        name: u16,
        time_stamp: u64,
        voltage: Option<(u8, u32)>,
    },
}
