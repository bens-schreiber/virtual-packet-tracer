use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ethernet::{interface::*, *},
    is_mac_multicast_or_broadcast, mac_addr,
};

use super::cable::*;

#[derive(Debug, PartialEq)]
enum StpPortRole {
    Root,
    Designated,
    Alternate,
    Backup,
    Disabled, // Nothing in, nothing out
}

#[derive(Debug, PartialEq)]
enum StpPortState {
    Discarding, // No forwarded frames, receives and transmits bpdus, no learning mac addresses
    Learning,   // No forwarded frames, receives and transmits BPDUs, learning mac addresses
    Forwarding, // Forwarded frames, receives and transmits BPDUs learning mac addresses
}

/// An ethernet interface that participates in the Spanning Tree Protocol (STP).
#[derive(Debug)]
struct StpPort {
    interface: EthernetInterface,
    stp_state: StpPortState,
    stp_role: StpPortRole,
    id: u16,
}

/// A layer two switch; forwards Ethernet frames to the correct interface.
///
/// Implements IEEE 802.1W Rapid Spanning Tree Protocol (RSTP) to prevent loops.
///
/// All ports are enabled by default in the forwarding state.
pub struct Switch {
    ports: [RefCell<StpPort>; 32],     // 32 physical ports
    table: HashMap<MacAddress, usize>, // maps an address to the interface it's connected to.

    pub mac_address: MacAddress,
    pub bridge_priority: u16, // The priority of the switch in the spanning tree protocol. Lowest priority is the root bridge.

    pub root_bid: u64,  // Root Bridge ID = Root MAC Address + Root Priority
    pub root_cost: u32, // The cost of the path to the root bridge ; 0 for the root bridge
    pub root_port: Option<usize>, // The port that leads to the root bridge ; None if the switch is the root bridge

    pub responds_to_bpdu: u32, // A bitmap that indicates which ports respond to BPDUs
}

