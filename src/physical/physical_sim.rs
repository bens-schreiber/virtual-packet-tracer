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

    /// Adds a port to the simulator.
    pub fn add(&mut self, port: Rc<RefCell<EthernetPort>>) {
        self.ports.push(port);
    }

    /// Adds multiple ports to the simulator.
    pub fn adds(&mut self, ports: Vec<Rc<RefCell<EthernetPort>>>) {
        for port in ports {
            self.add(port);
        }
    }

    /// Simulates the movement of data over the physical connection.
    /// 
    /// This means all ports will consume their outgoing buffer and move it to the other port's incoming buffer.
    /// 
    /// All data in this simulation is moved in a single tick, thus the simulator is synchronous.
    pub fn tick(&mut self) {
        for port in self.ports.iter() {
            let mut port = port.borrow_mut();

            // Connection, so move the outgoing buffer to the other port's incoming buffer
            if let Some(connection) = port.connection.clone() {
                port.consume_outgoing(&mut connection.borrow_mut());
                continue;
            }

            // No connection, so clear the outgoing buffer
            port.consume_outgoing(&mut EthernetPort::new());

        }
    }
}