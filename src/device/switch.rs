use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ethernet::{interface::*, *},
    is_mac_multicast_or_broadcast, mac_addr,
};

use super::cable::*;

#[derive(Debug, PartialEq, Clone)]
enum StpPortRole {
    Root,       // The port that leads to the root bridge
    Designated, // The lowest cost path to the root bridge for a network segment
    Alternate,  // The lowest cost path to the root bridge (that isn't the root port)
    Backup,     // A higher cost path to the root bridge for a network segment
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
    stp_role: Option<StpPortRole>, // None if the port hasn't initialized its role yet
    id: usize,
    root_cost: u32,   // 0 for the root bridge or if the port hasn't received a BPDU
    bid: Option<u64>, // The bridge ID of the connected port. None if the port has never received a BPDU.
}

/// A layer two switch; forwards Ethernet frames to the correct interface.
///
/// Implements IEEE 802.1W Rapid Spanning Tree Protocol (RSTP) to prevent loops.
pub struct Switch {
    ports: [RefCell<StpPort>; 32],     // 32 physical ports
    table: HashMap<MacAddress, usize>, // maps an address to the interface it's connected to.

    pub mac_address: MacAddress,
    pub bridge_priority: u16, // The priority of the switch in the spanning tree protocol. Lowest priority is the root bridge.

    pub root_bid: u64,  // Root Bridge ID = Root MAC Address + Root Priority
    pub root_cost: u32, // The cost of the path to the root bridge ; 0 for the root bridge
    pub root_port: Option<usize>, // The port that leads to the root bridge ; None if the switch is the root bridge

    debug_tag: u8,
}