impl Switch {
    /// Creates a new switch with 32 interfaces, each with a unique MAC address based on the given seed. All ports assume they
    /// are designated ports. The switch is assumed to be the root bridge.
    ///
    /// * `mac_seed` - The seed for the MAC addresses of the interfaces. Will take the range [mac_seed, mac_seed + 32].
    /// * `bridge_priority` - The priority of the switch in the spanning tree protocol.
    pub fn from_seed(mac_seed: u8, bridge_priority: u16) -> Switch {
        let ports: [RefCell<StpPort>; 32] = (0..32)
            .map(|i| {
                RefCell::new(StpPort {
                    interface: EthernetInterface::new(mac_addr!(mac_seed + i + 1)),
                    stp_state: StpPortState::Forwarding,
                    stp_role: StpPortRole::Designated,
                    id: i as u16,
                })
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Switch {
            ports,
            table: HashMap::new(),
            bridge_priority,
            mac_address: mac_addr!(mac_seed),
            root_bid: crate::bridge_id!(mac_addr!(mac_seed), bridge_priority), // Assume the switch is the root bridge
            root_cost: 0,
            root_port: None,
            responds_to_bpdu: 0,
        }
    }

    pub fn bid(&self) -> u64 {
        crate::bridge_id!(self.mac_address, self.bridge_priority)
    }

    /// Connects two ports together via EthernetPorts (bi-directional).
    /// * `switch_port_id` - The port on this switch to connect.
    /// * `interface` - An EthernetInterface to connect to the switch.
    pub fn connect(&mut self, switch_port_id: usize, interface: &mut EthernetInterface) {
        self.ports[switch_port_id]
            .borrow_mut()
            .interface
            .connect(interface);
    }

    /// Connects two switches ports together via EthernetPorts (bi-directional).
    /// * `port` - The port on this switch to connect.
    /// * `other_switch` - The other switch to connect to.
    /// * `other_port` - The port on the other switch to connect to.
    pub fn connect_switch(&mut self, port: usize, other_switch: &mut Switch, other_port: usize) {
        self.ports[port]
            .borrow_mut()
            .interface
            .connect(&other_switch.ports[other_port].borrow_mut().interface);
    }

    /// Initializes RSTP (Rapid Spanning Tree Protocol) on the switch by setting all ports to the Discarding state,
    /// and sending BPDU frames to all interfaces
    pub fn init_stp(&mut self) {
        for port in self.ports.iter() {
            port.borrow_mut().stp_state = StpPortState::Discarding;
        }

        let mut bpdu = BpduFrame::hello(
            self.mac_address,
            self.root_bid,
            self.root_cost,
            crate::bridge_id!(self.mac_address, self.bridge_priority),
            0,
        );

        // Flood BPDU frames to all interfaces
        for port in self.ports.iter() {
            bpdu.port = port.borrow().id;
            port.borrow_mut()
                .interface
                .send802_3(crate::mac_bpdu_addr!(), bpdu.to_bytes());
        }
    }

    /// Finishes the initialization stage of the switch by allowing traffic from end devices.
    pub fn finish_init_stp(&mut self) {
        for (i, port) in self.ports.iter_mut().enumerate() {
            let is_end_device = self.responds_to_bpdu & (1 << i) == 0;

            if is_end_device || port.get_mut().stp_state == StpPortState::Learning {
                port.get_mut().stp_state = StpPortState::Forwarding;
            }
        }
    }

    /// Forwards incoming frames to the correct interface based on the destination MAC address.
    /// If the destination MAC address is not in the table, the frame is flooded to all interfaces.
    pub fn forward(&mut self) {
        for (port, interface) in self.ports.iter().enumerate() {
            let frames = interface.borrow_mut().interface.receive();

            for frame in frames {
                // Invalid address; A source address cannot be a multicast or broadcast address
                if is_mac_multicast_or_broadcast!(frame.source_address()) {
                    continue;
                }

                match frame {
                    EthernetFrame::Ethernet2(f) => {
                        if interface.borrow().stp_state == StpPortState::Discarding {
                            continue;
                        }

                        // If the sender MAC address is not in the table, add it.
                        if !self.table.contains_key(&f.source_address) {
                            self.table.insert(f.source_address, port);
                        }

                        // If the destination MAC address is in the table, forward the mapped interface
                        if let Some(destination_index) = self.table.get(&f.destination_address) {
                            self.ports[*destination_index].borrow_mut().interface.sendv(
                                f.source_address,
                                f.destination_address,
                                f.ether_type,
                                f.data,
                            );
                            return;
                        }

                        // Destination isn't in table, flood to all interfaces (except the one it came from)
                        for (i, other_interface) in self.ports.iter().enumerate() {
                            if i == port
                                || other_interface.borrow().stp_role == StpPortRole::Disabled
                            {
                                continue;
                            }

                            other_interface.borrow_mut().interface.sendv(
                                f.source_address,
                                f.destination_address,
                                f.ether_type,
                                f.data.clone(),
                            );
                        }
                    }

                    EthernetFrame::Ethernet802_3(e802_3) => {
                        // Ethernet 802.3 is only used for BPDU in this simulation
                        let bpdu = match BpduFrame::from_bytes(e802_3.data) {
                            Ok(bpdu) => bpdu,
                            Err(_) => continue,
                        };

                        // This port sends BPDUs, mark as a switch
                        self.responds_to_bpdu |= 1 << port;

                        // If our BID is less than the incoming BID, this port should be designated. Else, disable.
                        if self.bid() < bpdu.bid {
                            self.ports[port].borrow_mut().stp_role = StpPortRole::Designated;
                            self.ports[port].borrow_mut().stp_state = StpPortState::Learning;
                        } else {
                            self.ports[port].borrow_mut().stp_role = StpPortRole::Disabled;
                            self.ports[port].borrow_mut().stp_state = StpPortState::Discarding;
                        }

                        if self.root_bid <= bpdu.root_bid {
                            continue; // Not fit to be the new root bridge
                        }

                        // Elect a new root bridge
                        self.root_bid = bpdu.root_bid;
                        self.root_cost = bpdu.root_cost + 1;
                        self.root_port = Some(port);
                        self.ports[port].borrow_mut().stp_state = StpPortState::Learning;
                        self.ports[port].borrow_mut().stp_role = StpPortRole::Root;

                        // Broadcast the new root bridge to all other interfaces
                        for (i, other_interface) in self.ports.iter().enumerate() {
                            if i == port
                                || other_interface.borrow().stp_role == StpPortRole::Disabled
                            {
                                continue;
                            }

                            let port_id = other_interface.borrow().id;
                            other_interface.borrow_mut().interface.send802_3(
                                crate::mac_bpdu_addr!(),
                                BpduFrame::hello(
                                    self.mac_address,
                                    self.root_bid,
                                    self.root_cost,
                                    crate::bridge_id!(self.mac_address, self.bridge_priority),
                                    port_id,
                                )
                                .to_bytes(),
                            );
                        }
                    }
                };
            }
        }
    }

    /// Returns a list of all the EthernetPorts connected to the switch.
    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.ports
            .iter()
            .map(|i| i.borrow().interface.port())
            .collect()
    }

    /// Returns all ports in the designated role.
    #[cfg(test)]
    pub(crate) fn designated_ports(&self) -> Vec<usize> {
        self.ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.borrow().stp_role == StpPortRole::Designated)
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns all ports in the disabled role.
    #[cfg(test)]
    pub(crate) fn disabled_ports(&self) -> Vec<usize> {
        self.ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.borrow().stp_role == StpPortRole::Disabled)
            .map(|(i, _)| i)
            .collect()
    }
}

