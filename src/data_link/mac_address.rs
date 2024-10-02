/// A data link physical address
pub type MacAddress = [u8; 6];

/// Broadcast MAC address
#[macro_export]
macro_rules! mac_broadcast_addr {
    () => {
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    };
}

/// BPDU MAC address for Spanning Tree Protocol
#[macro_export]
macro_rules! mac_bpdu_addr {
    () => {
        [0x01, 0x80, 0xC2, 0x00, 0x00, 0x00]
    };
}

/// Creates a MAC address from a u64
#[macro_export]
macro_rules! mac_addr {
    ($num:expr) => {{
        let num = $num as u64;
        [
            (((num >> 40) & 0xff) as u8 & 0xFE),  // Clear the least significant bit to avoid multicast
            ((num >> 32) & 0xff) as u8,
            ((num >> 24) & 0xff) as u8,
            ((num >> 16) & 0xff) as u8,
            ((num >> 8) & 0xff) as u8,
            (num & 0xff) as u8,
        ]
    }};
}

/// Returns true if the address is a multicast or broadcast address
#[macro_export]
macro_rules! is_multicast_or_broadcast {
    ($address:expr) => {
        $address[0] & 0x01 == 0x01 || $address == mac_broadcast_addr!()
    };
}
