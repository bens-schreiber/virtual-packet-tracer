use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{ethernet::{interface::*, *}, is_mac_multicast_or_broadcast, mac_addr};

use super::cable::*;


/// Spanning Tree Protocol (STP) Port States
#[derive(Debug)]
enum StpPortState {
    Discarding, // No forwarded frames, no bpdus, no learning mac addresses
    Learning,   // No forwarded frames, receives and transmits BPDUs, learning mac addresses
    Forwarding, // Forwarded frames, receives and transmits BPDUs learning mac addresses
}


/// Spanning Tree Protocol (STP) Port Roles
#[derive(Debug)]
enum StpPortRole {
    Root,           // Lowest cost to root bridge
    Designated,     // Best path to paticular segment
    Alternate,      // Backup path to root bridge
    Backup,         // Backup path to the designated port
    Disabled,       // Port is not participating in STP
}


/// An ethernet interface that participates in the Spanning Tree Protocol (STP).
#[derive(Debug)]
struct StpPort {
    interface: EthernetInterface,
    stp_state: StpPortState,
    stp_role: StpPortRole,
}


/// A layer two switch; forwards Ethernet frames to the correct interface.
/// 
/// Implements IEEE 802.1W Rapid Spanning Tree Protocol (RSTP) to prevent loops.
pub struct Switch {
    ports: [RefCell<StpPort>; 32],     // 32 physical ports
    table: HashMap<MacAddress, usize>,    // maps an address to the interface it's connected to.
    
    // Bridge Protocol Data Unit (BPDU) fields
    // mac_address: MacAddress,
    // bridge_priority: u16,
}

impl Switch {

    /// Creates a new switch with 32 interfaces, each with a unique MAC address based on the given seed.
    /// 
    /// All ports are in the forwarding state by default.
    pub fn from_seed(mac_seed: u8) -> Switch {

        let ports: [RefCell<StpPort>; 32] = (0..32)
            .map(|i| RefCell::new(StpPort {
                interface: EthernetInterface::new(mac_addr!(mac_seed + i + 1)),
                stp_state: StpPortState::Forwarding,
                stp_role: StpPortRole::Root,
            }))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
    
        Switch {
            ports,
            table: HashMap::new(),
            // root_cost: 0,
            // bridge_priority: 0,
            // root_mac,
        }
    }

    /// Returns a list of all the EthernetPorts connected to the switch.
    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.ports.iter().map(|i| i.borrow().interface.port()).collect()
    }

    /// Connects two interfaces together via EthernetPorts (bi-directional).
    pub fn connect(&mut self, port: usize, other_port: &mut EthernetInterface) {
       self.ports[port].borrow_mut().interface.connect(other_port);
    }

    /// Forwards all incoming frames to the correct interface based on the destination MAC address.
    /// 
    /// If the destination MAC address is not in the table, the frame is flooded to all interfaces (aside from the one it came from).
    pub fn forward(&mut self) {
        for (index, interface) in self.ports.iter().enumerate() {
            let frames = interface.borrow_mut().interface.receive();
            if frames.is_empty() {
                continue;
            }

            for frame in frames {

                // A source address cannot be a multicast or broadcast address
                if is_mac_multicast_or_broadcast!(frame.source_address()) {
                    continue;
                }

                // TODO: Implement STP
                let f = match frame {
                    EthernetFrame::Ethernet2(frame) => frame,
                    _ => continue  // Discard non-Ethernet2 frames
                };

                // If the sender MAC address is not in the table, add it.
                if !self.table.contains_key(&f.source_address) {
                    self.table.insert(f.source_address, index);
                }

                // If the destination MAC address is in the table, forward the mapped interface
                if let Some(destination_index) = self.table.get(&f.destination_address) {
                    self.ports[*destination_index].borrow_mut().interface.sendv(f.source_address, f.destination_address, f.ether_type, f.data);
                }

                // Destination isn't in table, flood to all interfaces (except the one it came from)
                else {
                    for (i, other_interface) in self.ports.iter().enumerate() {
                        if i != index {
                            other_interface.borrow_mut().interface.sendv(f.source_address, f.destination_address, f.ether_type, f.data.clone());
                        }
                    }
                }
            }
        }
    }
}


