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

    /// Connects two ethernet ports together. This is a bi-directional connection.
    pub fn connect_ports(port1: Rc<RefCell<EthernetPort>>, port2: Rc<RefCell<EthernetPort>>) {
        port1.borrow_mut().connection = Some(port2.clone());
        port2.borrow_mut().connection = Some(port1.clone());
    }

    /// Returns the connection if one exists.
    pub fn connection(&self) -> Option<Rc<RefCell<EthernetPort>>> {
        self.connection.clone()
    }

    /// Appends the data to the outgoing buffer.
    pub fn add_outgoing(&mut self, data: &mut Vec<u8>) {
        self.outgoing_buffer.append(data);
    }

    /// Consumes the outgoing buffer and appends it to the other's incoming buffer.
    pub fn consume_outgoing(&mut self, other: &mut EthernetPort) {
        other.incoming_buffer.append(&mut self.outgoing_buffer);
    }

    /// Consumes the incoming buffer and returns it.
    pub fn consume_incoming(&mut self) -> Vec<u8> {
        let mut incoming = vec![];
        incoming.append(&mut self.incoming_buffer);

        incoming
    }

    pub fn has_outgoing(&self) -> bool {
        !self.outgoing_buffer.is_empty()
    }
    
}