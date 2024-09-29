#[repr(u16)]
#[derive(Debug, PartialEq, Clone, Copy)]
/// An Ethernet II frame type
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

/// Creates a generic ethernet payload
pub fn ether_payload(value: u8) -> Vec<u8> {
    vec![value; 28]
}

/// Ethernet II frame format
#[derive(Debug, PartialEq, Clone)]
pub struct EthernetFrame {
    preamble: [u8; 7],
    start_frame_delimiter: u8,
    pub destination_address: MacAddress,
    pub source_address: MacAddress,
    pub ether_type: EtherType,
    data: Vec<u8>,
    frame_check_sequence: u32,
}

impl EthernetFrame {
    pub fn new(destination_address: MacAddress, source_address: MacAddress, data: Vec<u8>, ether_type: EtherType) -> EthernetFrame {
        EthernetFrame {
            preamble: [0x55; 7],
            start_frame_delimiter: 0xD5,
            destination_address,
            source_address,
            ether_type,
            data,
            frame_check_sequence: 0,        // TODO: Calculate FCS
        }
    }

    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }
    
    /// Creates an EthernetFrame from a byte array
    pub fn from_bytes(bytes: &Vec<u8>) -> Result<EthernetFrame, std::io::Error>  {
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

        Ok(EthernetFrame {
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
