mod device;
mod gui;

use gui::Gui;
use raylib::prelude::*;

mod utils;

pub fn run() {
    let (mut rl, thread) = raylib::init()
        .size(1400, 750)
        .title("Virtual Packet Tracer")
        .undecorated()
        .build();

    rl.set_target_fps(30);

    let mut gui = Gui::default();
    let mut dr = device::DeviceRepository::default();

    while !rl.window_should_close() {
        if !gui.tracer_enabled {
            dr.update();
        } else if gui.tracer_next {
            dr.update();
            gui.tracer_next = false;
        }
        gui.update(&rl, &mut dr);

        let mut d = rl.begin_drawing(&thread);
        dr.render(&mut d);
        gui.render(&mut d, &mut dr);
        d.clear_background(Color::BLACK);
    }
}
