use std::{cell::RefCell, rc::Rc};

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
    pub(super) connection: Option<Rc<RefCell<EthernetPort>>>,
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
    pub fn send(&mut self, data: Vec<u8>) {
        self.outgoing_buffer.push(data);
    }

    /// Clears the outgoing buffer and appends it to the other's incoming buffer.
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
    pub fn has_outgoing(&self) -> bool {
        !self.outgoing_buffer.is_empty()
    }
    
}