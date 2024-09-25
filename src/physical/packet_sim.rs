use std::{cell::RefCell, rc::Rc};

use super::port::EthernetPort;

/// Simulates the movement of data over a physical connection between two EthernetPorts.
pub struct PacketSimulator {
    ports: Vec<Rc<RefCell<EthernetPort>>>,
}

impl PacketSimulator {
    pub fn new() -> PacketSimulator {
        PacketSimulator {
            ports: Vec::new(),
        }
    }

    pub fn add_port(&mut self, port: Rc<RefCell<EthernetPort>>) {
        self.ports.push(port);
    }

    pub fn tick(&mut self) {
        for port in self.ports.iter() {
            let mut port = port.borrow_mut();
            if let Some(connection) = port.connection().clone() {
                let mut connection = connection.borrow_mut();
                port.consume_outgoing(&mut connection);
            }
        }
    }
}