use std::collections::VecDeque;

use raylib::prelude::*;

use crate::simulation::utils;

use super::device::{DeviceKind, DeviceRepository};

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
    selection: Option<GuiButtonClickKind>,
    debounce: bool,
    terminal_open: bool,
    terminal_out: VecDeque<String>,
    terminal_buffer: [u8; 0xFF],
}

impl Default for Gui {
    fn default() -> Self {
        Self {
            selection: None,
            debounce: false,
            terminal_open: false,
            terminal_out: VecDeque::new(),
            terminal_buffer: [0u8; 0xFF],
        }
    }
}

impl Gui {
    pub fn render(&mut self, d: &mut RaylibDrawHandle) {
        const FONT_SIZE: i32 = 20;
        const PADDING: i32 = 10;
        const ACTIVE_COLOR: Color = Color::RED;
        const DEFAULT_COLOR: Color = Color::WHITE;
        const MAX_TERMINAL_LINES: usize = 4;
        let (box_width, box_height) = (55, 55);
        let (screen_width, screen_height) = (d.get_screen_width(), d.get_screen_height());

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

            if d.gui_button(bounds, None) {
                self.selection = Some(*kind);
                self.debounce = true;
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

            if d.gui_button(bounds, None) {
                self.selection = Some(*kind);
                self.debounce = true;
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
        let mut terminal_y = (4.0 / 5.0) * screen_height as f32;
        d.gui_set_style(
            GuiControl::DEFAULT,
            GuiDefaultProperty::BACKGROUND_COLOR as i32,
            Color::new(0, 0, 0, 0).color_to_int(),
        );
        d.gui_panel(
            Rectangle {
                x: 0.0,
                y: terminal_y,
                width: screen_width as f32,
                height: screen_height as f32 - terminal_y - PADDING as f32,
            },
            Some(rstr!("Terminal")),
        );
        terminal_y += 3.0 * PADDING as f32;

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

        let out_y =
            std::cmp::min(self.terminal_out.len(), 7) as f32 * (FONT_SIZE as f32) + terminal_y;

        if !self.terminal_open {
            d.draw_text("Desktop %", PADDING, out_y as i32, FONT_SIZE, Color::WHITE);
        }

        let label_size = d.measure_text("Desktop %", FONT_SIZE);
        let max_text_size = d.measure_text("W", FONT_SIZE) * 30; // roughly 30 characters

        if !self.terminal_open
            && d.gui_text_box(
                Rectangle::new(
                    (label_size + 2 * PADDING) as f32,
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
            //     utils::array_to_string(&ds.cmd_line_buffer),
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
        d.gui_load_style_default();

        // Output
        // if let Some(out) = self.terminal.out() {
        //     ds.cmd_line_out.push_back(out);
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
        // ---------------------
    }

    pub fn update(&mut self, rl: &RaylibHandle, ds: &mut DeviceRepository) {
        if self.debounce {
            self.debounce = false; // The initial click of the button shouldn't be registered as a click on the next frame
            return;
        }

        let is_left_mouse_clicked = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_pos = rl.get_mouse_position();

        let selection = self.selection;
        if is_left_mouse_clicked {
            self.selection = None;
            match selection {
                Some(GuiButtonClickKind::Desktop) => {
                    ds.add(DeviceKind::Desktop, mouse_pos);
                }
                Some(GuiButtonClickKind::Switch) => {
                    ds.add(DeviceKind::Switch, mouse_pos);
                }
                Some(GuiButtonClickKind::Router) => {
                    ds.add(DeviceKind::Router, mouse_pos);
                }
                _ => {}
            }
        }
    }
}
