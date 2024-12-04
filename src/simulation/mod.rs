mod device;
mod terminal;
mod utils;

use std::collections::{HashMap, HashSet};

use device::{
    DesktopDevice, Device, DeviceId, Devices, DropdownKind, PacketEntity, RouterDevice,
    SwitchDevice,
};
use raylib::prelude::*;

use crate::tick::TimeProvider;

const SCREEN_BOX: Rectangle = Rectangle::new(0.0, 0.0, 800.0, 500.0);

const GUI_CONTROLS_PANEL: Rectangle = Rectangle::new(0.0, 380.0, 800.0, 120.0);

const TRACER_MODE_SELBOX: Rectangle = Rectangle::new(SCREEN_BOX.width - 90.0, 10.0, 37.0, 37.0);
const NEXT_TRACER_SELBOX: Rectangle = Rectangle::new(SCREEN_BOX.width - 46.0, 10.0, 37.0, 37.0);

const ETHERNET_SELBOX: Rectangle = Rectangle::new(10.0, 417.0, 70.0, 70.0);
const DISCONNECT_SELBOX: Rectangle = Rectangle::new(90.0, 417.0, 70.0, 70.0);
const DESKTOP_SELBOX: Rectangle = Rectangle::new(170.0, 417.0, 70.0, 70.0);
const SWITCH_SELBOX: Rectangle = Rectangle::new(250.0, 417.0, 70.0, 70.0);
const ROUTER_SELBOX: Rectangle = Rectangle::new(330.0, 417.0, 70.0, 70.0);

const PACKET_TABLE_SELBOX: Rectangle =
    Rectangle::new(420.0, 403.0, SCREEN_BOX.width - 420.0, 100.0);

const SELECT_SELBOX: Rectangle = Rectangle::new(
    SCREEN_BOX.width - 45.0,
    GUI_CONTROLS_PANEL.y - 45.0,
    37.0,
    37.0,
);

#[derive(PartialEq, Clone, Copy)]
enum DeviceKind {
    Desktop,
    Switch,
    Router,
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

    tracer_mode: bool,
    tracer_next: bool,

    open_dropdown: Option<DeviceId>,
    selected_window: Option<DeviceId>,
    open_windows: Vec<DeviceId>,

    place_type: Option<DeviceKind>,

    drag_prev_mode: GuiMode,
    drag_device: Option<DeviceId>,
    drag_origin: Vector2,

    remove_d: Option<(usize, DeviceId)>, // (port, id)

    connect_d1: Option<(usize, DeviceId)>,
    connect_d2: Option<(usize, DeviceId)>,
}

impl Default for GuiState {
    fn default() -> Self {
        Self {
            mode: GuiMode::Select,

            tracer_mode: false,
            tracer_next: false,

            open_dropdown: None,
            selected_window: None,
            open_windows: vec![],

            place_type: None,

            drag_prev_mode: GuiMode::Select,
            drag_device: None,
            drag_origin: Vector2::zero(),

            remove_d: None,

            connect_d1: None,
            connect_d2: None,
        }
    }
}

