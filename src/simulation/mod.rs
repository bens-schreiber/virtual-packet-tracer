pub mod terminal;
pub mod tick;

use std::{
    collections::{HashMap, VecDeque},
    net::Ipv4Addr,
};

use raylib::prelude::*;
use terminal::DesktopTerminal;
use tick::Tickable;

use crate::network::device::{cable::CableSimulator, desktop::Desktop};

/**
 *
 * Bug bounty:
 * - Dragging windows with devices on the screen can sometimes enter a state where the sim is in drag mode
 */

type EntityId = u64;

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

fn rstr_from_string(s: String) -> std::ffi::CString {
    std::ffi::CString::new(s).expect("CString::new failed")
}

fn array_to_string(array: &[u8]) -> String {
    let end = array.iter().position(|&c| c == 0).unwrap_or(array.len());
    let slice = &array[..end];
    String::from_utf8_lossy(slice).to_string()
}

enum DropdownKind {
    Options,
    Connections,
}

struct DropdownGuiState {
    selection: i32,
    pos: Vector2,
    kind: DropdownKind,
    bounds: Rectangle, // Staticly positioned, dynamically popualted; has to call first render to be set
}

struct DisplayGuiState {
    open: bool,
    pos: Vector2,
    drag_origin: Option<Vector2>,

    ip_buffer: [u8; 15],
    ip_edit_mode: bool,

    subnet_buffer: [u8; 15],
    subnet_edit_mode: bool,

    cmd_line_buffer: [u8; 0xFF],
    cmd_line_out: VecDeque<String>,
}

impl DisplayGuiState {
    fn new(ip_address: [u8; 4], subnet_mask: [u8; 4]) -> Self {
        let mut ip_buffer = [0u8; 15];
        let ip_string = format!(
            "{}.{}.{}.{}",
            ip_address[0], ip_address[1], ip_address[2], ip_address[3]
        );
        let ip_bytes = ip_string.as_bytes();
        ip_buffer[..ip_bytes.len()].copy_from_slice(ip_bytes);

        let mut subnet_buffer = [0u8; 15];
        let subnet_string = format!(
            "{}.{}.{}.{}",
            subnet_mask[0], subnet_mask[1], subnet_mask[2], subnet_mask[3]
        );
        let subnet_bytes = subnet_string.as_bytes();
        subnet_buffer[..subnet_bytes.len()].copy_from_slice(subnet_bytes);

        Self {
            open: false,
            pos: Vector2::zero(),
            drag_origin: None,
            ip_buffer,
            ip_edit_mode: false,
            subnet_buffer,
            subnet_edit_mode: false,
            cmd_line_buffer: [0u8; 0xFF],
            cmd_line_out: VecDeque::new(),
        }
    }

    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.pos.x, self.pos.y, 300.0, 200.0)
    }

    pub fn tab_bounds(&self) -> Rectangle {
        Rectangle::new(self.pos.x, self.pos.y, 280.0, 20.0)
    }
}

struct DesktopEntity {
    id: EntityId,
    pos: Vector2,
    desktop: Desktop,
    label: String,
    adj_list: Vec<EntityId>,

    dropdown_gui: Option<DropdownGuiState>,

    terminal: DesktopTerminal,
    display_gui: DisplayGuiState,

    deleted: bool,
}

