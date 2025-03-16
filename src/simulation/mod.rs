mod device;
mod gui;

use gui::Gui;
use raylib::prelude::*;

mod utils;

/*
todo:
- make devices deleteable                                                                done
- make connections detachable                                                            done
- add packet tracing for all packets                                                     done
- add packet full detail view                                                            done
- add packet tracer mode
- fix place in buttons bug
- tooltip for buttons
- key shortcut for buttons
- terminal messages dynamic height cap
- dont allow drag in gui
*/

pub fn run() {
    let (mut rl, thread) = raylib::init()
        .resizable()
        .size(1400, 750)
        .title("Virtual Packet Tracer")
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
