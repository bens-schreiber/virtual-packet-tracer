mod physical {
    pub mod physical_sim;
    pub mod ethernet_port;
}

mod data_link {
    pub mod ethernet_frame;
    pub mod ethernet_interface;
    pub mod arp_frame;

    pub mod device {
        pub mod switch;
    }
}

mod network {
    pub mod ipv4;
    pub mod ipv4_interface;
}

#[cfg(test)]
mod tests {
    pub mod ethernet_tests;
    pub mod physical_sim_tests;
    pub mod ipv4_interface_tests;
    pub mod switch_tests;
}

fn main() {
    println!("Hello, world!");
}
