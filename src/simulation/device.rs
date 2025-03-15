use std::collections::{HashMap, HashSet, VecDeque};

use raylib::prelude::*;

use crate::{
    network::{
        device::{
            cable::{CableSimulator, EthernetPort},
            desktop::Desktop,
            router::Router,
            switch::Switch,
        },
        ethernet::ByteSerializable,
        ipv4::{IcmpFrame, IcmpType},
    },
    simulation::utils,
    tick::{TickTimer, Tickable},
};

const ROUTER_DISPLAY_RADIUS: f32 = 25.0;
const SWITCH_DISPLAY_LENGTH: i32 = 45;
const DESKTOP_DISPLAY_SIZE: i32 = SWITCH_DISPLAY_LENGTH; // roughly

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceId {
    Router(usize),
    Switch(usize),
    Desktop(usize),
}

impl DeviceId {
    pub fn as_usize(&self) -> usize {
        match self {
            DeviceId::Router(i) => *i,
            DeviceId::Switch(i) => *i,
            DeviceId::Desktop(i) => *i,
        }
    }
}

pub enum DeviceKind {
    Desktop,
    Switch,
    Router,
}

pub enum DeviceGetQuery {
    Pos(Vector2),
    Id(DeviceId),
}

pub enum DeviceSetQuery {
    Pos(Vector2),
    Connect(DeviceId, usize, usize), // Adj Device, Self Port, Adj Port
    Disconnect(usize),
    TerminalInput(String),
}

#[derive(Clone)]
pub struct DeviceAttributes {
    pub id: DeviceId,
    pub label: String,
    pub pos: Vector2,
    pub ports_len: usize,
}

struct Components<T> {
    attributes: DeviceAttributes,
    terminal: Terminal<T>,
    device: T,
}

impl<T> Components<T> {
    fn new(
        id: DeviceId,
        terminal: Terminal<T>,
        label: String,
        pos: Vector2,
        device: T,
        ports_len: usize,
    ) -> Self {
        Self {
            attributes: DeviceAttributes {
                id,
                label,
                pos,
                ports_len,
            },
            terminal,
            device,
        }
    }

    fn input(&mut self, input: &str) {
        self.terminal.execute(&mut self.device, input);
    }
}

impl Components<Desktop> {
    fn tick(&mut self) {
        self.terminal.tick(&mut self.device);
        self.device.tick();
    }

    fn attributes(&self) -> DeviceAttributes {
        DeviceAttributes {
            pos: Vector2::new(
                self.attributes.pos.x + (DESKTOP_DISPLAY_SIZE / 2) as f32,
                self.attributes.pos.y + (DESKTOP_DISPLAY_SIZE / 2) as f32,
            ),
            ..self.attributes.clone()
        }
    }
}

impl Components<Switch> {
    fn attributes(&self) -> DeviceAttributes {
        DeviceAttributes {
            pos: Vector2::new(
                self.attributes.pos.x + (SWITCH_DISPLAY_LENGTH / 2) as f32,
                self.attributes.pos.y + (SWITCH_DISPLAY_LENGTH / 2) as f32,
            ),
            ..self.attributes.clone()
        }
    }
}

pub struct DeviceRepository {
    routers: Vec<Components<Router>>,
    switches: Vec<Components<Switch>>,
    desktops: Vec<Components<Desktop>>,
    label_seeds: (i32, i32, i32), // (router, switch, desktop)

    adj_devices: HashMap<DeviceId, Vec<(usize, DeviceId, usize)>>, // Id -> (Self Port, Adjacent Id, Adjacent Port)

    cable_simulator: CableSimulator,
    mac_seed: u64,
}

impl Default for DeviceRepository {
    fn default() -> Self {
        Self {
            routers: Vec::new(),
            switches: Vec::new(),
            desktops: Vec::new(),
            adj_devices: HashMap::new(),
            cable_simulator: CableSimulator::default(),
            mac_seed: 0,
            label_seeds: (0, 0, 0),
        }
    }
}

