use std::{cell::RefCell, rc::Rc};

use super::port::EthernetPort;

/// Simulates the movement of data over a physical connection between EthernetPorts.
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
            let connection = port.connection();

            // Connection, so move the outgoing buffer to the other port's incoming buffer
            if let Some(connection) = connection {
                port.consume_outgoing(&mut connection.borrow_mut());
                continue;
            }

            // No connection, so clear the outgoing buffer
            port.consume_outgoing(&mut EthernetPort::new());

        }
    }
}