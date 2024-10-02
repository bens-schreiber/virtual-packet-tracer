use crate::data_link::mac_address::MacAddress;

/// IEEE 802.3 Ethernet Frame
/// 
/// Used only for LLC frames
/// 
#[derive(Debug, PartialEq, Clone)]
pub struct Ethernet802_3Frame {
    preamble: [u8; 7],
    start_frame_delimiter: u8,
    pub destination_address: MacAddress,
    pub source_address: MacAddress,
    length: u16,
    dsap: u8,
    ssap: u8,
    control: u8,
    pub data: Vec<u8>,
    frame_check_sequence: u32,
}

impl Ethernet802_3Frame {
    pub fn new(destination_address: MacAddress, source_address: MacAddress, data: Vec<u8>) -> Ethernet802_3Frame {
        Ethernet802_3Frame {
            preamble: [0x55; 7],
            start_frame_delimiter: 0xD5,
            destination_address,
            source_address,
            length: data.len() as u16,
            dsap: 0x42,                     // Spanning Tree Protocol
            ssap: 0x42,                     // Spanning Tree Protocol
            control: 0x03,
            data,
            frame_check_sequence: 0,        // TODO: Calculate FCS
        }
    }
    
    /// Creates an EthernetFrame from a byte array
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Ethernet802_3Frame, std::io::Error>  {
        if bytes.len() < 64 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Insufficient bytes for Ethernet frame; Runt frame."));
        }

        if bytes.len() > 1518 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Oversized Ethernet frame; Giant frame."));
        }

        // Ignore the preamble and start frame delimiter. Unnecessary for virtual simulation.
        let preamble = [0x55; 7];
        let start_frame_delimiter = 0xD5;

        let destination_address = [bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13]];
        let source_address = [bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19]];

        let length = u16::from_be_bytes([bytes[20], bytes[21]]);

        let dsap = bytes[22];
        let ssap = bytes[23];
        let control = bytes[24];

        let data = bytes[25..bytes.len()-4].to_vec();

        let frame_check_sequence = u32::from_be_bytes([bytes[bytes.len()-4], bytes[bytes.len()-3], bytes[bytes.len()-2], bytes[bytes.len()-1]]);

        Ok(Ethernet802_3Frame {
            preamble,
            start_frame_delimiter,
            destination_address,
            source_address,
            length,
            dsap,
            ssap,
            control,
            data,
            frame_check_sequence,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.preamble);
        bytes.push(self.start_frame_delimiter);
        bytes.extend_from_slice(&self.destination_address);
        bytes.extend_from_slice(&self.source_address);
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.push(self.dsap);
        bytes.push(self.ssap);
        bytes.push(self.control);
        bytes.extend_from_slice(&self.data);
        bytes.extend_from_slice(&self.frame_check_sequence.to_be_bytes());
        bytes
    }
}