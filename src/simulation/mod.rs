mod device;
mod gui;

use gui::Gui;
use raylib::prelude::*;

mod utils;

pub fn run() {
    let (mut rl, thread) = raylib::init()
        .resizable()
        .size(1400, 750)
        .title("Virtual Packet Tracer")
        .build();

    rl.set_target_fps(30);

    let mut gui = Gui::default();
    let mut devices = device::DeviceRepository::default();

    while !rl.window_should_close() {
        gui.update(&rl, &mut devices);
        devices.update(&rl);

        let mut d = rl.begin_drawing(&thread);
        gui.render(&mut d);
        devices.render(&mut d);
        d.clear_background(Color::BLACK);
    }
}
