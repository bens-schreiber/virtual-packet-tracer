pub mod tick;

use raylib::prelude::*;

pub fn run() {
    let (mut rl, thread) = raylib::init()
        .size(640, 480)
        .title("Virtual Packet Tracer")
        .build();

    rl.set_target_fps(30);

    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);

        d.clear_background(Color::WHITE);
        d.draw_text("Hello, world!", 12, 12, 20, Color::BLACK);
    }
}
