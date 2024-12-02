use std::collections::{HashMap, HashSet, VecDeque};

use raylib::{
    color::Color,
    ffi::{self, GuiControl, GuiControlProperty, GuiIconName, KeyboardKey, MouseButton},
    math::{Rectangle, Vector2},
    prelude::{RaylibDraw, RaylibDrawHandle},
    rgui::RaylibDrawGui,
    rstr, RaylibHandle,
};

use crate::{
    network::device::{
        cable::{CableSimulator, EthernetPort},
        desktop::Desktop,
        router::Router,
        switch::Switch,
    },
    tick::Tickable,
};

use super::{
    terminal::{DesktopTerminal, RouterTerminal},
    utils, GuiMode, GuiState,
};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum EntityId {
    Desktop(u32),
    Switch(u32),
    Router(u32),
}

impl EntityId {
    pub fn as_u32(&self) -> u32 {
        match self {
            EntityId::Desktop(id) => *id,
            EntityId::Switch(id) => *id,
            EntityId::Router(id) => *id,
        }
    }
}

pub trait Entity: Tickable {
    fn render_entity(&self, d: &mut RaylibDrawHandle);
    fn render_gui(&mut self, d: &mut RaylibDrawHandle, s: &mut GuiState);

    fn handle_gui_click(&mut self, rl: &mut RaylibHandle, s: &mut GuiState) -> bool;

    fn dropdown(&mut self, kind: DropdownKind, pos: Vector2, s: &mut GuiState);

    fn is_deleted(&self) -> bool;
    fn id(&self) -> EntityId;
    fn pos(&self) -> Vector2;
    fn set_pos(&mut self, pos: Vector2);

    fn gui_bounds(&self) -> Rectangle;
    fn bounding_box(&self) -> Rectangle;
}

pub struct Entities {
    desktops: Vec<DesktopEntity>,
    switches: Vec<SwitchEntity>,
    routers: Vec<RouterEntity>,

    desktop_count: usize, // label purposes
    switch_count: usize,
    router_count: usize,

    pub adj_list: HashMap<EntityId, Vec<(usize, EntityId, usize)>>, // Id -> (Self Port, Adjacent Id, Adjacent Port)

    map: HashMap<EntityId, usize>,
    seed: u32,
    cable_sim: CableSimulator,
}

impl Entities {
    pub fn new() -> Self {
        Self {
            desktops: Vec::new(),
            switches: Vec::new(),
            routers: Vec::new(),
            desktop_count: 0,
            switch_count: 0,
            router_count: 0,
            adj_list: HashMap::new(),
            map: HashMap::new(),
            seed: 1,
            cable_sim: CableSimulator::new(),
        }
    }

    pub fn add_desktop(&mut self, pos: Vector2) -> EntityId {
        let id = EntityId::Desktop(self.seed);
        self.seed += 1;

        self.map.insert(id, self.desktops.len());
        self.adj_list.insert(id, Vec::new());
        self.desktops.push(DesktopEntity::new(
            id,
            pos,
            format!("Desktop {}", self.desktop_count),
        ));
        self.cable_sim.add(
            self.desktops
                .last()
                .unwrap()
                .desktop
                .interface
                .ethernet
                .port(),
        );

        self.desktop_count += 1;
        id
    }

    pub fn add_switch(&mut self, pos: Vector2) -> EntityId {
        let id = EntityId::Switch(self.seed);
        self.seed += 33;

        self.map.insert(id, self.switches.len());
        self.adj_list.insert(id, Vec::new());
        self.switches.push(SwitchEntity::new(
            id,
            pos,
            format!("Switch {}", self.switch_count),
        ));

        self.cable_sim
            .adds(self.switches.last().unwrap().switch.ports());
        self.switch_count += 1;
        id
    }

    pub fn add_router(&mut self, pos: Vector2) -> EntityId {
        let id = EntityId::Router(self.seed);
        self.seed += 9;

        self.map.insert(id, self.routers.len());
        self.adj_list.insert(id, Vec::new());
        self.routers.push(RouterEntity::new(
            id,
            pos,
            format!("Router {}", self.router_count),
        ));

        self.cable_sim
            .adds(self.routers.last().unwrap().router.ports());
        self.router_count += 1;
        id
    }

