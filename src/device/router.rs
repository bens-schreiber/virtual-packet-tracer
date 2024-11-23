use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    ethernet::{ByteSerialize, MacAddress},
    ipv4::{interface::Ipv4Interface, Ipv4Address},
    is_ipv4_multicast_or_broadcast, mac_addr, network_address,
};

use super::cable::EthernetPort;

#[derive(Debug)]
struct RouterPort {
    interface: RefCell<Ipv4Interface>,
    enabled: bool,
    rip_enabled: bool,
}

#[derive(Debug, Eq, PartialEq, Hash)]
struct Route {
    ip_address: Ipv4Address,
    subnet_mask: Ipv4Address,
    next_hop: Ipv4Address,
    metric: u32,
    port: usize,
}

impl Route {
    fn new(ip_address: Ipv4Address, subnet_mask: Ipv4Address, port: usize) -> Route {
        Route {
            ip_address,
            subnet_mask,
            next_hop: [0, 0, 0, 0],
            metric: 0,
            port,
        }
    }
}

pub struct Router {
    ports: [RefCell<RouterPort>; 8],    // 8 physical ports
    table: HashMap<Ipv4Address, Route>, // network address => route
    mac_address: MacAddress,
    debug_tag: u8,

    rip_enabled: bool,
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
                    rip_enabled: false,
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
            rip_enabled: false,
        }
    }

    /// Routes frames between interfaces on the router.
    /// Routes broadcast and multicast frames to their broadcast domain.
    pub fn route(&mut self) {
        for i in 0..self.ports.len() {
            let rp = &mut *self.ports[i].borrow_mut();
            if !rp.enabled {
                continue;
            }

            let frames = rp.interface.borrow_mut().receive();
            for frame in frames {
                if is_ipv4_multicast_or_broadcast!(frame.destination) {
                    let rip_frame = RipFrame::from_bytes(frame.data);
                    if rip_frame.is_err() {
                        continue; // not a RIP frame, drop it
                    }

                    let rip_frame = rip_frame.unwrap();
                    for rip_route in rip_frame.routes {
                        let new_route = Route {
                            ip_address: frame.source,
                            subnet_mask: rip_route.subnet_mask,
                            next_hop: rip_route.next_hop,
                            metric: rip_route.metric + 1,
                            port: i,
                        };

                        match self.table.get(&rip_route.ip_address) {
                            Some(current_route) if current_route.metric > new_route.metric => {
                                self.table.insert(rip_route.ip_address, new_route);
                            }
                            None => {
                                self.table.insert(rip_route.ip_address, new_route);
                            }
                            _ => {}
                        }
                    }

                    continue;
                }

                // TODO: A prefix trie could be more efficient here.
                let route = self
                    .table
                    .iter()
                    .find(|(k, v)| network_address!(frame.destination, v.subnet_mask) == **k)
                    .map(|(_, v)| v);

                if let Some(route) = route {
                    let d_rp = &mut *self.ports[route.port].borrow_mut();

                    // Send without modifying the source IP, just the MAC
                    d_rp.interface.borrow_mut().sendv(
                        frame.source,
                        frame.destination,
                        Some(route.ip_address),
                        frame.ttl - 1,
                        frame.data,
                    );
                }

                // TODO: ELSE: ICMP Destination Unreachable, maybe a default route?
            }
        }
    }

    /// Floods all enabled ports with a RIP frame.
    pub fn send_rip_frames(&mut self) {
        let data = {
            let mut frame = RipFrame::new_response();
            for (k, v) in &self.table {
                frame.routes.push(RipRoute::new(
                    k.clone(),
                    v.subnet_mask,
                    v.next_hop,
                    v.metric,
                ));
            }
            frame.to_bytes()
        };

        for i in 0..self.ports.len() {
            let rp = &mut *self.ports[i].borrow_mut();
            if !rp.enabled || !rp.rip_enabled {
                continue;
            }

            rp.interface.borrow_mut().multicast(data.clone());
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

// ##### Router Configuration
impl Router {
    /// Configures an interface with a ip address, subnet and network address.
    /// * `port` - The port number to enable.
    ///
    /// # Panics
    /// Panics if the port is out of range.
    pub fn enable_interface(
        &mut self,
        port: usize,
        ipv4_address: Ipv4Address,
        subnet_mask: Ipv4Address,
    ) {
        if port >= self.ports.len() {
            panic!("Port {} is out of range for the router.", port);
        }

        let rp = &mut *self.ports[port].borrow_mut();
        rp.enabled = true;

        // Set the IP address and subnet mask
        let rp_ipv4 = &mut *rp.interface.borrow_mut();
        rp_ipv4.ip_address = ipv4_address;
        rp_ipv4.subnet_mask = subnet_mask;

        // Add the route to the table
        self.table.insert(
            network_address!(ipv4_address, subnet_mask),
            Route::new(ipv4_address, subnet_mask, port),
        );
    }

    /// Enables RIP on a port on the router.
    /// * `port` - The port number to enable RIP on.
    pub fn enable_rip(&mut self, port: usize) {
        if port >= self.ports.len() {
            panic!("Port {} is out of range for the router.", port);
        }

        let mut rp = self.ports[port].borrow_mut();
        if !rp.enabled {
            panic!("Port {} is disabled.", port);
        }

        self.rip_enabled = true;
        rp.rip_enabled = true;
    }

    /// Connects an interface to the router with the given port number.
    /// * `port` - The port number to connect the interface to.
    /// * `interface` - The interface to connect to the router.
    ///
    /// # Panics
    /// Panics if the port is out of range
    pub fn connect(&mut self, port: usize, interface: &mut Ipv4Interface) {
        if port >= self.ports.len() {
            panic!("Port {} is out of range for the router.", port);
        }

        let rp = &mut *self.ports[port].borrow_mut();
        rp.interface.borrow_mut().connect(interface);
    }

    #[cfg(test)]
    pub fn connect_router(&mut self, port: usize, other_router: &mut Router, other_port: usize) {
        if port >= self.ports.len() {
            panic!("Port {} is out of range for the router.", port);
        }

        let rp = &mut *self.ports[port].borrow_mut();
        if !rp.enabled {
            panic!("Port {} is disabled.", port);
        }

        let other_rp = &mut *other_router.ports[other_port].borrow_mut();
        if !other_rp.enabled {
            panic!("Port {} is disabled.", other_port);
        }

        rp.interface
            .borrow_mut()
            .connect(&mut other_rp.interface.borrow_mut());
    }

    /// Disconnects an interface on the router with the given port number.
    /// * `port` - The port number to disable the interface on.
    ///
    /// # Panics
    /// Panics if the port is out of range
    pub fn disconnect_interface(&mut self, port: usize) {
        if port >= self.ports.len() {
            panic!("Port {} is out of range for the router.", port);
        }

        let rp = &mut *self.ports[port].borrow_mut();
        if !rp.enabled {
            return;
        }

        rp.interface.borrow_mut().disconnect();
        rp.enabled = false;
        rp.rip_enabled = false;
    }
}

struct RipRoute {
    address_family: u16, // 0x0002
    route_tag: u16,      // 0x0000
    ip_address: Ipv4Address,
    subnet_mask: Ipv4Address,
    next_hop: Ipv4Address,
    metric: u32,
}

impl RipRoute {
    fn new(
        ip_address: Ipv4Address,
        subnet_mask: Ipv4Address,
        next_hop: Ipv4Address,
        metric: u32,
    ) -> RipRoute {
        RipRoute {
            address_family: 0x0002,
            route_tag: 0x0000,
            ip_address,
            subnet_mask,
            next_hop,
            metric,
        }
    }
}

struct RipFrame {
    command: u8, // 1 = Request, 2 = Response
    version: u8,
    routes: Vec<RipRoute>,
}

impl RipFrame {
    fn new_response() -> RipFrame {
        RipFrame {
            command: 2,
            version: 2,
            routes: Vec::new(),
        }
    }

    fn new_request() -> RipFrame {
        RipFrame {
            command: 1,
            version: 2,
            routes: Vec::new(),
        }
    }
}

impl ByteSerialize for RipRoute {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.address_family.to_be_bytes());
        bytes.extend_from_slice(&self.route_tag.to_be_bytes());
        bytes.extend_from_slice(&self.ip_address);
        bytes.extend_from_slice(&self.subnet_mask);
        bytes.extend_from_slice(&self.next_hop);
        bytes.extend_from_slice(&self.metric.to_be_bytes());
        bytes
    }

    fn from_bytes(bytes: Vec<u8>) -> Result<Self, std::io::Error> {
        if bytes.len() != 20 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "RIP route must be 20 bytes.",
            ));
        }

        let address_family = u16::from_be_bytes([bytes[0], bytes[1]]);
        let route_tag = u16::from_be_bytes([bytes[2], bytes[3]]);
        let ip_address = bytes[4..8].try_into().unwrap();
        let subnet_mask = bytes[8..12].try_into().unwrap();
        let next_hop = bytes[12..16].try_into().unwrap();
        let metric = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);

        Ok(RipRoute {
            address_family,
            route_tag,
            ip_address,
            subnet_mask,
            next_hop,
            metric,
        })
    }
}

impl ByteSerialize for RipFrame {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.command);
        bytes.push(self.version);
        for route in &self.routes {
            bytes.extend_from_slice(&route.to_bytes());
        }
        bytes
    }

    fn from_bytes(bytes: Vec<u8>) -> Result<Self, std::io::Error> {
        if bytes.len() < 2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "RIP frame must be at least 2 bytes.",
            ));
        }

        let command = bytes[0];
        let version = bytes[1];
        let mut routes = Vec::new();
        let mut i = 2;
        while i + 20 <= bytes.len() {
            let route = RipRoute::from_bytes(bytes[i..i + 20].to_vec())?;
            routes.push(route);
            i += 20;
        }

        Ok(RipFrame {
            command,
            version,
            routes,
        })
    }
}