/// BPDU MAC address for Spanning Tree Protocol
#[macro_export]
macro_rules! mac_bpdu_addr {
    () => {
        [0x01, 0x80, 0xC2, 0x00, 0x00, 0x00]
    };
}

/// Take in a u8 array as a MAC address and a u16 as a priority to create a bridge ID.
#[macro_export]
macro_rules! bridge_id {
    ($mac:expr, $priority:expr) => {{
        let mut id: u64 = 0;
        for &byte in $mac.iter() {
            id = (id << 8) | (byte as u64);
        }
        (id << 16) | ($priority as u64)
    }};
}

#[derive(Debug, PartialEq)]
pub struct BpduFrame {
    destination_address: MacAddress,
    source_address: MacAddress,
    protocol_id: u16, // 0x0000 for STP, 0x0000 for RSTP
    version: u8,      // 0x00 for STP, 0x02 for RSTP. Always 0x02 in this implementation.
    bpdu_type: u8,    // 0x00 for Configuration BPDU, 0x02 for TCN BPDU
    flags: u8,
    root_bid: u64,  // Bridge ID = Root MAC Address + Root Priority
    root_cost: u32, // The cost of the path to the root bridge
    bid: u64,       // Bridge ID = Bridge MAC Address + Bridge Priority
    port: u16,      // Port ID = Port Priority + Port Number
    message_age: u16,
    max_age: u16,
    hello_time: u16,
    forward_delay: u16,
}

impl BpduFrame {
    /// * `tcn` - Topology Change Notification. Set to true if the BPDU is a TCN BPDU, ie a BPDU that indicates a topology change.
    /// * `proposal` - Set to true if the BPDU is a proposal BPDU. A proposal BPDU is sent by a designated port to the root port.
    /// * `port_role` - The role of the port sending the BPDU. 0 = Root, 1 = Designated, 2 = Alternate, 3 = Backup, 4 = Disabled
    /// * `learning` - Set to true if the port is in the learning state. This is the state where the port is learning MAC addresses.
    /// * `forwarding` - Set to true if the port is in the forwarding state. This is the state where the port is forwarding frames.
    /// * `agreement` - Set to true if the port has reached agreement with the other end of the link.
    ///
    /// ## Returns
    /// A u8 representing the flags field of the BPDU frame.
    pub fn flags(
        tcn: bool,
        proposal: bool,
        port_role: u8,
        learning: bool,
        forwarding: bool,
        agreement: bool,
    ) -> u8 {
        let mut flags = 0x00;

        if tcn {
            flags |= 0x01;
        }

        if proposal {
            flags |= 0x02;
        }

        flags |= port_role << 2;

        if learning {
            flags |= 0x10;
        }

        if forwarding {
            flags |= 0x20;
        }

        if agreement {
            flags |= 0x40;
        }

        flags
    }

    pub fn new(
        destination_address: MacAddress,
        source_address: MacAddress,
        config_type: bool,
        flags: u8,
        root_bid: u64,
        root_cost: u32,
        bid: u64,
        port: u16,
    ) -> BpduFrame {
        let bpdu_type = if config_type { 0x02 } else { 0x00 };

        BpduFrame {
            destination_address,
            source_address,
            protocol_id: 0x0000, // RSTP/STP
            version: 2,          // RSTP
            bpdu_type,           // Configuration or TCN BPDU
            flags,
            root_bid,
            root_cost,
            bid,
            port,

            // TODO: Implement timers
            message_age: 0,
            max_age: 0,
            hello_time: 0,
            forward_delay: 0,
        }
    }

    pub fn hello(
        source_address: MacAddress,
        root_bid: u64,
        root_cost: u32,
        bid: u64,
        port: u16,
    ) -> BpduFrame {
        BpduFrame::new(
            crate::mac_bpdu_addr!(),
            source_address,
            false,
            BpduFrame::flags(false, false, 1, false, true, false),
            root_bid,
            root_cost,
            bid,
            port,
        )
    }
}

impl ByteSerialize for BpduFrame {
    fn from_bytes(bytes: Vec<u8>) -> Result<BpduFrame, std::io::Error> {
        if bytes.len() < 35 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Insufficient bytes for BPDU frame; Runt frame.",
            ));
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
            root_bid: root_id,
            root_cost: root_path_cost,
            bid: bridge_id,
            port: port_id,
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
        bytes.extend_from_slice(&self.root_bid.to_be_bytes());
        bytes.extend_from_slice(&self.root_cost.to_be_bytes());
        bytes.extend_from_slice(&self.bid.to_be_bytes());
        bytes.extend_from_slice(&self.port.to_be_bytes());
        bytes.extend_from_slice(&self.message_age.to_be_bytes());
        bytes.extend_from_slice(&self.max_age.to_be_bytes());
        bytes.extend_from_slice(&self.hello_time.to_be_bytes());
        bytes.extend_from_slice(&self.forward_delay.to_be_bytes());

        bytes
    }
}
