use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    is_mac_multicast_or_broadcast, mac_addr,
    network::ethernet::{interface::*, *},
    tick::{TickTimer, Tickable},
};

use super::cable::*;

#[derive(Debug, PartialEq, Clone, Copy)]
enum StpRole {
    Root,       // The port that leads to the root bridge
    Designated, // The lowest cost path to the root bridge for a network segment
    Alternate,  // The lowest cost path to the root bridge (that isn't the root port)
    Backup,     // A higher cost path to the root bridge for a network segment
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum StpState {
    Discarding, // No forwarded frames, receives and transmits bpdus, no learning mac addresses
    Learning,   // No forwarded frames, receives and transmits BPDUs, learning mac addresses
    Forwarding, // Forwarded frames, receives and transmits BPDUs learning mac addresses
}

/// An ethernet interface that participates in the Spanning Tree Protocol (STP).
#[derive(Debug)]
struct SwitchPort {
    interface: EthernetInterface,
    stp_state: StpState,
    stp_role: Option<StpRole>, // None if the port hasn't initialized its role yet
    id: usize,
    root_cost: u32, // 0 for the root bridge or if the port hasn't received a BPDU
    connected_root_bid: Option<u64>, // The root bridge ID of the connected port. None if the port has never received a BPDU.
    connected_bid: Option<u64>, // The bridge ID of the connected port. None if the port has never received a BPDU.
    connected_role: Option<StpRole>, // The inferred role from the last BPDU received
}

#[derive(Hash, Eq, PartialEq, Clone)]
enum SwitchDelayedAction {
    BpduMulticast,
    RstpInit,
}

/// A layer two switch; forwards Ethernet frames to the correct interface.
///
/// Implements IEEE 802.1W Rapid Spanning Tree Protocol (RSTP) to prevent loops.
pub struct Switch {
    ports: [RefCell<SwitchPort>; 32],  // 32 physical ports
    table: HashMap<MacAddress, usize>, // maps an address to the interface it's connected to.

    pub mac_address: MacAddress,
    bridge_priority: u16, // The priority of the switch in the spanning tree protocol. Lowest priority is the root bridge.

    rstp: bool,               // if rstp is enabled
    root_bid: u64,            // Root Bridge ID = Root MAC Address + Root Priority
    root_cost: u32,           // The cost of the path to the root bridge ; 0 for the root bridge
    root_port: Option<usize>, // The port that leads to the root bridge ; None if the switch is the root bridge

