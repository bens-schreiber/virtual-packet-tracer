use std::{cell::RefCell, rc::Rc};

use crate::{
    ethernet::MacAddress,
    ipv4::{interface::Ipv4Interface, Ipv4Address},
    mac_addr, network_address,
};

use super::cable::EthernetPort;

#[derive(Debug)]
struct RouterPort {
    interface: RefCell<Ipv4Interface>,
    enabled: bool,
}

#[derive(Debug, Eq, PartialEq, Hash)]
struct Route {
    address: Ipv4Address,
    subnet_mask: Ipv4Address,
    network_address: Ipv4Address,
    port: usize,
}

pub struct Router {
    ports: [RefCell<RouterPort>; 8], // 8 physical ports
    table: Vec<Route>, // TODO: Could use a Trie here instead of checking an addr against the anded subnet mask of each route
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
    /// This will create ports of addresses `mac_addr!(1)` through `mac_addr!(7)`.
    pub fn from_seed(mac_seed: u8) -> Router {
        let ports: [RefCell<RouterPort>; 8] = (0..8)
            .map(|i| {
                RefCell::new(RouterPort {
                    interface: RefCell::new(Ipv4Interface::new(
                        mac_addr!(mac_seed + i),
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
            table: Vec::new(),
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

        // Set the IP address and subnet mask
        let rp_ipv4 = &mut *rp.interface.borrow_mut();
        rp_ipv4.ip_address = ipv4_address;
        rp_ipv4.subnet_mask = subnet_mask;
        rp.enabled = true;

        // Add the route to the table
        self.table.push(Route {
            address: ipv4_address,
            subnet_mask,
            network_address: network_address!(ipv4_address, subnet_mask),
            port,
        });
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

            let frames = rp.interface.borrow_mut().receive();
            for frame in frames {
                let route = self.table.iter().find(|r| {
                    network_address!(frame.destination, r.subnet_mask) == r.network_address
                    // TODO: A trie could be useful here
                });

                if let Some(route) = route {
                    let d_rp = &mut *self.ports[route.port].borrow_mut();

                    // Send without modifying the source IP, just the MAC
                    d_rp.interface.borrow_mut().sendv(
                        frame.source,
                        frame.destination,
                        frame.ttl - 1,
                        frame.data,
                    );
                }

                // TODO: ELSE: ICMP Destination Unreachable, maybe a default route?
            }
        }
    }

    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.ports
            .iter()
            .map(|i| i.borrow().interface.borrow().ethernet.port())
            .collect()
    }

    #[cfg(test)]
    pub fn receive_port(&mut self, port: usize) -> Vec<crate::ipv4::Ipv4Frame> {
        self.ports[port]
            .borrow_mut()
            .interface
            .borrow_mut()
            .receive()
    }
}
