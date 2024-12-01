use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
};

use raylib::{
    color::Color,
    ffi::{self, GuiControl, GuiControlProperty, GuiIconName, KeyboardKey, MouseButton},
    math::{Rectangle, Vector2},
    prelude::{RaylibDraw, RaylibDrawHandle},
    rgui::RaylibDrawGui,
    rstr, RaylibHandle,
};

use crate::{
    network::{
        device::{cable::CableSimulator, desktop::Desktop},
        ipv4::interface::Ipv4Interface,
    },
    tick::Tickable,
};

use super::{terminal::DesktopTerminal, utils, GuiMode, GuiState};

pub type EntityId = u32;

pub trait Entity: Tickable {
    fn render_entity(&self, d: &mut RaylibDrawHandle);
    fn render_gui(&mut self, d: &mut RaylibDrawHandle, s: &mut GuiState);

    fn handle_gui_click(&mut self, rl: &mut RaylibHandle, s: &mut GuiState) -> bool;

    fn connect(&mut self, port: usize, i: &mut Ipv4Interface);
    fn disconnect(&mut self, port: usize);

    fn dropdown(&mut self, kind: DropdownKind, pos: Vector2, s: &mut GuiState);

    fn is_deleted(&self) -> bool;
    fn delete(&mut self);
    fn id(&self) -> EntityId;
    fn pos(&self) -> Vector2;
    fn set_pos(&mut self, pos: Vector2);
    fn gui_bounds(&self) -> Rectangle;
    fn bounding_box(&self) -> Rectangle;
}

enum ComponentMapping {
    Desktop(usize),
}

pub struct Entities {
    desktops: Vec<DesktopEntity>,
    map: HashMap<EntityId, ComponentMapping>,
    pub adj_list: HashMap<EntityId, Vec<(EntityId, usize)>>, // Id -> (Adj Id, Port)
    seed: EntityId,
    cable_sim: CableSimulator,
}

impl Entities {
    pub fn new() -> Self {
        Self {
            desktops: Vec::new(),
            map: HashMap::new(),
            adj_list: HashMap::new(),
            seed: 0,
            cable_sim: CableSimulator::new(),
        }
    }

    pub fn add_desktop(&mut self, pos: Vector2, label: String) -> EntityId {
        let id = self.seed;
        self.seed += 1;
        self.map
            .insert(id, ComponentMapping::Desktop(self.desktops.len()));
        self.adj_list.insert(id, Vec::new());
        self.desktops.push(DesktopEntity::new(id, pos, label));
        self.cable_sim.add(
            self.desktops
                .last()
                .unwrap()
                .desktop
                .interface
                .ethernet
                .port(),
        );
        id
    }

    pub fn update(&mut self) {
        let mut delete = vec![];
        for (i, desktop) in self.desktops.iter_mut().enumerate() {
            if desktop.is_deleted() {
                delete.push((i, desktop.id));
                continue;
            }
            desktop.tick();
        }

        // Lazy delete
        for i in 0..delete.len() {
            let (e_i, id) = delete[i];

            let mut adj_to_modify = vec![];
            if let Some(adj_list) = self.adj_list.get(&id) {
                for (adj_id, port) in adj_list.iter() {
                    adj_to_modify.push((*adj_id, *port));
                }
            }

            for (adj_id, port) in adj_to_modify {
                if let Some(adj_list) = self.adj_list.get_mut(&adj_id) {
                    match self.map.get(&id).unwrap() {
                        ComponentMapping::Desktop(i) => {
                            self.desktops[*i].disconnect(port); // Disconnect
                            self.cable_sim
                                .remove(self.desktops[*i].desktop.interface.ethernet.port());
                        }
                    }
                    adj_list.retain(|(id_, _)| *id_ != id); // Remove from adj list
                }
            }

            // Delete entity, swap it's place with the last entity
            self.map.remove(&id);
            self.adj_list.remove(&id);
            self.desktops.swap_remove(e_i);

            // If a swap was made, update the swapped entity's index
            if e_i < self.desktops.len() {
                self.map
                    .insert(self.desktops[e_i].id, ComponentMapping::Desktop(e_i));
            }
        }

        self.cable_sim.tick();
    }

    pub fn render(&mut self, d: &mut RaylibDrawHandle, s: &mut GuiState) {
        for e in self.desktops.iter() {
            e.render_entity(d);
        }

        let mut selected_window: Option<&mut DesktopEntity> = None;
        for e in self.desktops.iter_mut() {
            if s.selected_window == Some(e.id) {
                selected_window = Some(e);
                continue;
            }
            e.render_gui(d, s);
        }

        // Selected window is on top
        if let Some(e) = selected_window {
            e.render_gui(d, s);
        }
    }

    pub fn get(&self, id: EntityId) -> &dyn Entity {
        match self.map.get(&id).unwrap() {
            ComponentMapping::Desktop(i) => &self.desktops[*i],
        }
    }

