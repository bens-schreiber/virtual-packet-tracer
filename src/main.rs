#![allow(dead_code)]

mod network {
    pub mod ethernet;
    pub mod ipv4;

    pub mod device {
        pub mod cable;
        pub mod router;
        pub mod switch;
    }
}

mod simulation;

#[cfg(test)]
mod tests {
    mod network {
        pub mod cable_tests;
        pub mod ethernet_tests;
        pub mod ipv4_interface_tests;
        pub mod router_tests;
        pub mod switch_tests;
    }

    mod simulation {
        pub mod tick_tests;
    }
}

fn main() {
    simulation::run();
}