/// Handles clicks within the simulator and updates the GUI state.
fn handle_click(s: &mut GuiState, rl: &mut RaylibHandle, ds: &mut Devices) {
    let mouse_pos = rl.get_mouse_position();

    // Clicked on a dropdown menu
    if let Some(id) = s.open_dropdown {
        let e = ds.get_mut(id);
        if !e.handle_gui_click(rl, s) {
            return;
        }
    }

    // Clicked on an open window
    if s.open_windows.iter().any(|id| {
        let e = ds.get_mut(*id);
        e.gui_bounds().check_collision_point_rec(mouse_pos)
    }) {
        return;
    }

    // todo: don't need to check this every frame, some lazy eval would be nice
    let mouse_collision: Option<&mut dyn Device> = {
        let mut res = None;
        for e in ds.iter_mut().rev() {
            if e.bounding_box().check_collision_point_rec(mouse_pos) {
                res = Some(e);
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
        if let Some(e) = mouse_collision {
            e.dropdown(DropdownKind::Edit, mouse_pos, s);
        }
        return;
    }

    if left_mouse_clicked {
        if ETHERNET_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
            s.mode = GuiMode::Connect;
            return;
        }
        if DISCONNECT_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
            s.mode = GuiMode::Remove;
            return;
        }
        if DESKTOP_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
            s.mode = GuiMode::Place;
            s.place_type = Some(DeviceKind::Desktop);
            return;
        }
        if SWITCH_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
            s.mode = GuiMode::Place;
            s.place_type = Some(DeviceKind::Switch);
            return;
        }
        if ROUTER_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
            s.mode = GuiMode::Place;
            s.place_type = Some(DeviceKind::Router);
            return;
        }

        if SELECT_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
            s.mode = GuiMode::Select;
            return;
        }

        if TRACER_MODE_SELBOX.check_collision_point_rec(rl.get_mouse_position()) {
            s.tracer_mode = !s.tracer_mode;

            // Freeze or unfreeze time
            if s.tracer_mode {
                TimeProvider::instance().lock().unwrap().freeze();
            } else {
                TimeProvider::instance().lock().unwrap().unfreeze();
            }
            return;
        }

        if NEXT_TRACER_SELBOX.check_collision_point_rec(rl.get_mouse_position()) && s.tracer_mode {
            // Jump forward approximately 1 frame (30 fps -> ~33ms)
            TimeProvider::instance()
                .lock()
                .unwrap()
                .advance(std::time::Duration::from_millis(33));
            s.tracer_next = true;
            return;
        }
    }
    //------------------------------------------------------

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
                    let e = mouse_collision.unwrap();
                    e.dropdown(DropdownKind::Connections, mouse_pos, s);
                    return;
                }
            }

            if s.remove_d.is_none() {
                return;
            }

            let (port, id) = s.remove_d.unwrap();
            ds.disconnect(id, port);
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

                let e = mouse_collision.unwrap();
                e.dropdown(DropdownKind::Connections, mouse_pos, s);
                return;
            }

            if s.connect_d1.is_none() || s.connect_d2.is_none() {
                return;
            }

            let (e1_port, e1_id) = s.connect_d1.unwrap();
            let (e2_port, e2_id) = s.connect_d2.unwrap();
            ds.connect(e1_id, e1_port, e2_id, e2_port);
            s.connect_d1 = None;
            s.connect_d2 = None;
            return;
        }
        GuiMode::Drag => {
            if !s.drag_device.is_some() || GUI_CONTROLS_PANEL.check_collision_point_rec(mouse_pos) {
                s.mode = s.drag_prev_mode;
                rl.gui_unlock();
                return;
            }

            if left_mouse_release {
                s.mode = s.drag_prev_mode;
                rl.gui_unlock();
                return;
            }
            let e = ds.get_mut(s.drag_device.unwrap());
            e.set_pos(mouse_pos - s.drag_origin);
            return;
        }
        GuiMode::Place => {
            if left_mouse_clicked
                && mouse_collision.is_none()
                && !GUI_CONTROLS_PANEL.check_collision_point_rec(mouse_pos)
            {
                match s.place_type {
                    Some(DeviceKind::Desktop) => {
                        ds.add::<DesktopDevice>(mouse_pos);
                    }
                    Some(DeviceKind::Switch) => {
                        ds.add::<SwitchDevice>(mouse_pos);
                    }
                    Some(DeviceKind::Router) => {
                        ds.add::<RouterDevice>(mouse_pos);
                    }
                    None => {}
                }
                return;
            }

            if left_mouse_down {
                if let Some(e) = mouse_collision {
                    s.mode = GuiMode::Drag;
                    s.drag_prev_mode = GuiMode::Place;
                    rl.gui_lock();
                    s.drag_device = Some(e.id());
                    s.drag_origin = mouse_pos - e.pos();
                }
                return;
            }
        }
        GuiMode::Select => {
            if left_mouse_down {
                if let Some(e) = mouse_collision {
                    s.mode = GuiMode::Drag;
                    s.drag_prev_mode = GuiMode::Select;
                    rl.gui_lock();
                    s.drag_device = Some(e.id());
                    s.drag_origin = mouse_pos - e.pos();
                }
                return;
            }
        }
    }
    // ------------------------------------------------------
}

struct Selbox {
    rec: Rectangle,
    text: &'static str,
    icon: GuiIconName,
    fixed: bool,
}

