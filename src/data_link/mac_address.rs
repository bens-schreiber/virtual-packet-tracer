/// A data link physical address
pub type MacAddress = [u8; 6];

/// Broadcast MAC address
#[macro_export]
macro_rules! mac_broadcast_addr {
    () => {
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    };
}

/// Creates a MAC address from a u64
#[macro_export]
macro_rules! mac_addr {
    ($num:expr) => {{
        let num = $num as u64;
        [
            ((num >> 40) & 0xff) as u8,
            ((num >> 32) & 0xff) as u8,
            ((num >> 24) & 0xff) as u8,
            ((num >> 16) & 0xff) as u8,
            ((num >> 8) & 0xff) as u8,
            (num & 0xff) as u8,
        ]
    }};
}