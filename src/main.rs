#![allow(dead_code)]

mod ethernet;
mod ipv4;

mod device {
    pub mod cable;
    pub mod switch;
}


#[cfg(test)]
mod tests {
    pub mod ethernet_tests;
    pub mod ipv4_interface_tests;
    pub mod cable_tests;
    pub mod switch_tests;
}

fn main() {
    println!("Hello, world!");
}
