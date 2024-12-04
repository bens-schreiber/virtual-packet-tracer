use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    is_ipv4_multicast_or_broadcast, mac_addr,
    network::{
        ethernet::{ByteSerialize, MacAddress},
        ipv4::{interface::Ipv4Interface, IcmpType, Ipv4Address, Ipv4Protocol},
    },
    network_address,
    tick::{TickTimer, Tickable},
};

use super::cable::EthernetPort;

/// A route in the router's routing table.
#[derive(Debug, Eq, PartialEq, Hash)]
struct Route {
    ip_address: Ipv4Address, // Network address
    subnet_mask: Ipv4Address,
    metric: u32, // Hops until the destination
    port: usize,
}

impl Route {
    fn new(ip_address: Ipv4Address, subnet_mask: Ipv4Address, port: usize) -> Route {
        Route {
            ip_address,
            subnet_mask,
            metric: 0,
            port,
        }
    }
}

#[derive(Debug)]
struct RouterPort {
    interface: RefCell<Ipv4Interface>,
    enabled: bool,
    rip_enabled: bool,
}

#[derive(Hash, Eq, PartialEq, Clone)]
enum RouterDelayedAction {
    RipMulticast,
}

/// A layer 3 router that routes IPv4 frames between interfaces, and broadcasts RIP frames on all RIP-enabled interfaces.
pub struct Router {
    ports: [RefCell<RouterPort>; 8],    // 8 physical ports
    table: HashMap<Ipv4Address, Route>, // network address => route
    mac_address: MacAddress,
    rip_enabled: bool,
    timer: TickTimer<RouterDelayedAction>,
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
    pub fn from_seed(mac_seed: u64) -> Router {
        let ports: [RefCell<RouterPort>; 8] = (0..8)
            .map(|i| {
                RefCell::new(RouterPort {
                    interface: RefCell::new(Ipv4Interface::router_interface(
                        mac_addr!(mac_seed + i),
                        [0, 0, 0, 0],
                        [0, 0, 0, 0],
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
            rip_enabled: false,
            timer: TickTimer::new(),
        }
    }

    /// Routes frames between interfaces on the router.
    /// Routes broadcast and multicast frames to their broadcast domain.
    pub fn route(&mut self) {
        for i in 0..self.ports.len() {
            let rp = &mut *self.ports[i].borrow_mut();
            if !rp.enabled {
                rp.interface.borrow_mut().ethernet.receive(); // Drop frames, bypass ipv4 processing
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
                    let _ = d_rp.interface.borrow_mut().sendv(
                        frame.source,
                        frame.destination,
                        Some(route.ip_address),
                        frame.ttl - 1,
                        frame.data,
                        Ipv4Protocol::from(frame.protocol),
                    );

                    continue;
                }

                let _ = rp
                    .interface
                    .borrow_mut()
                    .send_icmp(frame.source, IcmpType::Unreachable);
            }
        }
    }

    fn _create_rip_frame(&mut self) -> RipFrame {
        let mut frame = RipFrame::new_response();
        for (k, v) in &self.table {
            frame.routes.push(RipRoute::new(
                k.clone(),
                v.subnet_mask,
                [0, 0, 0, 0],
                v.metric,
            ));
        }
        frame
    }

    /// Floods all enabled ports with a RIP frame.
    pub fn send_rip_frames(&mut self) {
        let data = self._create_rip_frame().to_bytes();

        for i in 0..self.ports.len() {
            let rp = &mut *self.ports[i].borrow_mut();
            if !rp.enabled || !rp.rip_enabled {
                continue;
            }

            rp.interface
                .borrow_mut()
                .multicast(data.clone(), Ipv4Protocol::Rip);
        }
    }

    pub fn ports(&self) -> Vec<Rc<RefCell<EthernetPort>>> {
        self.ports
            .iter()
            .map(|i| i.borrow().interface.borrow().ethernet.port())
            .collect()
    }

    #[cfg(test)]
    /// Receives the IPv4 frames from the interface on a given port instead of routing them.
    pub fn receive_port(&mut self, port: usize) -> Vec<crate::network::ipv4::Ipv4Frame> {
        self.ports[port]
            .borrow_mut()
            .interface
            .borrow_mut()
            .receive()
    }

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
    /// Sends a RIP frame to the multicast address.
    /// * `port` - The port number to enable RIP on.
    pub fn enable_rip(&mut self, port: usize) {
        if !self.ports[port].borrow_mut().enabled {
            panic!("Port {} is disabled.", port);
        }

        let frame = self._create_rip_frame();
        let mut rp = self.ports[port].borrow_mut();
        self.rip_enabled = true;
        rp.rip_enabled = true;
        rp.interface
            .borrow_mut()
            .multicast(frame.to_bytes(), Ipv4Protocol::Rip);

        self.timer
            .schedule(RouterDelayedAction::RipMulticast, 5, true);
    }

    /// Connects an interface to the router with the given port number.
    /// * `port` - The port number to connect the interface to.
    /// * `interface` - The interface to connect to the router.
    ///
    /// # Panics
    /// Panics if the port is out of range
    pub fn connect(&mut self, port: usize, interface: &mut Ipv4Interface) {
        let rp = &mut *self.ports[port].borrow_mut();
        rp.interface.borrow_mut().connect(interface);
    }

    #[cfg(test)]
    /// Connects a router to another router on the given ports.
    pub fn connect_router(&mut self, port: usize, other_router: &mut Router, other_port: usize) {
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
    pub fn disconnect(&mut self, port: usize) {
        let rp = &mut *self.ports[port].borrow_mut();
        if !rp.enabled {
            return;
        }

        self.table.retain(|_, v| v.port != port);

        rp.interface.borrow_mut().disconnect();
        rp.enabled = false;
        rp.rip_enabled = false;
    }

    pub fn mac_addr(&self, port: usize) -> MacAddress {
        self.ports[port]
            .borrow()
            .interface
            .borrow()
            .ethernet
            .mac_address
    }

    pub fn is_port_up(&self, port: usize) -> bool {
        self.ports[port].borrow().enabled
    }
}

impl Tickable for Router {
    fn tick(&mut self) {
        self.route();

        for action in self.timer.ready() {
            match action {
                RouterDelayedAction::RipMulticast => {
                    self.send_rip_frames();
                }
            }
        }

        self.timer.tick();
    }
}

struct RipRoute {
    address_family: u16, // 0x0002
    route_tag: u16,      // 0x0000
    ip_address: Ipv4Address,
    subnet_mask: Ipv4Address,
    next_hop: Ipv4Address, // TODO: unused for now
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