/// The bottom panel GUI controls
fn draw_gui_controls(d: &mut RaylibDrawHandle, s: &mut GuiState) {
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

    // Packet Tracer Controls
    //----------------------------------------------
    d.draw_rectangle(SCREEN_BOX.width as i32 - 92, 8, 85, 41, Color::RAYWHITE);

    utils::draw_icon(
        if s.tracer_mode {
            GuiIconName::ICON_PLAYER_PLAY
        } else {
            GuiIconName::ICON_PLAYER_PAUSE
        },
        SCREEN_BOX.width as i32 - 88,
        12,
        2,
        Color::BLACK,
    );
    d.draw_rectangle_lines_ex(
        TRACER_MODE_SELBOX,
        1.5,
        border_selection_color(d, &TRACER_MODE_SELBOX, s.tracer_mode),
    );

    utils::draw_icon(
        GuiIconName::ICON_PLAYER_NEXT,
        SCREEN_BOX.width as i32 - 43,
        12,
        2,
        Color::BLACK,
    );
    d.draw_rectangle_lines_ex(
        NEXT_TRACER_SELBOX,
        1.5,
        if s.tracer_mode {
            border_selection_color(d, &NEXT_TRACER_SELBOX, false)
        } else {
            border_color
        },
    );
    //----------------------------------------------

    // Panel
    //----------------------------------------------
    d.gui_panel(GUI_CONTROLS_PANEL, Some(rstr!("Controls")));
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
    utils::draw_icon(
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
    let selboxes = [
        Selbox {
            rec: ETHERNET_SELBOX,
            text: "Ethernet",
            icon: GuiIconName::ICON_LINK_NET,
            fixed: s.mode == GuiMode::Connect,
        },
        Selbox {
            rec: DISCONNECT_SELBOX,
            text: "Discon.",
            icon: GuiIconName::ICON_CROSS,
            fixed: s.mode == GuiMode::Remove,
        },
        Selbox {
            rec: DESKTOP_SELBOX,
            text: "Desktop",
            icon: GuiIconName::ICON_MONITOR,
            fixed: s.place_type == Some(DeviceKind::Desktop) && s.mode == GuiMode::Place,
        },
        Selbox {
            rec: SWITCH_SELBOX,
            text: "Switch",
            icon: GuiIconName::ICON_CURSOR_SCALE_FILL,
            fixed: s.place_type == Some(DeviceKind::Switch) && s.mode == GuiMode::Place,
        },
        Selbox {
            rec: ROUTER_SELBOX,
            text: "Router",
            icon: GuiIconName::ICON_SHUFFLE_FILL,
            fixed: s.place_type == Some(DeviceKind::Router) && s.mode == GuiMode::Place,
        },
    ];

    for sb in selboxes.iter() {
        d.draw_rectangle_rec(sb.rec, Color::RAYWHITE);
        d.draw_rectangle_lines_ex(sb.rec, 1.5, border_selection_color(d, &sb.rec, sb.fixed));
        utils::draw_icon(
            sb.icon,
            sb.rec.x as i32 + 10,
            sb.rec.y as i32 + 2,
            3,
            Color::BLACK,
        );

        // center text
        let text_len = sb.text.len() as i32 * 8;
        d.draw_text(
            sb.text,
            sb.rec.x as i32 + (sb.rec.width as i32 - text_len) / 2,
            (sb.rec.y + sb.rec.height - 20.0) as i32,
            15,
            Color::BLACK,
        );
    }

    // Seperator
    let seperator_x = ROUTER_SELBOX.x + ROUTER_SELBOX.width + 10.0;
    d.draw_rectangle_v(
        Vector2::new(seperator_x, ROUTER_SELBOX.y - 12.0),
        Vector2::new(1.0, GUI_CONTROLS_PANEL.height - 20.0),
        border_color,
    );
    //----------------------------------------------

    // Packet Tracer Table
    //----------------------------------------------
    d.draw_rectangle_rec(PACKET_TABLE_SELBOX, Color::GRAY.alpha(0.1));

    // center text in the middle of the box
    if !s.tracer_mode {
        let text = "Packet Tracer Stopped";
        let text_len = text.len() as f32 * 4.5;
        d.draw_text(
            text,
            (PACKET_TABLE_SELBOX.x + (PACKET_TABLE_SELBOX.width / 2.0 - text_len)) as i32,
            (PACKET_TABLE_SELBOX.y + (PACKET_TABLE_SELBOX.height / 2.0 - 10.0)) as i32,
            16,
            Color::BLACK.alpha(0.3),
        );
    }

    //
}

/// Adds the "tracer packets" (visual representation of packets) to the devices list for rendering
fn add_tracer_packets(ds: &mut Devices) {
    let mut packets = vec![];
    for (id, adjs) in ds.adj_devices.iter() {
        let e = ds.get(*id);
        let mut port_id_lookup = HashMap::new();
        let mut ports = vec![];
        for (port, adj_id, _) in adjs {
            port_id_lookup.insert(*port, *adj_id);
            ports.push(*port);
        }

        let traffic = e.traffic(ports);

        for (port, ingress) in traffic {
            let origin = if ingress {
                ds.get(port_id_lookup.get(&port).unwrap().clone()).pos()
            } else {
                e.pos()
            };

            let destination = if ingress {
                e.pos()
            } else {
                ds.get(port_id_lookup.get(&port).unwrap().clone()).pos()
            };

            packets.push(if ingress {
                PacketEntity::egress(origin, destination) // ingress means the prev device is sending to this device
            } else {
                PacketEntity::ingress(origin) // egress means this device is sending to the next device next frame, queue it up
            });
        }
    }

    ds.packets = packets;
}

/// Draws the connections (adjacencies) between devices as well as their port status
fn draw_connections(d: &mut RaylibDrawHandle, ds: &Devices) {
    let mut set: HashSet<DeviceId> = HashSet::new(); // Only need to draw a line once per device
    for (id, adjs) in ds.adj_devices.iter() {
        let e = ds.get(*id);
        for (e_port, adj_id, _) in adjs {
            let target = ds.get(*adj_id);
            let start_pos = Vector2::new(e.pos().x, e.pos().y);
            let end_pos = Vector2::new(target.pos().x, target.pos().y);
            if !set.contains(adj_id) {
                d.draw_line_ex(start_pos, end_pos, 2.5, Color::RAYWHITE);
            }
            set.insert(*id);

            let dir_e = (end_pos - start_pos).normalized();
            d.draw_circle(
                (e.pos().x + dir_e.x * 35.0) as i32,
                (e.pos().y + dir_e.y * 35.0) as i32,
                5.0,
                if e.is_port_up(*e_port) {
                    Color::LIMEGREEN
                } else {
                    Color::RED
                },
            );
        }
    }
}

pub fn run() {
    let (mut rl, thread) = raylib::init()
        .size(800, 500)
        .title("Virtual Packet Tracer")
        .build();

    rl.set_target_fps(30);

    let mut ds = Devices::new();
    let mut s = GuiState::default();

    let mut last_connected_pos = Vector2::zero();

    while !rl.window_should_close() {
        // In tracer mode, after the next button has been clicked, add the tracer packets to the render list
        if s.tracer_mode && s.tracer_next {
            add_tracer_packets(&mut ds);
        }

        // Update all devices by calling their `tick`. In tracer mode, only `tick` after the next button has been clicked
        ds.update(!s.tracer_mode || s.tracer_next);
        s.tracer_next = false;

        handle_click(&mut s, &mut rl, &mut ds);

        let mut d = rl.begin_drawing(&thread);
        draw_connections(&mut d, &ds);

        // Draw a line to the mouse if connecting devices
        if s.mode == GuiMode::Connect && s.connect_d1.is_some() && s.connect_d2.is_none() {
            last_connected_pos = if s.open_dropdown.is_some() {
                last_connected_pos
            } else {
                d.get_mouse_position()
            };

            let (_, id) = s.connect_d1.unwrap();
            let e = ds.get(id);
            d.draw_line_ex(
                Vector2::new(e.pos().x, e.pos().y),
                Vector2::new(last_connected_pos.x, last_connected_pos.y),
                2.5,
                Color::RAYWHITE,
            );
        }

        // Draw the device being placed if in place mode
        match s.mode {
            GuiMode::Place => {
                if let Some(kind) = s.place_type {
                    let icon = match kind {
                        DeviceKind::Desktop => GuiIconName::ICON_MONITOR,
                        DeviceKind::Switch => GuiIconName::ICON_CURSOR_SCALE_FILL,
                        DeviceKind::Router => GuiIconName::ICON_SHUFFLE_FILL,
                    };

                    utils::draw_icon(
                        icon,
                        d.get_mouse_x() + 15,
                        d.get_mouse_y() + 15,
                        1,
                        Color::RAYWHITE,
                    );
                }
            }
            _ => {}
        }

        ds.render(&mut d, &mut s);
        draw_gui_controls(&mut d, &mut s);
        d.clear_background(Color::BLACK);
    }
}
