use std::collections::VecDeque;

use raylib::prelude::*;

use crate::simulation::utils;

use super::device::{DeviceGetQuery, DeviceId, DeviceKind, DeviceRepository, DeviceSetQuery};

struct FrameTimer {
    threshold: i32,
    value: i32,
}

impl FrameTimer {
    fn new(threshold: i32) -> Self {
        Self {
            threshold,
            value: 0,
        }
    }

    fn ready(&mut self) -> bool {
        if self.value > 0 {
            self.value -= 1;
            return false;
        }
        true
    }

    fn set(&mut self) {
        self.value = self.threshold;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GuiMode {
    EthernetConnection,
    Drag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GuiButtonClickKind {
    Desktop,
    Switch,
    Router,
    Ethernet,
    PlayerPlay,
    PlayerNext,
    PlayerPause,
}

pub struct Gui {
    mode: Option<GuiMode>,
    selection: Option<GuiButtonClickKind>,

    dropdown_device: Option<DeviceId>,
    dropdown_value: i32,
    dropdown_scroll_index: i32,
    dropdown_bounds: Rectangle,

    frame_timer: FrameTimer,

    drag_device: Option<DeviceId>,
    connect_d1: Option<(DeviceId, usize)>,
    connect_d2: Option<(DeviceId, usize)>,

    terminal_open: bool,
    terminal_out: VecDeque<String>,
    terminal_buffer: [u8; 0xFF],
}

impl Default for Gui {
    fn default() -> Self {
        Self {
            mode: None,
            drag_device: None,
            selection: None,
            dropdown_device: None,
            dropdown_value: -1,
            dropdown_scroll_index: 0,
            dropdown_bounds: Rectangle::default(),
            frame_timer: FrameTimer::new(1),
            connect_d1: None,
            connect_d2: None,
            terminal_open: false,
            terminal_out: VecDeque::new(),
            terminal_buffer: [0u8; 0xFF],
        }
    }
}

impl Gui {
    fn close_dropdown(&mut self) {
        self.dropdown_device = None;
        self.dropdown_value = -1;
        self.dropdown_scroll_index = 0;
        self.dropdown_bounds = Rectangle::default();
    }

    fn reset(&mut self) {
        self.mode = None;
        self.connect_d1 = None;
        self.connect_d2 = None;
        self.selection = None;
        self.close_dropdown();
    }

    pub fn render(&mut self, d: &mut RaylibDrawHandle, dr: &DeviceRepository) {
        const FONT_SIZE: i32 = 10;
        const PADDING: i32 = 10;
        const ACTIVE_COLOR: Color = Color::RED;
        const DEFAULT_COLOR: Color = Color::WHITE;
        const MAX_TERMINAL_LINES: usize = 13;
        const DROPDOWN_WIDTH: i32 = 140;
        const DROPDOWN_MAX_HEIGHT: i32 = 200;
        let (box_width, box_height) = (55, 55);
        let (screen_width, screen_height) = (d.get_screen_width(), d.get_screen_height());
        let mouse_pos = d.get_mouse_position();

        let can_listen_mouse_event = self.frame_timer.ready();

        // Gui Mode Rendering
        // -----------------------------------
        match self.mode {
            Some(GuiMode::EthernetConnection) => {
                if let (Some(d1), Some(d2)) = (self.connect_d1, self.dropdown_device) {
                    let pos1 = dr.get(DeviceGetQuery::Id(d1.0)).map(|d| d.pos());
                    let pos2 = dr.get(DeviceGetQuery::Id(d2)).map(|d| d.pos());

                    if let (Some(pos1), Some(pos2)) = (pos1, pos2) {
                        d.draw_line_ex(pos1, pos2, 2.0, Color::WHITE);
                    }
                }
                //
                else if let Some((device, _)) = self.connect_d1 {
                    if let Some(pos1) = dr.get(DeviceGetQuery::Id(device)).map(|d| d.pos()) {
                        d.draw_line_ex(pos1, mouse_pos, 2.0, Color::WHITE);
                    }
                }
            }
            _ => {}
        }
        // -----------------------------------

        // Ethernet Dropdown Menu
        // -----------------------------------
        if !can_listen_mouse_event { // checkmate rust
        } else if let Some(id) = self.dropdown_device {
            let device = dr.get(DeviceGetQuery::Id(id));
            if device.is_none() {
                self.dropdown_device = None;
                return;
            }

            let pos = device.unwrap().pos();
            let ports_len = device.unwrap().ports_len();

            // A dropdown with ports_len options saying "Ethernet Port 0/i" for desktops, switches and "GigabitEthernet 0/i" for routers
            let height = std::cmp::min(DROPDOWN_MAX_HEIGHT, ports_len as i32 * (3 * FONT_SIZE));
            self.dropdown_bounds = Rectangle::new(
                pos.x + PADDING as f32,
                pos.y + PADDING as f32,
                DROPDOWN_WIDTH as f32,
                height as f32,
            );

            let label = match id {
                DeviceId::Desktop(_) | DeviceId::Switch(_) => "Ethernet Port",
                DeviceId::Router(_) => "GigabitEthernet",
            };
            let options = (0..ports_len)
                .map(|i| format!("{} 0/{}", label, i))
                .collect::<Vec<String>>();

            d.gui_list_view(
                self.dropdown_bounds,
                Some(utils::rstr_from_string(options.join(";")).as_c_str()),
                &mut self.dropdown_scroll_index,
                &mut self.dropdown_value,
            );

            if self.dropdown_value >= 0 {
                if self.connect_d1.is_none() {
                    self.connect_d1 = Some((id, self.dropdown_value as usize));
                    self.mode = Some(GuiMode::EthernetConnection);
                } else if self.connect_d2.is_none() {
                    self.connect_d2 = Some((id, self.dropdown_value as usize));
                    self.mode = None;
                }

                self.close_dropdown();
            }
        }
        // -----------------------------------

        {
            d.gui_set_style(
                GuiControl::BUTTON,
                GuiControlProperty::BASE_COLOR_NORMAL as i32,
                Color::new(0, 0, 0, 0).color_to_int(),
            );

            d.gui_set_style(
                GuiControl::BUTTON,
                GuiControlProperty::BASE_COLOR_FOCUSED as i32,
                Color::new(0, 0, 0, 0).color_to_int(),
            );

            d.gui_set_style(
                GuiControl::BUTTON,
                GuiControlProperty::BASE_COLOR_PRESSED as i32,
                Color::new(0, 0, 0, 0).color_to_int(),
            );

            d.gui_set_style(
                GuiControl::BUTTON,
                GuiControlProperty::BORDER_COLOR_PRESSED as i32,
                ACTIVE_COLOR.color_to_int(),
            );

            d.gui_set_style(
                GuiControl::BUTTON,
                GuiControlProperty::BORDER_COLOR_FOCUSED as i32,
                ACTIVE_COLOR.color_to_int(),
            );
        }

        // Left menu
        // -----------------------------------
        const LEFT_MENU: [(GuiButtonClickKind, GuiIconName); 4] = [
            (GuiButtonClickKind::Desktop, GuiIconName::ICON_MONITOR),
            (
                GuiButtonClickKind::Switch,
                GuiIconName::ICON_CURSOR_SCALE_FILL,
            ),
            (GuiButtonClickKind::Router, GuiIconName::ICON_SHUFFLE_FILL),
            (GuiButtonClickKind::Ethernet, GuiIconName::ICON_LINK_NET),
        ];

        for (i, (kind, icon)) in LEFT_MENU.iter().enumerate() {
            let y = PADDING + (box_height + PADDING) * (i as i32);
            let x = PADDING;
            let bounds = Rectangle::new(x as f32, y as f32, box_width as f32, box_height as f32);

            if d.gui_button(bounds, None) && can_listen_mouse_event {
                self.selection = Some(*kind);
            }

            if self.selection == Some(*kind) {
                d.draw_rectangle_lines_ex(bounds, 2.0, ACTIVE_COLOR);

                // make the icon follow the mouse when selected
                let mouse_pos = d.get_mouse_position();
                utils::draw_icon(
                    *icon,
                    mouse_pos.x as i32 + PADDING,
                    mouse_pos.y as i32 + PADDING,
                    2,
                    Color::WHITE,
                );
            }

            utils::draw_icon(
                *icon,
                x + box_width / 4,
                y + box_height / 4,
                2,
                Color::WHITE,
            );
        }
        // -----------------------------------

        // Player controls
        // -----------------------------------
        const RIGHT_CORNER_MENU: [(GuiButtonClickKind, GuiIconName); 2] = [
            (
                GuiButtonClickKind::PlayerPlay,
                GuiIconName::ICON_PLAYER_PLAY,
            ),
            (
                GuiButtonClickKind::PlayerPause,
                GuiIconName::ICON_PLAYER_PAUSE,
            ),
        ];

        for (i, (kind, icon)) in RIGHT_CORNER_MENU.iter().enumerate() {
            let x = (screen_width - PADDING) - (PADDING + box_width) * (i as i32) - box_width;
            let y = PADDING;
            let bounds = Rectangle::new(x as f32, y as f32, box_width as f32, box_height as f32);

            if d.gui_button(bounds, None) && can_listen_mouse_event {
                self.selection = Some(*kind);
            }

            utils::draw_icon(
                *icon,
                x + box_width / 4,
                y + box_height / 4,
                2,
                Color::WHITE,
            );
        }
        // -----------------------------------

        // Terminal
        // -----------------------------------
        let mut terminal_y = (3.0 / 4.0) * screen_height as f32;
        d.gui_set_style(
            GuiControl::DEFAULT,
            GuiDefaultProperty::BACKGROUND_COLOR as i32,
            Color::new(0, 0, 0, 0).color_to_int(),
        );
        d.gui_panel(
            Rectangle {
                x: 0.0,
                y: terminal_y,
                width: (screen_width / 3) as f32,
                height: screen_height as f32 - terminal_y - PADDING as f32,
            },
            Some(rstr!("Terminal")),
        );
        terminal_y += 3.0 * PADDING as f32;

        {
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
            d.gui_set_style(
                GuiControl::DEFAULT,
                GuiDefaultProperty::TEXT_SIZE as i32,
                FONT_SIZE,
            );
        }

        let out_y = std::cmp::min(self.terminal_out.len(), MAX_TERMINAL_LINES) as f32
            * (FONT_SIZE as f32)
            + terminal_y;

        if !self.terminal_open {
            d.draw_text("Desktop %", PADDING, out_y as i32, FONT_SIZE, Color::WHITE);
        }

        let label_size = d.measure_text("Desktop %", FONT_SIZE);
        let max_text_size = d.measure_text("W", FONT_SIZE) * 30; // roughly 30 characters

        if !self.terminal_open
            && d.gui_text_box(
                Rectangle::new(
                    (label_size + PADDING) as f32,
                    out_y,
                    max_text_size as f32,
                    FONT_SIZE as f32,
                ),
                &mut self.terminal_buffer,
                true,
            )
            && d.is_key_pressed(KeyboardKey::KEY_ENTER)
        {
            // self.terminal.input(
            //     utils::array_to_string(&dr.cmd_line_buffer),
            //     &mut self.desktop,
            // );

            self.terminal_out.push_back(format!(
                "Desktop % {}",
                utils::array_to_string(&self.terminal_buffer)
            ));

            if self.terminal_out.len() > MAX_TERMINAL_LINES {
                self.terminal_out.pop_front();
            }
            self.terminal_buffer = [0u8; 0xFF];
        }

        // Output
        // if let Some(out) = self.terminal.out() {
        //     dr.cmd_line_out.push_back(out);
        // }

        let mut out_y = terminal_y;
        for line in self
            .terminal_out
            .iter()
            .rev()
            .take(MAX_TERMINAL_LINES)
            .rev()
        {
            d.draw_text(line, PADDING, out_y as i32, FONT_SIZE, Color::WHITE);
            out_y += FONT_SIZE as f32;
        }
        // -----------------------------------

        // Packet Tracer
        // -----------------------------------
        let mut table_y = (3.0 / 4.0) * screen_height as f32;
        d.gui_panel(
            Rectangle {
                x: (screen_width / 3 - 1) as f32,
                y: table_y,
                width: (2.0 / 3.0) * screen_width as f32,
                height: screen_height as f32 - table_y - PADDING as f32,
            },
            Some(rstr!("Packet Tracer")),
        );
        table_y += 3.0 * PADDING as f32;

        d.gui_load_style_default();
    }

    pub fn update(&mut self, rl: &RaylibHandle, dr: &mut DeviceRepository) {
        let is_left_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        let is_left_mouse_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_pos = rl.get_mouse_position();

        // Drag Device
        // -----------------------------------
        if is_left_mouse_clicked && self.mode == Some(GuiMode::Drag) {
            self.mode = None;
            self.drag_device = None;
            return;
        }

        if is_left_mouse_down && self.mode == Some(GuiMode::Drag) {
            if self.drag_device.is_none() {
                self.mode = None;
            } else {
                dr.set(self.drag_device.unwrap(), DeviceSetQuery::Pos(mouse_pos));
            }
            return;
        }

        if is_left_mouse_down && self.mode == None && self.selection == None {
            if let Some(d) = dr.get(DeviceGetQuery::Pos(mouse_pos)) {
                self.mode = Some(GuiMode::Drag);
                self.drag_device = Some(d.id());
            }
            return;
        }
        // -----------------------------------

        // Ethernet Connect
        // -----------------------------------
        if let (Some((d1_id, d1_port)), Some((d2_id, d2_port))) = (self.connect_d1, self.connect_d2)
        {
            dr.set(d1_id, DeviceSetQuery::Connect(d2_id, d1_port, d2_port));
            self.reset();
        }
        // -----------------------------------

        if !is_left_mouse_clicked || self.dropdown_bounds.check_collision_point_rec(mouse_pos) {
            return;
        }
        self.frame_timer.set(); // RayGUI does click logic in render. If a click is consumed here, we don't want the render to consume it again.

        // Ethernet Connection Mode
        // -----------------------------------
        if self.selection == Some(GuiButtonClickKind::Ethernet) {
            if let Some(d) = dr.get(DeviceGetQuery::Pos(mouse_pos)) {
                self.dropdown_device = Some(d.id());
            } else {
                self.reset();
            }
            return;
        }
        // -----------------------------------

        // GUI Button Clicks
        // -----------------------------------
        if let Some(selection) = self.selection {
            match selection {
                GuiButtonClickKind::Desktop => {
                    dr.add(DeviceKind::Desktop, mouse_pos);
                    self.selection = None;
                }
                GuiButtonClickKind::Switch => {
                    dr.add(DeviceKind::Switch, mouse_pos);
                    self.selection = None;
                }
                GuiButtonClickKind::Router => {
                    dr.add(DeviceKind::Router, mouse_pos);
                    self.selection = None;
                }
                _ => {}
            }
        }
        // -----------------------------------
    }
}
