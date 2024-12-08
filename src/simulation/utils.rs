use core::panic;

use raylib::{
    color::Color,
    ffi::{self, GuiIconName},
};

use crate::network::{
    ethernet::{ByteSerializable, EtherType, Ethernet2Frame, Ethernet802_3Frame, MacAddress},
    ipv4::Ipv4Frame,
};

pub fn draw_icon(icon: GuiIconName, pos_x: i32, pos_y: i32, pixel_size: i32, color: Color) {
    unsafe {
        ffi::GuiDrawIcon(
            icon as i32,
            pos_x,
            pos_y,
            pixel_size,
            ffi::Color {
                r: color.r,
                g: color.g,
                b: color.b,
                a: color.a,
            },
        );
    };
}

/// Creates a Raylib String (C-String) from a Rust string
pub fn rstr_from_string(s: String) -> std::ffi::CString {
    std::ffi::CString::new(s).expect("CString::new failed")
}

/// Converts a null terminated array of bytes to a string
pub fn array_to_string(array: &[u8]) -> String {
    let end = array.iter().position(|&c| c == 0).unwrap_or(array.len());
    let slice = &array[..end];
    String::from_utf8_lossy(slice).to_string()
}

pub enum PacketKind {
    Arp(Ethernet2Frame),
    Bpdu(Ethernet802_3Frame),
    Rip(Ethernet2Frame),
    Icmp(Ethernet2Frame),
}

impl PacketKind {
    pub fn source_mac(&self) -> MacAddress {
        match self {
            PacketKind::Arp(frame) => frame.source_address,
            PacketKind::Bpdu(frame) => frame.source_address,
            PacketKind::Rip(frame) => frame.source_address,
            PacketKind::Icmp(frame) => frame.source_address,
        }
    }

    // TODO: assumes the packet is something we can handle. Currently, there is no "custom" sending of frames, so
    // there is no need to handle unknown frames. This will need to be updated if some kind of custom frame sending is added.
    pub fn from_bytes(packet: &Vec<u8>) -> PacketKind {
        // Determine if the frame is EthernetII or Ethernet802_3
        let ether_type_or_length = u16::from_be_bytes(packet[20..22].try_into().unwrap());
        let eth_frame = if ether_type_or_length >= 0x0600 {
            Ethernet2Frame::from_bytes(packet.clone()).unwrap()
        } else {
            return PacketKind::Bpdu(Ethernet802_3Frame::from_bytes(packet.clone()).unwrap());
        };

        match eth_frame.ether_type {
            EtherType::Arp => PacketKind::Arp(eth_frame),
            EtherType::Ipv4 => {
                let ipv4_frame = Ipv4Frame::from_bytes(eth_frame.data.clone()).unwrap();
                match ipv4_frame.protocol {
                    1 => PacketKind::Icmp(eth_frame),
                    17 => PacketKind::Rip(eth_frame),
                    _ => panic!("Unknown protocol: {}", ipv4_frame.protocol),
                }
            }
            _ => panic!("Unknown ether type: {:?}", eth_frame.ether_type),
        }
    }
}
