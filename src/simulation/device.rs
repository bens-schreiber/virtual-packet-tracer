use std::collections::{HashMap, HashSet};

use raylib::prelude::*;

use crate::{
    network::device::{
        cable::{CableSimulator, EthernetPort},
        desktop::Desktop,
        router::Router,
        switch::Switch,
    },
    tick::Tickable,
};

use super::utils;

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
}

pub trait Device {
    fn pos(&self) -> Vector2;
    fn id(&self) -> DeviceId;
    fn is_port_up(&self, _port: usize) -> bool {
        true // TODO: port_status => Up/Down/Waiting
    }
    fn ports_len(&self) -> usize {
        1
    }
}

pub struct DeviceRepository {
    routers: Vec<RouterDevice>,
    switches: Vec<SwitchDevice>,
    desktops: Vec<DesktopDevice>,
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

const ROUTER_DISPLAY_RADIUS: f32 = 25.0;
const SWITCH_DISPLAY_LENGTH: i32 = 45;
const DESKTOP_DISPLAY_SIZE: i32 = SWITCH_DISPLAY_LENGTH; // roughly

impl DeviceRepository {
    pub fn add(&mut self, kind: DeviceKind, pos: Vector2) {
        self.mac_seed += 1;
        match kind {
            DeviceKind::Router => {
                let label: i32 = {
                    self.label_seeds.0 += 1;
                    self.label_seeds.0
                };
                self.routers.push(RouterDevice {
                    id: self.routers.len(),
                    label: format!("Router {}", label),
                    pos,
                    router: Router::from_seed(self.mac_seed),
                });
                self.mac_seed += 8;
            }
            DeviceKind::Switch => {
                let label: i32 = {
                    self.label_seeds.1 += 1;
                    self.label_seeds.1
                };
                self.switches.push(SwitchDevice {
                    id: self.switches.len(),
                    label: format!("Switch {}", label),
                    pos,
                    switch: Switch::from_seed(self.mac_seed, label as u16),
                });
                self.mac_seed += 32;
            }
            DeviceKind::Desktop => {
                let label: i32 = {
                    self.label_seeds.2 += 1;
                    self.label_seeds.2
                };
                self.desktops.push(DesktopDevice {
                    id: self.desktops.len(),
                    label: format!("Desktop {}", label),
                    pos,
                    desktop: Desktop::from_seed(self.mac_seed),
                });
                self.mac_seed += 1;
            }
        }
    }

    pub fn get(&self, query: DeviceGetQuery) -> Option<&dyn Device> {
        match query {
            // Linear search, no point in optimizing this for now.
            DeviceGetQuery::Pos(pos) => {
                for router in self.routers.iter() {
                    if router.pos.distance_to(pos) < ROUTER_DISPLAY_RADIUS {
                        return Some(router);
                    }
                }

                for switch in self.switches.iter() {
                    let rec = Rectangle {
                        x: switch.pos.x,
                        y: switch.pos.y,
                        width: SWITCH_DISPLAY_LENGTH as f32,
                        height: SWITCH_DISPLAY_LENGTH as f32,
                    };
                    if rec.check_collision_point_rec(pos) {
                        return Some(switch);
                    }
                }

                for desktop in self.desktops.iter() {
                    let rec = Rectangle {
                        x: desktop.pos.x,
                        y: desktop.pos.y,
                        width: DESKTOP_DISPLAY_SIZE as f32,
                        height: DESKTOP_DISPLAY_SIZE as f32,
                    };
                    if rec.check_collision_point_rec(pos) {
                        return Some(desktop);
                    }
                }

                None
            }
            DeviceGetQuery::Id(id) => match id {
                DeviceId::Router(i) => self.routers.get(i).map(|r| r as &dyn Device),
                DeviceId::Switch(i) => self.switches.get(i).map(|s| s as &dyn Device),
                DeviceId::Desktop(i) => self.desktops.get(i).map(|d| d as &dyn Device),
            },
        }
    }

