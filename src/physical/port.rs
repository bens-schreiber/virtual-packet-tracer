use std::{cell::RefCell, rc::Rc};

/// A physical ethernet port capable of sending and receiving bytes via a physical (cable) connection.
pub struct EthernetPort {

    /// Incoming bytes from the physical connection
    incoming_buffer: Vec<u8>,

    /// Outgoing bytes to the physical connection.
    /// Note that the EthernetPort is only responsible for putting bytes into this buffer.
    /// The simulator will take care of moving the bytes to the other port.
    outgoing_buffer: Vec<u8>,

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

    /// Returns a reference to the connected port, if any.
    pub fn connection(&self) -> Option<Rc<RefCell<EthernetPort>>> {
        self.connection.clone()
    }

    /// Consumes the outgoing buffer of another ethernet port and appends it to this port's incoming buffer.
    pub fn consume_outgoing(&mut self, other: &mut EthernetPort) {
        other.incoming_buffer.append(&mut self.outgoing_buffer);
        self.outgoing_buffer.clear();
    }

    /// Connects two ethernet ports together. This is a bi-directional connection.
    pub fn connect(port1: Rc<RefCell<EthernetPort>>, port2: Rc<RefCell<EthernetPort>>) {
        port1.borrow_mut().connection = Some(port2.clone());
        port2.borrow_mut().connection = Some(port1.clone());
    }

    /// Sends data to the outgoing buffer.
    pub fn send(&mut self, data: Vec<u8>) {
        if self.connection.is_none() {
            return;
        }

        self.outgoing_buffer = data;
    }

    /// Receives data from the incoming buffer.
    pub fn receive(&mut self) -> Option<Vec<u8>> {
        if self.incoming_buffer.len() > 0 {
            Some(self.incoming_buffer.clone())
        } else {
            None
        }
    }
}