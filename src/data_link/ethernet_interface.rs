use std::{cell::RefCell, rc::Rc};

use crate::{mac_addr, mac_broadcast_addr, network::ipv4::IPv4Address, physical::ethernet_port::EthernetPort};
use super::{arp_frame::{ArpFrame, ArpOperation}, ethernet_frame::{EtherType, EthernetFrame, MacAddress}};

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
    pub fn connect_port(&mut self, other: &mut EthernetInterface) {
        EthernetPort::connect_ports(self.port.clone(), other.port.clone());
    }

    /// Sends an ARP request; find the MAC address of the target IP address.
    pub fn send_arp(&mut self, sender: IPv4Address, target: IPv4Address) {
        let arp = ArpFrame::new(
            ArpOperation::Request,
            self.mac_address,
            sender,
            mac_addr!(0),
            target,
        ).to_bytes();

        self.send(mac_broadcast_addr!(), arp);
    }

    /// Sends data as an EthernetFrame.
    /// 
    /// NOTE: The data is not sent immediately. It is added to the outgoing buffer. Simulator must be ticked to send the data.
    pub fn send(&mut self, destination: MacAddress, data: Vec<u8>) {
        let frame = EthernetFrame::new(destination, self.mac_address, data, EtherType::Arp);
        self.port.borrow_mut().add_outgoing(&frame.to_bytes());
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