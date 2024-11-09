use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ethernet::MacAddress,
    ipv4::{interface::Ipv4Interface, Ipv4Address},
    mac_addr,
};

use super::cable::EthernetPort;

#[derive(Debug)]
struct RouterPort {
    interface: RefCell<Ipv4Interface>,
    enabled: bool,
}

pub struct Router {
    ports: [RefCell<RouterPort>; 8],    // 8 physical ports
    table: HashMap<Ipv4Address, usize>, // maps an address to the interface it's connected to.
    mac_address: MacAddress,
    debug_tag: u8,
}

impl Router {
    /// Creates a new router with 8 network interfaces, each with a MAC address derived from the seed.
    /// All interfaces are disconnected.
    /// * `mac_seed` - The range of MAC addresses for the router's interfaces.
    ///
    /// # Example
    /// ```
    /// let router = Router::from_seed(1);
    /// ```
    /// This will create a router with the routers MAC address as `mac_addr!(1)` and the interfaces MAC addresses as `mac_addr!(2)` through `mac_addr!(9)`.
    pub fn from_seed(mac_seed: u8) -> Router {
        let ports: [RefCell<RouterPort>; 8] = (0..8)
            .map(|i| {
                RefCell::new(RouterPort {
                    interface: RefCell::new(Ipv4Interface::new(
                        mac_addr!(mac_seed + i + 1),
                        [0, 0, 0, 0],
                        [0, 0, 0, 0],
                        None,
                    )),
                    enabled: false,
                })
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        Router {
            ports,
            table: HashMap::new(),
            mac_address: mac_addr!(mac_seed),
            debug_tag: mac_seed,
        }
    }

    /// Enables an interface on the router with the given port number, IP address, and subnet mask.
    /// * `port` - The port number to enable the interface on.
    /// * `ipv4_address` - The IP address to assign to the interface.
    /// * `subnet_mask` - The subnet mask to assign to the interface.
    ///
    /// # Panics
    /// Panics if the port is out of range or if the port is already enabled.
    pub fn enable_interface(
        &mut self,
        port: usize,
        ipv4_address: Ipv4Address,
        subnet_mask: Ipv4Address,
    ) {
        if port >= 8 {
            panic!("Port {} is out of range for the router.", port);
        }

        let rp = &mut *self.ports[port].borrow_mut();
        if rp.enabled {
            panic!("Port {} is already enabled.", port);
        }

        let rp_ipv4 = &mut *rp.interface.borrow_mut();
        rp_ipv4.ip_address = ipv4_address;
        rp_ipv4.subnet_mask = subnet_mask;
        rp.enabled = true;
    }

    /// Disables an interface on the router with the given port number.
    /// * `port` - The port number to disable the interface on.
    ///
    /// # Panics
    /// Panics if the port is out of range or if the port is already disabled.
    pub fn disable_interface(&mut self, port: usize) {
        if port >= 8 {
            panic!("Port {} is out of range for the router.", port);
        }

        let rp = &mut *self.ports[port].borrow_mut();
        if !rp.enabled {
            panic!("Port {} is already disabled.", port);
        }

        rp.enabled = false;
    }

    /// Connects an interface to the router with the given port number.
    /// * `port` - The port number to connect the interface to.
    /// * `interface` - The interface to connect to the router.
    ///
    /// # Panics
    /// Panics if the port is out of range or if the port is disabled.
    pub fn connect(&mut self, port: usize, interface: &mut Ipv4Interface) {
        if port >= 8 {
            panic!("Port {} is out of range for the router.", port);
        }

        let rp = &mut *self.ports[port].borrow_mut();
        if !rp.enabled {
            panic!("Port {} is disabled.", port);
        }

        rp.interface.borrow_mut().connect(interface);
    }

    pub fn route(&mut self) {
        for i in 0..8 {
            let rp = &mut *self.ports[i].borrow_mut();
            if !rp.enabled {
                continue;
            }

            // TODO: Implement routing
        }
    }

    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.ports
            .iter()
            .map(|i| i.borrow().interface.borrow().ethernet.port())
            .collect()
    }
}
