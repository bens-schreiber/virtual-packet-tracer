pub mod tick;

use std::collections::HashMap;

use raylib::prelude::*;
use tick::Tickable;

use crate::network::device::{cable::CableSimulator, desktop::Desktop};

type EntityId = u64;

struct DropdownGuiState {
    open: bool,
    selection: i32,
    pos: Vector2,
}

impl Default for DropdownGuiState {
    fn default() -> Self {
        Self {
            open: false,
            selection: -1,
            pos: Vector2::zero(),
        }
    }
}

struct DesktopEntity {
    id: EntityId,
    pos: Vector2,
    desktop: Desktop,
    label: String,
    adj_list: Vec<EntityId>,

    options_gui: DropdownGuiState,
    connection_gui: DropdownGuiState,

    deleted: bool,
}

impl DesktopEntity {
    fn new(id: EntityId, pos: Vector2, label: String) -> Self {
        Self {
            id,
            pos,
            desktop: Desktop::from_seed(id),
            label,
            adj_list: vec![],
            options_gui: DropdownGuiState::default(),
            connection_gui: DropdownGuiState::default(),
            deleted: false,
        }
    }

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

    fn open_options(&mut self, pos: Vector2) {
        self.options_gui = DropdownGuiState {
            open: true,
            selection: -1,
            pos,
        }
    }

    fn open_connections(&mut self, pos: Vector2) {
        self.connection_gui = DropdownGuiState {
            open: true,
            selection: -1,
            pos,
        };
    }

    /// Returns true if some poppable state is open
    fn render_gui(&mut self, d: &mut RaylibDrawHandle) {
        let mut render_options = |d: &mut RaylibDrawHandle| {
            let ds = &mut self.options_gui;
            if !ds.open {
                return;
            }

            let mut _scroll_index = 0;

            let out = d.gui_list_view(
                Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 32.5),
                Some(rstr!("Delete")),
                &mut _scroll_index,
                &mut ds.selection,
            );
        };

        let mut render_connections = |d: &mut RaylibDrawHandle| {
            let ds = &mut self.connection_gui;
            if !ds.open {
                return;
            }

            let mut _scroll_index = 0;

            d.gui_list_view(
                Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 32.5),
                Some(rstr!("Ethernet0/1")),
                &mut _scroll_index,
                &mut ds.selection,
            );
        };

        render_options(d);
        render_connections(d);
    }

    fn handle_gui_clicked(&mut self, rl: &mut RaylibHandle, s: &mut GuiState) {
        fn close(ds: &mut DropdownGuiState, s: &mut GuiState) {
            ds.open = false;
            ds.selection = -1;
            s.sim_lock = false;
        }

        fn dismiss(ds: &mut DropdownGuiState, s: &mut GuiState, rl: &RaylibHandle) {
            if ds.selection == -1 && ds.open {
                if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                    let mouse_pos = rl.get_mouse_position();
                    if !Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 32.5)
                        .check_collision_point_rec(mouse_pos)
                    {
                        close(ds, s);
                    }
                }
            }
        }

        let mut handle_options = |rl: &mut RaylibHandle, s: &mut GuiState| {
            let ds = &mut self.options_gui;
            if !ds.open {
                return;
            }

            // Handle dropdown clicked
            match ds.selection {
                // Delete
                0 => {
                    close(ds, s);
                    self.deleted = true;
                }
                _ => {}
            }

            // Dismiss dropdown menu
            dismiss(ds, s, rl);
        };

        let mut handle_connections = |rl: &mut RaylibHandle, s: &mut GuiState| {
            let ds = &mut self.connection_gui;
            if !ds.open {
                return;
            }

            // Handle dropdown clicked
            match ds.selection {
                // Ethernet0/1
                0 => {
                    if s.connect_d1.is_none() {
                        s.connect_d1 = Some(self.id);
                    } else {
                        s.connect_d2 = Some(self.id);
                    }
                    close(ds, s);
                }
                _ => {}
            }

            // Dismiss dropdown menu
            dismiss(ds, s, rl);
        };

        handle_options(rl, s);
        handle_connections(rl, s);
    }

    fn connect(a_i: usize, b_i: usize, devices: &mut Vec<DesktopEntity>) {
        if a_i == b_i {
            return;
        }

        DesktopEntity::disconnect(a_i, devices);
        DesktopEntity::disconnect(b_i, devices);

        // Compiler gymnastics to satisfy borrow checker
        let (left, right) = if a_i < b_i {
            devices.split_at_mut(b_i)
        } else {
            devices.split_at_mut(a_i)
        };

        let (a, b) = if a_i < b_i {
            (&mut left[a_i], &mut right[0])
        } else {
            (&mut right[0], &mut left[b_i])
        };

        a.adj_list.push(b.id);
        b.adj_list.push(a.id);

        a.desktop.interface.connect(&mut b.desktop.interface);
    }

    fn disconnect(i: usize, devices: &mut Vec<DesktopEntity>) -> usize {
        let adj_list = devices[i].adj_list.clone();
        let id = devices[i].id;

        for adj_id in adj_list {
            let (adj_i, _) = get_entity(adj_id, devices);
            devices[adj_i].adj_list.retain(|&id_| id_ != id);
            devices[adj_i].desktop.interface.disconnect();
        }

        devices[i].adj_list.clear();
        devices[i].desktop.interface.disconnect();
        i
    }

    fn collides(&self, point: Vector2) -> bool {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
            .check_collision_point_rec(point)
    }

    fn tick(&mut self) {
        self.desktop.tick();
    }
}

