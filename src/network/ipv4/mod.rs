use crate::network::{ethernet::ByteSerializable, ethernet::MacAddress};

pub mod interface;

/// Internet Protocol version 4 (IPv4) address
pub type Ipv4Address = [u8; 4];

#[macro_export]
macro_rules! localhost {
    () => {
        [127, 0, 0, 1]
    };
}

/// 224.0.0.0 -> 239.255.255.255 or 255.255.255.255
#[macro_export]
macro_rules! is_ipv4_multicast_or_broadcast {
    ($address:expr) => {{
        if $address.len() != 4 {
            false
        } else if (224..=239).contains(&($address[0])) {
            true
        } else {
            $address == [255, 255, 255, 255]
        }
    }};
}

pub enum Ipv4Protocol {
    Icmp = 1,
    Rip = 17,
    Test = 255,
}

impl From<u8> for Ipv4Protocol {
    fn from(item: u8) -> Self {
        match item {
            1 => Self::Icmp,
            17 => Self::Rip,
            255 => Self::Test,
            _ => panic!("Invalid Ipv4 protocol"),
        }
    }
}

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
    pub fn new(
        source: Ipv4Address,
        destination: Ipv4Address,
        ttl: u8,
        data: Vec<u8>,
        protocol: Ipv4Protocol,
    ) -> Self {
        Self {
            version_hlen: 0x45, // Ipv4, 5 words
            tos: 0,
            total_length: 20 + data.len() as u16,
            id: 0,
            flags_fragment_offset: 0,
            ttl,
            protocol: protocol as u8,
            checksum: 0, // TODO: Calculate checksum
            source,
            destination,
            option: Vec::new(),
            data,
        }
    }

    pub fn test(source: Ipv4Address, destination: Ipv4Address, ttl: u8, data: u8) -> Self {
        Self::new(source, destination, ttl, vec![data], Ipv4Protocol::Test)
    }
}

impl ByteSerializable for Ipv4Frame {
    fn from_bytes(bytes: Vec<u8>) -> Result<Self, std::io::Error> {
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

        Ok(Self {
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
}

/// Address Resolution Protocol (ARP) operation code
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ArpOperation {
    Request = 1,
    Reply = 2,
}

impl From<u16> for ArpOperation {
    fn from(item: u16) -> Self {
        match item {
            1 => Self::Request,
            2 => Self::Reply,
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
    ) -> Self {
        Self {
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

impl ByteSerializable for ArpFrame {
    fn from_bytes(bytes: Vec<u8>) -> Result<Self, std::io::Error> {
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

        Ok(Self {
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

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(self.hardware_type).to_be_bytes());
        bytes.extend_from_slice(&(self.protocol_type).to_be_bytes());
        bytes.push(self.hardware_size);
        bytes.push(self.protocol_size);
        bytes.extend_from_slice(&(self.opcode as u16).to_be_bytes());
        bytes.extend_from_slice(&self.sender_mac);
        bytes.extend_from_slice(&self.sender_ip);
        bytes.extend_from_slice(&self.target_mac);
        bytes.extend_from_slice(&self.target_ip);
        bytes
    }
}

#[derive(Debug, PartialEq)]
pub enum IcmpType {
    EchoRequest = 8,
    EchoReply = 0,
    Unreachable = 3,
}

#[derive(Debug, PartialEq)]
pub struct IcmpFrame {
    pub icmp_type: u8, // 0: Echo reply, 3: destination unreachable, 8: Echo request
    pub code: u8,
    pub checksum: u16,
    pub identifier: u16,
    pub sequence_number: u16,
    pub data: Vec<u8>,
}

impl IcmpFrame {
    pub fn new(
        icmp_type: u8,
        icmp_code: u8,
        identifier: u16,
        sequence_number: u16,
        data: Vec<u8>,
    ) -> Self {
        Self {
            icmp_type,
            code: icmp_code,
            checksum: 0, // TODO: Calculate checksum
            identifier,
            sequence_number,
            data,
        }
    }

    pub fn echo_request(identifier: u16, sequence_number: u16, data: Vec<u8>) -> Self {
        Self::new(8, 0, identifier, sequence_number, data)
    }

    pub fn echo_reply(identifier: u16, sequence_number: u16, data: Vec<u8>) -> Self {
        Self::new(0, 0, identifier, sequence_number, data)
    }

    pub fn destination_unreachable(code: u8, data: Vec<u8>) -> Self {
        Self::new(3, code, 0, 0, data)
    }
}

impl ByteSerializable for IcmpFrame {
    fn from_bytes(bytes: Vec<u8>) -> Result<Self, std::io::Error> {
        if bytes.len() < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Insufficient bytes for ICMP frame; Runt frame.",
            ));
        }

        if bytes.len() > 65535 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Oversized ICMP frame; Giant frame.",
            ));
        }

        let icmp_type = bytes[0];
        let code = bytes[1];
        let checksum = u16::from_be_bytes([bytes[2], bytes[3]]);
        let identifier = u16::from_be_bytes([bytes[4], bytes[5]]);
        let sequence_number = u16::from_be_bytes([bytes[6], bytes[7]]);
        let data = bytes[8..].to_vec();

        Ok(Self {
            icmp_type,
            code,
            checksum,
            identifier,
            sequence_number,
            data,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.icmp_type);
        bytes.push(self.code);
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes.extend_from_slice(&self.identifier.to_be_bytes());
        bytes.extend_from_slice(&self.sequence_number.to_be_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }
}
