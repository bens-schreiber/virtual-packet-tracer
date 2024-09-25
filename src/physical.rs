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
    pub fn connect(port1: Rc<RefCell<EthernetPort>>, port2: Rc<RefCell<EthernetPort>>) {
        port1.borrow_mut().connection = Some(port2.clone());
        port2.borrow_mut().connection = Some(port1.clone());
    }

    pub fn send(&mut self, data: Vec<u8>) {
        if self.connection.is_none() {
            return;
        }

        self.outgoing_buffer = data;
    }

    pub fn receive(&mut self) -> Option<Vec<u8>> {
        if self.incoming_buffer.len() > 0 {
            Some(self.incoming_buffer.clone())
        } else {
            None
        }
    }
}

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
            if let Some(connection) = port.connection.clone() {
                let mut connection = connection.borrow_mut();
                connection.incoming_buffer = port.outgoing_buffer.clone();
                port.outgoing_buffer.clear();
            }
        }
    }
}