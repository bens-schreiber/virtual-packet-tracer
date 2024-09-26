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

        let mut frames = Vec::new();
        let mut preamble_count = 0;
        let mut i = 0;

        // Find all valid frames in the incoming bytes
        // Need to be careful because the data could contain our preamble and sfd
        // so we need to skip over those bytes
        while i < bytes.len() {

            // Found a valid frame
            if preamble_count == 7 && bytes[i] == 0xD5 {
                let frame = EthernetFrame::from_bytes(&bytes[i - 7..]);
                if frame.is_ok() {
                    let unwrp = frame.unwrap();
                    i += unwrp.size() - 7;
                    frames.push(unwrp);
                }
                preamble_count = 0;
                continue;
            }

            if bytes[i] == 0x55 {
                preamble_count += 1;
            } else {
                preamble_count = 0;
            }
            
            i += 1;
        }
        
        frames
    }
}