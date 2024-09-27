mod physical {
    pub mod packet_sim;
    pub mod ethernet_port;
}

mod data_link {
    pub mod ethernet_frame;
    pub mod ethernet_interface;
    pub mod arp_frame;
}

mod network {
    pub mod ipv4;
    pub mod network_interface;
}

#[cfg(test)]
mod tests;

fn main() {
    println!("Hello, world!");
}