    pub fn update(&mut self) {
        let mut lazy_delete = vec![];

        for (i, desktop) in self.desktops.iter_mut().enumerate() {
            if desktop.is_deleted() {
                lazy_delete.push((i, desktop.id));
                continue;
            }
            desktop.tick();
        }

        for (i, switch) in self.switches.iter_mut().enumerate() {
            if switch.is_deleted() {
                lazy_delete.push((i, switch.id));
                continue;
            }
            switch.tick();
        }

        for (i, router) in self.routers.iter_mut().enumerate() {
            if router.is_deleted() {
                lazy_delete.push((i, router.id));
                continue;
            }
            router.tick();
        }

        let mut removed: HashSet<EntityId> = HashSet::new();
        for (i, id) in lazy_delete {
            let adj_list = self.adj_list.remove(&id).unwrap();

            // Remove from adj list
            for (_, adj_id, _) in adj_list {
                if removed.contains(&adj_id) {
                    continue;
                }

                self.adj_list
                    .get_mut(&adj_id)
                    .unwrap()
                    .retain(|(_, adj_id, _)| *adj_id != id);
                removed.insert(adj_id);
            }

            removed.clear();

            // Remove from entity list and sim
            match id {
                EntityId::Desktop(_) => {
                    self.cable_sim
                        .remove(self.desktops[i].desktop.interface.ethernet.port());
                    self.desktops.swap_remove(i);

                    if i < self.desktops.len() {
                        self.map.insert(self.desktops[i].id, i);
                    }
                }
                EntityId::Switch(_) => {
                    self.cable_sim.removes(self.switches[i].switch.ports());
                    self.switches.swap_remove(i);

                    if i < self.switches.len() {
                        self.map.insert(self.switches[i].id, i);
                    }
                }
                EntityId::Router(_) => {
                    self.cable_sim.removes(self.routers[i].router.ports());
                    self.routers.swap_remove(i);

                    if i < self.routers.len() {
                        self.map.insert(self.routers[i].id, i);
                    }
                }
            }
        }

        self.cable_sim.tick();
    }

