use super::ethernet_frame::MacAddress;
use crate::network::ipv4::Ipv4Address;

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

/// IEEE Arp Frame
pub struct ArpFrame {
    hardware_type: u16,
    protocol_type: u16,
    hardware_size: u8,
    protocol_size: u8,
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
            hardware_type: 1,       // Ethernet
            protocol_type: 0x0800,  // Ipv4
            hardware_size: 6,
            protocol_size: 4,
            opcode,
            sender_mac,
            sender_ip,
            target_mac,
            target_ip,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
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

    pub fn from_bytes(bytes: &[u8]) -> Result<ArpFrame, &'static str> {
        if bytes.len() != 28 {
            return Err("ARP frame does not have 28 bytes");
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
            hardware_type: hardware_type,
            protocol_type: protocol_type,
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