use std::{cell::RefCell, rc::Rc};

use super::ethernet_port::EthernetPort;

/// Simulates the movement of data over a physical connection between EthernetPorts.
pub struct PhysicalSimulator {
    ports: Vec<Rc<RefCell<EthernetPort>>>,
}

impl PhysicalSimulator {
    pub fn new() -> PhysicalSimulator {
        PhysicalSimulator {
            ports: Vec::new(),
        }
    }

    pub fn add_port(&mut self, port: Rc<RefCell<EthernetPort>>) {
        self.ports.push(port);
    }

    pub fn add_ports(&mut self, ports: Vec<Rc<RefCell<EthernetPort>>>) {
        for port in ports {
            self.add_port(port);
        }
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