pub struct BpduFrame {
    pub destination_address: MacAddress,
    pub source_address: MacAddress,
    pub protocol_id: u16,
    pub version: u8,
    pub bpdu_type: u8,
    pub flags: u8,
    pub root_id: u64,
    pub root_path_cost: u32,
    pub bridge_id: u64,
    pub port_id: u16,
    pub message_age: u16,
    pub max_age: u16,
    pub hello_time: u16,
    pub forward_delay: u16,
}

impl BpduFrame {
    pub fn new(destination_address: MacAddress, source_address: MacAddress, protocol_id: u16, version: u8, bpdu_type: u8, flags: u8, root_id: u64, root_path_cost: u32, bridge_id: u64, port_id: u16, message_age: u16, max_age: u16, hello_time: u16, forward_delay: u16) -> BpduFrame {
        BpduFrame {
            destination_address,
            source_address,
            protocol_id,
            version,
            bpdu_type,
            flags,
            root_id,
            root_path_cost,
            bridge_id,
            port_id,
            message_age,
            max_age,
            hello_time,
            forward_delay,
        }
    }
}

impl ByteSerialize for BpduFrame {

    fn from_bytes(bytes: Vec<u8>) -> Result<BpduFrame, std::io::Error> {
        if bytes.len() < 35 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Insufficient bytes for BPDU frame; Runt frame."));
        }

        if bytes.len() > 35 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Oversized BPDU frame; Giant frame."));
        }

        let destination_address = bytes[0..6].try_into().unwrap();
        let source_address = bytes[6..12].try_into().unwrap();
        let protocol_id = u16::from_be_bytes(bytes[12..14].try_into().unwrap());
        let version = bytes[14];
        let bpdu_type = bytes[15];
        let flags = bytes[16];
        let root_id = u64::from_be_bytes(bytes[17..25].try_into().unwrap());
        let root_path_cost = u32::from_be_bytes(bytes[25..29].try_into().unwrap());
        let bridge_id = u64::from_be_bytes(bytes[29..37].try_into().unwrap());
        let port_id = u16::from_be_bytes(bytes[37..39].try_into().unwrap());
        let message_age = u16::from_be_bytes(bytes[39..41].try_into().unwrap());
        let max_age = u16::from_be_bytes(bytes[41..43].try_into().unwrap());
        let hello_time = u16::from_be_bytes(bytes[43..45].try_into().unwrap());
        let forward_delay = u16::from_be_bytes(bytes[45..47].try_into().unwrap());

        Ok(BpduFrame {
            destination_address,
            source_address,
            protocol_id,
            version,
            bpdu_type,
            flags,
            root_id,
            root_path_cost,
            bridge_id,
            port_id,
            message_age,
            max_age,
            hello_time,
            forward_delay,
        })
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend_from_slice(&self.destination_address);
        bytes.extend_from_slice(&self.source_address);
        bytes.extend_from_slice(&self.protocol_id.to_be_bytes());
        bytes.push(self.version);
        bytes.push(self.bpdu_type);
        bytes.push(self.flags);
        bytes.extend_from_slice(&self.root_id.to_be_bytes());
        bytes.extend_from_slice(&self.root_path_cost.to_be_bytes());
        bytes.extend_from_slice(&self.bridge_id.to_be_bytes());
        bytes.extend_from_slice(&self.port_id.to_be_bytes());
        bytes.extend_from_slice(&self.message_age.to_be_bytes());
        bytes.extend_from_slice(&self.max_age.to_be_bytes());
        bytes.extend_from_slice(&self.hello_time.to_be_bytes());
        bytes.extend_from_slice(&self.forward_delay.to_be_bytes());

        bytes
    }
}

/// BPDU MAC address for Spanning Tree Protocol
#[macro_export]
macro_rules! mac_bpdu_addr {
    () => {
        [0x01, 0x80, 0xC2, 0x00, 0x00, 0x00]
    };
}