fn handle_sim_clicked(
    s: &mut GuiState,
    rl: &RaylibHandle,
    devices: &mut Vec<DesktopEntity>,
    desktop_count: &mut u64,
    entity_seed: &mut EntityId,
) {
    let mouse_pos = rl.get_mouse_position();

    // todo: don't need to check this every frame, some lazy eval would be nice
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
            if mouse_collision.is_none() && s.connect_d1.is_some() {
                s.connect_mode = false;
                s.connect_d1 = None;
                s.connect_d2 = None;
                return;
            }

            if mouse_collision.is_none() {
                return;
            }

            let (i, entity) = mouse_collision.unwrap();
            s.dropdown_device = Some(entity.id);
            s.sim_lock = true;
            devices[i].open_connections(mouse_pos);
            return;
        }

        if s.connect_d1.is_none() || s.connect_d2.is_none() {
            return;
        }

        let d1 = s.connect_d1.unwrap();
        let d2 = s.connect_d2.unwrap();
        let (d1_i, _) = get_entity(d1, devices);
        let (d2_i, _) = get_entity(d2, devices);
        DesktopEntity::connect(d1_i, d2_i, devices);

        s.connect_mode = false;
        s.connect_d1 = None;
        s.connect_d2 = None;
        return;
    }

    if s.drag_mode {
        if GUI_CONTROLS_PANEL.check_collision_point_rec(mouse_pos) {
            s.drag_mode = false;
            return;
        }

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
        if let Some((i, entity)) = mouse_collision {
            print!("Clicked device {}\n", entity.id);
            s.dropdown_device = Some(entity.id);
            s.sim_lock = true;
            devices[i].open_options(mouse_pos);
        }
        return;
    }

    if left_mouse_clicked {
        // Create a new device if no collision
        if mouse_collision.is_none() && !GUI_CONTROLS_PANEL.check_collision_point_rec(mouse_pos) {
            devices.push(DesktopEntity::new(
                *entity_seed,
                mouse_pos,
                format!("Desktop {}", *desktop_count),
            ));
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
            s.drag_origin = mouse_pos - entity.pos;
        }
    }
}

fn draw_connections(d: &mut RaylibDrawHandle, devices: &Vec<DesktopEntity>) {
    let id_lookup: HashMap<EntityId, usize> = devices
        .iter()
        .enumerate()
        .map(|(i, device)| (device.id, i))
        .collect();

    for device in devices {
        let origin = &device.pos;
        for adj_id in &device.adj_list {
            let target = &devices[id_lookup[adj_id]].pos;
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

const GUI_CONTROLS_PANEL: Rectangle = Rectangle::new(0.0, 400.0, 800.0, 150.0);

fn draw_controls_panel(d: &mut RaylibDrawHandle, s: &mut GuiState) {
    d.gui_panel(GUI_CONTROLS_PANEL, Some(rstr!("Controls")));

    // Connection mode button
    if d.gui_button(
        Rectangle::new(10.0, 430.0, 100.0, 30.0),
        Some(rstr!("Ethernet")),
    ) {
        s.connect_mode = !s.connect_mode;
    }
}

struct GuiState {
    sim_lock: bool,

    dropdown_device: Option<EntityId>,

    drag_mode: bool,
    drag_device: EntityId,
    drag_origin: Vector2,

    connect_mode: bool,
    connect_d1: Option<EntityId>,
    connect_d2: Option<EntityId>,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            sim_lock: false,

            dropdown_device: None,

            drag_mode: false,
            drag_device: 0,
            drag_origin: Vector2::zero(),

            connect_mode: false,
            connect_d1: None,
            connect_d2: None,
        }
    }
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
    let mut deleted_devices: Vec<usize> = vec![];

    let mut last_connected_pos = Vector2::zero();

    let mut s = GuiState::default();

    while !rl.window_should_close() {
        if !s.sim_lock {
            handle_sim_clicked(
                &mut s,
                &rl,
                &mut devices,
                &mut desktop_count,
                &mut entity_seed,
            );
        } else {
            let id = s.dropdown_device.unwrap();
            let (i, _) = get_entity(id, &devices);
            devices[i].handle_gui_clicked(&mut rl, &mut s);
        }

        cable_sim.tick();

        for (i, device) in devices.iter_mut().enumerate() {
            if device.deleted {
                deleted_devices.push(i);
                continue;
            }
            device.tick();
        }

        // Lazy delete devices
        for i in deleted_devices.iter().rev() {
            DesktopEntity::disconnect(*i, &mut devices);
            devices.remove(*i);
        }
        deleted_devices.clear();

        let mut d = rl.begin_drawing(&thread);

        draw_connections(&mut d, &devices);

        if s.connect_mode && s.connect_d1.is_some() && s.connect_d2.is_none() {
            last_connected_pos = if s.sim_lock {
                last_connected_pos
            } else {
                d.get_mouse_position()
            };

            let (_, entity) = get_entity(s.connect_d1.unwrap(), &mut devices);
            d.draw_line(
                entity.pos.x as i32,
                entity.pos.y as i32,
                last_connected_pos.x as i32,
                last_connected_pos.y as i32,
                Color::WHITE,
            );
        }

        for device in devices.iter() {
            device.render(&mut d);
        }

        for device in devices.iter_mut() {
            device.render_gui(&mut d);
        }

        draw_controls_panel(&mut d, &mut s);

        d.clear_background(Color::BLACK);
    }
}