impl DesktopEntity {
    fn new(id: EntityId, pos: Vector2, label: String) -> Self {
        let desktop = Desktop::from_seed(id);
        let display_gui =
            DisplayGuiState::new(desktop.interface.ip_address, desktop.interface.subnet_mask);
        Self {
            id,
            pos,
            desktop,
            label,
            adj_list: vec![],
            dropdown_gui: None,
            terminal: DesktopTerminal::new(),
            display_gui,
            deleted: false,
        }
    }

    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
    }

    fn render(&self, d: &mut RaylibDrawHandle) {
        draw_icon(
            GuiIconName::ICON_MONITOR,
            self.pos.x as i32 - 25,
            self.pos.y as i32 - 25,
            3,
            Color::WHITE,
        );

        d.draw_text(
            &self.label,
            (self.pos.x - 32.0) as i32,
            (self.pos.y + 25.0) as i32,
            15,
            Color::WHITE,
        );
    }

    fn dropdown(&mut self, kind: DropdownKind, pos: Vector2, s: &mut GuiState) {
        self.dropdown_gui = Some(DropdownGuiState {
            selection: -1,
            pos,
            kind,
            bounds: Rectangle::new(pos.x, pos.y, 75.0, 16.0), // Contains at least one option
        });
        s.open_dropdown = Some(self.id);
    }

    /// Returns true if some poppable state is open
    fn render_gui(&mut self, d: &mut RaylibDrawHandle, s: &mut GuiState) {
        let mut render_display = |ds: &mut DisplayGuiState, d: &mut RaylibDrawHandle| {
            if d.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT)
                && ds
                    .bounds()
                    .check_collision_point_rec(d.get_mouse_position())
            {
                s.selected_window = Some(self.id); // Window engaged
            }

            if s.selected_window == Some(self.id) {
                d.gui_set_state(ffi::GuiState::STATE_FOCUSED);
            }

            if d.gui_window_box(
                ds.bounds(),
                Some(rstr_from_string(self.label.clone()).as_c_str()),
            ) {
                return true;
            }

            if s.selected_window == Some(self.id) {
                d.gui_set_state(ffi::GuiState::STATE_NORMAL);
            }

            // Configure IP
            //----------------------------------------------
            d.gui_label(
                Rectangle::new(ds.pos.x + 10.0, ds.pos.y + 30.0, 100.0, 20.0),
                Some(rstr!("IP Address")),
            );

            if d.gui_text_box(
                Rectangle::new(ds.pos.x + 120.0, ds.pos.y + 30.0, 150.0, 20.0),
                &mut ds.ip_buffer,
                ds.ip_edit_mode,
            ) {
                ds.ip_edit_mode = !ds.ip_edit_mode;
                match array_to_string(&ds.ip_buffer).parse::<Ipv4Addr>() {
                    Ok(ip) => {
                        self.desktop.interface.ip_address = ip.octets();
                    }
                    _ => {}
                }
            }
            //----------------------------------------------

            // Configure Subnet Mask
            //----------------------------------------------
            d.gui_label(
                Rectangle::new(ds.pos.x + 10.0, ds.pos.y + 60.0, 100.0, 20.0),
                Some(rstr!("Subnet Mask")),
            );

            if d.gui_text_box(
                Rectangle::new(ds.pos.x + 120.0, ds.pos.y + 60.0, 150.0, 20.0),
                &mut ds.subnet_buffer,
                ds.subnet_edit_mode,
            ) {
                ds.subnet_edit_mode = !ds.subnet_edit_mode;
            }
            //----------------------------------------------

            // Command Line
            //----------------------------------------------
            d.draw_rectangle_rec(
                Rectangle::new(ds.pos.x + 10.0, ds.pos.y + 90.0, 280.0, 100.0),
                Color::BLACK,
            );

            // Output
            if let Some(out) = self.terminal.out() {
                ds.cmd_line_out.push_back(out);
            }

            let mut y = ds.pos.y + 160.0;
            for line in ds.cmd_line_out.iter().rev() {
                d.draw_text(line, ds.pos.x as i32 + 15, y as i32, 10, Color::WHITE);
                y -= 15.0;
                if y < ds.pos.y + 90.0 {
                    break;
                }
            }

            d.gui_set_style(
                GuiControl::TEXTBOX,
                GuiControlProperty::BORDER_COLOR_NORMAL as i32,
                Color::BLACK.color_to_int(),
            );
            d.gui_set_style(
                GuiControl::TEXTBOX,
                GuiControlProperty::BORDER_COLOR_PRESSED as i32,
                Color::BLACK.color_to_int(),
            );
            d.gui_set_style(
                GuiControl::TEXTBOX,
                GuiControlProperty::BASE_COLOR_PRESSED as i32,
                Color::BLACK.color_to_int(),
            );
            d.gui_set_style(
                GuiControl::TEXTBOX,
                GuiControlProperty::TEXT_COLOR_PRESSED as i32,
                Color::WHITE.color_to_int(),
            );
            if !self.terminal.channel_open {
                d.draw_text(
                    "Desktop %",
                    ds.pos.x as i32 + 15,
                    ds.pos.y.trunc() as i32 + 175,
                    10,
                    Color::WHITE,
                );
            }

            if !self.terminal.channel_open
                && d.gui_text_box(
                    Rectangle::new(ds.pos.x + 71.0, ds.pos.y + 170.0, 210.0, 20.0),
                    &mut ds.cmd_line_buffer,
                    !ds.ip_edit_mode && !ds.subnet_edit_mode && s.selected_window == Some(self.id),
                )
                && d.is_key_pressed(KeyboardKey::KEY_ENTER)
            {
                self.terminal
                    .input(array_to_string(&ds.cmd_line_buffer), &mut self.desktop);
                ds.cmd_line_out.push_back(format!(
                    "Desktop % {}",
                    array_to_string(&ds.cmd_line_buffer)
                ));

                if ds.cmd_line_out.len() > 8 {
                    ds.cmd_line_out.pop_front();
                }
                ds.cmd_line_buffer = [0u8; 0xFF];
            }
            d.gui_load_style_default();
            //----------------------------------------------
            return false;
        };

        if self.dropdown_gui.is_some() {
            let ds = self.dropdown_gui.as_mut().unwrap();
            match ds.kind {
                DropdownKind::Options => {
                    let mut _scroll_index = 0;
                    ds.bounds = Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 65.0);
                    d.gui_list_view(
                        ds.bounds,
                        Some(rstr!("Options;Delete")),
                        &mut _scroll_index,
                        &mut ds.selection,
                    );
                }
                DropdownKind::Connections => {
                    let mut _scroll_index = 0;
                    ds.bounds = Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 32.5);
                    d.gui_list_view(
                        ds.bounds,
                        Some(rstr!("Ethernet0/1")),
                        &mut _scroll_index,
                        &mut ds.selection,
                    );
                }
            }
        }

        if self.display_gui.open {
            if render_display(&mut self.display_gui, d) {
                s.open_windows.retain(|id| *id != self.id);
                self.display_gui.open = false;
                return;
            }

            if self.display_gui.drag_origin.is_some()
                && d.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT)
            {
                self.display_gui.drag_origin = None;
                return;
            }

            if d.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                if self.display_gui.drag_origin.is_none()
                    && self
                        .display_gui
                        .tab_bounds()
                        .check_collision_point_rec(d.get_mouse_position())
                {
                    self.display_gui.drag_origin =
                        Some(d.get_mouse_position() - self.display_gui.pos);
                }

                if self.display_gui.drag_origin.is_some() {
                    self.display_gui.pos =
                        d.get_mouse_position() - self.display_gui.drag_origin.unwrap();
                }
            }
        }
    }

    /// Returns true if the click should be propogated to the sim
    fn handle_dropdown_clicked(&mut self, rl: &mut RaylibHandle, s: &mut GuiState) -> bool {
        let mut handle_options = |ds: &DropdownGuiState, rl: &RaylibHandle| {
            // Handle dropdown clicked
            match ds.selection {
                // Options
                0 => {
                    self.display_gui.open = true;
                    self.display_gui.pos = rl.get_mouse_position();
                    s.open_windows.push(self.id);
                    return true;
                }
                // Delete
                1 => {
                    self.deleted = true;
                    return true;
                }
                _ => {
                    return false;
                }
            }
        };

        let mut handle_connections = |ds: &DropdownGuiState| {
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

                    return true;
                }
                _ => {
                    return false;
                }
            }
        };

        if let Some(ds) = &self.dropdown_gui {
            let close = match ds.kind {
                DropdownKind::Options => handle_options(ds, rl),
                DropdownKind::Connections => handle_connections(ds),
            };

            if close {
                s.open_dropdown = None;
                self.dropdown_gui = None;
                return false;
            }

            if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) {
                let mouse_pos = rl.get_mouse_position();
                if !ds.bounds.check_collision_point_rec(mouse_pos) {
                    s.open_dropdown = None;
                    self.dropdown_gui = None;
                    return true;
                }
            }
        }

        false
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
        self.bounding_box().check_collision_point_rec(point)
    }

    fn tick(&mut self) {
        self.terminal.tick(&mut self.desktop);
        self.desktop.tick();
    }
}

