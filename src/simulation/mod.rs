pub mod tick;

use std::{cell::RefCell, rc::Rc};

use raylib::prelude::*;
use tick::Tickable;

use crate::network::device::{cable::CableSimulator, desktop::Desktop};

struct DesktopEntity {
    pos: Vector2,
    connection: Option<Rc<RefCell<DesktopEntity>>>,
    desktop: Desktop,
}

impl DesktopEntity {
    fn render(&self, d: &mut RaylibDrawHandle) {
        d.draw_rectangle_lines_ex(
            Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0),
            2.0,
            Color::WHITE,
        );

        // Format the MAC address
        let mac_address = self.desktop.interface.ethernet.mac_address;
        let formatted_mac = format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            mac_address[0],
            mac_address[1],
            mac_address[2],
            mac_address[3],
            mac_address[4],
            mac_address[5]
        );

        // Draw the formatted MAC address text under the rectangle
        d.draw_text(
            &formatted_mac,
            (self.pos.x - 25.0) as i32,
            (self.pos.y + 25.0) as i32,
            10,
            Color::WHITE,
        );

        // draw a line to the connected device
        if let Some(connection) = self.connection.as_ref() {
            d.draw_line(
                self.pos.x as i32,
                self.pos.y as i32,
                connection.borrow().pos.x as i32,
                connection.borrow().pos.y as i32,
                Color::WHITE,
            );
        }
    }

    fn collides(&self, point: Vector2) -> bool {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
            .check_collision_point_rec(point)
    }

    fn tick(&mut self) {
        self.desktop.tick();
    }

    fn connect(a: &Rc<RefCell<DesktopEntity>>, b: &Rc<RefCell<DesktopEntity>>) {
        let mut a_mut = a.borrow_mut();
        let mut b_mut = b.borrow_mut();

        a_mut.disconnect();
        b_mut.disconnect();

        a_mut
            .desktop
            .interface
            .connect(&mut b_mut.desktop.interface);

        a_mut.connection = Some(Rc::clone(&b));
        b_mut.connection = Some(Rc::clone(&a));
    }

    fn disconnect(&mut self) {
        if let Some(connection) = self.connection.as_ref() {
            if let Ok(mut v) = connection.try_borrow_mut() {
                v.connection = None;
                v.desktop.interface.disconnect();
            }
            self.connection = None;
            self.desktop.interface.disconnect();
        }
    }
}

fn detect_collision(entities: &Vec<Rc<RefCell<DesktopEntity>>>, point: Vector2) -> Option<usize> {
    for (i, entity) in entities.iter().enumerate() {
        if entity.borrow().collides(point) {
            return Some(i);
        }
    }

    None
}

struct GuiState {
    dropdown_active: i32,
    dropdown_open: bool,
    dropdown_pos: Vector2,
    dropdown_device: usize,

    drag_mode: bool,
    drag_device: usize,

    connect_mode: bool,
    connect_origin_device: usize,

    ignore_next_mouse_down: u8,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            dropdown_active: -1,
            dropdown_open: false,
            dropdown_pos: Vector2::zero(),
            dropdown_device: 0,

            drag_mode: false,
            drag_device: 0,

            connect_mode: false,
            connect_origin_device: 0,

            ignore_next_mouse_down: 0,
        }
    }
}

fn close_dropdown(s: &mut GuiState) {
    s.dropdown_open = false;
    s.dropdown_active = -1;
}

fn handle_click(
    s: &mut GuiState,
    rl: &RaylibHandle,
    devices: &mut Vec<Rc<RefCell<DesktopEntity>>>,
    mac_seed: &mut u64,
) {
    // Dropdown is open, ignore other input
    if s.dropdown_open {
        return;
    }

    let mouse_pos = rl.get_mouse_position();

    let mouse_collision = detect_collision(devices, mouse_pos);
    let right_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT);
    let left_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
    let left_mouse_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
    let left_mouse_release = rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT);

    // Connect mode
    if s.connect_mode {
        if left_mouse_clicked {
            if let Some(collision) = mouse_collision {
                DesktopEntity::connect(&devices[s.connect_origin_device], &devices[collision]);
            }
            s.connect_mode = false;
        }

        return;
    }

    if s.drag_mode {
        if left_mouse_release {
            s.drag_mode = false;
        } else {
            devices[s.drag_device].borrow_mut().pos = mouse_pos;
        }
        return;
    }

    if right_mouse_clicked {
        // Open a dropdown menu for a device if collision
        if let Some(collision) = mouse_collision {
            s.dropdown_open = true;
            s.dropdown_pos = mouse_pos;
            s.dropdown_active = -1;
            s.dropdown_device = collision;
        }
        return;
    }

    if left_mouse_clicked {
        // Create a new device if no collision
        if mouse_collision.is_none() {
            devices.push(Rc::new(RefCell::new(DesktopEntity {
                pos: mouse_pos,
                connection: None,
                desktop: Desktop::from_seed(*mac_seed),
            })));
            *mac_seed += 1;
        }
        return;
    }

    if left_mouse_down {
        // Start dragging a device
        if let Some(collision) = mouse_collision {
            s.drag_mode = true;
            s.drag_device = collision;
            s.dropdown_open = false;
        }
    }
}

pub fn run() {
    let (mut rl, thread) = raylib::init()
        .size(800, 500)
        .title("Virtual Packet Tracer")
        .build();

    rl.set_target_fps(30);

    let mut mac_seed = 1;
    let mut cable_sim = CableSimulator::new();
    let mut devices: Vec<Rc<RefCell<DesktopEntity>>> = vec![];

    let mut s = GuiState::default();

    while !rl.window_should_close() {
        handle_click(&mut s, &rl, &mut devices, &mut mac_seed);

        cable_sim.tick();
        for device in devices.iter_mut() {
            device.borrow_mut().tick();
        }

        let mut d = rl.begin_drawing(&thread);

        for device in devices.iter() {
            device.borrow().render(&mut d);
        }

        if s.dropdown_open {
            let mut _scroll_index = 0;

            d.gui_list_view(
                Rectangle::new(s.dropdown_pos.x, s.dropdown_pos.y, 100.0, 100.0),
                Some(rstr!("Connect;Delete")),
                &mut _scroll_index,
                &mut s.dropdown_active,
            );

            // Handle dropdown clicked
            match s.dropdown_active {
                0 => {
                    s.connect_mode = true;
                    s.connect_origin_device = s.dropdown_device;
                    close_dropdown(&mut s);
                }
                1 => {
                    devices[s.dropdown_device].borrow_mut().disconnect();
                    devices.remove(s.dropdown_device);
                    close_dropdown(&mut s);
                }
                _ => {}
            }

            // Dismiss dropdown menu
            if s.dropdown_active == -1 && s.dropdown_open {
                if d.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                    close_dropdown(&mut s);
                }
            }
        }

        if s.connect_mode {
            let mouse_pos = d.get_mouse_position();
            d.draw_line(
                devices[s.connect_origin_device].borrow().pos.x as i32,
                devices[s.connect_origin_device].borrow().pos.y as i32,
                mouse_pos.x as i32,
                mouse_pos.y as i32,
                Color::WHITE,
            );
        }

        let text = match s.dropdown_active {
            0 => "Connect",
            1 => "Delete",
            _ => "None",
        };
        d.draw_text(text, 10, 10, 20, Color::WHITE);

        d.clear_background(Color::BLACK);
    }
}
