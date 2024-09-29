use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{data_link::{ethernet_frame::MacAddress, ethernet_interface::EthernetInterface}, mac_addr, mac_broadcast_addr, physical::ethernet_port::EthernetPort};

/// A layer two switch that simply forwards Ethernet frames to the correct interface.
/// 
/// Does not have any layer three functionality.
pub struct Switch {
    interfaces: [RefCell<EthernetInterface>; 32],     // 32 physical ports
    table: HashMap<MacAddress, usize>,                // maps an address to the interface it's connected to.
}

impl Switch {

    /// Creates a new switch with 32 interfaces, each with a unique MAC address based on the given seed.
    pub fn from_seed(mac_seed: u8) -> Switch {
        let interfaces: [RefCell<EthernetInterface>; 32] = (0..32)
            .map(|i| RefCell::new(EthernetInterface::new(mac_addr!(mac_seed + i))))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap_or_else(|v: Vec<_>| panic!("Expected a Vec of length 32, but it was {}", v.len()));
    
        Switch {
            interfaces,
            table: HashMap::new(),
        }
    }

    /// Connects two interfaces together via EthernetPorts (bi-directional).
    pub fn connect(&mut self, port: usize, other_port: &mut EthernetInterface) {
       self.interfaces[port].borrow_mut().connect(other_port);
    }

    /// Forwards the given frame to the correct interface.
    /// 
    /// If the destination MAC address is not in the table, the frame is flooded to all interfaces (aside from the one it came from).
    pub fn receive(&mut self) {
        for (index, interface) in self.interfaces.iter().enumerate() {
            let frames = interface.borrow_mut().receive();
            if frames.is_empty() {
                continue;
            }

            for f in frames {

                // A source address can never be the broadcast address.
                if f.source_address == mac_broadcast_addr!() {
                    continue; // Discard 
                }

                // If the sender MAC address is not in the table, add it.
                if !self.table.contains_key(&f.source_address) {
                    self.table.insert(f.source_address, index);
                }

                // If the destination MAC address is in the table, forward the mapped interface
                if let Some(destination_index) = self.table.get(&f.destination_address) {
                    self.interfaces[*destination_index].borrow_mut().sends(f.source_address, f.destination_address, f.ether_type, f.data());
                }

                // Destination isn't in table, flood to all interfaces (except the one it came from)
                else {
                    for (i, other_interface) in self.interfaces.iter().enumerate() {
                        if i != index {
                            other_interface.borrow_mut().sends(f.source_address, f.destination_address, f.ether_type, f.data());
                        }
                    }
                }
            }
        }
    }

    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.interfaces.iter().map(|i| i.borrow().port()).collect()
    }
}