impl Switch {
    /// Creates a new switch with 32 interfaces, each with a unique MAC address based on the given seed. All ports assume they
    /// are designated ports. The switch is assumed to be the root bridge.
    /// * `mac_seed` - The seed for the MAC addresses of the interfaces. Will take the range [mac_seed, mac_seed + 32].
    /// * `bridge_priority` - The priority of the switch in the spanning tree protocol.
    pub fn from_seed(mac_seed: u8, bridge_priority: u16) -> Switch {
        let ports: [RefCell<StpPort>; 32] = (0..32)
            .map(|i| {
                RefCell::new(StpPort {
                    interface: EthernetInterface::new(mac_addr!(mac_seed + i + 1)),
                    stp_state: StpPortState::Forwarding,
                    stp_role: None,
                    id: i.into(),
                    root_cost: 0,
                    bid: None,
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
            debug_tag: 0,
        }
    }

    /// Connects two ports together via EthernetPorts (bi-directional).
    /// * `port_id` - The id of the port on this switch to connect.
    /// * `interface` - An EthernetInterface to connect to the switch.
    pub fn connect(&mut self, port_id: usize, interface: &mut EthernetInterface) {
        self.ports[port_id]
            .borrow_mut()
            .interface
            .connect(interface);
    }

    /// Connects two switches ports together via EthernetPorts (bi-directional).
    /// * `port_id` - The port on this switch to connect.
    /// * `other_switch` - The other switch to connect to.
    /// * `other_port_id` - The port on the other switch to connect to.
    pub fn connect_switch(
        &mut self,
        port_id: usize,
        other_switch: &mut Switch,
        other_port_id: usize,
    ) {
        self.ports[port_id]
            .borrow_mut()
            .interface
            .connect(&other_switch.ports[other_port_id].borrow_mut().interface);
    }

    /// Forwards incoming frames to the correct interface based on the destination MAC address.
    /// If the destination MAC address is not in the table, the frame is flooded to all interfaces.
    ///
    /// If the switch receives a BPDU frame, it will update its STP state and forward the BPDU to the correct interface.
    pub fn forward(&mut self) {
        let mut valid_frames: Vec<(usize, EthernetFrame)> = vec![];

        for (port_id, stp_port) in self.ports.iter().enumerate() {
            let frames = stp_port.borrow_mut().interface.receive();

            for frame in frames {
                if is_mac_multicast_or_broadcast!(frame.source_address()) {
                    continue; // Invalid address; A source address cannot be a multicast or broadcast address
                }

                match frame {
                    EthernetFrame::Ethernet2(f) => {
                        if stp_port.borrow().stp_state != StpPortState::Discarding {
                            valid_frames.push((port_id, EthernetFrame::Ethernet2(f)));
                        }
                    }
                    _ => {
                        valid_frames.push((port_id, frame));
                    }
                }
            }
        }

        for (port, frame) in valid_frames {
            match frame {
                EthernetFrame::Ethernet2(f) => self._receive_ethernet2(f, port),
                EthernetFrame::Ethernet802_3(f) => {
                    if let Ok(bpdu) = BpduFrame::from_bytes(f.data) {
                        self._receive_bpdu(bpdu, port);
                    }
                }
            }
        }
    }

    fn _receive_ethernet2(&mut self, f: Ethernet2Frame, port: usize) {
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

        // Destination isn't in table, flood to all interfaces (except the one it came from, and disabled ports)
        for (i, other_interface) in self.ports.iter().enumerate() {
            if i == port || other_interface.borrow().stp_state == StpPortState::Discarding {
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

    /// Returns a list of all the EthernetPorts connected to the switch.
    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.ports
            .iter()
            .map(|i| i.borrow().interface.port())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn set_debug_tag(&mut self, tag: u8) {
        self.debug_tag = tag;
    }

    /// Returns all ports in the designated role.
    #[cfg(test)]
    pub(crate) fn designated_ports(&self) -> Vec<usize> {
        self.ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.borrow().stp_role == Some(StpPortRole::Designated))
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns all ports in the discarding state.
    #[cfg(test)]
    pub(crate) fn discarding_ports(&self) -> Vec<usize> {
        self.ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.borrow().stp_state == StpPortState::Discarding)
            .map(|(i, _)| i)
            .collect()
    }
}

// Spanning Tree Protocol (STP) methods
impl Switch {
    /// Returns the Bridge ID of the switch. (Bridge MAC Address + Bridge Priority)
    pub fn bid(&self) -> u64 {
        crate::bridge_id!(self.mac_address, self.bridge_priority)
    }

    pub fn is_root_bridge(&self) -> bool {
        self.root_bid == self.bid()
    }

    /// Compares two BIDs and returns true if bid1 is better than bid2.
    /// * `bid1` - The first bridge ID to compare.
    /// * `bid2` - The second bridge ID to compare.
    /// ## Returns
    /// * `None` if the two BIDs are equal.
    /// * `Some(true)` if bid1 is better than bid2.
    /// * `Some(false)` if bid2 is better than bid1.
    fn compare_bids(bid1: u64, bid2: u64) -> Option<bool> {
        if bid1 == bid2 {
            return None;
        }

        let priority1 = (bid1 & 0x0000_0000_0000_FFFF) as u16;
        let priority2 = (bid2 & 0x0000_0000_0000_FFFF) as u16;

        if priority1 < priority2 {
            return Some(true);
        }

        if priority1 == priority2 && bid1 < bid2 {
            return Some(true);
        }

        Some(false)
    }

    /// Sends a Hello BPDU to all interfaces.
    /// * `tcn` - Topology Change Notification. Set to true if the BPDU is a TCN BPDU, ie a BPDU that indicates a topology change.
    /// * `proposal` - Set to true if the BPDU is a proposal BPDU.
    /// * `flood_to_all` - Set to true if the BPDU should be flooded to all interfaces.
    fn send_bpdus(&self, mut bpdu: BpduFrame, tcn: bool, proposal: bool, flood_to_all: bool) {
        for stp_port in self.ports.iter() {
            if !flood_to_all && stp_port.borrow().bid.is_none() {
                continue;
            }

            let port_role = match stp_port.borrow().stp_role {
                Some(StpPortRole::Root) => 0,
                Some(StpPortRole::Designated) => 1,
                Some(StpPortRole::Alternate) => 2,
                Some(StpPortRole::Backup) => 3,
                None => 4,
            };
            bpdu.port = stp_port.borrow().id as u16;
            bpdu.flags = BpduFrame::flags(
                tcn,
                proposal,
                port_role,
                stp_port.borrow().stp_state == StpPortState::Learning,
                port_role == 0 || port_role == 1,
                false,
            );
            stp_port
                .borrow_mut()
                .interface
                .send8023(crate::mac_bpdu_addr!(), bpdu.to_bytes());
        }
    }

    /// Begins STP by initializing all ports to the Discarding state and flooding Hello BPDUs.
    pub fn init_stp(&mut self) {
        for stp_port in self.ports.iter() {
            stp_port.borrow_mut().stp_state = StpPortState::Discarding;
        }
        let bpdu = BpduFrame::hello(
            self.mac_address,
            self.root_bid,
            self.root_cost,
            self.bid(),
            0,
        );
        self.send_bpdus(bpdu, true, true, true);
    }

    /// Opens all ports that haven't acted in the STP process to the Forwarding state.
    pub fn finish_stp(&mut self) {
        for stp_port in self.ports.iter() {
            if stp_port.borrow().bid.is_none() {
                stp_port.borrow_mut().stp_role = Some(StpPortRole::Designated);
                stp_port.borrow_mut().stp_state = StpPortState::Forwarding;
            }
        }
    }

    fn _receive_bpdu(&mut self, bpdu: BpduFrame, port_id: usize) {
        let prev_root_bid = self.root_bid;
        self.ports[port_id].borrow_mut().bid = Some(bpdu.bid);

        let cmpr_root_bids = Switch::compare_bids(self.root_bid, bpdu.root_bid);

        // Incoming BPDUs root is worse, send an outgoing hello BPDU
        if cmpr_root_bids == Some(true) {
            let bpdu = BpduFrame::hello(
                self.mac_address,
                self.root_bid,
                self.root_cost,
                self.bid(),
                port_id,
            );
            self.ports[port_id]
                .borrow_mut()
                .interface
                .send8023(crate::mac_bpdu_addr!(), bpdu.to_bytes());

            // On the root bridge, all ports are designated forwarding ports
            if self.is_root_bridge() {
                let mut sp = self.ports[port_id].borrow_mut();
                sp.stp_role = Some(StpPortRole::Designated);
                sp.stp_state = StpPortState::Forwarding;
                sp.root_cost = 0;
            }

            return;
        }

        // Equivalent root bridges
        if cmpr_root_bids.is_none() {
            let mut sp = self.ports[port_id].borrow_mut();

            // On the root bridge, all ports are designated forwarding ports
            if self.is_root_bridge() {
                sp.stp_role = Some(StpPortRole::Designated);
                sp.stp_state = StpPortState::Forwarding;
                sp.root_cost = 0;
                return;
            }

            // Roots are equivalent, but who has the best root cost?
            let cmpr_bids = Switch::compare_bids(bpdu.bid, self.bid());
            if bpdu.root_cost + 1 < self.root_cost {
                sp.stp_role = Some(StpPortRole::Root);
                sp.stp_state = StpPortState::Forwarding;
                self.root_cost = bpdu.root_cost + 1;
                self.root_port = Some(port_id);
            }
            // Tiebreaker: If the root costs are the same, the switch with the lower BID wins.
            else if bpdu.root_cost + 1 == self.root_cost && cmpr_bids == Some(true) {
                sp.stp_role = Some(StpPortRole::Root);
                sp.stp_state = StpPortState::Forwarding;
                self.root_port = Some(port_id);
            }
            // Cost is worse, but should this be a backup port?
            else if sp.stp_role != Some(StpPortRole::Root) {
                sp.root_cost = bpdu.root_cost + 1;

                // Redundancy: designated to designated
                if bpdu.is_designated() && cmpr_bids.is_some_and(|bpdu_is_better| bpdu_is_better) {
                    sp.stp_role = Some(StpPortRole::Backup);
                    sp.stp_state = StpPortState::Discarding;
                }
            }
        }

        // Incoming BPDUs root is better
        if cmpr_root_bids.is_some_and(|better| !better) {
            // Change the root bridge
            let mut sp = self.ports[port_id].borrow_mut();
            sp.stp_role = Some(StpPortRole::Root);
            sp.stp_state = StpPortState::Forwarding;
            self.root_bid = bpdu.root_bid;
            self.root_cost = bpdu.root_cost + 1;
            self.root_port = Some(port_id);
        }

        let mut network_segment_to_port: HashMap<u64, usize> = HashMap::new();
        for stp_port in self.ports.iter() {
            let mut sp = stp_port.borrow_mut();
            if sp.bid.is_none() || sp.id == self.root_port.unwrap() {
                continue;
            }

            sp.root_cost = std::cmp::max(sp.root_cost, self.root_cost); // Cost cannot be less than the root cost

            if let Some(min_port_id) = network_segment_to_port.get(&sp.bid.unwrap()) {
                let bid = sp.bid.unwrap();
                let min_bid = self.ports[*min_port_id].borrow().bid.unwrap();
                let min_cost = self.ports[*min_port_id].borrow().root_cost;

                let is_min = {
                    // If the rot bridge has not changed, and the costs are not equal, compare by cost
                    if prev_root_bid == self.root_bid && min_cost != sp.root_cost {
                        sp.root_cost < self.ports[*min_port_id].borrow().root_cost
                    }
                    // Tiebreaker: Compare by port number if the bids are equivalent
                    else if bid == min_bid {
                        sp.id > *min_port_id
                    }
                    // Tiebreaker: Compare by bid if the costs are equal
                    else {
                        Switch::compare_bids(bid, min_bid).unwrap()
                    }
                };

                if is_min {
                    network_segment_to_port.insert(bid, sp.id);
                }
            } else {
                network_segment_to_port.insert(sp.bid.unwrap(), sp.id);
            }
        }

        // Assign roles to each port
        for stp_port in self.ports.iter() {
            let mut sp = stp_port.borrow_mut();

            if sp.id == self.root_port.unwrap() {
                continue; // Root port has already been assigned
            }

            if sp.bid.is_none() {
                continue; // Port is not connected to a switch, no recalculation needed
            }

            let bid = sp.bid.unwrap();

            if bid == self.root_bid {
                sp.stp_role = Some(StpPortRole::Alternate);
                sp.stp_state = StpPortState::Discarding;
                continue;
            }

            if !network_segment_to_port.contains_key(&bid) {
                sp.stp_role = Some(StpPortRole::Backup);
                sp.stp_state = StpPortState::Discarding;
                continue;
            }

            // Check if this is a redundant port
            // If the port is a backup and the root bridge hasn't been changed, keep the port in the backup role
            let port_role = sp.stp_role.clone();
            if port_role.is_some_and(|r| r == StpPortRole::Backup) && prev_root_bid == self.root_bid
            {
                continue;
            }

            sp.stp_role = Some(StpPortRole::Designated);
            sp.stp_state = StpPortState::Forwarding;
        }

        // Flood the new BPDU to all interfaces
        if prev_root_bid != self.root_bid {
            let bpdu = BpduFrame::hello(
                self.mac_address,
                self.root_bid,
                self.root_cost,
                self.bid(),
                port_id,
            );
            self.send_bpdus(bpdu, true, true, true);
        }
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
    /// * `proposal` - Set to true if the BPDU is a proposal BPDU.
    /// * `port_role` - The role of the port sending the BPDU. 0 = Root, 1 = Designated, 2 = Alternate, 3 = Backup, 4 = Disabled
    /// * `learning` - Set to true if the port is in the learning state.
    /// * `forwarding` - Set to true if the port is in the forwarding state.
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
        port: usize,
    ) -> BpduFrame {
        BpduFrame::new(
            crate::mac_bpdu_addr!(),
            source_address,
            false,
            BpduFrame::flags(false, false, 1, false, true, false),
            root_bid,
            root_cost,
            bid,
            port as u16,
        )
    }

    pub fn is_designated(&self) -> bool {
        (self.flags & 0b0000_1100) == (1 << 2)
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