    pub fn render(&mut self, d: &mut RaylibDrawHandle, s: &mut GuiState) {
        for e in self.desktops.iter() {
            e.render_entity(d);
        }

        for e in self.switches.iter() {
            e.render_entity(d);
        }

        for e in self.routers.iter() {
            e.render_entity(d);
        }

        let mut selected_window: Option<&mut dyn Entity> = None;
        for e in self.desktops.iter_mut() {
            if s.selected_window == Some(e.id) {
                selected_window = Some(e);
                continue;
            }
            e.render_gui(d, s);
        }

        for e in self.switches.iter_mut() {
            if s.selected_window == Some(e.id) {
                selected_window = Some(e);
                continue;
            }
            e.render_gui(d, s);
        }

        for e in self.routers.iter_mut() {
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
        let i = *self.map.get(&id).unwrap();
        match id {
            EntityId::Desktop(_) => &self.desktops[i],
            EntityId::Switch(_) => &self.switches[i],
            EntityId::Router(_) => &self.routers[i],
        }
    }

    pub fn get_mut(&mut self, id: EntityId) -> &mut dyn Entity {
        let i = *self.map.get(&id).unwrap();
        match id {
            EntityId::Desktop(_) => &mut self.desktops[i],
            EntityId::Switch(_) => &mut self.switches[i],
            EntityId::Router(_) => &mut self.routers[i],
        }
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &dyn Entity> {
        let res: Vec<&dyn Entity> = self
            .desktops
            .iter()
            .map(|e| e as &dyn Entity)
            .chain(self.switches.iter().map(|e| e as &dyn Entity))
            .chain(self.routers.iter().map(|e| e as &dyn Entity))
            .collect();
        res.into_iter()
    }

    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut dyn Entity> {
        let res: Vec<&mut dyn Entity> = self
            .desktops
            .iter_mut()
            .map(|e| e as &mut dyn Entity)
            .chain(self.switches.iter_mut().map(|e| e as &mut dyn Entity))
            .chain(self.routers.iter_mut().map(|e| e as &mut dyn Entity))
            .collect();
        res.into_iter()
    }

    pub fn disconnect(&mut self, id: EntityId, port: usize) {
        fn _dc(es: &mut Entities, id: EntityId, i: usize, port: usize) {
            match id {
                EntityId::Desktop(_) => {
                    es.desktops[i].desktop.interface.disconnect();
                }
                EntityId::Switch(_) => {
                    es.switches[i].switch.disconnect(port);
                }
                EntityId::Router(_) => {
                    es.routers[i].router.disconnect(port);
                }
            }
        }

        let e1_id = id;
        let e1_adjacency = {
            let adj_list = self.adj_list.get(&e1_id);
            adj_list.and_then(|adj| adj.iter().find(|(p, _, _)| *p == port).cloned())
        };

        if let Some((e1_port, e2_id, e2_port)) = e1_adjacency {
            if let Some(adj) = self.adj_list.get_mut(&e1_id) {
                adj.retain(|(p, _, _)| *p != e1_port);
            }
            if let Some(adj) = self.adj_list.get_mut(&e2_id) {
                adj.retain(|(p, _, _)| *p != e2_port);
            }

            _dc(self, e1_id, *self.map.get(&e1_id).unwrap(), e1_port);
            _dc(self, e2_id, *self.map.get(&e2_id).unwrap(), e2_port);
            return;
        }
    }

    pub fn connect(&mut self, e1: EntityId, p1: usize, e2: EntityId, p2: usize) {
        if e1 == e2 {
            return;
        }

        self.disconnect(e1, p1);
        self.disconnect(e2, p2);

        let e1_i = *self.map.get(&e1).unwrap();
        let e2_i = *self.map.get(&e2).unwrap();

        fn connect_desktop_entity(
            es: &mut Entities,
            e_i: usize,
            other_port: usize,
            other_id: EntityId,
            other_i: usize,
        ) {
            let e = &mut es.desktops[e_i];
            match other_id {
                EntityId::Desktop(_) => {
                    EthernetPort::connect(
                        &mut e.desktop.interface.ethernet.port(),
                        &mut es.desktops[other_i].desktop.interface.ethernet.port(),
                    );
                }
                EntityId::Switch(_) => {
                    es.switches[other_i]
                        .switch
                        .connect(other_port, &mut e.desktop.interface.ethernet);
                }
                EntityId::Router(_) => {
                    es.routers[other_i]
                        .router
                        .connect(other_port, &mut e.desktop.interface);
                }
            }
        }

        fn connect_switch_entity(
            es: &mut Entities,
            e_i: usize,
            port: usize,
            other_port: usize,
            other_id: EntityId,
            other_i: usize,
        ) {
            let e = &mut es.switches[e_i];
            match other_id {
                EntityId::Desktop(_) => {
                    e.switch
                        .connect(port, &mut es.desktops[other_i].desktop.interface.ethernet);
                }
                EntityId::Switch(_) => {
                    EthernetPort::connect(
                        &mut e.switch.ports()[port],
                        &mut es.switches[other_i].switch.ports()[other_port],
                    );
                }
                EntityId::Router(_) => {
                    EthernetPort::connect(
                        &mut e.switch.ports()[port],
                        &mut es.routers[other_i].router.ports()[other_port],
                    );
                }
            }
        }

        fn connect_router_entity(
            es: &mut Entities,
            e_i: usize,
            port: usize,
            other_port: usize,
            other_id: EntityId,
            other_i: usize,
        ) {
            let e = &mut es.routers[e_i];
            match other_id {
                EntityId::Desktop(_) => {
                    e.router
                        .connect(port, &mut es.desktops[other_i].desktop.interface);
                }
                EntityId::Switch(_) => {
                    EthernetPort::connect(
                        &mut e.router.ports()[port],
                        &mut es.switches[other_i].switch.ports()[other_port],
                    );
                }
                EntityId::Router(_) => {
                    EthernetPort::connect(
                        &mut e.router.ports()[port],
                        &mut es.routers[other_i].router.ports()[other_port],
                    );
                }
            }
        }

        match e1 {
            EntityId::Desktop(_) => {
                connect_desktop_entity(self, e1_i, p2, e2, e2_i);
            }
            EntityId::Switch(_) => {
                connect_switch_entity(self, e1_i, p1, p2, e2, e2_i);
            }
            EntityId::Router(_) => {
                connect_router_entity(self, e1_i, p1, p2, e2, e2_i);
            }
        }

        self.adj_list.get_mut(&e1).unwrap().push((p1, e2, p2));
        self.adj_list.get_mut(&e2).unwrap().push((p2, e1, p1));
    }
}

pub enum DropdownKind {
    Edit,
    Connections,
}

struct DropdownGuiState {
    selection: i32,
    pos: Vector2,
    kind: DropdownKind,
    bounds: Rectangle, // Staticly positioned, dynamically popualted; has to call first render to be set
    scroll_index: i32,
}

// Desktop Entity
// ----------------------------------------------

struct DesktopEditGuiState {
    open: bool,
    pos: Vector2,
    drag_origin: Option<Vector2>,

    ip_buffer: [u8; 15],
    ip_edit_mode: bool,

    subnet_buffer: [u8; 15],
    subnet_edit_mode: bool,

    default_gateway_buffer: [u8; 15],
    default_gateway_edit_mode: bool,

    cmd_line_buffer: [u8; 0xFF],
    cmd_line_out: VecDeque<String>,
}

impl DesktopEditGuiState {
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
            default_gateway_buffer: [0u8; 15],
            default_gateway_edit_mode: false,
            cmd_line_buffer: [0u8; 0xFF],
            cmd_line_out: VecDeque::new(),
        }
    }

    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.pos.x, self.pos.y, 300.0, 230.0)
    }

    pub fn tab_bounds(&self) -> Rectangle {
        Rectangle::new(self.pos.x, self.pos.y, 280.0, 20.0)
    }
}