    timer: TickTimer<SwitchDelayedAction>,
    missed_hellos: [u8; 32], // Number of missed hellos for each port, 3 missed hellos will trigger a topology change
    received_bpdu: [bool; 32], // Whether a BPDU has been received on the port in the last 2 seconds
}

impl Switch {
    /// Creates a new switch with 32 interfaces, each with a unique MAC address based on the given seed. All ports assume they
    /// are designated ports. The switch is assumed to be the root bridge.
    /// * `mac_seed` - The seed for the MAC addresses of the interfaces. Will take the range [mac_seed, mac_seed + 32].
    /// * `bridge_priority` - The priority of the switch in the spanning tree protocol.
    ///
    /// # Example
    /// ```
    /// let switch = Switch::from_seed(1, 1);
    /// ```
    /// This will create a switch with the switch's MAC address as `mac_addr!(1)` and the interfaces MAC addresses as `mac_addr!(2)` through `mac_addr!(33)`.
    /// The switch will have a bridge priority of 1.
    pub fn from_seed(mac_seed: u64, bridge_priority: u16) -> Self {
        let ports: [RefCell<SwitchPort>; 32] = (0..32)
            .map(|i| {
                RefCell::new(SwitchPort {
                    interface: EthernetInterface::new(mac_addr!(mac_seed + i + 1)),
                    stp_state: StpState::Forwarding,
                    stp_role: None,
                    id: i as usize,
                    root_cost: 0,
                    connected_root_bid: None,
                    connected_bid: None,
                    connected_role: None,
                })
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Self {
            ports,
            table: HashMap::new(),
            bridge_priority,
            mac_address: mac_addr!(mac_seed),
            rstp: false,
            root_bid: crate::bridge_id!(mac_addr!(mac_seed), bridge_priority), // Assume the switch is the root bridge
            root_cost: 0,
            root_port: None,
            timer: TickTimer::default(),
            missed_hellos: [0; 32],
            received_bpdu: [false; 32],
        }
    }

    /// Connects two ports together via EthernetPorts (bi-directional).
    pub fn connect(&mut self, port_id: usize, interface: &mut EthernetInterface) {
        self.ports[port_id]
            .borrow_mut()
            .interface
            .connect(interface);

        if self.rstp {
            self.ports[port_id].borrow_mut().interface.send8023(
                crate::mac_bpdu_addr!(),
                BpduFrame::hello(
                    self.mac_address,
                    self.root_bid,
                    self.root_cost,
                    self.bid(),
                    port_id,
                )
                .to_bytes(),
            );
        }
    }

    /// Shorthand for connecting two switches ports together via EthernetPorts (bi-directional).
    pub fn connect_switch(
        &mut self,
        port_id: usize,
        other_switch: &mut Switch,
        other_port_id: usize,
    ) {
        self.connect(
            port_id,
            &mut other_switch.ports[other_port_id].borrow_mut().interface,
        );
    }

    /// Forwards incoming frames to the correct interface based on the destination MAC address.
    /// If the destination MAC address is not in the table, the frame is flooded to all interfaces.
    ///
    /// On a BPDU frame, it will update its port roles and states, and flood it's own BPDU if necessary.
    pub fn forward(&mut self) {
        for i in 0..32 {
            let (state, frames) = {
                let mut p = self.ports[i].borrow_mut();
                let state = p.stp_state;
                let frames = p.interface.receive();
                (state, frames)
            };

            for frame in frames {
                if is_mac_multicast_or_broadcast!(frame.source_address()) {
                    continue; // Invalid address; A source address cannot be a multicast or broadcast address
                }

                match frame {
                    EthernetFrame::Ethernet2(f) => {
                        if state != StpState::Discarding {
                            self._receive_ethernet2(f, i);
                        }
                    }
                    EthernetFrame::Ethernet802_3(f) => {
                        if !self.rstp {
                            continue; // TODO: Switch should just forward the frame if RSTP is disabled
                        }
                        if let Ok(bpdu) = BpduFrame::from_bytes(f.data) {
                            self._receive_bpdu(bpdu, i);
                        }
                    }
                }
            }
        }
    }

    fn _receive_ethernet2(&mut self, f: Ethernet2Frame, port: usize) {
        // If the sender MAC address is not in the table, add it.
        self.table.entry(f.source_address).or_insert(port);

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
            if i == port || other_interface.borrow().stp_state == StpState::Discarding {
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

    /// Returns a list of all the physical EthernetPorts on the switch.
    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.ports
            .iter()
            .map(|i| i.borrow().interface.port().clone())
            .collect()
    }

    /// Returns the STP state of the port.
    pub fn is_port_up(&self, port_id: usize) -> bool {
        self.ports[port_id].borrow().stp_state != StpState::Discarding
    }

    pub fn bridge_priority(&self) -> u16 {
        self.bridge_priority
    }

    pub fn root_bid(&self) -> u64 {
        self.root_bid
    }

    pub fn root_cost(&self) -> u32 {
        self.root_cost
    }

    pub fn root_port(&self) -> Option<usize> {
        self.root_port
    }

    pub fn mac_addr(&self, port_id: usize) -> MacAddress {
        self.ports[port_id].borrow().interface.mac_address
    }

    pub fn set_bridge_priority(&mut self, priority: u16) {
        self.bridge_priority = priority;
        self.root_bid = crate::bridge_id!(self.mac_address, priority);
    }

    pub fn mac_table(&self) -> HashMap<MacAddress, usize> {
        self.table.clone()
    }

    /// Returns all ports in the designated role.
    #[cfg(test)]
    pub(crate) fn designated_ports(&self) -> Vec<usize> {
        self.ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.borrow().stp_role == Some(StpRole::Designated))
            .map(|(i, _)| i)
            .collect()
    }

    /// Returns all ports in the discarding state.
    #[cfg(test)]
    pub(crate) fn discarding_ports(&self) -> Vec<usize> {
        self.ports
            .iter()
            .enumerate()
            .filter(|(_, p)| p.borrow().stp_state == StpState::Discarding)
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

    /// Returns true if the switch is the root bridge of the network.
    pub fn is_root_bridge(&self) -> bool {
        self.root_bid == self.bid()
    }

    /// Compares two BIDs and returns true if bid1 is better than bid2.
    /// * `bid1` - The first bridge ID to compare.
    /// * `bid2` - The second bridge ID to compare.
    ///
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

        Some(priority1 < priority2 || (priority1 == priority2 && bid1 < bid2))
    }

    /// Sends a Hello BPDU to all interfaces.
    /// * `tcn` - Topology Change Notification. Set to true if the BPDU is a TCN BPDU, ie a BPDU that indicates a topology change.
    /// * `proposal` - Set to true if the BPDU is a proposal BPDU.
    /// * `flood_to_all` - Set to true if the BPDU should be flooded to all interfaces.
    fn _send_bpdus(&self, tcn: bool, proposal: bool, flood_to_all: bool) {
        let mut bpdu = BpduFrame::hello(
            self.mac_address,
            self.root_bid,
            self.root_cost,
            self.bid(),
            0,
        );

        for stp_port in self.ports.iter() {
            if !flood_to_all && stp_port.borrow().connected_bid.is_none() {
                continue;
            }

            let port_role = match stp_port.borrow().stp_role {
                Some(StpRole::Root) => 0,
                Some(StpRole::Designated) => 1,
                Some(StpRole::Alternate) => 2,
                Some(StpRole::Backup) => 3,
                None => 4,
            };
            bpdu.port = stp_port.borrow().id as u16;
            bpdu.flags = BpduFrame::flags(
                tcn,
                proposal,
                port_role,
                stp_port.borrow().stp_state == StpState::Learning,
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
    ///
    /// The switch will wait 15 seconds before transitioning to `finish_init_stp`.
    pub fn init_stp(&mut self) {
        self.rstp = true;
        for stp_port in self.ports.iter() {
            stp_port.borrow_mut().stp_state = StpState::Discarding;
            stp_port.borrow_mut().stp_role = Some(StpRole::Root);
        }
        self._send_bpdus(true, true, true);

        self.timer
            .schedule(SwitchDelayedAction::RstpInit, 15, false);
    }

    /// Opens all ports that haven't acted in the STP process to the Forwarding state.
    ///
    /// Begins the BPDU multicast timer.
    pub fn finish_init_stp(&mut self) {
        for stp_port in self.ports.iter() {
            if stp_port.borrow().connected_bid.is_none() {
                stp_port.borrow_mut().stp_role = Some(StpRole::Designated);
                stp_port.borrow_mut().stp_state = StpState::Forwarding;
            }
        }

        self.timer
            .schedule(SwitchDelayedAction::BpduMulticast, 2, true);
    }

    fn _link_down(&mut self, port_id: usize) {
        let mut port = self.ports[port_id].borrow_mut();
        port.connected_bid = None;
        port.connected_role = None;
        port.connected_root_bid = None;
        port.stp_role = Some(StpRole::Designated);
        port.stp_state = StpState::Forwarding;
        port.root_cost = 0;

        self.missed_hellos[port_id] = 0;
    }

    /// Disconnects a port from the switch as well as from STP.
    pub fn disconnect(&mut self, port_id: usize) {
        self.ports[port_id]
            .borrow_mut()
            .interface
            .port()
            .borrow_mut()
            .disconnect();
        self._link_down(port_id);

        let (role_changed, root_changed) = self._calculate_port_roles();
        if role_changed || root_changed {
            self._send_bpdus(true, true, root_changed); // Flood to all IFF the root has changed
        }
    }

    fn _receive_bpdu(&mut self, bpdu: BpduFrame, port_id: usize) {
        self.received_bpdu[port_id] = true;
        {
            let mut sp = self.ports[port_id].borrow_mut();
            sp.connected_root_bid = Some(bpdu.root_bid);
            sp.connected_bid = Some(bpdu.bid);
            sp.connected_role = bpdu.stp_role();
            sp.root_cost = bpdu.root_cost + 1;

            match Self::compare_bids(self.root_bid, bpdu.root_bid) {
                Some(true) => {
                    let bpdu = BpduFrame::hello(
                        self.mac_address,
                        self.root_bid,
                        self.root_cost,
                        self.bid(),
                        port_id,
                    );

                    // Broadcast how much cooler we are
                    sp.interface
                        .send8023(crate::mac_bpdu_addr!(), bpdu.to_bytes());

                    if self.is_root_bridge() {
                        // Only done during STP initialization
                        sp.stp_role = Some(StpRole::Designated);
                        sp.stp_state = StpState::Forwarding;
                    }

                    return; // Role recalculations aren't necessary
                }
                Some(false) => {
                    self.root_bid = bpdu.root_bid; // Root is better
                }
                _ => {}
            }
        }

        let (role_changed, root_changed) = self._calculate_port_roles();
        if role_changed || root_changed {
            self._send_bpdus(true, true, root_changed); // Flood to all IFF the root has changed
        }
    }

    /// Returns (role_changed, root_changed)
    fn _calculate_port_roles(&mut self) -> (bool, bool) {
        fn enter_election(switch: &mut Switch) {
            for stp_port in switch.ports.iter() {
                let mut sp = stp_port.borrow_mut();
                sp.stp_role = Some(StpRole::Designated);
                sp.stp_state = StpState::Forwarding;
                sp.root_cost = 0;
                sp.connected_bid = None;
                sp.connected_role = None;
            }

            switch.root_bid = switch.bid();
            switch.root_cost = 0;
            switch.root_port = None;
        }

        if self.is_root_bridge() {
            return (false, false);
        }

        let new_root_port = {
            let mut rp: Option<(u32, u64, usize)> = None; // (root_cost, bid, port_id)

            for stp_port in self.ports.iter() {
                let sp = stp_port.borrow();
                if !sp
                    .connected_root_bid
                    .is_some_and(|bid| bid == self.root_bid)
                    || sp.connected_bid.is_none()
                {
                    continue;
                }

                if let Some((root_cost, bid, _)) = rp {
                    if sp.root_cost > root_cost {
                        continue;
                    }

                    let compare = Switch::compare_bids(sp.connected_bid.unwrap(), bid)
                        .expect("Bids are equivalent");
                    if sp.root_cost < root_cost || compare {
                        rp = Some((sp.root_cost, sp.connected_bid.unwrap(), sp.id));
                    }
                    continue;
                }

                if let Some(connected_bid) = sp.connected_bid {
                    rp = Some((sp.root_cost, connected_bid, sp.id));
                }
            }

            rp
        };

        let (new_root_cost, new_root_port_id, root_changed) = match new_root_port {
            Some((root_cost, _, root_port_id)) => {
                if self.root_port.is_some_and(|rp| rp != root_port_id)
                    && self.ports[root_port_id].borrow().stp_role != Some(StpRole::Alternate)
                {
                    enter_election(self); // A new root port has been found, but it isn't an alternate port.
                    return (true, true);
                }
                (
                    root_cost,
                    root_port_id,
                    self.root_port != Some(root_port_id),
                )
            }
            None => {
                enter_election(self); // There is no port leading to the root bridge.
                return (true, true);
            }
        };

        self.root_port = Some(new_root_port_id);
        self.root_cost = new_root_cost;
        self.ports[new_root_port_id].borrow_mut().stp_role = Some(StpRole::Root);
        self.ports[new_root_port_id].borrow_mut().stp_state = StpState::Forwarding;
        let mut role_changed = root_changed;

        let segment_to_port = {
            let mut segment_to_port: HashMap<u64, usize> = HashMap::new();
            for stp_port in self.ports.iter() {
                let mut sp = stp_port.borrow_mut();
                if sp.connected_bid.is_none() || sp.id == new_root_port_id {
                    continue;
                }

                sp.root_cost = std::cmp::max(sp.root_cost, new_root_cost); // Cost cannot be less than the root cost

                if let Some(min_port_id) = segment_to_port.get(&sp.connected_bid.unwrap()) {
                    let bid = sp.connected_bid.unwrap();
                    let min_bid = self.ports[*min_port_id].borrow().connected_bid.unwrap();
                    let min_cost = self.ports[*min_port_id].borrow().root_cost;

                    let is_min = {
                        if min_cost != sp.root_cost {
                            sp.root_cost < self.ports[*min_port_id].borrow().root_cost
                        }
                        // Tiebreaker: Compare by port number if the bids are equivalent
                        else if bid == min_bid {
                            sp.id > *min_port_id
                        }
                        // Tiebreaker: Compare by bid if the costs are equal
                        else {
                            Switch::compare_bids(bid, min_bid).expect("Bids are equivalent")
                        }
                    };

                    if is_min {
                        segment_to_port.insert(bid, sp.id);
                    }

                    continue;
                }
                segment_to_port.insert(sp.connected_bid.unwrap(), sp.id);
            }
            segment_to_port
        };

        for stp_port in self.ports.iter() {
            let mut sp = stp_port.borrow_mut();
            let prev_role = sp.stp_role;

            if sp.connected_bid.is_none()
                || sp.connected_role.is_none()
                || sp.id == self.root_port.unwrap()
            {
                continue; // Port doesnt participate in STP, or is the root port (which is already determined `find_root_port`)
            }

            let bid = sp.connected_bid.unwrap();
            if bid == self.root_bid {
                sp.stp_role = Some(StpRole::Alternate);
                sp.stp_state = StpState::Discarding;
            } else if !segment_to_port.contains_key(&bid) {
                sp.stp_role = Some(StpRole::Backup);
                sp.stp_state = StpState::Discarding;
            } else {
                sp.stp_role = Some(StpRole::Designated);
                sp.stp_state = StpState::Forwarding;
            }

            let conn_role = sp.connected_role.unwrap();
            // Never block another switches root port
            if conn_role == StpRole::Root {
                sp.stp_role = Some(StpRole::Designated);
                sp.stp_state = StpState::Forwarding;
            }
            // Designated to designated is a redundancy
            else if conn_role == StpRole::Designated && sp.stp_role == Some(StpRole::Designated) {
                // Only one of the switches has to block.
                let compare = Self::compare_bids(self.bid(), bid);
                match compare {
                    Some(true) => {
                        continue; // This switch wins, stay designated
                    }
                    Some(false) => {
                        sp.stp_role = Some(StpRole::Backup);
                        sp.stp_state = StpState::Discarding;
                    }
                    None => {
                        todo!("Further tiebreakers. Will not occur in simulation.");
                    }
                }
            }

            if prev_role != sp.stp_role {
                role_changed = true;
            }
        }

        (role_changed, root_changed)
    }
}

impl Tickable for Switch {
    fn tick(&mut self) {
        self.forward();

        for action in self.timer.ready() {
            match action {
                SwitchDelayedAction::BpduMulticast => {
                    let mut should_recalc = false;

                    // Find links that are down, (max age = 6 seconds)
                    for i in 0..32 {
                        if !self.received_bpdu[i] && self.ports[i].borrow().connected_bid.is_some()
                        {
                            self.missed_hellos[i] += 1;
                            if self.missed_hellos[i] >= 3 {
                                self._link_down(i);
                                should_recalc = true;
                            }
                        } else {
                            self.missed_hellos[i] = 0;
                        }

                        self.received_bpdu[i] = false;
                    }

                    let (role_changed, root_changed) = if should_recalc {
                        self._calculate_port_roles()
                    } else {
                        (false, false)
                    };

                    self._send_bpdus(role_changed || root_changed, false, root_changed);
                }
                SwitchDelayedAction::RstpInit => {
                    self.finish_init_stp();
                }
            }
        }

        self.timer.tick();
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
    pub destination_address: MacAddress,
    pub source_address: MacAddress,
    pub protocol_id: u16, // 0x0000 for STP, 0x0000 for RSTP
    pub version: u8,      // 0x00 for STP, 0x02 for RSTP. Always 0x02 in this implementation.
    pub bpdu_type: u8,    // 0x00 for Configuration BPDU, 0x02 for TCN BPDU
    pub flags: u8,
    pub root_bid: u64,  // Bridge ID = Root MAC Address + Root Priority
    pub root_cost: u32, // The cost of the path to the root bridge
    pub bid: u64,       // Bridge ID = Bridge MAC Address + Bridge Priority
    pub port: u16,      // Port ID = Port Priority + Port Number
    pub message_age: u16,
    pub max_age: u16,
    pub hello_time: u16,
    pub forward_delay: u16,
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
    ) -> Self {
        let bpdu_type = if config_type { 0x02 } else { 0x00 };

        Self {
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

            // TODO: These aren't configurable, likely won't change.
            message_age: 0,
            max_age: 0,    // Treated as 6 seconds
            hello_time: 0, // Treated as 2 seconds
            forward_delay: 0,
        }
    }

    pub fn hello(
        source_address: MacAddress,
        root_bid: u64,
        root_cost: u32,
        bid: u64,
        port: usize,
    ) -> Self {
        Self::new(
            crate::mac_bpdu_addr!(),
            source_address,
            false,
            Self::flags(false, false, 1, false, true, false),
            root_bid,
            root_cost,
            bid,
            port as u16,
        )
    }

    fn stp_role(&self) -> Option<StpRole> {
        match (self.flags & 0b0000_1100) >> 2 {
            0 => Some(StpRole::Root),
            1 => Some(StpRole::Designated),
            2 => Some(StpRole::Alternate),
            3 => Some(StpRole::Backup),
            _ => None,
        }
    }
}

impl ByteSerializable for BpduFrame {
    fn from_bytes(bytes: Vec<u8>) -> Result<Self, std::io::Error> {
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

        Ok(Self {
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