impl DeviceRepository {
    pub fn add(&mut self, kind: DeviceKind, pos: Vector2) {
        self.mac_seed += 1;
        match kind {
            DeviceKind::Router => {
                let label: i32 = {
                    self.label_seeds.0 += 1;
                    self.label_seeds.0
                };
                let component = Components::new(
                    DeviceId::Router(self.routers.len()),
                    Terminal::new_router(),
                    format!("Router {}", label),
                    pos,
                    Router::from_seed(self.mac_seed),
                    8,
                );

                self.cable_simulator.adds(component.device.ports());
                self.mac_seed += component.attributes.ports_len as u64;
                self.routers.push(component);
            }
            DeviceKind::Switch => {
                let label: i32 = {
                    self.label_seeds.1 += 1;
                    self.label_seeds.1
                };
                let component = Components::new(
                    DeviceId::Switch(self.switches.len()),
                    Terminal::new_switch(),
                    format!("Switch {}", label),
                    pos,
                    Switch::from_seed(self.mac_seed, label as u16),
                    32,
                );

                self.cable_simulator.adds(component.device.ports());
                self.mac_seed += component.attributes.ports_len as u64;
                self.switches.push(component);
            }
            DeviceKind::Desktop => {
                let label: i32 = {
                    self.label_seeds.2 += 1;
                    self.label_seeds.2
                };
                let component = Components::new(
                    DeviceId::Desktop(self.desktops.len()),
                    Terminal::new_desktop(),
                    format!("Desktop {}", label),
                    pos,
                    Desktop::from_seed(self.mac_seed),
                    1,
                );

                self.cable_simulator
                    .add(component.device.interface.ethernet.port());
                self.mac_seed += component.attributes.ports_len as u64;
                self.desktops.push(component);
            }
        }
    }

    pub fn get(&self, query: DeviceGetQuery) -> Option<DeviceAttributes> {
        match query {
            // Linear search, no point in optimizing this for now.
            DeviceGetQuery::Pos(pos) => {
                for component in self.routers.iter() {
                    if component.attributes.pos.distance_to(pos) < ROUTER_DISPLAY_RADIUS {
                        return Some(component.attributes.clone());
                    }
                }

                for component in self.switches.iter() {
                    let rec = Rectangle {
                        x: component.attributes.pos.x,
                        y: component.attributes.pos.y,
                        width: SWITCH_DISPLAY_LENGTH as f32,
                        height: SWITCH_DISPLAY_LENGTH as f32,
                    };
                    if rec.check_collision_point_rec(pos) {
                        return Some(component.attributes());
                    }
                }

                for component in self.desktops.iter() {
                    let rec = Rectangle {
                        x: component.attributes.pos.x,
                        y: component.attributes.pos.y,
                        width: DESKTOP_DISPLAY_SIZE as f32,
                        height: DESKTOP_DISPLAY_SIZE as f32,
                    };
                    if rec.check_collision_point_rec(pos) {
                        return Some(component.attributes());
                    }
                }

                None
            }
            DeviceGetQuery::Id(id) => match id {
                DeviceId::Router(i) => Some(self.routers[i].attributes.clone()),
                DeviceId::Switch(i) => Some(self.switches[i].attributes()),
                DeviceId::Desktop(i) => Some(self.desktops[i].attributes()),
            },
        }
    }

    pub fn get_terminal(&mut self, id: DeviceId) -> Vec<String> {
        match id {
            DeviceId::Router(i) => self.switches[i].terminal.out_buf.drain(..).collect(),
            DeviceId::Switch(i) => self.switches[i].terminal.out_buf.drain(..).collect(),
            DeviceId::Desktop(i) => self.desktops[i].terminal.out_buf.drain(..).collect(),
        }
    }

