pub mod tick;

use std::collections::HashMap;

use rand::random;
use raylib::prelude::*;
use tick::Tickable;

use crate::network::device::{cable::CableSimulator, desktop::Desktop};

type EntityId = u64;

struct DesktopEntity {
    id: EntityId,
    pos: Vector2,
    desktop: Desktop,
    label: String,
}

impl DesktopEntity {
    fn render(&self, d: &mut RaylibDrawHandle) {
        d.draw_rectangle(
            (self.pos.x - 25.0) as i32,
            (self.pos.y - 25.0) as i32,
            50,
            50,
            Color::BLACK,
        );

        d.draw_rectangle_lines_ex(
            Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0),
            2.0,
            Color::WHITE,
        );

        d.draw_text(
            &self.label,
            (self.pos.x - 25.0) as i32,
            (self.pos.y + 30.0) as i32,
            15,
            Color::WHITE,
        );
    }

    fn connect(&mut self, other: &mut DesktopEntity) {
        let i1 = &mut self.desktop.interface;
        let i2 = &mut other.desktop.interface;

        i1.disconnect();
        i2.disconnect();

        i1.connect(i2);
    }

    fn collides(&self, point: Vector2) -> bool {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
            .check_collision_point_rec(point)
    }

    fn tick(&mut self) {
        self.desktop.tick();
    }

    fn disconnect(&mut self) {
        self.desktop.interface.disconnect();
    }
}

struct GuiState {
    dropdown_active: i32,
    dropdown_open: bool,
    dropdown_pos: Vector2,
    dropdown_device: EntityId,

    drag_mode: bool,
    drag_device: EntityId,
    drag_origin: Vector2,

    connect_mode: bool,
    connect_origin_device: EntityId,
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
            drag_origin: Vector2::zero(),

            connect_mode: false,
            connect_origin_device: 0,
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
    devices: &mut Vec<DesktopEntity>,
    desktop_count: &mut u64,
    entity_seed: &mut EntityId,
    adj_list: &mut HashMap<EntityId, Vec<EntityId>>,
) {
    // Dropdown is open, ignore other input
    if s.dropdown_open {
        return;
    }

    let mouse_pos = rl.get_mouse_position();
    let mouse_collision: Option<(usize, &DesktopEntity)> = {
        let mut res = None;
        for (i, device) in devices.iter().enumerate() {
            if device.collides(mouse_pos) {
                res = Some((i, device));
                break;
            }
        }
        res
    };

    let right_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT);
    let left_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
    let left_mouse_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
    let left_mouse_release = rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT);

    // Connect mode
    if s.connect_mode {
        if left_mouse_clicked {
            s.connect_mode = false;

            if let Some((target_i, _target)) = mouse_collision {
                if _target.id == s.connect_origin_device {
                    return;
                }

                let (origin_i, _) = get_entity(s.connect_origin_device, devices);

                // Compiler gymnastics B.S
                let (left, right) = devices.split_at_mut(std::cmp::max(origin_i, target_i));
                let (origin, target) = if origin_i < target_i {
                    (&mut left[origin_i], &mut right[target_i - origin_i - 1])
                } else {
                    (&mut right[origin_i - target_i - 1], &mut left[target_i])
                };

                origin.connect(target);

                // Update adjacency list
                for id in adj_list.get(&origin.id).unwrap_or(&vec![]).clone() {
                    adj_list.remove(&id);
                }

                for id in adj_list.get(&target.id).unwrap_or(&vec![]).clone() {
                    adj_list.remove(&id);
                }

                adj_list.insert(origin.id, vec![target.id]);
                adj_list.insert(target.id, vec![origin.id]);
            }
        }

        return;
    }

    if s.drag_mode {
        if left_mouse_release {
            s.drag_mode = false;
        } else {
            let (i, _) = get_entity(s.drag_device, devices);
            let entity = &mut devices[i];

            entity.pos = mouse_pos - s.drag_origin;
        }
        return;
    }

    if right_mouse_clicked {
        // Open a dropdown menu for a device if collision
        if let Some((_, entity)) = mouse_collision {
            s.dropdown_open = true;
            s.dropdown_pos = mouse_pos;
            s.dropdown_active = -1;
            s.dropdown_device = entity.id;
        }
        return;
    }

    if left_mouse_clicked {
        // Create a new device if no collision
        if mouse_collision.is_none() {
            devices.push(DesktopEntity {
                id: *entity_seed,
                pos: mouse_pos,
                desktop: Desktop::from_seed(*entity_seed),
                label: format!("Desktop {}", desktop_count),
            });
            *entity_seed += 1;
            *desktop_count += 1;
        }
        return;
    }

    if left_mouse_down {
        // Start dragging a device
        if let Some((_, entity)) = mouse_collision {
            s.drag_mode = true;
            s.drag_device = entity.id;
            s.dropdown_open = false;
            s.drag_origin = mouse_pos - entity.pos;
        }
    }
}