    pub fn set(&mut self, id: DeviceId, query: DeviceSetQuery) {
        match query {
            DeviceSetQuery::Pos(pos) => match id {
                DeviceId::Router(i) => self.routers[i].pos = pos,
                DeviceId::Switch(i) => self.switches[i].pos = pos,
                DeviceId::Desktop(i) => self.desktops[i].pos = pos,
            },
            DeviceSetQuery::Connect(adj_id, self_port, adj_port) => {
                self.connect(id, self_port, adj_id, adj_port);
            }
            DeviceSetQuery::Disconnect(port) => {
                self.disconnect(id, port);
            }
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
            let d = &mut dr.desktops[d_i];
            match other_id {
                DeviceId::Desktop(_) => {
                    EthernetPort::connect(
                        &mut d.desktop.interface.ethernet.port(),
                        &mut dr.desktops[other_i].desktop.interface.ethernet.port(),
                    );
                }
                DeviceId::Switch(_) => {
                    dr.switches[other_i]
                        .switch
                        .connect(other_port, &mut d.desktop.interface.ethernet);
                }
                DeviceId::Router(_) => {
                    dr.routers[other_i]
                        .router
                        .connect(other_port, &mut d.desktop.interface);
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
            let d = &mut dr.switches[d_i];
            match other_id {
                DeviceId::Desktop(_) => {
                    d.switch
                        .connect(port, &mut dr.desktops[other_i].desktop.interface.ethernet);
                }
                DeviceId::Switch(_) => {
                    // have to call connect on the switch device so the switch hello bpdu is sent.
                    // compiler gymnastics ensue...
                    let (d, other_switch) = if d_i < other_i {
                        let (left, right) = dr.switches.split_at_mut(other_i);
                        (&mut left[d_i], &mut right[0])
                    } else {
                        let (left, right) = dr.switches.split_at_mut(d_i);
                        (&mut right[0], &mut left[other_i])
                    };

                    d.switch
                        .connect_switch(port, &mut other_switch.switch, other_port);
                }
                DeviceId::Router(_) => {
                    EthernetPort::connect(
                        &mut d.switch.ports()[port],
                        &mut dr.routers[other_i].router.ports()[other_port],
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
            let d = &mut dr.routers[d_i];
            match other_id {
                DeviceId::Desktop(_) => {
                    d.router
                        .connect(port, &mut dr.desktops[other_i].desktop.interface);
                }
                DeviceId::Switch(_) => {
                    EthernetPort::connect(
                        &mut d.router.ports()[port],
                        &mut dr.switches[other_i].switch.ports()[other_port],
                    );
                }
                DeviceId::Router(_) => {
                    EthernetPort::connect(
                        &mut d.router.ports()[port],
                        &mut dr.routers[other_i].router.ports()[other_port],
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
                    dr.desktops[i].desktop.interface.disconnect();
                }
                DeviceId::Switch(_) => {
                    dr.switches[i].switch.disconnect(port);
                }
                DeviceId::Router(_) => {
                    dr.routers[i].router.disconnect(port);
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
            let dv = self.get(DeviceGetQuery::Id(*id)).unwrap();

            for (e_port, adj_id, _) in adjs {
                let target = self.get(DeviceGetQuery::Id(*adj_id)).unwrap();
                let start_pos = Vector2::new(dv.pos().x, dv.pos().y);
                let end_pos = Vector2::new(target.pos().x, target.pos().y);
                if !set.contains(adj_id) {
                    d.draw_line_ex(start_pos, end_pos, 2.5, Color::RAYWHITE);
                }
                set.insert(*id);

                let dir_e = (end_pos - start_pos).normalized();
                d.draw_circle(
                    (dv.pos().x + dir_e.x * 35.0) as i32,
                    (dv.pos().y + dir_e.y * 35.0) as i32,
                    5.0,
                    if dv.is_port_up(*e_port) {
                        Color::LIMEGREEN
                    } else {
                        Color::RED
                    },
                );
            }
        }

        for router in &mut self.routers {
            d.draw_circle(
                router.pos.x as i32,
                router.pos.y as i32,
                ROUTER_DISPLAY_RADIUS + 2.0,
                Color::WHITE,
            );

            d.draw_circle(
                router.pos.x as i32,
                router.pos.y as i32,
                ROUTER_DISPLAY_RADIUS,
                Color::BLACK,
            );

            utils::draw_icon(
                GuiIconName::ICON_SHUFFLE_FILL,
                (router.pos.x - (ROUTER_DISPLAY_RADIUS / 1.5)) as i32,
                (router.pos.y - (ROUTER_DISPLAY_RADIUS / 1.5)) as i32,
                2,
                Color::WHITE,
            );

            d.draw_text(
                router.label.as_str(),
                router.pos.x as i32 - d.measure_text(&router.label, FONT_SIZE) / 2,
                (router.pos.y + ROUTER_DISPLAY_RADIUS) as i32 + PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }

        for switch in &mut self.switches {
            d.draw_rectangle(
                switch.pos.x as i32,
                switch.pos.y as i32,
                SWITCH_DISPLAY_LENGTH,
                SWITCH_DISPLAY_LENGTH,
                Color::BLACK,
            );
            d.draw_rectangle_lines(
                switch.pos.x as i32,
                switch.pos.y as i32,
                SWITCH_DISPLAY_LENGTH,
                SWITCH_DISPLAY_LENGTH,
                Color::WHITE,
            );

            utils::draw_icon(
                GuiIconName::ICON_CURSOR_SCALE_FILL,
                switch.pos.x as i32 + (SWITCH_DISPLAY_LENGTH / 6),
                switch.pos.y as i32 + (SWITCH_DISPLAY_LENGTH / 6),
                2,
                Color::WHITE,
            );

            d.draw_text(
                switch.label.as_str(),
                switch.pos.x as i32,
                switch.pos.y as i32 + SWITCH_DISPLAY_LENGTH + PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }

        for desktop in &mut self.desktops {
            utils::draw_icon(
                GuiIconName::ICON_MONITOR,
                desktop.pos.x as i32,
                desktop.pos.y as i32,
                3,
                Color::WHITE,
            );

            d.draw_rectangle(
                desktop.pos.x as i32,
                desktop.pos.y as i32,
                DESKTOP_DISPLAY_SIZE,
                DESKTOP_DISPLAY_SIZE,
                Color::BLACK,
            );

            d.draw_text(
                desktop.label.as_str(),
                desktop.pos.x as i32,
                desktop.pos.y as i32 + 5 * PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }
    }

    pub fn update(&mut self) {
        for router in &mut self.routers {
            router.router.tick();
        }

        for switch in &mut self.switches {
            switch.switch.tick();
        }

        for desktop in &mut self.desktops {
            desktop.desktop.tick();
        }

        self.cable_simulator.tick();
    }
}

struct RouterDevice {
    id: usize,
    label: String,
    pos: Vector2,
    router: Router,
}

impl Device for RouterDevice {
    fn pos(&self) -> Vector2 {
        self.pos
    }

    fn id(&self) -> DeviceId {
        DeviceId::Router(self.id)
    }

    fn is_port_up(&self, port: usize) -> bool {
        self.router.is_port_up(port)
    }
}

struct SwitchDevice {
    id: usize,
    label: String,
    pos: Vector2,
    switch: Switch,
}

impl Device for SwitchDevice {
    fn pos(&self) -> Vector2 {
        self.pos
            + Vector2::new(
                SWITCH_DISPLAY_LENGTH as f32 / 2.0,
                SWITCH_DISPLAY_LENGTH as f32 / 2.0,
            )
    }

    fn id(&self) -> DeviceId {
        DeviceId::Switch(self.id)
    }

    fn is_port_up(&self, port: usize) -> bool {
        self.switch.is_port_up(port)
    }

    fn ports_len(&self) -> usize {
        self.switch.ports().len()
    }
}
struct DesktopDevice {
    id: usize,
    label: String,
    pos: Vector2,
    desktop: Desktop,
}

impl Device for DesktopDevice {
    fn pos(&self) -> Vector2 {
        self.pos
            + Vector2::new(
                DESKTOP_DISPLAY_SIZE as f32 / 2.0,
                DESKTOP_DISPLAY_SIZE as f32 / 2.0,
            )
    }

    fn id(&self) -> DeviceId {
        DeviceId::Desktop(self.id)
    }
}
