pub mod interface;

/// A data link physical address
pub type MacAddress = [u8; 6];

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

/// Broadcast MAC address
#[macro_export]
macro_rules! mac_broadcast_addr {
    () => {
        [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    };
}

/// Returns true if the address is a multicast or broadcast address
#[macro_export]
macro_rules! is_mac_multicast_or_broadcast {
    ($address:expr) => {
        $address[0] & 0x01 == 0x01 || $address == crate::mac_broadcast_addr!()
    };
}

/// An Ethernet frame that can be EthernetII or Ethernet802_3.
#[derive(Debug, PartialEq)]
pub enum EthernetFrame {
    Ethernet2(Ethernet2Frame),
    Ethernet802_3(Ethernet802_3Frame),
}

impl EthernetFrame {
    pub fn destination_address(&self) -> MacAddress {
        match self {
            EthernetFrame::Ethernet2(frame) => frame.destination_address,
            EthernetFrame::Ethernet802_3(frame) => frame.destination_address,
        }
    }

    pub fn source_address(&self) -> MacAddress {
        match self {
            EthernetFrame::Ethernet2(frame) => frame.source_address,
            EthernetFrame::Ethernet802_3(frame) => frame.source_address,
        }
    }
}

impl ByteSerializable for EthernetFrame {
    fn from_bytes(bytes: Vec<u8>) -> Result<Self, std::io::Error> {
        let ether_type_or_length = u16::from_be_bytes(bytes[20..22].try_into().unwrap());

        let frame = if ether_type_or_length >= 0x0600 {
            Ethernet2Frame::from_bytes(bytes).map(EthernetFrame::Ethernet2)
        } else {
            Ethernet802_3Frame::from_bytes(bytes).map(EthernetFrame::Ethernet802_3)
        };

        frame.map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid Ethernet frame")
        })
    }
}

/// Creates an EthernetII frame with the given destination address, source address, data, and ether type.
#[macro_export]
macro_rules! eth2 {
    ($destination_address:expr, $source_address:expr, $data:expr, $ether_type:expr) => {
        crate::network::ethernet::EthernetFrame::Ethernet2(
            crate::network::ethernet::Ethernet2Frame::new(
                $destination_address,
                $source_address,
                $data,
                $ether_type,
            ),
        )
    };
}

/// Creates a generic ethernet payload with a given value
#[cfg(test)]
#[macro_export]
macro_rules! eth2_data {
    ($value:expr) => {{
        vec![$value; 28]
    }};
}

/// Creates a generic ethernet payload with a given value
#[cfg(test)]
#[macro_export]
macro_rules! eth802_3_data {
    ($value:expr) => {{
        vec![$value; 46]
    }};
}

/// Ethernet II EtherType field
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum EtherType {
    Ipv4 = 0x0800,
    Arp = 0x0806,
    Debug = 0xFFFF,
}

impl From<u16> for EtherType {
    fn from(item: u16) -> Self {
        match item {
            0x0800 => Self::Ipv4,
            0x0806 => Self::Arp,
            _ => Self::Debug,
        }
    }
}

/// Ethernet II frame format
#[derive(Debug, PartialEq, Clone)]
pub struct Ethernet2Frame {
    pub preamble: [u8; 7],
    pub start_frame_delimiter: u8,
    pub destination_address: MacAddress,
    pub source_address: MacAddress,
    pub ether_type: EtherType,
    pub data: Vec<u8>,
    pub frame_check_sequence: u32,
}

impl Ethernet2Frame {
    pub fn new(
        destination_address: MacAddress,
        source_address: MacAddress,
        data: Vec<u8>,
        ether_type: EtherType,
    ) -> Self {
        Self {
            preamble: [0x55; 7],
            start_frame_delimiter: 0xD5,
            destination_address,
            source_address,
            ether_type,
            data,
            frame_check_sequence: 0, // TODO: Calculate FCS
        }
    }
}

impl ByteSerializable for Ethernet2Frame {
    fn from_bytes(bytes: Vec<u8>) -> Result<Ethernet2Frame, std::io::Error> {
        if bytes.len() < 46 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Insufficient bytes for Ethernet frame; Runt frame.",
            ));
        }

        if bytes.len() > 1500 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Oversized Ethernet frame; Giant frame.",
            ));
        }

        // Ignore the preamble and start frame delimiter. Unnecessary for virtual simulation.
        let preamble = [0x55; 7];
        let start_frame_delimiter = 0xD5;

        let destination_address = [
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13],
        ];
        let source_address = [
            bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19],
        ];

        let ether_type: EtherType = u16::from_be_bytes([bytes[20], bytes[21]]).into();

        let data = bytes[22..bytes.len() - 4].to_vec();

        let frame_check_sequence = u32::from_be_bytes([
            bytes[bytes.len() - 4],
            bytes[bytes.len() - 3],
            bytes[bytes.len() - 2],
            bytes[bytes.len() - 1],
        ]);

        Ok(Self {
            preamble,
            start_frame_delimiter,
            destination_address,
            source_address,
            ether_type,
            data,
            frame_check_sequence,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
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

/// IEEE 802.3 Ethernet Frame
#[derive(Debug, PartialEq, Clone)]
pub struct Ethernet802_3Frame {
    pub preamble: [u8; 7],
    pub start_frame_delimiter: u8,
    pub destination_address: MacAddress,
    pub source_address: MacAddress,
    pub length: u16,
    pub dsap: u8,
    pub ssap: u8,
    pub control: u8,
    pub data: Vec<u8>,
    pub frame_check_sequence: u32,
}

impl Ethernet802_3Frame {
    pub fn new(destination_address: MacAddress, source_address: MacAddress, data: Vec<u8>) -> Self {
        Self {
            preamble: [0x55; 7],
            start_frame_delimiter: 0xD5,
            destination_address,
            source_address,
            length: data.len() as u16,
            dsap: 0x42, // Spanning Tree Protocol
            ssap: 0x42, // Spanning Tree Protocol
            control: 0x03,
            data,
            frame_check_sequence: 0, // TODO: Calculate FCS
        }
    }
}

impl ByteSerializable for Ethernet802_3Frame {
    fn from_bytes(bytes: Vec<u8>) -> Result<Ethernet802_3Frame, std::io::Error> {
        if bytes.len() < 64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Insufficient bytes for Ethernet frame; Runt frame.",
            ));
        }

        if bytes.len() > 1518 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Oversized Ethernet frame; Giant frame.",
            ));
        }

        // Ignore the preamble and start frame delimiter. Unnecessary for virtual simulation.
        let preamble = [0x55; 7];
        let start_frame_delimiter = 0xD5;

        let destination_address = [
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13],
        ];
        let source_address = [
            bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19],
        ];

        let length = u16::from_be_bytes([bytes[20], bytes[21]]);

        let dsap = bytes[22];
        let ssap = bytes[23];
        let control = bytes[24];

        let data = bytes[25..bytes.len() - 4].to_vec();

        let frame_check_sequence = u32::from_be_bytes([
            bytes[bytes.len() - 4],
            bytes[bytes.len() - 3],
            bytes[bytes.len() - 2],
            bytes[bytes.len() - 1],
        ]);

        Ok(Self {
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

    fn to_bytes(&self) -> Vec<u8> {
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

pub trait ByteSerializable {
    // Convert a byte array to a struct
    fn from_bytes(bytes: Vec<u8>) -> Result<Self, std::io::Error>
    where
        Self: Sized;

    // Convert the struct to a byte array
    fn to_bytes(&self) -> Vec<u8> {
        Vec::new()
    }
}
