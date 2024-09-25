use std::{cell::RefCell, rc::Rc};

use crate::{mac_broadcast_addr, physical::port::EthernetPort};
use super::frame::{EtherType, EthernetFrame, MacAddress};

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

    /// TODO: Assumes ARP for now
    pub fn send(&mut self, data: Vec<u8>) {
        let frame = EthernetFrame::new(
            mac_broadcast_addr!(),
            self.mac_address,
            data,
            EtherType::Arp
        );
        self.port.borrow_mut().send(frame.to_bytes());
    }

    pub fn receive(&mut self) -> Option<EthernetFrame> {
        let bytes = self.port.borrow_mut().receive()?;
        let frame = EthernetFrame::from_bytes(&bytes);
        match frame {
            Ok(f) => Some(f),
            Err(_) => None,
        }
    }

    pub fn connect(&mut self, other: &mut EthernetInterface) {
        EthernetPort::connect(self.port.clone(), other.port.clone());
    }
}