    pub fn set(&mut self, id: DeviceId, query: DeviceSetQuery) {
        match query {
            DeviceSetQuery::Pos(pos) => match id {
                DeviceId::Router(i) => self.routers[i].attributes.pos = pos,
                DeviceId::Switch(i) => self.switches[i].attributes.pos = pos,
                DeviceId::Desktop(i) => self.desktops[i].attributes.pos = pos,
            },
            DeviceSetQuery::Connect(adj_id, self_port, adj_port) => {
                self.connect(id, self_port, adj_id, adj_port);
            }
            DeviceSetQuery::Disconnect(port) => {
                self.disconnect(id, port);
            }
            DeviceSetQuery::TerminalInput(input) => match id {
                DeviceId::Router(i) => {
                    self.routers[i].input(&input);
                }
                DeviceId::Switch(i) => {
                    self.switches[i].input(&input);
                }
                DeviceId::Desktop(i) => {
                    self.desktops[i].input(&input);
                }
            },
        }
    }

    fn connect(&mut self, d1: DeviceId, p1: usize, d2: DeviceId, p2: usize) {
        if d1 == d2 {
            return;
        }

        self.disconnect(d1, p1);
        self.disconnect(d2, p2);

        let (d1_i, d2_i) = (d1.as_usize(), d2.as_usize());

        fn connect_desktop(
            dr: &mut DeviceRepository,
            d_i: usize,
            other_port: usize,
            other_id: DeviceId,
            other_i: usize,
        ) {
            let component = &mut dr.desktops[d_i];
            match other_id {
                DeviceId::Desktop(_) => {
                    EthernetPort::connect(
                        &mut component.device.interface.ethernet.port(),
                        &mut dr.desktops[other_i].device.interface.ethernet.port(),
                    );
                }
                DeviceId::Switch(_) => {
                    dr.switches[other_i]
                        .device
                        .connect(other_port, &mut component.device.interface.ethernet);
                }
                DeviceId::Router(_) => {
                    dr.routers[other_i]
                        .device
                        .connect(other_port, &mut component.device.interface);
                }
            }
        }

        fn connect_switch(
            dr: &mut DeviceRepository,
            d_i: usize,
            port: usize,
            other_port: usize,
            other_id: DeviceId,
            other_i: usize,
        ) {
            let component = &mut dr.switches[d_i];
            match other_id {
                DeviceId::Desktop(_) => {
                    component
                        .device
                        .connect(port, &mut dr.desktops[other_i].device.interface.ethernet);
                }
                DeviceId::Switch(_) => {
                    // have to call connect on the switch device so the switch hello bpdu is sent.
                    // compiler gymnastics ensue...
                    let (component, other_component) = if d_i < other_i {
                        let (left, right) = dr.switches.split_at_mut(other_i);
                        (&mut left[d_i], &mut right[0])
                    } else {
                        let (left, right) = dr.switches.split_at_mut(d_i);
                        (&mut right[0], &mut left[other_i])
                    };

                    component
                        .device
                        .connect_switch(port, &mut other_component.device, other_port);
                }
                DeviceId::Router(_) => {
                    EthernetPort::connect(
                        &mut component.device.ports()[port],
                        &mut dr.routers[other_i].device.ports()[other_port],
                    );
                }
            }
        }

        fn connect_router(
            dr: &mut DeviceRepository,
            d_i: usize,
            port: usize,
            other_port: usize,
            other_id: DeviceId,
            other_i: usize,
        ) {
            let component = &mut dr.routers[d_i];
            match other_id {
                DeviceId::Desktop(_) => {
                    component
                        .device
                        .connect(port, &mut dr.desktops[other_i].device.interface);
                }
                DeviceId::Switch(_) => {
                    EthernetPort::connect(
                        &mut component.device.ports()[port],
                        &mut dr.switches[other_i].device.ports()[other_port],
                    );
                }
                DeviceId::Router(_) => {
                    EthernetPort::connect(
                        &mut component.device.ports()[port],
                        &mut dr.routers[other_i].device.ports()[other_port],
                    );
                }
            }
        }

        match d1 {
            DeviceId::Desktop(_) => {
                connect_desktop(self, d1_i, p2, d2, d2_i);
            }
            DeviceId::Switch(_) => {
                connect_switch(self, d1_i, p1, p2, d2, d2_i);
            }
            DeviceId::Router(_) => {
                connect_router(self, d1_i, p1, p2, d2, d2_i);
            }
        }

        self.adj_devices
            .entry(d1)
            .or_insert(Vec::new())
            .push((p1, d2, p2));
        self.adj_devices
            .entry(d2)
            .or_insert(Vec::new())
            .push((p2, d1, p1));
    }