fn handle_click(
    s: &mut GuiState,
    sim: &mut CableSimulator,
    rl: &mut RaylibHandle,
    devices: &mut Vec<DesktopEntity>,
    desktop_count: &mut u64,
    entity_seed: &mut EntityId,
) {
    let mouse_pos = rl.get_mouse_position();

    if s.open_dropdown.is_some() {
        let id = s.open_dropdown.unwrap();
        let (i, _) = get_entity(id, devices);
        if !devices[i].handle_dropdown_clicked(rl, s) {
            return;
        }
    }

    if s.open_windows.iter().any(|id| {
        let (i, _) = get_entity(*id, devices);
        devices[i]
            .display_gui
            .bounds()
            .check_collision_point_rec(mouse_pos)
    }) {
        return;
    }

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
        if let Some((i, _)) = mouse_collision {
            devices[i].dropdown(DropdownKind::Options, mouse_pos, s);
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

        if SELECT_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
            s.mode = GuiMode::Select;
            return;
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
                    let (i, _) = mouse_collision.unwrap();
                    devices[i].dropdown(DropdownKind::Connections, mouse_pos, s);
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

                let (i, _) = mouse_collision.unwrap();
                devices[i].dropdown(DropdownKind::Connections, mouse_pos, s);
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
                s.mode = s.drag_prev_mode;
                rl.gui_unlock();
                return;
            }

            if left_mouse_release {
                s.mode = s.drag_prev_mode;
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
                sim.add(devices.last().unwrap().desktop.interface.ethernet.port());
                *entity_seed += 1;
                *desktop_count += 1;
                return;
            }

            if left_mouse_down {
                if let Some((_, entity)) = mouse_collision {
                    s.mode = GuiMode::Drag;
                    s.drag_prev_mode = GuiMode::Place;
                    rl.gui_lock();
                    s.drag_device = entity.id;
                    s.drag_origin = mouse_pos - entity.pos;
                }
                return;
            }
        }
        GuiMode::Select => {
            if left_mouse_down {
                if let Some((_, entity)) = mouse_collision {
                    s.mode = GuiMode::Drag;
                    s.drag_prev_mode = GuiMode::Select;
                    rl.gui_lock();
                    s.drag_device = entity.id;
                    s.drag_origin = mouse_pos - entity.pos;
                }
                return;
            }
        }
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
            d.draw_line_ex(
                Vector2::new(origin.x, origin.y),
                Vector2::new(target.x, target.y),
                1.5,
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

const SCREEN_BOX: Rectangle = Rectangle::new(0.0, 0.0, 800.0, 500.0);

const GUI_CONTROLS_PANEL: Rectangle = Rectangle::new(0.0, 375.0, 800.0, 125.0);

const ETHERNET_SELBOX: Rectangle = Rectangle::new(137.0, 408.0, 70.0, 70.0);
const DISCONNECT_SELBOX: Rectangle = Rectangle::new(215.0, 408.0, 70.0, 70.0);

const DESKTOP_SELBOX: Rectangle = Rectangle::new(137.0, 408.0, 70.0, 70.0);
const SWITCH_SELBOX: Rectangle = Rectangle::new(215.0, 408.0, 70.0, 70.0);
const ROUTER_SELBOX: Rectangle = Rectangle::new(293.0, 408.0, 70.0, 70.0);

const SELECT_SELBOX: Rectangle = Rectangle::new(
    SCREEN_BOX.width - 45.0,
    GUI_CONTROLS_PANEL.y - 45.0,
    37.0,
    37.0,
);

fn draw_controls_panel(d: &mut RaylibDrawHandle, s: &mut GuiState) {
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

    // Select Mode Button
    //----------------------------------------------
    if s.mode == GuiMode::Select {
        d.gui_set_state(ffi::GuiState::STATE_PRESSED);
    }

    d.draw_rectangle_rec(SELECT_SELBOX, Color::RAYWHITE);
    d.draw_rectangle_lines_ex(
        SELECT_SELBOX,
        1.5,
        border_selection_color(
            d,
            &SELECT_SELBOX,
            s.mode == GuiMode::Select
                || s.mode == GuiMode::Drag && s.drag_prev_mode == GuiMode::Select,
        ),
    );
    draw_icon(
        GuiIconName::ICON_CURSOR_MOVE,
        SELECT_SELBOX.x.trunc() as i32 + 2,
        SELECT_SELBOX.y.trunc() as i32 + 1,
        2,
        Color::BLACK,
    );

    if s.mode == GuiMode::Select {
        d.gui_set_state(ffi::GuiState::STATE_NORMAL);
    }
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
                1.5,
                border_selection_color(d, &ETHERNET_SELBOX, s.mode == GuiMode::Connect),
            );

            // Disconnect
            draw_icon(GuiIconName::ICON_CROSS, 225, 410, 3, Color::BLACK);
            d.draw_text("Remove", 225, 455, 15, Color::BLACK);
            d.draw_rectangle_lines_ex(
                DISCONNECT_SELBOX,
                1.5,
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
                1.5,
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
                1.5,
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
                1.5,
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

#[derive(PartialEq, Clone, Copy)]
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

#[derive(PartialEq, Clone, Copy)]
enum GuiMode {
    Place,
    Drag,
    Remove,
    Connect,
    Select,
}

struct GuiState {
    mode: GuiMode,

    open_dropdown: Option<EntityId>,
    selected_window: Option<EntityId>,
    open_windows: Vec<EntityId>,

    place_type: Option<DeviceKind>,

    drag_prev_mode: GuiMode,
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
            mode: GuiMode::Select,

            open_dropdown: None,
            selected_window: None,
            open_windows: vec![],

            place_type: None,

            drag_prev_mode: GuiMode::Select,
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
        handle_click(
            &mut s,
            &mut cable_sim,
            &mut rl,
            &mut devices,
            &mut desktop_count,
            &mut entity_seed,
        );

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
            cable_sim.remove(devices[*i].desktop.interface.ethernet.port());
            devices.remove(*i);
        }
        deleted_devices.clear();

        let mut d = rl.begin_drawing(&thread);

        draw_connections(&mut d, &devices);

        if s.mode == GuiMode::Connect && s.connect_d1.is_some() && s.connect_d2.is_none() {
            last_connected_pos = if s.open_dropdown.is_some() {
                last_connected_pos
            } else {
                d.get_mouse_position()
            };

            let (_, entity) = get_entity(s.connect_d1.unwrap(), &mut devices);
            d.draw_line_ex(
                Vector2::new(entity.pos.x, entity.pos.y),
                Vector2::new(last_connected_pos.x, last_connected_pos.y),
                1.5,
                Color::WHITE,
            );
        }

        for device in devices.iter() {
            device.render(&mut d);
        }

        match s.mode {
            GuiMode::Place => {
                if let Some(kind) = s.place_type {
                    let icon = match kind {
                        DeviceKind::Desktop => GuiIconName::ICON_MONITOR,
                        DeviceKind::Switch => GuiIconName::ICON_CURSOR_SCALE_FILL,
                        DeviceKind::Router => GuiIconName::ICON_SHUFFLE_FILL,
                    };

                    draw_icon(
                        icon,
                        d.get_mouse_x() + 15,
                        d.get_mouse_y() + 15,
                        1,
                        Color::WHITE,
                    );
                }
            }
            _ => {}
        }

        for device in devices.iter_mut() {
            if Some(device.id) == s.selected_window {
                continue;
            }
            device.render_gui(&mut d, &mut s);
        }

        if let Some(selected_window) = s.selected_window {
            let (i, _) = get_entity(selected_window, &devices);
            devices[i].render_gui(&mut d, &mut s);
        }

        draw_controls_panel(&mut d, &mut s);

        d.clear_background(Color::BLACK);
    }
}
