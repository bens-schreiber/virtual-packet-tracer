mod device;
mod terminal;
mod utils;

use device::{DropdownKind, Entities, Entity, EntityId};
use raylib::prelude::*;

fn handle_click(s: &mut GuiState, rl: &mut RaylibHandle, es: &mut Entities) {
    let mouse_pos = rl.get_mouse_position();

    if let Some(id) = s.open_dropdown {
        let e = es.get_mut(id);
        if !e.handle_gui_click(rl, s) {
            return;
        }
    }

    if s.open_windows.iter().any(|id| {
        let e = es.get(*id);
        e.gui_bounds().check_collision_point_rec(mouse_pos)
    }) {
        return;
    }

    // todo: don't need to check this every frame, some lazy eval would be nice
    let mouse_collision: Option<&mut dyn Entity> = {
        let mut res = None;
        for e in es.iter_mut().rev() {
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
                    let e = mouse_collision.unwrap();
                    e.dropdown(DropdownKind::Connections, mouse_pos, s);
                    return;
                }
            }

            if s.remove_d.is_none() {
                return;
            }

            let (port, id) = s.remove_d.unwrap();
            es.disconnect(id, port);
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
            es.connect(e1_id, e1_port, e2_id, e2_port);
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
            let e = es.get_mut(s.drag_device);
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
                        es.add_desktop(mouse_pos);
                    }
                    Some(DeviceKind::Switch) => {
                        es.add_switch(mouse_pos);
                    }
                    _ => {
                        todo!()
                    }
                }
                return;
            }

            if left_mouse_down {
                if let Some(e) = mouse_collision {
                    s.mode = GuiMode::Drag;
                    s.drag_prev_mode = GuiMode::Place;
                    rl.gui_lock();
                    s.drag_device = e.id();
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
                    s.drag_device = e.id();
                    s.drag_origin = mouse_pos - e.pos();
                }
                return;
            }
        }
    }
    // ------------------------------------------------------
}

fn draw_connections(d: &mut RaylibDrawHandle, es: &Entities) {
    for e in es.iter() {
        for (adj_id, _) in es.adj_list.get(&e.id()).or(Some(&vec![])).unwrap() {
            let target = es.get(*adj_id).pos();
            d.draw_line_ex(
                Vector2::new(e.pos().x, e.pos().y),
                Vector2::new(target.x, target.y),
                1.5,
                Color::WHITE,
            );
        }
    }
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
            utils::draw_icon(GuiIconName::ICON_CROSS, 225, 410, 3, Color::BLACK);
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
            utils::draw_icon(GuiIconName::ICON_MONITOR, 147, 410, 3, Color::BLACK);
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
            utils::draw_icon(
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
            utils::draw_icon(GuiIconName::ICON_SHUFFLE_FILL, 314, 420, 2, Color::BLACK);
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

    remove_d: Option<(usize, EntityId)>,

    connect_d1: Option<(usize, EntityId)>,
    connect_d2: Option<(usize, EntityId)>,

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

    let mut es = Entities::new();

    let mut last_connected_pos = Vector2::zero();

    let mut s = GuiState::default();

    while !rl.window_should_close() {
        es.update();
        handle_click(&mut s, &mut rl, &mut es);

        let mut d = rl.begin_drawing(&thread);

        draw_connections(&mut d, &es);

        // Draw a line to the mouse if connecting devices
        if s.mode == GuiMode::Connect && s.connect_d1.is_some() && s.connect_d2.is_none() {
            last_connected_pos = if s.open_dropdown.is_some() {
                last_connected_pos
            } else {
                d.get_mouse_position()
            };

            let (_, id) = s.connect_d1.unwrap();
            let e = es.get(id);
            d.draw_line_ex(
                Vector2::new(e.pos().x, e.pos().y),
                Vector2::new(last_connected_pos.x, last_connected_pos.y),
                1.5,
                Color::WHITE,
            );
        }

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
                        Color::WHITE,
                    );
                }
            }
            _ => {}
        }

        es.render(&mut d, &mut s);

        draw_controls_panel(&mut d, &mut s);

        d.clear_background(Color::BLACK);
    }
}
