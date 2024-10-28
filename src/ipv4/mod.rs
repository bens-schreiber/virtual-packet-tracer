use crate::{ethernet::ByteSerialize, ethernet::MacAddress};

pub mod interface;

/// Internet Protocol version 4 (IPv4) address
pub type Ipv4Address = [u8; 4];

/// A network layer frame for IPv4 communication
#[derive(Debug, PartialEq)]
pub struct Ipv4Frame {
    pub version_hlen: u8,  // 4 bits version, 4 bits header length
    pub tos: u8,           // Type of service
    pub total_length: u16, // Total length of the frame
    pub id: u16,
    pub flags_fragment_offset: u16, // 3 bits flags, 13 bits fragment offset
    pub ttl: u8,                    // Time to live
    pub protocol: u8,
    pub checksum: u16,
    pub source: Ipv4Address,
    pub destination: Ipv4Address,
    pub option: Vec<u8>,
    pub data: Vec<u8>,
}

impl Ipv4Frame {
    pub fn new(source: Ipv4Address, destination: Ipv4Address, data: Vec<u8>) -> Ipv4Frame {
        Ipv4Frame {
            version_hlen: 0x45, // Ipv4, 5 words
            tos: 0,
            total_length: 20 + data.len() as u16,
            id: 0,
            flags_fragment_offset: 0,
            ttl: 64, // Default TTL
            protocol: 0,
            checksum: 0, // TODO: Calculate checksum
            source,
            destination,
            option: Vec::new(),
            data,
        }
    }
}

impl ByteSerialize for Ipv4Frame {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.version_hlen);
        bytes.push(self.tos);
        bytes.extend_from_slice(&self.total_length.to_be_bytes());
        bytes.extend_from_slice(&self.id.to_be_bytes());
        bytes.extend_from_slice(&self.flags_fragment_offset.to_be_bytes());
        bytes.push(self.ttl);
        bytes.push(self.protocol);
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes.extend_from_slice(&self.source);
        bytes.extend_from_slice(&self.destination);
        bytes.extend_from_slice(&self.option);
        bytes.extend_from_slice(&self.data);
        bytes
    }

    fn from_bytes(bytes: Vec<u8>) -> Result<Ipv4Frame, std::io::Error> {
        if bytes.len() < 20 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Insufficient bytes for Ipv4 frame; Runt frame.",
            ));
        }

        if bytes.len() > 65535 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Oversized Ipv4 frame; Giant frame.",
            ));
        }

        let version_hlen = bytes[0];
        let tos = bytes[1];
        let total_length = u16::from_be_bytes([bytes[2], bytes[3]]);
        let id = u16::from_be_bytes([bytes[4], bytes[5]]);
        let flags_fragment_offset = u16::from_be_bytes([bytes[6], bytes[7]]);
        let ttl = bytes[8];
        let protocol = bytes[9];
        let checksum = u16::from_be_bytes([bytes[10], bytes[11]]);
        let source = [bytes[12], bytes[13], bytes[14], bytes[15]];
        let destination = [bytes[16], bytes[17], bytes[18], bytes[19]];
        let option = bytes[20..(version_hlen & 0x0F) as usize * 4].to_vec();
        let data = bytes[(version_hlen & 0x0F) as usize * 4..].to_vec();

        Ok(Ipv4Frame {
            version_hlen,
            tos,
            total_length,
            id,
            flags_fragment_offset,
            ttl,
            protocol,
            checksum,
            source,
            destination,
            option,
            data,
        })
    }
}

/// Address Resolution Protocol (ARP) operation code
#[repr(u16)]
#[derive(Copy, Clone, PartialEq)]
pub enum ArpOperation {
    Request = 1,
    Reply = 2,
}

impl From<u16> for ArpOperation {
    fn from(item: u16) -> Self {
        match item {
            1 => ArpOperation::Request,
            2 => ArpOperation::Reply,
            _ => panic!("Invalid ARP operation"),
        }
    }
}

/// Address Resolution Protocol (ARP)
pub struct ArpFrame {
    pub hardware_type: u16,
    pub protocol_type: u16,
    pub hardware_size: u8,
    pub protocol_size: u8,
    pub opcode: ArpOperation,
    pub sender_mac: MacAddress,
    pub sender_ip: Ipv4Address,
    pub target_mac: MacAddress,
    pub target_ip: Ipv4Address,
}

impl ArpFrame {
    pub fn new(
        opcode: ArpOperation,
        sender_mac: MacAddress,
        sender_ip: Ipv4Address,
        target_mac: MacAddress,
        target_ip: Ipv4Address,
    ) -> ArpFrame {
        ArpFrame {
            hardware_type: 1,      // Ethernet
            protocol_type: 0x0800, // Ipv4
            hardware_size: 6,
            protocol_size: 4,
            opcode,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        }
    }
}

impl ByteSerialize for ArpFrame {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(self.hardware_type as u16).to_be_bytes());
        bytes.extend_from_slice(&(self.protocol_type as u16).to_be_bytes());
        bytes.push(self.hardware_size);
        bytes.push(self.protocol_size);
        bytes.extend_from_slice(&(self.opcode as u16).to_be_bytes());
        bytes.extend_from_slice(&self.sender_mac);
        bytes.extend_from_slice(&self.sender_ip);
        bytes.extend_from_slice(&self.target_mac);
        bytes.extend_from_slice(&self.target_ip);
        bytes
    }

    fn from_bytes(bytes: Vec<u8>) -> Result<ArpFrame, std::io::Error> {
        if bytes.len() != 28 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid ARP frame",
            ));
        }

        let hardware_type = u16::from_be_bytes([bytes[0], bytes[1]]);
        let protocol_type = u16::from_be_bytes([bytes[2], bytes[3]]);
        let hardware_size = bytes[4];
        let protocol_size = bytes[5];
        let opcode = u16::from_be_bytes([bytes[6], bytes[7]]);
        let sender_mac = bytes[8..14].try_into().unwrap();
        let sender_ip = bytes[14..18].try_into().unwrap();
        let target_mac = bytes[18..24].try_into().unwrap();
        let target_ip = bytes[24..28].try_into().unwrap();

        Ok(ArpFrame {
            hardware_type,
            protocol_type,
            hardware_size,
            protocol_size,
            opcode: opcode.into(),
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        })
    }
}