    fn disconnect(&mut self, id: DeviceId, port: usize) {
        fn _dc(dr: &mut DeviceRepository, id: DeviceId, i: usize, port: usize) {
            match id {
                DeviceId::Desktop(_) => {
                    dr.desktops[i].device.interface.disconnect();
                }
                DeviceId::Switch(_) => {
                    dr.switches[i].device.disconnect(port);
                }
                DeviceId::Router(_) => {
                    dr.routers[i].device.disconnect(port);
                }
            }
        }

        let d1_id = id;
        let d1_adjacency = {
            let adj_list = self.adj_devices.get(&d1_id);
            adj_list.and_then(|adj| adj.iter().find(|(p, _, _)| *p == port).cloned())
        };

        if let Some((d1_port, d2_id, d2_port)) = d1_adjacency {
            if let Some(adj) = self.adj_devices.get_mut(&d1_id) {
                adj.retain(|(p, _, _)| *p != d1_port);
            }
            if let Some(adj) = self.adj_devices.get_mut(&d2_id) {
                adj.retain(|(p, _, _)| *p != d2_port);
            }

            _dc(self, d1_id, d1_id.as_usize(), d1_port);
            _dc(self, d2_id, d2_id.as_usize(), d2_port);
            return;
        }
    }

    pub fn render(&mut self, d: &mut RaylibDrawHandle) {
        const FONT_SIZE: i32 = 10;
        const PADDING: i32 = 10;

        // Draw ethernet (adjacencies)
        let mut set: HashSet<DeviceId> = HashSet::new(); // Only need to draw a line once per device
        for (id, adjs) in self.adj_devices.iter() {
            let c = self.get(DeviceGetQuery::Id(*id)).unwrap();

            for (e_port, adj_id, _) in adjs {
                let target = self.get(DeviceGetQuery::Id(*adj_id)).unwrap();
                let start_pos = Vector2::new(c.pos.x, c.pos.y);
                let end_pos = Vector2::new(target.pos.x, target.pos.y);
                if !set.contains(adj_id) {
                    d.draw_line_ex(start_pos, end_pos, 2.5, Color::RAYWHITE);
                }
                set.insert(*id);

                let is_port_up = match id {
                    DeviceId::Switch(i) => self.switches[*i].device.is_port_up(*e_port),
                    DeviceId::Router(i) => self.routers[*i].device.is_port_up(*e_port),
                    _ => true,
                };

                let dir_e = (end_pos - start_pos).normalized();
                d.draw_circle(
                    (c.pos.x + dir_e.x * 35.0) as i32,
                    (c.pos.y + dir_e.y * 35.0) as i32,
                    5.0,
                    if is_port_up {
                        Color::LIMEGREEN
                    } else {
                        Color::RED
                    },
                );
            }
        }

        for component in &mut self.routers {
            d.draw_circle(
                component.attributes.pos.x as i32,
                component.attributes.pos.y as i32,
                ROUTER_DISPLAY_RADIUS + 2.0,
                Color::WHITE,
            );

            d.draw_circle(
                component.attributes.pos.x as i32,
                component.attributes.pos.y as i32,
                ROUTER_DISPLAY_RADIUS,
                Color::BLACK,
            );

            utils::draw_icon(
                GuiIconName::ICON_SHUFFLE_FILL,
                (component.attributes.pos.x - (ROUTER_DISPLAY_RADIUS / 1.5)) as i32,
                (component.attributes.pos.y - (ROUTER_DISPLAY_RADIUS / 1.5)) as i32,
                2,
                Color::WHITE,
            );

            d.draw_text(
                component.attributes.label.as_str(),
                component.attributes.pos.x as i32
                    - d.measure_text(&component.attributes.label, FONT_SIZE) / 2,
                (component.attributes.pos.y + ROUTER_DISPLAY_RADIUS) as i32 + PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }

        for component in &mut self.switches {
            d.draw_rectangle(
                component.attributes.pos.x as i32,
                component.attributes.pos.y as i32,
                SWITCH_DISPLAY_LENGTH,
                SWITCH_DISPLAY_LENGTH,
                Color::BLACK,
            );
            d.draw_rectangle_lines(
                component.attributes.pos.x as i32,
                component.attributes.pos.y as i32,
                SWITCH_DISPLAY_LENGTH,
                SWITCH_DISPLAY_LENGTH,
                Color::WHITE,
            );

            utils::draw_icon(
                GuiIconName::ICON_CURSOR_SCALE_FILL,
                component.attributes.pos.x as i32 + (SWITCH_DISPLAY_LENGTH / 6),
                component.attributes.pos.y as i32 + (SWITCH_DISPLAY_LENGTH / 6),
                2,
                Color::WHITE,
            );

            d.draw_text(
                component.attributes.label.as_str(),
                component.attributes.pos.x as i32,
                component.attributes.pos.y as i32 + SWITCH_DISPLAY_LENGTH + PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }

        for component in &mut self.desktops {
            d.draw_rectangle(
                component.attributes.pos.x as i32,
                component.attributes.pos.y as i32,
                DESKTOP_DISPLAY_SIZE,
                DESKTOP_DISPLAY_SIZE,
                Color::BLACK,
            );

            utils::draw_icon(
                GuiIconName::ICON_MONITOR,
                component.attributes.pos.x as i32,
                component.attributes.pos.y as i32,
                3,
                Color::WHITE,
            );

            d.draw_text(
                component.attributes.label.as_str(),
                component.attributes.pos.x as i32,
                component.attributes.pos.y as i32 + 5 * PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }
    }

    pub fn update(&mut self) {
        for component in &mut self.routers {
            component.device.tick();
        }

        for component in &mut self.switches {
            component.device.tick();
        }

        for componnet in &mut self.desktops {
            componnet.tick();
        }

        self.cable_simulator.tick();
    }
}

type CommandFunction<T> = fn(&mut Terminal<T>, &mut T, &[&str]) -> ();
struct Terminal<T> {
    out_buf: VecDeque<String>,
    dict: HashMap<String, (CommandFunction<T>, String)>,
    awaiting_command: Option<String>,
    timer: TickTimer<String>,
}

impl<T> Terminal<T> {
    fn new() -> Self {
        let mut dict = HashMap::new();

        // Insert the help command into the dictionary
        dict.insert(
            "help".to_string(),
            (
                Self::help as CommandFunction<T>,
                "Prints this help message".to_string(),
            ),
        );

        dict.insert(
            "clear".to_string(),
            (
                Self::clear as CommandFunction<T>,
                "Clear the terminal screen".to_string(),
            ),
        );

        Self {
            out_buf: VecDeque::new(),
            dict,
            awaiting_command: None,
            timer: TickTimer::default(),
        }
    }

    fn help(term: &mut Terminal<T>, _device: &mut T, _args: &[&str]) {
        for (cmd, (_, manual)) in term.dict.iter() {
            term.out_buf.push_back(format!("{}: {}", cmd, manual));
        }
    }

    fn clear(term: &mut Terminal<T>, _device: &mut T, _args: &[&str]) {
        // append a bunch of newlines to clear the terminal
        for _ in 0..100 {
            term.out_buf.push_back("".to_string());
        }
    }

    fn execute(&mut self, device: &mut T, input: &str) {
        if input.is_empty() {
            return;
        }

        let mut args = input.split_whitespace();
        let cmd = args.next().unwrap_or_default();

        if let Some((func, _)) = self.dict.get(cmd) {
            func(self, device, &args.collect::<Vec<&str>>());
        } else {
            self.out_buf
                .push_back(format!("Error: '{}' is not a valid command", cmd));
        }
    }
}

macro_rules! ipv4_fmt {
    ($ip:expr) => {
        format!("{}.{}.{}.{}", $ip[0], $ip[1], $ip[2], $ip[3])
    };
}

macro_rules! mac_fmt {
    ($mac:expr) => {
        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            $mac[0], $mac[1], $mac[2], $mac[3], $mac[4], $mac[5]
        )
    };
}

impl Terminal<Router> {
    fn new_router() -> Self {
        let mut term = Self::new();
        term.dict.insert(
            "enable".to_string(),
            (
                Self::enable as CommandFunction<Router>,
                "Enable a port. Usage: enable <port> <ip> <subnet>".to_string(),
            ),
        );

        term.dict.insert(
            "rip".to_string(),
            (
                Self::rip as CommandFunction<Router>,
                "Enable or disable RIP. Usage: rip <port>".to_string(),
            ),
        );

        term.dict.insert(
            "routes".to_string(),
            (
                Self::routes as CommandFunction<Router>,
                "Print the routing table".to_string(),
            ),
        );

        term.dict.insert(
            "ipconfig".to_string(),
            (
                Self::ifconfig as CommandFunction<Router>,
                "Print the current IP configuration of the router".to_string(),
            ),
        );

        term
    }

    fn enable(term: &mut Terminal<Router>, router: &mut Router, args: &[&str]) {
        if args.len() != 3 {
            term.out_buf
                .push_back("Usage: enable <port> <ip> <subnet>".to_string());
            return;
        }

        let port = match args[0].parse::<usize>() {
            Ok(port) => port,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid port", args[0]));
                return;
            }
        };

        let ip = match args[1].parse::<std::net::Ipv4Addr>() {
            Ok(ip) => ip,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid IPv4 address", args[1]));
                return;
            }
        };

        let subnet = match args[2].parse::<std::net::Ipv4Addr>() {
            Ok(subnet) => subnet,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid subnet mask", args[2]));
                return;
            }
        };

