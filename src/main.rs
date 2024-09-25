mod physical {
    pub mod packet_sim;
    pub mod port;
}

mod data_link {
    pub mod frame;
    pub mod interface;
}

#[cfg(test)]
mod tests;

fn main() {
    println!("Hello, world!");
}