    pub fn get_mut(&mut self, id: EntityId) -> &mut dyn Entity {
        match self.map.get(&id).unwrap() {
            ComponentMapping::Desktop(i) => &mut self.desktops[*i],
        }
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &dyn Entity> {
        self.desktops.iter().map(|e| e as &dyn Entity)
    }

    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut dyn Entity> {
        self.desktops.iter_mut().map(|e| e as &mut dyn Entity)
    }

    pub fn disconnect(&mut self, id: EntityId, port: usize) {
        if self.adj_list.get(&id).is_none() || self.adj_list.get(&id).is_some_and(|l| l.is_empty())
        {
            return;
        }

        // Extract the adjacency list entry for the given id
        let adj_list = self.adj_list.get(&id).unwrap().clone();
        let (adj_entity, adj_id) = {
            let (id, _) = adj_list.iter().find(|(_, p)| *p == port).unwrap();
            (self.map.get(id).unwrap(), id)
        };

        // Disconnect adjancent entity.
        match adj_entity {
            ComponentMapping::Desktop(i) => {
                self.desktops[*i].disconnect(port); // Two-way disconnection
            }
        }

        // Remove the adjacency list entry for the given id
        self.adj_list
            .get_mut(&adj_id)
            .unwrap()
            .retain(|(id_, _)| *id_ != id);
        self.adj_list
            .get_mut(&id)
            .unwrap()
            .retain(|(_, p)| *p != port);
    }

    pub fn connect(&mut self, e1: EntityId, p1: usize, e2: EntityId, p2: usize) {
        self.disconnect(e1, p1);
        self.disconnect(e2, p2);

        let e1_cm = self.map.get(&e1).unwrap();
        let e2_cm = self.map.get(&e2).unwrap();

        match (e1_cm, e2_cm) {
            (ComponentMapping::Desktop(i1), ComponentMapping::Desktop(i2)) => {
                if i1 == i2 {
                    return;
                }

                // compiler gymnastics
                let (left, right) = if i1 < i2 {
                    let (left, right) = self.desktops.split_at_mut(*i2);
                    (&mut left[*i1], &mut right[0])
                } else {
                    let (left, right) = self.desktops.split_at_mut(*i1);
                    (&mut right[0], &mut left[*i2])
                };

                self.adj_list
                    .get_mut(&left.id)
                    .unwrap()
                    .push((right.id, p1));
                self.adj_list
                    .get_mut(&right.id)
                    .unwrap()
                    .push((left.id, p2));

                left.connect(p1, &mut right.desktop.interface);
            }
        }
    }
}

pub enum DropdownKind {
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

pub struct DesktopEntity {
    id: EntityId,
    pos: Vector2,
    desktop: Desktop,
    label: String,

    adj: Option<Rc<RefCell<dyn Entity>>>,

    dropdown_gui: Option<DropdownGuiState>,

    terminal: DesktopTerminal,
    display_gui: DisplayGuiState,

    deleted: bool,
}

impl DesktopEntity {
    fn new(id: EntityId, pos: Vector2, label: String) -> Self {
        let desktop = Desktop::from_seed(id as u64);
        let display_gui =
            DisplayGuiState::new(desktop.interface.ip_address, desktop.interface.subnet_mask);
        Self {
            id,
            pos,
            desktop,
            label,
            adj: None,
            dropdown_gui: None,
            terminal: DesktopTerminal::new(),
            display_gui,
            deleted: false,
        }
    }

    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
    }
}

impl Entity for DesktopEntity {
    fn delete(&mut self) {
        self.deleted = true;
    }

    fn id(&self) -> EntityId {
        self.id
    }

    fn set_pos(&mut self, pos: Vector2) {
        self.pos = pos;
    }

    fn is_deleted(&self) -> bool {
        self.deleted
    }

    fn pos(&self) -> Vector2 {
        self.pos
    }

    fn gui_bounds(&self) -> Rectangle {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
    }

    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
    }

    fn connect(&mut self, _port: usize, i: &mut Ipv4Interface) {
        self.desktop.interface.connect(i);
    }

    fn disconnect(&mut self, _port: usize) {
        self.desktop.interface.disconnect();
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

    fn render_entity(&self, d: &mut RaylibDrawHandle) {
        utils::draw_icon(
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
                Some(utils::rstr_from_string(self.label.clone()).as_c_str()),
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
                match utils::array_to_string(&ds.ip_buffer).parse::<std::net::Ipv4Addr>() {
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
                self.terminal.input(
                    utils::array_to_string(&ds.cmd_line_buffer),
                    &mut self.desktop,
                );
                ds.cmd_line_out.push_back(format!(
                    "Desktop % {}",
                    utils::array_to_string(&ds.cmd_line_buffer)
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
    fn handle_gui_click(&mut self, rl: &mut RaylibHandle, s: &mut GuiState) -> bool {
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
                                s.connect_d1 = Some((0, self.id));
                            } else {
                                s.connect_d2 = Some((0, self.id));
                            }
                        }
                        GuiMode::Remove => {
                            s.remove_d = Some((0, self.id));
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
}

impl Tickable for DesktopEntity {
    fn tick(&mut self) {
        self.terminal.tick(&mut self.desktop);
        self.desktop.tick();
    }
}