        router.enable_interface(port, ip.octets(), subnet.octets());
        term.out_buf.push_back(format!(
            "Port {} enabled with IP {} and subnet mask {}",
            port, ip, subnet
        ));
    }

    fn rip(term: &mut Terminal<Router>, router: &mut Router, args: &[&str]) {
        if args.len() != 1 {
            term.out_buf.push_back("Usage: rip <port>".to_string());
            return;
        }

        let port = match args[0].parse::<usize>() {
            Ok(port) => port,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid port", args[0]));
                return;
            }
        };

        match router.enable_rip(port) {
            Ok(_) => {
                term.out_buf
                    .push_back(format!("RIP enabled on port {}", port));
            }
            Err(e) => {
                term.out_buf.push_back(format!("Error: {}", e));
            }
        }
    }

    fn routes(term: &mut Terminal<Router>, router: &mut Router, _args: &[&str]) {
        term.out_buf.push_back("Routing Table:".to_string());

        for (key, route) in router.routing_table().iter() {
            term.out_buf.push_back(format!(
                "{} -> {} via port {}",
                ipv4_fmt!(key),
                ipv4_fmt!(route.ip_address),
                route.port
            ));
        }
    }

    fn ifconfig(term: &mut Terminal<Router>, router: &mut Router, _args: &[&str]) {
        term.out_buf.push_back("IP Configuration:".to_string());

        for (ip, subnet, port, enabled, rip_enabled) in router.interface_config().iter() {
            term.out_buf.push_back(format!(
                "Port {}: IP: {}, Subnet: {}, Enabled: {}, RIP: {}",
                port,
                ipv4_fmt!(ip),
                ipv4_fmt!(subnet),
                enabled,
                rip_enabled
            ));
        }
    }
}

