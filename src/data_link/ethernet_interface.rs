use std::{cell::RefCell, rc::Rc};

use crate::{mac_addr, mac_broadcast_addr, network::ipv4::Ipv4Address, physical::ethernet_port::EthernetPort};
use super::{arp_frame::{ArpFrame, ArpOperation}, ethernet_frame::{EtherType, EthernetFrame, MacAddress}};

/// A layer 2 interface for ethernet actions, sending and receiving Ethernet frames through a physical port stamped with a MAC address.
#[derive(Debug, Clone)]
pub struct EthernetInterface {
    port: Rc<RefCell<EthernetPort>>,
    mac_address: MacAddress,
}

impl EthernetInterface {
    pub fn new(mac_address: MacAddress) -> EthernetInterface {
        EthernetInterface {
            port: Rc::new(RefCell::new(EthernetPort::new())),
            mac_address,
        }
    }

    pub fn mac_address(&self) -> MacAddress {
        self.mac_address
    }

    pub fn port(&self) -> Rc<RefCell<EthernetPort>> {
        self.port.clone()
    }

    /// Connects two EthernetInterfaces together via EthernetPorts (bi-directional).
    pub fn connect(&mut self, other: &mut EthernetInterface) {
        EthernetPort::connect(self.port.clone(), other.port.clone());
    }

    /// Sends an ARP request; find the MAC address of the target IP address.
    pub fn send_arp_request(&mut self, sender: Ipv4Address, target: Ipv4Address) {
        let arp = ArpFrame::new(
            ArpOperation::Request,
            self.mac_address,
            sender,
            mac_addr!(0),
            target,
        ).to_bytes();

        self.send(mac_broadcast_addr!(), EtherType::Arp, &arp);
    }

    /// Sends an ARP reply; respond to an ARP request.
    pub fn send_arp_reply(&mut self, sender_ip: Ipv4Address, target: Ipv4Address) {
        let arp = ArpFrame::new(
            ArpOperation::Reply,
            self.mac_address,
            sender_ip,
            self.mac_address,
            target,
        ).to_bytes();

        self.send(mac_broadcast_addr!(), EtherType::Arp, &arp);
    }

    /// Sends data as an EthernetFrame from this interface to the destination MAC address.
    /// 
    /// The source MAC address is variable.
    pub fn sends(&mut self, source: MacAddress, destination: MacAddress, ether_type: EtherType, data: &Vec<u8>) {
        let frame = EthernetFrame::new(destination, source, data.clone(), ether_type);
        self.port.borrow_mut().send(&frame.to_bytes());
    }

    /// Sends data as an EthernetFrame from this interface to the destination MAC address.
    /// 
    /// The source MAC address is this interface's MAC address.
    pub fn send(&mut self, destination: MacAddress, ether_type: EtherType, data: &Vec<u8>) {
        self.sends(self.mac_address, destination, ether_type, data);
    }

    /// Returns a list of Ethernet frames that were received since the last call.
    pub fn receive(&mut self) -> Vec<EthernetFrame> {
        let bytes = self.port.borrow_mut().consume_incoming();
        if bytes.is_empty() {
            return vec![];
        }

        let frames = bytes
            .iter()
            .map(|b| EthernetFrame::from_bytes(b))
            .filter(|f| f.is_ok()).map(|f| f.unwrap())
            .collect();
        
        frames
    }
}