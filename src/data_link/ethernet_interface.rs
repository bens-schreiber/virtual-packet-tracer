use std::{cell::RefCell, rc::Rc};

use crate::{mac_broadcast_addr, physical::ethernet_port::EthernetPort};
use super::ethernet_frame::{EtherType, EthernetFrame, MacAddress};

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

    /// TODO: Assumes ARP for now
    /// 
    /// Sends data as an EthernetFrame.
    /// 
    /// NOTE: The data is not sent immediately. It is added to the outgoing buffer. Simulator must be ticked to send the data.
    pub fn send_data(&mut self, data: Vec<u8>) {
        self.port.borrow_mut().add_outgoing(&mut EthernetFrame::new(
            mac_broadcast_addr!(),
            self.mac_address,
            data,
            EtherType::Arp
        ).to_bytes()
    );
    }

    /// Returns a list of Ethernet frames that were received since the last call.
    pub fn receive_frames(&mut self) -> Vec<EthernetFrame> {
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