impl Terminal<Switch> {
    fn new_switch() -> Self {
        let mut term = Self::new();
        term.dict.insert(
            "stp".to_string(),
            (
                Self::stp as CommandFunction<Switch>,
                "Enable or disable Spanning Tree Protocol. Usage: stp <priority>".to_string(),
            ),
        );

        term.dict.insert(
            "table".to_string(),
            (
                Self::table as CommandFunction<Switch>,
                "Print the MAC address table".to_string(),
            ),
        );

        term
    }

    fn stp(term: &mut Terminal<Switch>, switch: &mut Switch, args: &[&str]) {
        if args.len() != 1 {
            term.out_buf.push_back("Usage: stp <priority>".to_string());
            return;
        }

        let priority = match args[0].parse::<u16>() {
            Ok(priority) => priority,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid priority", args[0]));
                return;
            }
        };

        switch.set_bridge_priority(priority);
        switch.init_stp();
        term.out_buf.push_back(format!(
            "Spanning Tree Protocol priority set to {}",
            priority
        ));
    }

    fn table(term: &mut Terminal<Switch>, switch: &mut Switch, _args: &[&str]) {
        term.out_buf.push_back("MAC Address Table:".to_string());
        for (mac, port) in switch.mac_table().iter() {
            term.out_buf
                .push_back(format!("{} -> Port {}", mac_fmt!(mac), port));
        }
    }
}

