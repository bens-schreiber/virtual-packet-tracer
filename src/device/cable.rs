use std::{cell::RefCell, rc::Rc};

/// Simulates the movement of data.
///
/// Holds a collection of EthernetPorts and moves data between them in a synchronous manner.
pub struct CableSimulator {
    ports: Vec<Rc<RefCell<EthernetPort>>>,
}

impl CableSimulator {
    pub fn new() -> CableSimulator {
        CableSimulator { ports: Vec::new() }
    }

    /// Adds a port to the simulator.
    /// * `ethernet_port` - The port to add to the simulator.
    pub fn add(&mut self, ethernet_port: Rc<RefCell<EthernetPort>>) {
        self.ports.push(ethernet_port);
    }

    /// Adds multiple ports to the simulator.
    /// * `ethernet_ports` - The ports to add to the simulator.
    pub fn adds(&mut self, ethernet_ports: Vec<Rc<RefCell<EthernetPort>>>) {
        for e in ethernet_ports {
            self.add(e);
        }
    }

    /// Simulates the movement of data over the physical connection.
    ///
    /// This means all ports will consume their outgoing buffer and move it to the other port's incoming buffer.
    ///
    /// All data in this simulation is moved in a single tick, thus the simulator is synchronous.
    pub fn tick(&self) {
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

/// A physical ethernet port capable of sending and receiving bytes via a physical (cable) connection.
///
/// This simulated port uses the idea of an Interpacket Gap (IPG) to prepare between frames for transmission
/// (represented by the Vec<Vec<u8>>, each Vec<u8> is a frame, able to be individually received because of the IPG).
#[derive(Debug, Clone)]
pub struct EthernetPort {
    /// Incoming bytes from the physical connection
    incoming_buffer: Vec<Vec<u8>>,

    /// Outgoing bytes to the physical connection.
    /// Note that the EthernetPort is only responsible for putting bytes into this buffer.
    /// The simulator will take care of moving the bytes to the other port.
    outgoing_buffer: Vec<Vec<u8>>,

    /// None if a physical connection is not established
    connection: Option<Rc<RefCell<EthernetPort>>>,
}

impl EthernetPort {
    pub fn new() -> EthernetPort {
        EthernetPort {
            incoming_buffer: Vec::new(),
            outgoing_buffer: Vec::new(),
            connection: None,
        }
    }

    /// Connects two ethernet ports together. This is a bi-directional connection.
    pub fn connect(port1: &Rc<RefCell<EthernetPort>>, port2: &Rc<RefCell<EthernetPort>>) {
        port1.borrow_mut().connection = Some(port2.clone());
        port2.borrow_mut().connection = Some(port1.clone());
    }

    /// Appends the data to the outgoing buffer.
    /// * `data` - The data to append to the outgoing buffer.
    pub fn send(&mut self, data: Vec<u8>) {
        self.outgoing_buffer.push(data);
    }

    /// Clears the outgoing buffer and appends it to the other's incoming buffer.
    /// * `consumable` - The port to consume the outgoing buffer.
    pub fn consume_outgoing(&mut self, consumable: &mut EthernetPort) {
        consumable.incoming_buffer.append(&mut self.outgoing_buffer);
    }

    /// Clears the incoming buffer and returns it.
    pub fn consume_incoming(&mut self) -> Vec<Vec<u8>> {
        let mut incoming = vec![];
        incoming.append(&mut self.incoming_buffer);

        incoming
    }

    /// Returns true if there are bytes in the incoming buffer.
    #[cfg(test)]
    pub(crate) fn has_outgoing(&self) -> bool {
        !self.outgoing_buffer.is_empty()
    }
}
