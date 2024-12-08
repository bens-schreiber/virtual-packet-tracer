use crate::tick::Tickable;
use std::{cell::RefCell, rc::Rc};

/// Simulates the movement of data.
///
/// Holds a collection of EthernetPorts and moves data between on each `tick`.
#[derive(Default)]
pub struct CableSimulator {
    ports: Vec<Rc<RefCell<EthernetPort>>>,
}

impl CableSimulator {
    /// Adds a port to the simulator.
    pub fn add(&mut self, ethernet_port: Rc<RefCell<EthernetPort>>) {
        self.ports.push(ethernet_port.clone());
    }

    /// Adds multiple ports to the simulator.
    pub fn adds(&mut self, ethernet_ports: Vec<Rc<RefCell<EthernetPort>>>) {
        for e in ethernet_ports {
            self.add(e);
        }
    }

    /// Removes a port from the simulator.
    pub fn remove(&mut self, ethernet_port: Rc<RefCell<EthernetPort>>) {
        self.ports.retain(|port| Rc::ptr_eq(port, &ethernet_port));
    }

    /// Removes multiple ports from the simulator.
    pub fn removes(&mut self, ethernet_ports: Vec<Rc<RefCell<EthernetPort>>>) {
        self.ports.retain(|port| {
            !ethernet_ports
                .iter()
                .any(|ethernet_port| Rc::ptr_eq(port, ethernet_port))
        });
    }

    /// All ports consume their outgoing buffer and move it to the other port's incoming buffer.
    pub fn transmit(&mut self) {
        for port in self.ports.iter() {
            let mut port = port.borrow_mut();

            if let Some(ref mut connection) = port.connection.clone() {
                port.consume_outgoing(&mut connection.borrow_mut());
            } else {
                port.consume_outgoing(&mut EthernetPort::default());
            }
        }
    }
}

impl Tickable for CableSimulator {
    fn tick(&mut self) {
        self.transmit();
    }
}

/// A physical ethernet port capable of sending and receiving bytes via a physical (cable) connection.
///
/// This simulated port uses the idea of an Interpacket Gap (IPG) to prepare between frames for transmission
/// (represented by the Vec<Vec<u8>>, each Vec<u8> is a frame, able to be individually received because of the IPG).
#[derive(Debug, Clone, Default)]
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
    /// Connects two ethernet ports together. This is a bi-directional connection.
    /// * `port1` - The first port to connect.
    /// * `port2` - The second port to connect.
    ///
    /// # Panics
    /// Panics if either port is already connected to a different port.
    pub fn connect(port1: &Rc<RefCell<EthernetPort>>, port2: &Rc<RefCell<EthernetPort>>) {
        if port1.borrow().connection.is_some() || port2.borrow().connection.is_some() {
            panic!("A port already has a connection");
        }
        port1.borrow_mut().connection = Some(port2.clone());
        port2.borrow_mut().connection = Some(port1.clone());
    }

    /// Mutually disconnects the connection between two ethernet ports.
    //
    // A computer can tell if a cable is connected if the electrical circuit is closed.
    // Instead of handling the situation of dangling connections, just disconnect both ports.
    pub fn disconnect(&mut self) {
        if let Some(connection) = self.connection.clone() {
            connection.borrow_mut().connection = None;
        }
        self.connection = None;
    }

    /// Appends the data to the outgoing buffer.
    pub fn send(&mut self, data: Vec<u8>) {
        self.outgoing_buffer.push(data);
    }

    /// Appends the data the incoming buffer.
    pub fn send_to_self(&mut self, data: Vec<u8>) {
        self.incoming_buffer.push(data);
    }

    /// Clears the outgoing buffer and appends it to the other's incoming buffer.
    fn consume_outgoing(&mut self, consumable: &mut EthernetPort) {
        consumable.incoming_buffer.append(&mut self.outgoing_buffer);
    }

    /// Clears the incoming buffer and returns it.
    pub fn consume_incoming(&mut self) -> Vec<Vec<u8>> {
        let mut incoming = vec![];
        incoming.append(&mut self.incoming_buffer);
        incoming
    }

    pub fn sniff(&self) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        (self.incoming_buffer.clone(), self.outgoing_buffer.clone())
    }

    pub fn has_outgoing(&self) -> bool {
        !self.outgoing_buffer.is_empty()
    }

    pub fn has_incoming(&self) -> bool {
        !self.incoming_buffer.is_empty()
    }
}