impl Terminal<Desktop> {
    fn new_desktop() -> Self {
        let mut term = Self::new();
        term.dict.insert(
            "ipset".to_string(),
            (
                Self::ipset as CommandFunction<Desktop>,
                "Set the IP address of the desktop. Usage: ipset <ipv4 addr> <subnet addr>"
                    .to_string(),
            ),
        );

        term.dict.insert(
            "dgateway".to_string(),
            (
                Self::dgateway as CommandFunction<Desktop>,
                "Set the default gateway of the desktop. Usage: dgateway <ipv4 addr>".to_string(),
            ),
        );

        term.dict.insert(
            "ipconfig".to_string(),
            (
                Self::ipconfig as CommandFunction<Desktop>,
                "Print the current IP configuration of the desktop".to_string(),
            ),
        );

        term.dict.insert(
            "arptab".to_string(),
            (
                Self::arptab as CommandFunction<Desktop>,
                "Print the ARP table".to_string(),
            ),
        );

        term.dict.insert(
            "ping".to_string(),
            (
                Self::ping as CommandFunction<Desktop>,
                "Ping an IP address. Usage: ping <ipv4 addr>".to_string(),
            ),
        );

        term
    }

    fn ipset(term: &mut Terminal<Desktop>, desktop: &mut Desktop, args: &[&str]) {
        if args.len() != 2 {
            term.out_buf
                .push_back("Usage: ipset <ipv4 addr> <subnet mask>".to_string());
            return;
        }

        let ip = match args[0].parse::<std::net::Ipv4Addr>() {
            Ok(ip) => ip,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid IPv4 address", args[0]));
                return;
            }
        };

