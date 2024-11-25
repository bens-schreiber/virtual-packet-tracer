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

mod simulation {
    pub mod tick;
}

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
use raylib::prelude::*;

fn main() {
    let (mut rl, thread) = raylib::init().size(640, 480).title("Hello, World").build();

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);
        d.draw_text("Hello, world!", 12, 12, 20, Color::BLACK);
    }
}
