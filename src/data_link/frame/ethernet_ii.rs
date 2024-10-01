use crate::data_link::mac_address::MacAddress;

/// Creates a generic ethernet payload with a given value
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! eth2_data {
    ($value:expr) => {{
        vec![$value; 28]
    }};
}

/// Creates a generic ethernet payload with a given value
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! eth802_3_data {
    ($value:expr) => {{
        vec![$value; 46]
    }};
}

/// Ethernet II frame format
/// 
/// Used for all Ethernet frames except LLC frames
/// 
#[derive(Debug, PartialEq, Clone)]
pub struct Ethernet2Frame {
    preamble: [u8; 7],
    start_frame_delimiter: u8,
    pub destination_address: MacAddress,
    pub source_address: MacAddress,
    pub ether_type: EtherType,
    pub data: Vec<u8>,
    frame_check_sequence: u32,
}

impl Ethernet2Frame {
    pub fn new(destination_address: MacAddress, source_address: MacAddress, data: Vec<u8>, ether_type: EtherType) -> Ethernet2Frame {
        Ethernet2Frame {
            preamble: [0x55; 7],
            start_frame_delimiter: 0xD5,
            destination_address,
            source_address,
            ether_type,
            data,
            frame_check_sequence: 0,        // TODO: Calculate FCS
        }
    }

    /// Creates an EthernetFrame from a byte array
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Ethernet2Frame, std::io::Error>  {
        if bytes.len() < 46 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Insufficient bytes for Ethernet frame; Runt frame."));
        }

        if bytes.len() > 1500 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Oversized Ethernet frame; Giant frame."));
        }

        // Ignore the preamble and start frame delimiter. Unnecessary for virtual simulation.
        let preamble = [0x55; 7];
        let start_frame_delimiter = 0xD5;

        let destination_address = [bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13]];
        let source_address = [bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19]];

        let ether_type: EtherType = u16::from_be_bytes([bytes[20], bytes[21]]).into();

        let data = bytes[22..bytes.len()-4].to_vec();

        let frame_check_sequence = u32::from_be_bytes([bytes[bytes.len()-4], bytes[bytes.len()-3], bytes[bytes.len()-2], bytes[bytes.len()-1]]);

        Ok(Ethernet2Frame {
            preamble,
            start_frame_delimiter,
            destination_address,
            source_address,
            ether_type,
            data,
            frame_check_sequence,
        })
    }

    /// Converts the EthernetFrame to a byte array
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let ether_type = self.ether_type.clone() as u16;

        bytes.extend_from_slice(&self.preamble);
        bytes.push(self.start_frame_delimiter);
        bytes.extend_from_slice(&self.destination_address);
        bytes.extend_from_slice(&self.source_address);
        bytes.extend_from_slice(&ether_type.to_be_bytes());
        bytes.extend_from_slice(&self.data);
        bytes.extend_from_slice(&self.frame_check_sequence.to_be_bytes());

        bytes
    }
}

#[repr(u16)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum EtherType {
    Ipv4 = 0x0800,
    Arp = 0x0806,
    Debug = 0xFFFF,
}

impl From<u16> for EtherType {
    fn from(item: u16) -> Self {
        match item {
            0x0800 => EtherType::Ipv4,
            0x0806 => EtherType::Arp,
            _ => EtherType::Debug,
        }
    }
}