        let subnet = match args[1].parse::<std::net::Ipv4Addr>() {
            Ok(subnet) => subnet,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid subnet mask", args[1]));
                return;
            }
        };

        desktop.interface.ip_address = ip.octets();
        desktop.interface.subnet_mask = subnet.octets();

        term.out_buf.push_back(format!(
            "IP address set to {} with subnet mask {}",
            ip, subnet
        ));
    }

    fn dgateway(term: &mut Terminal<Desktop>, desktop: &mut Desktop, args: &[&str]) {
        if args.len() != 1 {
            term.out_buf
                .push_back("Usage: dgateway <ipv4 addr>".to_string());
            return;
        }

        let ip = match args[0].parse::<std::net::Ipv4Addr>() {
            Ok(ip) => ip,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid IPv4 address", args[0]));
                return;
            }
        };

        desktop.interface.default_gateway = Some(ip.octets());

        term.out_buf
            .push_back(format!("Default gateway set to {}", ip));
    }

    fn ipconfig(term: &mut Terminal<Desktop>, desktop: &mut Desktop, _args: &[&str]) {
        term.out_buf.push_back(format!(
            "IP Address: {}",
            ipv4_fmt!(desktop.interface.ip_address)
        ));

        term.out_buf.push_back(format!(
            "Subnet Mask: {}",
            ipv4_fmt!(desktop.interface.subnet_mask)
        ));
        if let Some(gateway) = desktop.interface.default_gateway {
            term.out_buf
                .push_back(format!("Default Gateway: {}", ipv4_fmt!(gateway)));
        } else {
            term.out_buf.push_back("Default Gateway: None".to_string());
        }

        let mac = desktop.interface.ethernet.mac_address;
        term.out_buf
            .push_back(format!("MAC Address: {}", mac_fmt!(mac)));
    }

    fn arptab(term: &mut Terminal<Desktop>, desktop: &mut Desktop, _args: &[&str]) {
        term.out_buf.push_back("ARP Table:".to_string());
        for (ip, mac) in desktop.interface.arp_table().iter() {
            term.out_buf
                .push_back(format!("{} -> {}", ipv4_fmt!(ip), mac_fmt!(mac)));
        }
    }

    fn ping(term: &mut Terminal<Desktop>, desktop: &mut Desktop, args: &[&str]) {
        if args.len() != 1 {
            term.out_buf
                .push_back("Usage: ping <ipv4 addr>".to_string());
            return;
        }

        let ip = match args[0].parse::<std::net::Ipv4Addr>() {
            Ok(ip) => ip,
            Err(_) => {
                term.out_buf
                    .push_back(format!("Error: '{}' is not a valid IPv4 address", args[0]));
                return;
            }
        };

        match desktop
            .interface
            .send_icmp(ip.octets(), IcmpType::EchoRequest)
        {
            Ok(_) => {
                term.out_buf.push_back(format!("Pinging {}...", ip));
                term.awaiting_command = Some("ping".to_string());
                term.timer.schedule("ping".to_string(), 3, false);
            }
            Err(e) => {
                term.out_buf.push_back(format!("Error: {}", e));
            }
        };
    }

    fn tick(&mut self, desktop: &mut Desktop) {
        if self.awaiting_command.is_none() {
            self.timer.tick();
            return;
        }

        for event in self.timer.ready() {
            self.out_buf.push_back(format!("'{}' timed out.", event));
            self.awaiting_command = None;
        }

        self.timer.tick();

        match self.awaiting_command.as_deref() {
            Some("ping") => {
                // Manually tick a desktop device. Find an ICMP reply frame to close the channel.
                for frame in desktop.interface.receive() {
                    if frame.destination != desktop.interface.ip_address {
                        continue;
                    }

                    if frame.protocol == 1 {
                        let icmp = match IcmpFrame::from_bytes(frame.data) {
                            Ok(icmp) => icmp,
                            Err(_) => {
                                continue;
                            }
                        };

                        if icmp.icmp_type == IcmpType::EchoReply as u8 {
                            self.out_buf.push_back(String::from("Pong!"));
                            self.awaiting_command = None;
                            return;
                        }
                    } else {
                        desktop.received.push(frame);
                    }
                }
            }
            _ => {}
        }
    }
}
