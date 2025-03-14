use raylib::prelude::*;

use crate::{network::device::cable::CableSimulator, tick::Tickable};

use super::utils;

pub enum DeviceId {
    Router(usize),
    Switch(usize),
    Desktop(usize),
}

pub trait Device {}

pub enum DeviceKind {
    Desktop,
    Switch,
    Router,
}

pub struct DeviceRepository {
    routers: Vec<RouterDevice>,
    switches: Vec<SwitchDevice>,
    desktops: Vec<DesktopDevice>,
    label_seed: (i32, i32, i32), // (router, switch, desktop)

    cable_simulator: CableSimulator,
    seed: i32,
}

impl Default for DeviceRepository {
    fn default() -> Self {
        Self {
            routers: Vec::new(),
            switches: Vec::new(),
            desktops: Vec::new(),
            cable_simulator: CableSimulator::default(),
            seed: 0,
            label_seed: (0, 0, 0),
        }
    }
}

impl DeviceRepository {
    pub fn add(&mut self, kind: DeviceKind, pos: Vector2) {
        match kind {
            DeviceKind::Router => {
                self.seed += 8;
                let label: i32 = {
                    self.label_seed.0 += 1;
                    self.label_seed.0
                };
                self.routers.push(RouterDevice {
                    label: format!("Router {}", label),
                    pos,
                })
            }
            DeviceKind::Switch => {
                self.seed += 32;
                let label: i32 = {
                    self.label_seed.1 += 1;
                    self.label_seed.1
                };
                self.switches.push(SwitchDevice {
                    label: format!("Switch {}", label),
                    pos,
                })
            }
            DeviceKind::Desktop => {
                self.seed += 16;
                let label: i32 = {
                    self.label_seed.2 += 1;
                    self.label_seed.2
                };
                self.desktops.push(DesktopDevice {
                    label: format!("Desktop {}", label),
                    pos,
                })
            }
        }
    }

    pub fn get(&self, id: DeviceId) -> Option<&dyn Device> {
        match id {
            DeviceId::Router(i) => self.routers.get(i).map(|r| r as &dyn Device),
            DeviceId::Switch(i) => self.switches.get(i).map(|s| s as &dyn Device),
            DeviceId::Desktop(i) => self.desktops.get(i).map(|d| d as &dyn Device),
        }
    }

    pub fn render(&mut self, d: &mut RaylibDrawHandle) {
        const FONT_SIZE: i32 = 20;
        const PADDING: i32 = 10;
        for router in &mut self.routers {
            const RADIUS: f32 = 35.0;

            d.draw_circle(
                router.pos.x as i32,
                router.pos.y as i32,
                RADIUS + 2.0,
                Color::WHITE,
            );

            d.draw_circle(
                router.pos.x as i32,
                router.pos.y as i32,
                RADIUS,
                Color::BLACK,
            );

            utils::draw_icon(
                GuiIconName::ICON_SHUFFLE_FILL,
                (router.pos.x - (RADIUS / 1.5)) as i32,
                (router.pos.y - (RADIUS / 1.5)) as i32,
                3,
                Color::WHITE,
            );

            d.draw_text(
                router.label.as_str(),
                router.pos.x as i32 - d.measure_text(&router.label, FONT_SIZE) / 2,
                (router.pos.y + RADIUS) as i32 + PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }

        for switch in &mut self.switches {
            const LENGTH: i32 = 70;
            d.draw_rectangle_lines(
                switch.pos.x as i32,
                switch.pos.y as i32,
                LENGTH,
                LENGTH,
                Color::WHITE,
            );

            utils::draw_icon(
                GuiIconName::ICON_CURSOR_SCALE_FILL,
                switch.pos.x as i32 + (LENGTH / 6),
                switch.pos.y as i32 + (LENGTH / 6),
                3,
                Color::WHITE,
            );

            d.draw_text(
                switch.label.as_str(),
                switch.pos.x as i32,
                switch.pos.y as i32 + LENGTH + PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }

        for desktop in &mut self.desktops {
            utils::draw_icon(
                GuiIconName::ICON_MONITOR,
                desktop.pos.x as i32,
                desktop.pos.y as i32,
                5,
                Color::WHITE,
            );

            d.draw_text(
                desktop.label.as_str(),
                desktop.pos.x as i32,
                desktop.pos.y as i32 + 8 * PADDING,
                FONT_SIZE,
                Color::WHITE,
            );
        }
    }

    pub fn update(&mut self, rl: &RaylibHandle) {}
}

struct RouterDevice {
    label: String,
    pos: Vector2,
}

impl Device for RouterDevice {}

struct SwitchDevice {
    label: String,
    pos: Vector2,
}
impl Device for SwitchDevice {}

struct DesktopDevice {
    label: String,
    pos: Vector2,
}
impl Device for DesktopDevice {}