struct DesktopEntity {
    id: EntityId,
    desktop: Desktop,

    pos: Vector2,
    label: String,

    dropdown_gui: Option<DropdownGuiState>,

    terminal: DesktopTerminal,
    edit_gui: DesktopEditGuiState,

    deleted: bool,
}

impl DesktopEntity {
    fn new(id: EntityId, pos: Vector2, label: String) -> Self {
        let desktop = Desktop::from_seed(id.as_u32() as u64);
        let display_gui =
            DesktopEditGuiState::new(desktop.interface.ip_address, desktop.interface.subnet_mask);
        Self {
            id,
            pos,
            desktop,
            label,
            dropdown_gui: None,
            terminal: DesktopTerminal::new(),
            edit_gui: display_gui,
            deleted: false,
        }
    }

    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
    }
}

impl Entity for DesktopEntity {
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
        self.edit_gui.bounds()
    }

    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.pos.x - 25.0, self.pos.y - 25.0, 50.0, 50.0)
    }

    fn dropdown(&mut self, kind: DropdownKind, pos: Vector2, s: &mut GuiState) {
        self.dropdown_gui = Some(DropdownGuiState {
            selection: -1,
            pos,
            kind,
            bounds: Rectangle::new(pos.x, pos.y, 75.0, 16.0), // Contains at least one option
            scroll_index: 0,
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
        let mut render_display = |ds: &mut DesktopEditGuiState, d: &mut RaylibDrawHandle| {
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
                match utils::array_to_string(&ds.ip_buffer).parse::<std::net::Ipv4Addr>() {
                    Ok(ip) => {
                        self.desktop.interface.subnet_mask = ip.octets();
                    }
                    _ => {}
                }
            }
            //----------------------------------------------

            // Configure Default Gateway
            //----------------------------------------------
            d.gui_label(
                Rectangle::new(ds.pos.x + 10.0, ds.pos.y + 90.0, 100.0, 20.0),
                Some(rstr!("Default Gateway")),
            );

            if d.gui_text_box(
                Rectangle::new(ds.pos.x + 120.0, ds.pos.y + 90.0, 150.0, 20.0),
                &mut ds.default_gateway_buffer,
                ds.default_gateway_edit_mode,
            ) {
                ds.default_gateway_edit_mode = !ds.default_gateway_edit_mode;
                match utils::array_to_string(&ds.default_gateway_buffer)
                    .parse::<std::net::Ipv4Addr>()
                {
                    Ok(ip) => {
                        self.desktop.interface.default_gateway = Some(ip.octets());
                    }
                    _ => {}
                }
            }
            //----------------------------------------------

            // Command Line
            //----------------------------------------------
            let terminal_starting_y = ds.pos.y + 115.0;
            d.draw_rectangle_rec(
                Rectangle::new(ds.pos.x + 10.0, terminal_starting_y, 280.0, 105.0),
                Color::BLACK,
            );

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
                GuiControlProperty::BASE_COLOR_PRESSED as i32,
                Color::BLACK.color_to_int(),
            );
            d.gui_set_style(
                GuiControl::TEXTBOX,
                GuiControlProperty::TEXT_COLOR_PRESSED as i32,
                Color::WHITE.color_to_int(),
            );

            let out_starting_y = terminal_starting_y + 8.0;
            let out_y = std::cmp::min(ds.cmd_line_out.len(), 7) as f32 * 11.0 + out_starting_y;
            if !self.terminal.channel_open {
                d.draw_text(
                    "Desktop %",
                    ds.pos.x as i32 + 15,
                    out_y as i32,
                    10,
                    Color::WHITE,
                );
            }

            if !self.terminal.channel_open
                && d.gui_text_box(
                    Rectangle::new(ds.pos.x + 70.0, out_y - 5.0, 210.0, 20.0),
                    &mut ds.cmd_line_buffer,
                    !ds.ip_edit_mode
                        && !ds.subnet_edit_mode
                        && !ds.default_gateway_edit_mode
                        && s.selected_window == Some(self.id),
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

            // Output
            if let Some(out) = self.terminal.out() {
                ds.cmd_line_out.push_back(out);
            }

            let mut out_y = out_starting_y;
            for line in ds.cmd_line_out.iter().rev().take(7).rev() {
                d.draw_text(line, ds.pos.x as i32 + 15, out_y as i32, 10, Color::WHITE);
                out_y += 11.0;
            }
            //----------------------------------------------
            return false;
        };

        if self.dropdown_gui.is_some() {
            let ds = self.dropdown_gui.as_mut().unwrap();
            match ds.kind {
                DropdownKind::Edit => {
                    ds.bounds = Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 65.0);
                    d.gui_list_view(
                        ds.bounds,
                        Some(rstr!("Edit;Delete")),
                        &mut ds.scroll_index,
                        &mut ds.selection,
                    );
                }
                DropdownKind::Connections => {
                    ds.bounds = Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 32.5);
                    d.gui_list_view(
                        ds.bounds,
                        Some(rstr!("Ethernet0/1")),
                        &mut ds.scroll_index,
                        &mut ds.selection,
                    );
                }
            }
        }

        if self.edit_gui.open {
            if render_display(&mut self.edit_gui, d) {
                s.open_windows.retain(|id| *id != self.id);
                self.edit_gui.open = false;
                return;
            }

            if self.edit_gui.drag_origin.is_some()
                && d.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT)
            {
                self.edit_gui.drag_origin = None;
                return;
            }

            if d.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                if self.edit_gui.drag_origin.is_none()
                    && self
                        .edit_gui
                        .tab_bounds()
                        .check_collision_point_rec(d.get_mouse_position())
                {
                    self.edit_gui.drag_origin = Some(d.get_mouse_position() - self.edit_gui.pos);
                }

                if self.edit_gui.drag_origin.is_some() {
                    self.edit_gui.pos = d.get_mouse_position() - self.edit_gui.drag_origin.unwrap();
                }
            }
        }
    }

    /// Returns true if the click should be propogated to the sim
    fn handle_gui_click(&mut self, rl: &mut RaylibHandle, s: &mut GuiState) -> bool {
        let mut handle_edit = |ds: &DropdownGuiState, rl: &RaylibHandle| {
            // Handle dropdown clicked
            match ds.selection {
                // Edit
                0 => {
                    self.edit_gui.open = true;
                    self.edit_gui.pos = rl.get_mouse_position();
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
                DropdownKind::Edit => handle_edit(ds, rl),
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
// ----------------------------------------------

// Switch Entity
// ----------------------------------------------

struct SwitchEntity {
    id: EntityId,
    switch: Switch,
    pos: Vector2,
    label: String,
    deleted: bool,

    dropdown_gui: Option<DropdownGuiState>,
}

impl SwitchEntity {
    fn new(id: EntityId, pos: Vector2, label: String) -> Self {
        let switch = Switch::from_seed(id.as_u32() as u64, 0);
        Self {
            id,
            switch,
            pos,
            label,
            deleted: false,
            dropdown_gui: None,
        }
    }
}

impl Entity for SwitchEntity {
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
        Rectangle::new(self.pos.x, self.pos.y, 0.0, 0.0)
    }

    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.pos.x, self.pos.y, 30.0, 40.0)
    }

    fn dropdown(&mut self, kind: DropdownKind, pos: Vector2, s: &mut GuiState) {
        self.dropdown_gui = Some(DropdownGuiState {
            selection: -1,
            pos,
            kind,
            bounds: Rectangle::new(pos.x, pos.y, 75.0, 16.0), // Contains at least one option
            scroll_index: 0,
        });
        s.open_dropdown = Some(self.id);
    }

    fn render_entity(&self, d: &mut RaylibDrawHandle) {
        d.draw_rectangle_lines_ex(
            Rectangle::new(self.pos.x, self.pos.y, 40.0, 40.0),
            3.0,
            Color::WHITE,
        );
        utils::draw_icon(
            GuiIconName::ICON_CURSOR_SCALE_FILL,
            self.pos.x as i32 + 5,
            self.pos.y as i32 + 5,
            2,
            Color::WHITE,
        );
        d.draw_text(
            &self.label,
            self.pos.x as i32 - 4,
            self.pos.y as i32 + 50,
            15,
            Color::WHITE,
        );
    }

    fn render_gui(&mut self, d: &mut RaylibDrawHandle, _: &mut GuiState) {
        if self.dropdown_gui.is_some() {
            let ds = self.dropdown_gui.as_mut().unwrap();
            match ds.kind {
                DropdownKind::Edit => {
                    ds.bounds = Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 32.5);
                    d.gui_list_view(
                        ds.bounds,
                        Some(rstr!("Delete")),
                        &mut ds.scroll_index,
                        &mut ds.selection,
                    );
                }
                DropdownKind::Connections => {
                    ds.bounds = Rectangle::new(ds.pos.x, ds.pos.y, 100.0, 200.5);

                    let options = self
                        .switch
                        .ports()
                        .iter()
                        .enumerate()
                        .map(|(p, _)| format!("Ethernet0/{p}"))
                        .collect::<Vec<String>>();

                    d.gui_list_view(
                        ds.bounds,
                        Some(utils::rstr_from_string(options.join(";")).as_c_str()),
                        &mut ds.scroll_index,
                        &mut ds.selection,
                    );
                }
            }
        }
    }

    fn handle_gui_click(&mut self, rl: &mut RaylibHandle, s: &mut GuiState) -> bool {
        let mut handle_edit = |ds: &DropdownGuiState| {
            // Handle dropdown clicked
            match ds.selection {
                // Delete
                0 => {
                    self.deleted = true;
                    return true;
                }
                _ => {
                    return false;
                }
            }
        };

        let mut handle_connections = |ds: &DropdownGuiState| {
            if ds.selection == -1 {
                return false;
            }

            let selection = ds.selection as usize;

            match s.mode {
                GuiMode::Connect => {
                    if s.connect_d1.is_none() {
                        s.connect_d1 = Some((selection, self.id));
                    } else {
                        s.connect_d2 = Some((selection, self.id));
                    }
                }
                GuiMode::Remove => {
                    s.remove_d = Some((selection, self.id));
                }
                _ => {}
            }

            return true;
        };

        if let Some(ds) = &self.dropdown_gui {
            let close = match ds.kind {
                DropdownKind::Edit => handle_edit(ds),
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

impl Tickable for SwitchEntity {
    fn tick(&mut self) {
        self.switch.tick();
    }
}
// ----------------------------------------------

// Router Entity
// ----------------------------------------------

struct RouterEditGuiState {
    open: bool,
    pos: Vector2,
    drag_origin: Option<Vector2>,

    cmd_line_buffer: [u8; 0xFF],
    cmd_line_out: VecDeque<String>,
}

impl RouterEditGuiState {
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.pos.x, self.pos.y, 300.0, 160.0)
    }

    pub fn tab_bounds(&self) -> Rectangle {
        Rectangle::new(self.pos.x, self.pos.y, 280.0, 20.0)
    }
}

struct RouterEntity {
    id: EntityId,
    router: Router,
    pos: Vector2,
    label: String,
    deleted: bool,

    dropdown_gui: Option<DropdownGuiState>,
    edit_gui: RouterEditGuiState,
    terminal: RouterTerminal,
}

impl RouterEntity {
    fn new(id: EntityId, pos: Vector2, label: String) -> Self {
        let router = Router::from_seed(id.as_u32() as u64);
        Self {
            id,
            router,
            pos,
            label,
            deleted: false,
            dropdown_gui: None,
            edit_gui: RouterEditGuiState {
                open: false,
                pos: Vector2::zero(),
                drag_origin: None,
                cmd_line_buffer: [0u8; 0xFF],
                cmd_line_out: VecDeque::new(),
            },
            terminal: RouterTerminal::new(),
        }
    }
}

impl Entity for RouterEntity {
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
        self.edit_gui.bounds()
    }

    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(self.pos.x - 20.0, self.pos.y - 20.0, 40.0, 40.0)
    }

    fn dropdown(&mut self, kind: DropdownKind, pos: Vector2, s: &mut GuiState) {
        self.dropdown_gui = Some(DropdownGuiState {
            selection: -1,
            pos,
            kind,
            bounds: Rectangle::new(pos.x, pos.y, 75.0, 16.0), // Contains at least one option
            scroll_index: 0,
        });
        s.open_dropdown = Some(self.id);
    }

    fn render_entity(&self, d: &mut RaylibDrawHandle) {
        d.draw_circle_v(self.pos, 25.0, Color::WHITE);
        d.draw_circle_v(self.pos, 23.0, Color::BLACK);

        utils::draw_icon(
            GuiIconName::ICON_SHUFFLE_FILL,
            self.pos.x as i32 - 15,
            self.pos.y as i32 - 15,
            2,
            Color::WHITE,
        );
        d.draw_text(
            &self.label,
            self.pos.x as i32 - 30,
            self.pos.y as i32 + 30,
            15,
            Color::WHITE,
        );
    }

    fn render_gui(&mut self, d: &mut RaylibDrawHandle, s: &mut GuiState) {
        if self.dropdown_gui.is_some() {
            let ds = self.dropdown_gui.as_mut().unwrap();
            match ds.kind {
                DropdownKind::Edit => {
                    ds.bounds = Rectangle::new(ds.pos.x, ds.pos.y, 75.0, 65.0);
                    d.gui_list_view(
                        ds.bounds,
                        Some(rstr!("Edit;Delete")),
                        &mut ds.scroll_index,
                        &mut ds.selection,
                    );
                }
                DropdownKind::Connections => {
                    ds.bounds = Rectangle::new(ds.pos.x, ds.pos.y, 100.0, 200.5);

                    let options = self
                        .router
                        .ports()
                        .iter()
                        .enumerate()
                        .map(|(p, _)| format!("Gigabit0/{p}"))
                        .collect::<Vec<String>>();

                    d.gui_list_view(
                        ds.bounds,
                        Some(utils::rstr_from_string(options.join(";")).as_c_str()),
                        &mut ds.scroll_index,
                        &mut ds.selection,
                    );
                }
            }
        }

        let mut render_display = |ds: &mut RouterEditGuiState, d: &mut RaylibDrawHandle| {
            // Window
            //----------------------------------------------
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
            //----------------------------------------------

            // Command Line
            //----------------------------------------------
            d.draw_rectangle_rec(
                Rectangle::new(ds.pos.x + 10.0, ds.pos.y + 40.0, 280.0, 105.0),
                Color::BLACK,
            );

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
                GuiControlProperty::BASE_COLOR_PRESSED as i32,
                Color::BLACK.color_to_int(),
            );
            d.gui_set_style(
                GuiControl::TEXTBOX,
                GuiControlProperty::TEXT_COLOR_PRESSED as i32,
                Color::WHITE.color_to_int(),
            );

            let out_y = std::cmp::min(ds.cmd_line_out.len(), 7) as f32 * 11.0 + ds.pos.y + 53.0;
            if !self.terminal.channel_open {
                d.draw_text(
                    "Router %",
                    ds.pos.x as i32 + 15,
                    out_y as i32,
                    10,
                    Color::WHITE,
                );
            }

            if !self.terminal.channel_open
                && d.gui_text_box(
                    Rectangle::new(ds.pos.x + 70.0, out_y - 5.0, 210.0, 20.0),
                    &mut ds.cmd_line_buffer,
                    s.selected_window == Some(self.id),
                )
                && d.is_key_pressed(KeyboardKey::KEY_ENTER)
            {
                self.terminal.input(
                    utils::array_to_string(&ds.cmd_line_buffer),
                    &mut self.router,
                );
                ds.cmd_line_out.push_back(format!(
                    "Router % {}",
                    utils::array_to_string(&ds.cmd_line_buffer)
                ));

                if ds.cmd_line_out.len() > 8 {
                    ds.cmd_line_out.pop_front();
                }
                ds.cmd_line_buffer = [0u8; 0xFF];
            }
            d.gui_load_style_default();

            // Output
            if let Some(out) = self.terminal.out() {
                ds.cmd_line_out.push_back(out);
            }

            let mut out_y = ds.pos.y + 53.0;
            for line in ds.cmd_line_out.iter().rev().take(7).rev() {
                d.draw_text(line, ds.pos.x as i32 + 15, out_y as i32, 10, Color::WHITE);
                if out_y > ds.pos.y + 150.0 {
                    break;
                }
                out_y += 11.0;
            }
            //----------------------------------------------
            return false;
        };

        if self.edit_gui.open {
            if render_display(&mut self.edit_gui, d) {
                s.open_windows.retain(|id| *id != self.id);
                self.edit_gui.open = false;
                return;
            }

            if self.edit_gui.drag_origin.is_some()
                && d.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT)
            {
                self.edit_gui.drag_origin = None;
                return;
            }

            if d.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
                if self.edit_gui.drag_origin.is_none()
                    && self
                        .edit_gui
                        .tab_bounds()
                        .check_collision_point_rec(d.get_mouse_position())
                {
                    self.edit_gui.drag_origin = Some(d.get_mouse_position() - self.edit_gui.pos);
                }

                if self.edit_gui.drag_origin.is_some() {
                    self.edit_gui.pos = d.get_mouse_position() - self.edit_gui.drag_origin.unwrap();
                }
            }
        }
    }

    fn handle_gui_click(&mut self, rl: &mut RaylibHandle, s: &mut GuiState) -> bool {
        let mut handle_edit = |ds: &DropdownGuiState| {
            // Handle dropdown clicked
            match ds.selection {
                // Edit
                0 => {
                    self.edit_gui.open = true;
                    self.edit_gui.pos = rl.get_mouse_position();
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
            if ds.selection == -1 {
                return false;
            }

            let selection = ds.selection as usize;

            match s.mode {
                GuiMode::Connect => {
                    if s.connect_d1.is_none() {
                        s.connect_d1 = Some((selection, self.id));
                    } else {
                        s.connect_d2 = Some((selection, self.id));
                    }
                }
                GuiMode::Remove => {
                    s.remove_d = Some((selection, self.id));
                }
                _ => {}
            }

            return true;
        };

        if let Some(ds) = &self.dropdown_gui {
            let close = match ds.kind {
                DropdownKind::Edit => handle_edit(ds),
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

impl Tickable for RouterEntity {
    fn tick(&mut self) {
        self.router.tick();
    }
}