fn handle_dropdown(
    d: &mut RaylibDrawHandle,
    s: &mut GuiState,
    devices: &mut Vec<DesktopEntity>,
    adj_list: &mut HashMap<EntityId, Vec<EntityId>>,
) {
    if s.dropdown_open {
        let mut _scroll_index = 0;

        d.gui_list_view(
            Rectangle::new(s.dropdown_pos.x, s.dropdown_pos.y, 75.0, 65.0),
            Some(rstr!("Connect;Delete")),
            &mut _scroll_index,
            &mut s.dropdown_active,
        );

        // Handle dropdown clicked
        match s.dropdown_active {
            // Connect
            0 => {
                s.connect_mode = true;
                s.connect_origin_device = s.dropdown_device;
                close_dropdown(s);
            }

            // Delete
            1 => {
                let (i, _) = get_entity(s.dropdown_device, devices);
                let device = &mut devices[i];

                // Remove from network
                device.disconnect();

                // Remove from adjacency list
                for i in adj_list.remove(&s.dropdown_device).unwrap_or_default() {
                    adj_list.remove(&i);
                }
                adj_list.remove(&s.dropdown_device);
                devices.remove(i);
                close_dropdown(s);
            }
            _ => {}
        }

        // Dismiss dropdown menu
        if s.dropdown_active == -1 && s.dropdown_open {
            if d.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                close_dropdown(s);
            }
        }
    }
}

fn draw_connections(
    d: &mut RaylibDrawHandle,
    devices: &Vec<DesktopEntity>,
    adj_list: &HashMap<EntityId, Vec<EntityId>>,
) {
    let id_to_index: HashMap<EntityId, usize> = devices
        .iter()
        .enumerate()
        .map(|(i, device)| (device.id, i))
        .collect();

    for (id, connections) in adj_list.iter() {
        let origin = devices[id_to_index[&id]].pos;
        for connection in connections {
            let target = devices[id_to_index[connection]].pos;
            d.draw_line(
                origin.x as i32,
                origin.y as i32,
                target.x as i32,
                target.y as i32,
                Color::WHITE,
            );
        }
    }
}

/// devices is effectively a sorted list (by id, increasing), so we can bin search; O(log n) lookup
///
/// Returns a reference to the entity and its index in the devices list
fn get_entity(id: EntityId, devices: &Vec<DesktopEntity>) -> (usize, &DesktopEntity) {
    let res = devices
        .binary_search_by(|device| device.id.cmp(&id))
        .expect("Device not found");

    (res, &devices[res])
}

pub fn run() {
    let (mut rl, thread) = raylib::init()
        .size(800, 500)
        .title("Virtual Packet Tracer")
        .build();

    rl.set_target_fps(30);

    let mut entity_seed: EntityId = 0;
    let mut desktop_count: u64 = 0;

    let mut cable_sim = CableSimulator::new();
    let mut devices: Vec<DesktopEntity> = vec![];

    let mut adj_list: HashMap<EntityId, Vec<EntityId>> = HashMap::new();

    let mut s = GuiState::default();

    while !rl.window_should_close() {
        handle_click(
            &mut s,
            &rl,
            &mut devices,
            &mut desktop_count,
            &mut entity_seed,
            &mut adj_list,
        );

        cable_sim.tick();
        for device in devices.iter_mut() {
            device.tick();
        }

        let mut d = rl.begin_drawing(&thread);

        draw_connections(&mut d, &devices, &adj_list);

        if s.connect_mode {
            let mouse_pos = d.get_mouse_position();
            let (_, entity) = get_entity(s.connect_origin_device, &mut devices);
            d.draw_line(
                entity.pos.x as i32,
                entity.pos.y as i32,
                mouse_pos.x as i32,
                mouse_pos.y as i32,
                Color::WHITE,
            );
        }

        for device in devices.iter() {
            device.render(&mut d);
        }

        handle_dropdown(&mut d, &mut s, &mut devices, &mut adj_list);

        d.clear_background(Color::BLACK);
    }
}
