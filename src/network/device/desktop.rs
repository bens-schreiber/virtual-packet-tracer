use crate::{mac_addr, network::ipv4::interface::Ipv4Interface, tick::Tickable};

pub struct Desktop {
    pub interface: Ipv4Interface,
}

impl Desktop {
    pub fn from_seed(mac_seed: u64) -> Self {
        let mac_addr = mac_addr!(mac_seed);
        let ip_addr = [192, 168, 1, 1];
        let subnet_mask = [255, 255, 255, 0];
        let default_gateway = None;

        Self {
            interface: Ipv4Interface::new(mac_addr, ip_addr, subnet_mask, default_gateway),
        }
    }
}

impl Tickable for Desktop {
    fn tick(&mut self) {
        self.interface.receive();
    }
}
