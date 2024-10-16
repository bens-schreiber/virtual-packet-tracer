use std::{cell::RefCell, rc::Rc};

use crate::{
    device::cable::EthernetPort,
    ipv4::{ArpFrame, ArpOperation, Ipv4Address},
    mac_addr, mac_broadcast_addr,
};

use super::*;

/// A layer 2 interface for ethernet actions, sending and receiving Ethernet frames through a physical port stamped with a MAC address.
#[derive(Debug, Clone)]
pub struct EthernetInterface {
    port: Rc<RefCell<EthernetPort>>,
    pub mac_address: MacAddress,
}

impl EthernetInterface {
    pub fn new(mac_address: MacAddress) -> EthernetInterface {
        EthernetInterface {
            port: Rc::new(RefCell::new(EthernetPort::new())),
            mac_address,
        }
    }

    pub fn port(&self) -> Rc<RefCell<EthernetPort>> {
        self.port.clone()
    }

    /// Connects two EthernetInterfaces together via EthernetPorts
    /// * `other` - The interface to establish a bidirectional connection with. (self -> other and other -> self)
    pub fn connect(&self, other: &EthernetInterface) {
        EthernetPort::connect(&self.port, &other.port);
    }

    /// Sends data as EthernetII from this interface to the destination MAC address.
    /// The source MAC address is assumed to be this interface's MAC address.
    /// * `destination` - The destination MAC address to send the data to.
    /// * `ether_type` - The EtherType field of the EthernetII frame.
    /// * `data` - Byte data to send in the frame.
    pub fn send(&mut self, destination: MacAddress, ether_type: EtherType, data: Vec<u8>) {
        self.sendv(self.mac_address, destination, ether_type, data);
    }

    /// Sends data as EthernetII from this interface to the destination MAC address.
    /// * `source` - The source MAC address to send the data from.
    /// * `destination` - The destination MAC address to send the data to.
    /// * `ether_type` - The EtherType field of the EthernetII frame.
    /// * `data` - Byte data to send in the frame.
    pub fn sendv(
        &mut self,
        source: MacAddress,
        destination: MacAddress,
        ether_type: EtherType,
        data: Vec<u8>,
    ) {
        let frame = Ethernet2Frame::new(destination, source, data, ether_type);
        self.port.borrow_mut().send(frame.to_bytes());
    }

    /// Sends data as Ethernet802_3 from this interface to the destination MAC address.
    /// The source MAC address is assumed to be this interface's MAC address.
    /// * `destination` - The destination MAC address to send the data to.
    /// * `data` - Byte data to send in the frame.
    pub fn send8023(&mut self, destination: MacAddress, data: Vec<u8>) {
        let frame = Ethernet802_3Frame::new(destination, self.mac_address, data);
        self.port.borrow_mut().send(frame.to_bytes());
    }

    /// Broadcasts an ARP request to find the MAC address of the target IP address over EthernetII.
    /// The source MAC address is assumed to be this interface's MAC address.
    /// * `sender_ip` - The IP address of the sender.
    /// * `target_ip` - The IP address of the target to find the MAC address of.
    pub fn arp_request(&mut self, sender_ip: Ipv4Address, target_ip: Ipv4Address) {
        let arp = ArpFrame::new(
            ArpOperation::Request,
            self.mac_address,
            sender_ip,
            mac_addr!(0),
            target_ip,
        )
        .to_bytes();

        self.send(mac_broadcast_addr!(), EtherType::Arp, arp);
    }

    /// Unicasts an ARP reply to the destination MAC address over EthernetII.
    /// The source MAC address is assumed to be this interface's MAC address.
    /// * `sender_ip` - The IP address of the sender.
    /// * `destination_mac` - The MAC address to send the reply to.
    /// * `destination_ip` - The IP address of the destination to reply to.
    pub fn arp_reply(
        &mut self,
        sender_ip: Ipv4Address,
        destination_mac: MacAddress,
        destination_ip: Ipv4Address,
    ) {
        let arp = ArpFrame::new(
            ArpOperation::Reply,
            self.mac_address,
            sender_ip,
            destination_mac,
            destination_ip,
        )
        .to_bytes();

        self.send(destination_mac, EtherType::Arp, arp);
    }

    /// Returns a list of Ethernet frames that were received since the last call.
    ///
    /// Frames are filtered to only include those with a destination MAC address that matches this interface's MAC address or are broadcast/multicast.
    ///
    /// Frames with a source MAC address that is broadcast or multicast are also filtered out.
    pub fn receive(&mut self) -> Vec<EthernetFrame> {
        let bytes = self.port.borrow_mut().consume_incoming();
        if bytes.is_empty() {
            return vec![];
        }

        let frames = bytes
            .into_iter()
            .map(|b| EthernetFrame::from_bytes(b))
            .filter(|f| f.is_ok())
            .map(|f| f.unwrap())
            .collect();

        frames
    }

    /// Returns a list of EthernetII frames that were received since the last call.
    #[cfg(test)]
    pub(crate) fn receive_eth2(&mut self) -> Vec<Ethernet2Frame> {
        let frames = self.receive();
        let mut eth2_frames = Vec::new();

        for frame in frames {
            match frame {
                EthernetFrame::Ethernet2(eth2_frame) => eth2_frames.push(eth2_frame),
                _ => continue,
            }
        }

        eth2_frames
    }
}
