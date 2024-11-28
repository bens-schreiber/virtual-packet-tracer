pub mod tick;

use std::collections::HashMap;

use raylib::prelude::*;
use tick::Tickable;

use crate::network::device::{cable::CableSimulator, desktop::Desktop};

type EntityId = u64;

// TODO: When connect mode is active, can't exit it if no devices on screen.
// TODO: Can select multiple modes at once, should be mutually exclusive.
// TODO: GUI should lock during drag, connect, remove, etc.

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

            d.gui_list_view(
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
                    match s.mode {
                        GuiMode::Connect => {
                            if s.connect_d1.is_none() {
                                s.connect_d1 = Some(self.id);
                            } else {
                                s.connect_d2 = Some(self.id);
                            }
                        }
                        GuiMode::Remove => {
                            s.remove_d = Some(self.id);
                        }
                        _ => {}
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
    rl: &mut RaylibHandle,
    devices: &mut Vec<DesktopEntity>,
    desktop_count: &mut u64,
    entity_seed: &mut EntityId,
) {
    let mouse_pos = rl.get_mouse_position();

    // todo: don't need to check this every frame, some lazy eval would be nice
    let mouse_collision: Option<(usize, &DesktopEntity)> = {
        let mut res = None;
        for (i, device) in devices.iter().rev().enumerate() {
            if device.collides(mouse_pos) {
                res = Some((devices.len() - 1 - i, device));
                break;
            }
        }
        res
    };

    let right_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT);
    let left_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
    let left_mouse_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
    let left_mouse_release = rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT);

    // GUI Controls
    //------------------------------------------------------
    if right_mouse_clicked {
        // Open a dropdown menu for a device if collision
        if let Some((i, entity)) = mouse_collision {
            s.dropdown_device = Some(entity.id);
            s.sim_lock = true;
            devices[i].open_options(mouse_pos);
        }
        return;
    }

    if left_mouse_clicked {
        match s.menu_selected {
            MenuKind::Connection => {
                if ETHERNET_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
                    s.mode = GuiMode::Connect;
                    return;
                } else if DISCONNECT_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
                    s.mode = GuiMode::Remove;
                    return;
                }
            }
            MenuKind::Device => {
                if DESKTOP_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
                    s.mode = GuiMode::Place;
                    s.place_type = Some(DeviceKind::Desktop);
                    return;
                } else if SWITCH_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
                    s.mode = GuiMode::Place;
                    s.place_type = Some(DeviceKind::Switch);
                    return;
                } else if ROUTER_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
                    s.mode = GuiMode::Place;
                    s.place_type = Some(DeviceKind::Router);
                    return;
                }
            }
        }
    }

    // Sim Controls
    //------------------------------------------------------
    match s.mode {
        GuiMode::Remove => {
            if left_mouse_clicked {
                if mouse_collision.is_none() {
                    s.remove_d = None;
                    return;
                }

                if s.remove_d.is_none() {
                    let (i, entity) = mouse_collision.unwrap();
                    s.dropdown_device = Some(entity.id);
                    s.sim_lock = true;
                    devices[i].open_connections(mouse_pos);
                    return;
                }
            }

            if s.remove_d.is_none() {
                return;
            }

            let id = s.remove_d.unwrap();
            let (i, _) = get_entity(id, devices);
            DesktopEntity::disconnect(i, devices);
            s.remove_d = None;
            return;
        }
        GuiMode::Connect => {
            if left_mouse_clicked {
                if mouse_collision.is_none() {
                    s.connect_d1 = None;
                    s.connect_d2 = None;
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

            s.connect_d1 = None;
            s.connect_d2 = None;
            return;
        }
        GuiMode::Drag => {
            if GUI_CONTROLS_PANEL.check_collision_point_rec(mouse_pos) {
                s.mode = GuiMode::Place;
                rl.gui_unlock();
                return;
            }

            if left_mouse_release {
                s.mode = GuiMode::Place;
                rl.gui_unlock();
                return;
            }
            let (i, _) = get_entity(s.drag_device, devices);
            let entity = &mut devices[i];

            entity.pos = mouse_pos - s.drag_origin;
            return;
        }
        GuiMode::Place => {
            if left_mouse_clicked
                && mouse_collision.is_none()
                && !GUI_CONTROLS_PANEL.check_collision_point_rec(mouse_pos)
            {
                devices.push(DesktopEntity::new(
                    *entity_seed,
                    mouse_pos,
                    format!("Desktop {}", *desktop_count),
                ));
                *entity_seed += 1;
                *desktop_count += 1;
                return;
            }

            if left_mouse_down {
                if let Some((_, entity)) = mouse_collision {
                    s.mode = GuiMode::Drag;
                    rl.gui_lock();
                    s.drag_device = entity.id;
                    s.drag_origin = mouse_pos - entity.pos;
                }
                return;
            }
        }
        GuiMode::Select => {}
    }
    // ------------------------------------------------------
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

const GUI_CONTROLS_PANEL: Rectangle = Rectangle::new(0.0, 375.0, 800.0, 125.0);

const ETHERNET_SELBOX: Rectangle = Rectangle::new(137.0, 408.0, 70.0, 70.0);
const DISCONNECT_SELBOX: Rectangle = Rectangle::new(215.0, 408.0, 70.0, 70.0);

const DESKTOP_SELBOX: Rectangle = Rectangle::new(137.0, 408.0, 70.0, 70.0);
const SWITCH_SELBOX: Rectangle = Rectangle::new(215.0, 408.0, 70.0, 70.0);
const ROUTER_SELBOX: Rectangle = Rectangle::new(293.0, 408.0, 70.0, 70.0);

fn draw_controls_panel(d: &mut RaylibDrawHandle, s: &mut GuiState) {
    fn draw_icon(icon: GuiIconName, pos_x: i32, pos_y: i32, pixel_size: i32, color: Color) {
        unsafe {
            ffi::GuiDrawIcon(
                icon as i32,
                pos_x,
                pos_y,
                pixel_size,
                ffi::Color {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                    a: color.a,
                },
            );
        };
    }

    let border_color = Color::get_color(d.gui_get_style(
        GuiControl::STATUSBAR,
        GuiControlProperty::BORDER_COLOR_DISABLED as i32,
    ) as u32);

    fn border_selection_color(d: &RaylibDrawHandle, rec: &Rectangle, fixed: bool) -> Color {
        let color = Color::get_color(d.gui_get_style(
            GuiControl::STATUSBAR,
            GuiControlProperty::BORDER_COLOR_DISABLED as i32,
        ) as u32);

        if fixed || rec.check_collision_point_rec(d.get_mouse_position()) {
            return Color::get_color(d.gui_get_style(
                GuiControl::BUTTON,
                GuiControlProperty::BORDER_COLOR_PRESSED as i32,
            ) as u32);
        }

        color
    }

    d.gui_panel(GUI_CONTROLS_PANEL, Some(rstr!("Controls")));

    // Connection Type Button
    //----------------------------------------------
    if s.menu_selected == MenuKind::Connection {
        d.gui_set_state(ffi::GuiState::STATE_FOCUSED);
    }

    if d.gui_button(
        Rectangle::new(15.0, 415.0, 100.0, 30.0),
        Some(rstr!("Connection Types")),
    ) {
        s.menu_selected = MenuKind::Connection;
    }

    if s.menu_selected == MenuKind::Connection {
        d.gui_set_state(ffi::GuiState::STATE_NORMAL);
    }
    //------------------------------------------------

    // Device Type Button
    //------------------------------------------------
    if s.menu_selected == MenuKind::Device {
        d.gui_set_state(ffi::GuiState::STATE_FOCUSED);
    }

    if d.gui_button(
        Rectangle::new(15.0, 455.0, 100.0, 30.0),
        Some(rstr!("Device Types")),
    ) {
        s.menu_selected = MenuKind::Device;
    }

    if s.menu_selected == MenuKind::Device {
        d.gui_set_state(ffi::GuiState::STATE_NORMAL);
    }
    //------------------------------------------------

    // Box for devices
    //----------------------------------------------
    d.draw_rectangle_v(
        Vector2::new(130.0, 398.0),
        Vector2::new(1.0, 101.0),
        border_color,
    );

    d.draw_rectangle_v(
        Vector2::new(370.0, 398.0),
        Vector2::new(1.0, 101.0),
        border_color,
    );
    //----------------------------------------------

    // Menu Options
    //----------------------------------------------
    match s.menu_selected {
        MenuKind::Connection => {
            // Ethernet
            d.draw_line_ex(
                Vector2::new(145.0, 450.0),
                Vector2::new(200.0, 420.0),
                2.0,
                Color::BLACK,
            );
            d.draw_text("Ethernet", 140, 455, 15, Color::BLACK);
            d.draw_rectangle_lines_ex(
                ETHERNET_SELBOX,
                1.0,
                border_selection_color(d, &ETHERNET_SELBOX, s.mode == GuiMode::Connect),
            );

            // Disconnect
            draw_icon(GuiIconName::ICON_CROSS, 225, 410, 3, Color::BLACK);
            d.draw_text("Remove", 225, 455, 15, Color::BLACK);
            d.draw_rectangle_lines_ex(
                DISCONNECT_SELBOX,
                1.0,
                border_selection_color(d, &DISCONNECT_SELBOX, s.mode == GuiMode::Remove),
            );
        }
        MenuKind::Device => {
            let mode = s.mode == GuiMode::Place || s.mode == GuiMode::Drag;

            // Desktop (rectangle)
            draw_icon(GuiIconName::ICON_MONITOR, 147, 410, 3, Color::BLACK);
            d.draw_text("Desktop", 143, 455, 15, Color::BLACK);
            d.draw_rectangle_lines_ex(
                DESKTOP_SELBOX,
                1.0,
                border_selection_color(
                    d,
                    &DESKTOP_SELBOX,
                    s.place_type == Some(DeviceKind::Desktop) && mode,
                ),
            );

            // Switch
            d.draw_rectangle_lines_ex(Rectangle::new(230.0, 415.0, 38.0, 38.0), 3.0, Color::BLACK);
            draw_icon(
                GuiIconName::ICON_CURSOR_SCALE_FILL,
                233,
                418,
                2,
                Color::BLACK,
            );
            d.draw_text("Switch", 227, 455, 15, Color::BLACK);
            d.draw_rectangle_lines_ex(
                SWITCH_SELBOX,
                1.0,
                border_selection_color(
                    d,
                    &SWITCH_SELBOX,
                    s.place_type == Some(DeviceKind::Switch) && mode,
                ),
            );

            // Router
            d.draw_circle(328, 435, 21.0, Color::BLACK);
            d.draw_circle(328, 435, 18.5, Color::RAYWHITE);
            draw_icon(GuiIconName::ICON_SHUFFLE_FILL, 314, 420, 2, Color::BLACK);
            d.draw_text("Router", 305, 455, 15, Color::BLACK);
            d.draw_rectangle_lines_ex(
                ROUTER_SELBOX,
                1.0,
                border_selection_color(
                    d,
                    &ROUTER_SELBOX,
                    s.place_type == Some(DeviceKind::Router) && mode,
                ),
            );
        }
    }
    //----------------------------------------------
}

#[derive(PartialEq)]
enum DeviceKind {
    Desktop,
    Switch,
    Router,
}

#[derive(PartialEq)]
enum MenuKind {
    Connection,
    Device,
}

#[derive(PartialEq)]
enum GuiMode {
    Place,
    Drag,
    Remove,
    Connect,
    Select,
}

struct GuiState {
    sim_lock: bool,
    tracer_lock: bool,

    mode: GuiMode,

    dropdown_device: Option<EntityId>,

    place_type: Option<DeviceKind>,

    drag_device: EntityId,
    drag_origin: Vector2,

    remove_d: Option<EntityId>,

    connect_d1: Option<EntityId>,
    connect_d2: Option<EntityId>,

    menu_selected: MenuKind,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            sim_lock: false,
            tracer_lock: false,

            mode: GuiMode::Select,

            dropdown_device: None,

            place_type: None,

            drag_device: 0,
            drag_origin: Vector2::zero(),

            remove_d: None,

            connect_d1: None,
            connect_d2: None,
            menu_selected: MenuKind::Device,
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
                &mut rl,
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

        if s.mode == GuiMode::Connect && s.connect_d1.is_some() && s.connect_d2.is_none() {
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
