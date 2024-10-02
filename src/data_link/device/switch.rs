use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{data_link::{ethernet_interface::{EthernetFrame, EthernetInterface}, mac_address::MacAddress}, is_multicast_or_broadcast, mac_addr, mac_broadcast_addr, physical::ethernet_port::EthernetPort};

#[derive(Debug)]
enum StpPortState {
    Discarding, // No forwarded frames, no bpdus, no learning mac addresses
    Learning,   // No forwarded frames, receives and transmits BPDUs, learning mac addresses
    Forwarding, // Forwarded frames, receives and transmits BPDUs learning mac addresses
}

#[derive(Debug)]
enum StpPortRole {
    Root,           // Lowest cost to root bridge
    Designated,     // Best path to paticular segment
    Alternate,      // Backup path to root bridge
    Backup,         // Backup path to the designated port
    Disabled,       // Port is not participating in STP
}

#[derive(Debug)]
struct SwitchPort {
    interface: EthernetInterface,
    stp_state: StpPortState,
    stp_role: StpPortRole,
}

/// A layer two switch that simply forwards Ethernet frames to the correct interface.
/// 
/// Does not have any layer three functionality.
/// 
/// Implements IEEE 802.1W Rapid Spanning Tree Protocol (RSTP) to prevent loops.
pub struct Switch {
    ports: [RefCell<SwitchPort>; 32],     // 32 physical ports
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

        let ports: [RefCell<SwitchPort>; 32] = (0..32)
            .map(|i| RefCell::new(SwitchPort {
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

    /// Connects two interfaces together via EthernetPorts (bi-directional).
    pub fn connect(&mut self, port: usize, other_port: &mut EthernetInterface) {
       self.ports[port].borrow_mut().interface.connect(other_port);
    }

    /// Forwards the given frame to the correct interface.
    /// 
    /// If the destination MAC address is not in the table, the frame is flooded to all interfaces (aside from the one it came from).
    pub fn receive(&mut self) {
        for (index, interface) in self.ports.iter().enumerate() {
            let frames = interface.borrow_mut().interface.receive();
            if frames.is_empty() {
                continue;
            }

            for frame in frames {

                // A source address cannot be a multicast or broadcast address
                if is_multicast_or_broadcast!(frame.source_address()) {
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

    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.ports.iter().map(|i| i.borrow().interface.port()).collect()
    }
}