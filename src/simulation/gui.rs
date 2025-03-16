use std::{collections::VecDeque, time::SystemTime};

use raylib::prelude::*;

use crate::{
    ipv4_fmt, mac_fmt,
    network::{
        device::router::RipFrame,
        ethernet::{ByteSerializable, Ethernet2Frame, Ethernet802_3Frame},
        ipv4::{ArpFrame, IcmpFrame, Ipv4Frame},
    },
    simulation::{
        device::DeviceAttributes,
        utils::{self, rstr_from_string},
    },
    tick::TimeProvider,
};

use super::{
    device::{DeviceGetQuery, DeviceId, DeviceKind, DeviceRepository, DeviceSetQuery},
    utils::PacketKind,
};

#[derive(Clone)]
struct Packet {
    last: Option<DeviceId>,
    current: DeviceId,
    kind: PacketKind,
    time: SystemTime,
}

#[derive(Copy, Clone)]
struct Dropdown {
    device: DeviceId,
    value: i32,
    scroll_index: i32,
    bounds: Rectangle,
}

impl Dropdown {
    fn new(device: DeviceId) -> Self {
        Self {
            device,
            value: -1,
            scroll_index: 0,
            bounds: Rectangle::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GuiMode {
    EthernetDisconnect,
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
    Terminal,
}

pub struct Gui {
    mode: Option<GuiMode>,
    selection: Option<GuiButtonClickKind>,

    ethernet_dropdown: Option<Dropdown>,
    edit_dropdown: Option<Dropdown>,

    gui_consume_this_click: bool,

    drag_device: Option<DeviceId>,
    connect_d1: Option<(DeviceId, usize)>,
    connect_d2: Option<(DeviceId, usize)>,

    terminal_out: VecDeque<String>,
    terminal_buffer: [u8; 0xFF],
    terminal_device: Option<DeviceId>,

    packet_buffer: VecDeque<Packet>,
    packet_selected: Option<Packet>,
}

impl Default for Gui {
    fn default() -> Self {
        Self {
            mode: None,
            drag_device: None,
            selection: None,
            ethernet_dropdown: None,
            edit_dropdown: None,
            gui_consume_this_click: true,
            connect_d1: None,
            connect_d2: None,
            terminal_out: VecDeque::new(),
            terminal_buffer: [0u8; 0xFF],
            terminal_device: None,
            packet_buffer: VecDeque::new(),
            packet_selected: None,
        }
    }
}

impl Gui {
    fn reset_states(&mut self) {
        self.mode = None;
        self.connect_d1 = None;
        self.connect_d2 = None;
        self.selection = None;
        self.ethernet_dropdown = None;
        self.edit_dropdown = None;
    }

    pub fn render(&mut self, d: &mut RaylibDrawHandle, dr: &mut DeviceRepository) {
        const FONT_SIZE: i32 = 10;
        const PADDING: i32 = 10;
        const ACTIVE_COLOR: Color = Color::RED;
        const DEFAULT_COLOR: Color = Color::WHITE;
        const MAX_TERMINAL_LINES: usize = 8;
        const DROPDOWN_WIDTH: i32 = 140;
        const DROPDOWN_MAX_HEIGHT: i32 = 200;
        let (box_width, box_height) = (55, 55);
        let (screen_width, screen_height) = (d.get_screen_width(), d.get_screen_height());
        let mouse_pos = d.get_mouse_position();

        // Gui Mode Rendering
        // -----------------------------------
        if let Some(GuiMode::EthernetConnection) = self.mode {
            if let (Some(d1), Some(d2)) =
                (self.connect_d1, self.ethernet_dropdown.map(|d| d.device))
            {
                let pos1 = dr.get(DeviceGetQuery::Id(d1.0)).map(|da| da.pos);
                let pos2 = dr.get(DeviceGetQuery::Id(d2)).map(|da| da.pos);

                if let (Some(pos1), Some(pos2)) = (pos1, pos2) {
                    d.draw_line_ex(pos1, pos2, 2.0, Color::WHITE);
                }
            }
            //
            else if let Some((device, _)) = self.connect_d1 {
                if let Some(pos1) = dr.get(DeviceGetQuery::Id(device)).map(|da| da.pos) {
                    d.draw_line_ex(pos1, mouse_pos, 2.0, Color::WHITE)
                }
            }
        }
        // -----------------------------------

        // Edit Dropdown Menu
        // -----------------------------------
        if let Some(mut dropdown) = self.edit_dropdown {
            let da = dr.get(DeviceGetQuery::Id(dropdown.device)).unwrap();
            let pos = da.pos;

            let options = ["Terminal", "Delete; Disconnect"];
            let height = 10 * FONT_SIZE;
            dropdown.bounds = Rectangle::new(
                pos.x + PADDING as f32,
                pos.y + PADDING as f32,
                DROPDOWN_WIDTH as f32 / 1.5,
                height as f32,
            );

            d.gui_list_view(
                dropdown.bounds,
                Some(utils::rstr_from_string(options.join(";")).as_c_str()),
                &mut dropdown.scroll_index,
                &mut dropdown.value,
            );

            match dropdown.value {
                0 => {
                    self.edit_dropdown = None;
                    self.terminal_device = Some(dropdown.device);

                    // TODO: Save terminal instances in memory?
                    self.terminal_buffer = [0u8; 0xFF];
                    self.terminal_out.clear();
                }
                1 => {
                    dr.set(dropdown.device, DeviceSetQuery::Delete);
                    self.reset_states();
                }
                2 => {
                    self.edit_dropdown = None;
                    self.ethernet_dropdown = Some(Dropdown::new(dropdown.device));
                    self.mode = Some(GuiMode::EthernetDisconnect);
                    self.gui_consume_this_click = false; // Ethernet dropdown will immediately consume click, stop it from doing so.
                }
                _ => {
                    self.edit_dropdown = Some(dropdown);
                }
            }
        }
        // -----------------------------------

        // Ethernet Dropdown Menu
        // -----------------------------------
        if !self.gui_consume_this_click { // checkmate rust
        } else if let Some(mut dropdown) = self.ethernet_dropdown {
            let da = dr.get(DeviceGetQuery::Id(dropdown.device)).unwrap();
            let DeviceAttributes { ports_len, pos, .. } = da;

            // A dropdown with ports_len options saying "Ethernet Port 0/i" for desktops, switches and "GigabitEthernet 0/i" for routers
            let height = std::cmp::min(DROPDOWN_MAX_HEIGHT, ports_len as i32 * (3 * FONT_SIZE));
            dropdown.bounds = Rectangle::new(
                pos.x + PADDING as f32,
                pos.y + PADDING as f32,
                DROPDOWN_WIDTH as f32,
                height as f32,
            );

            let label = match dropdown.device {
                DeviceId::Desktop(_) | DeviceId::Switch(_) => "Ethernet Port",
                DeviceId::Router(_) => "GigabitEthernet",
            };
            let options = (0..ports_len)
                .map(|i| format!("{} 0/{}", label, i))
                .collect::<Vec<String>>();

            d.gui_list_view(
                dropdown.bounds,
                Some(utils::rstr_from_string(options.join(";")).as_c_str()),
                &mut dropdown.scroll_index,
                &mut dropdown.value,
            );

            if dropdown.value >= 0 {
                if self.mode == Some(GuiMode::EthernetDisconnect) {
                    dr.set(
                        dropdown.device,
                        DeviceSetQuery::Disconnect(dropdown.value as usize),
                    );
                    self.mode = None;
                }
                //
                else if self.connect_d1.is_none() {
                    self.connect_d1 = Some((dropdown.device, dropdown.value as usize));
                    self.mode = Some(GuiMode::EthernetConnection);
                }
                //
                else if self.connect_d2.is_none() {
                    self.connect_d2 = Some((dropdown.device, dropdown.value as usize));
                    self.mode = None;
                }
                self.ethernet_dropdown = None;
            } else {
                self.ethernet_dropdown = Some(dropdown);
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

            if d.gui_button(bounds, None) && self.gui_consume_this_click {
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

            if d.gui_button(bounds, None) && self.gui_consume_this_click {
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
        let terminal_y = (3.0 / 4.0) * screen_height as f32;

        // Terminal header
        d.draw_text(
            "Terminal",
            PADDING,
            terminal_y as i32 + FONT_SIZE / 2,
            2 * FONT_SIZE,
            Color::WHITE,
        );

        // lones above and below temrinal just like packet tracer has lines for columns
        d.draw_line(
            0,
            (terminal_y + 3.0 * FONT_SIZE as f32) as i32,
            screen_width - PADDING,
            (terminal_y + 3.0 * FONT_SIZE as f32) as i32,
            Color::WHITE,
        );

        // lnine above header
        d.draw_line(
            0,
            terminal_y as i32,
            screen_width - PADDING,
            terminal_y as i32,
            Color::WHITE,
        );

        let terminal_text_start_y = terminal_y + 4.0 * FONT_SIZE as f32;

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
            * ((FONT_SIZE + PADDING / 2) as f32)
            + terminal_text_start_y;

        if let Some(Some(da)) = self
            .terminal_device
            .map(|id| dr.get(DeviceGetQuery::Id(id)))
        {
            let prompt = format!("{} %", da.label);
            let label_size = d.measure_text(&prompt, FONT_SIZE);
            let max_text_size = d.measure_text("W", FONT_SIZE) * 30; // roughly 30 characters

            d.draw_text(&prompt, PADDING, out_y as i32, FONT_SIZE, Color::WHITE);

            if d.gui_text_box(
                Rectangle::new(
                    (label_size + PADDING) as f32,
                    out_y,
                    max_text_size as f32,
                    FONT_SIZE as f32,
                ),
                &mut self.terminal_buffer,
                true,
            ) && d.is_key_pressed(KeyboardKey::KEY_ENTER)
            {
                dr.set(
                    da.id,
                    DeviceSetQuery::TerminalInput(utils::array_to_string(&self.terminal_buffer)),
                );

                self.terminal_out.push_back(format!(
                    "{} {}",
                    prompt,
                    utils::array_to_string(&self.terminal_buffer)
                ));

                if self.terminal_out.len() > MAX_TERMINAL_LINES {
                    self.terminal_out.pop_front();
                }
                self.terminal_buffer = [0u8; 0xFF];
            }

            // Output
            self.terminal_out.extend(dr.get_terminal_out(da.id));

            let mut out_y = terminal_text_start_y;
            for line in self
                .terminal_out
                .iter()
                .rev()
                .take(MAX_TERMINAL_LINES)
                .rev()
            {
                d.draw_text(line, PADDING, out_y as i32, FONT_SIZE, Color::WHITE);
                out_y += (FONT_SIZE + PADDING / 2) as f32;
            }
        } else {
            let message = "Right click a device to open terminal";
            let message_size = d.measure_text(message, FONT_SIZE);
            d.draw_text(
                message,
                screen_width / 6 - message_size,
                terminal_y as i32 + (screen_height - terminal_y as i32) / 2,
                2 * FONT_SIZE,
                Color::GRAY,
            );
        }
        // -----------------------------------

        // Packet Tracer
        // -----------------------------------
        let table_y = (3.0 / 4.0) * screen_height as f32;
        let table_bounds = Rectangle {
            x: (screen_width / 3) as f32,
            y: table_y,
            width: (2.0 / 3.0) * screen_width as f32,
            height: screen_height as f32 - table_y,
        };

        for (
            id,
            ((incoming_device_id, incoming_packets), (outgoing_device_id, outgoing_packets)),
        ) in dr.sniff()
        {
            let time = {
                let tp = TimeProvider::instance().lock().unwrap();
                tp.now()
            };

            for packet in incoming_packets {
                let packet = Packet {
                    last: incoming_device_id,
                    current: id,
                    kind: packet,
                    time,
                };

                self.packet_buffer.push_back(packet);
            }

            // for packet in outgoing_packets {
            //     self.packet_buffer.push_back(Packet {
            //         last: incoming_device_id,
            //         current: id,
            //         kind: packet,
            //         time,
            //     });
            // }
        }

        let col_width = table_bounds.width / 6.0;

        const COLUMN_HEADERS: [&str; 4] = ["Time (ms)", "Last Device", "At Device", "Type"];

        d.gui_set_style(
            GuiControl::DEFAULT,
            GuiDefaultProperty::TEXT_SIZE as i32,
            2 * FONT_SIZE,
        );
        d.gui_set_style(
            GuiControl::LABEL,
            GuiControlProperty::TEXT_COLOR_NORMAL as i32,
            Color::WHITE.color_to_int(),
        );

        for (i, column) in COLUMN_HEADERS.iter().enumerate() {
            d.draw_text(
                column,
                (table_bounds.x + col_width * i as f32) as i32 + 10,
                table_bounds.y as i32 + FONT_SIZE / 2,
                2 * FONT_SIZE,
                Color::WHITE,
            );

            d.draw_line(
                (table_bounds.x + col_width * i as f32) as i32,
                table_bounds.y as i32,
                (table_bounds.x + col_width * i as f32) as i32,
                (table_bounds.y + table_bounds.height) as i32,
                Color::WHITE,
            );
        }

        let mut y = table_bounds.y + 4.0 * FONT_SIZE as f32;
        for packet in self.packet_buffer.iter().rev() {
            let last_device = packet.last.map_or("-----".to_string(), |id| {
                dr.get(DeviceGetQuery::Id(id)).unwrap().label
            });
            let at_device = dr.get(DeviceGetQuery::Id(packet.current)).unwrap().label;
            let packet_type = match packet.kind {
                PacketKind::Arp(_) => "ARP",
                PacketKind::Bpdu(_) => "BPDU",
                PacketKind::Rip(_) => "RIP",
                PacketKind::Icmp(_) => "ICMP",
            };

            let mut label_clicked = false;

            let mut bounds = Rectangle::new(table_bounds.x + 10.0, y, col_width, FONT_SIZE as f32);

            label_clicked |= d.gui_label_button(bounds, Some(rstr!("0")));
            bounds.x += col_width;

            label_clicked |=
                d.gui_label_button(bounds, Some(rstr_from_string(last_device).as_c_str()));
            bounds.x += col_width;

            label_clicked |=
                d.gui_label_button(bounds, Some(rstr_from_string(at_device).as_c_str()));
            bounds.x += col_width;

            label_clicked |= d.gui_label_button(
                bounds,
                Some(rstr_from_string(packet_type.into()).as_c_str()),
            );
            bounds.x += col_width;
            y += 2.0 * FONT_SIZE as f32;

            if label_clicked {
                self.packet_selected = Some(packet.clone());
            }
        }

        // -----------------------------------

        // Packet Detail
        // -----------------------------------
        d.draw_line(
            (table_bounds.x + 4.0 * col_width) as i32,
            table_bounds.y as i32,
            (table_bounds.x + 4.0 * col_width) as i32,
            (table_bounds.y + table_bounds.height) as i32,
            Color::WHITE,
        );

        // "Selected Packet Details" header
        d.draw_text(
            "Selected Packet Details",
            (table_bounds.x + 4.0 * col_width) as i32 + 10,
            table_bounds.y as i32 + FONT_SIZE / 2,
            2 * FONT_SIZE,
            Color::WHITE,
        );

        fn display_eth2_info(y: &mut i32, x: i32, eth: &Ethernet2Frame, d: &mut RaylibDrawHandle) {
            let source_address = mac_fmt!(eth.source_address);
            let destination_address = mac_fmt!(eth.destination_address);

            d.draw_text("Ethernet II", x, *y, FONT_SIZE, Color::WHITE);
            d.draw_line(
                x,
                *y + FONT_SIZE,
                x + d.measure_text("Ethernet II", FONT_SIZE),
                *y + FONT_SIZE,
                Color::WHITE,
            );
            *y += FONT_SIZE + PADDING / 2;

            d.draw_text(
                &format!("Source: {}", source_address),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("Destination: {}", destination_address),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("EtherType: {:?}", eth.ether_type),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );
            *y += FONT_SIZE;
        }

        fn display_eth802_3_info(
            y: &mut i32,
            x: i32,
            eth: &Ethernet802_3Frame,
            d: &mut RaylibDrawHandle,
        ) {
            let source_address = mac_fmt!(eth.source_address);
            let destination_address = mac_fmt!(eth.destination_address);

            d.draw_text("Ethernet 802.3", x, *y, FONT_SIZE, Color::WHITE);
            d.draw_line(
                x,
                *y + FONT_SIZE,
                x + d.measure_text("Ethernet 802.3", FONT_SIZE),
                *y + FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE + PADDING / 2;

            d.draw_text(
                &format!("Source: {}", source_address),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("Destination: {}", destination_address),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("Length: {}", eth.length),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("DSAP: {:02X}", eth.dsap),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("SSAP: {:02X}", eth.ssap),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("Control: {:02X}", eth.control),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;
        }

        fn display_ipv4_info(y: &mut i32, x: i32, ipv4: &Ipv4Frame, d: &mut RaylibDrawHandle) {
            d.draw_text("IPv4", x, *y, FONT_SIZE, Color::WHITE);
            d.draw_line(
                x,
                *y + FONT_SIZE,
                x + d.measure_text("IPv4", FONT_SIZE),
                *y + FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE + PADDING / 2;

            d.draw_text(
                format!("Destination: {}", ipv4_fmt!(ipv4.destination)).as_str(),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                format!("Source: {}", ipv4_fmt!(ipv4.source)).as_str(),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("Protocol: {:?}", ipv4.protocol),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;

            d.draw_text(
                &format!("TTL: {}", ipv4.ttl),
                x,
                *y,
                FONT_SIZE,
                Color::WHITE,
            );

            *y += FONT_SIZE;
        }

        if let Some(packet) = &self.packet_selected {
            let mut y = table_bounds.y as i32 + 4 * FONT_SIZE;
            let x = (table_bounds.x + 4.0 * col_width) as i32 + 10;
            match &packet.kind {
                PacketKind::Arp(eth) => {
                    display_eth2_info(&mut y, x, eth, d);

                    y += (1.5 * PADDING as f32) as i32;

                    let arp_frame = ArpFrame::from_bytes(eth.data.clone()).unwrap();

                    d.draw_text("ARP", x, y, FONT_SIZE, Color::WHITE);
                    d.draw_line(
                        x,
                        y + FONT_SIZE,
                        x + d.measure_text("ARP", FONT_SIZE),
                        y + FONT_SIZE,
                        Color::WHITE,
                    );

                    y += FONT_SIZE + PADDING / 2;

                    d.draw_text(
                        &format!("Operation: {:?}", arp_frame.opcode),
                        x,
                        y,
                        FONT_SIZE,
                        Color::WHITE,
                    );

                    y += FONT_SIZE;

                    d.draw_text(
                        &format!("Sender MAC: {}", mac_fmt!(arp_frame.sender_mac)),
                        x,
                        y,
                        FONT_SIZE,
                        Color::WHITE,
                    );

                    y += FONT_SIZE;

                    d.draw_text(
                        &format!("Sender IP: {}", ipv4_fmt!(arp_frame.sender_ip)),
                        x,
                        y,
                        FONT_SIZE,
                        Color::WHITE,
                    );

                    y += FONT_SIZE;

                    d.draw_text(
                        &format!("Target MAC: {}", mac_fmt!(arp_frame.target_mac)),
                        x,
                        y,
                        FONT_SIZE,
                        Color::WHITE,
                    );

                    y += FONT_SIZE;

                    d.draw_text(
                        &format!("Target IP: {}", ipv4_fmt!(arp_frame.target_ip)),
                        x,
                        y,
                        FONT_SIZE,
                        Color::WHITE,
                    );
                }
                PacketKind::Bpdu(eth) => {
                    display_eth802_3_info(&mut y, x, eth, d);
                }
                PacketKind::Rip(eth) => {
                    display_eth2_info(&mut y, x, eth, d);

                    y += (1.5 * PADDING as f32) as i32;

                    let ipv4_frame = Ipv4Frame::from_bytes(eth.data.clone()).unwrap();
                    display_ipv4_info(&mut y, x, &ipv4_frame, d);

                    // Switch to column 2
                    y = table_bounds.y as i32 + 4 * FONT_SIZE;
                    let x = (table_bounds.x + 5.0 * col_width) as i32 + 10;
                    let rip_frame = RipFrame::from_bytes(ipv4_frame.data.clone()).unwrap();

                    d.draw_text("RIP", x, y, FONT_SIZE, Color::WHITE);
                    d.draw_line(
                        x,
                        y + FONT_SIZE,
                        x + d.measure_text("RIP", FONT_SIZE),
                        y + FONT_SIZE,
                        Color::WHITE,
                    );

                    y += FONT_SIZE + PADDING / 2;

                    d.draw_text(
                        &format!("Command: {:?}", rip_frame.command),
                        x,
                        y,
                        FONT_SIZE,
                        Color::WHITE,
                    );

                    y += FONT_SIZE;

                    d.draw_text("Routes: TODO", x, y, FONT_SIZE, Color::WHITE); // TODO: Display routes
                }
                PacketKind::Icmp(eth) => {
                    display_eth2_info(&mut y, x, eth, d);

                    y += (1.5 * PADDING as f32) as i32;

                    let ipv4_frame = Ipv4Frame::from_bytes(eth.data.clone()).unwrap();
                    display_ipv4_info(&mut y, x, &ipv4_frame, d);

                    // Switch to column 2
                    y = table_bounds.y as i32 + 4 * FONT_SIZE;
                    let x = (table_bounds.x + 5.0 * col_width) as i32 + 10;
                    let icmp_frame = IcmpFrame::from_bytes(ipv4_frame.data.clone()).unwrap();

                    d.draw_text("ICMP", x, y, FONT_SIZE, Color::WHITE);
                    d.draw_line(
                        x,
                        y + FONT_SIZE,
                        x + d.measure_text("ICMP", FONT_SIZE),
                        y + FONT_SIZE,
                        Color::WHITE,
                    );

                    y += FONT_SIZE + PADDING / 2;

                    d.draw_text(
                        &format!("Type: {:?}", icmp_frame.icmp_type),
                        x,
                        y,
                        FONT_SIZE,
                        Color::WHITE,
                    );
                }
            }
        } else {
            let message = "Click a table row\n to view details";
            let message_size = d.measure_text(message, FONT_SIZE);
            d.draw_text(
                message,
                (table_bounds.x + 4.0 * col_width) as i32 + message_size,
                table_bounds.y as i32 + (table_bounds.height as i32) / 2,
                2 * FONT_SIZE,
                Color::GRAY,
            );
        }
        // -----------------------------------

        d.gui_load_style_default();
        self.gui_consume_this_click = true;
    }

    pub fn update(&mut self, rl: &RaylibHandle, dr: &mut DeviceRepository) {
        let is_left_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        let is_left_mouse_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        let is_right_mouse_clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_RIGHT);
        let mouse_pos = rl.get_mouse_position();

        // Edit Dropdown
        // -----------------------------------
        if is_right_mouse_clicked {
            self.edit_dropdown = dr
                .get(DeviceGetQuery::Pos(mouse_pos))
                .map(|da| Dropdown::new(da.id))
                .or(None);
        }

        if is_left_mouse_clicked
            && self
                .edit_dropdown
                .as_ref()
                .is_none_or(|d| !d.bounds.check_collision_point_rec(mouse_pos))
        {
            self.edit_dropdown = None;
        }
        // -----------------------------------

        // Drag Device
        // -----------------------------------
        if is_left_mouse_clicked && self.mode == Some(GuiMode::Drag) {
            self.mode = None;
            self.drag_device = None;
            return;
        }

        if is_left_mouse_down && self.mode == Some(GuiMode::Drag) {
            if let Some(device) = self.drag_device {
                dr.set(device, DeviceSetQuery::Pos(mouse_pos));
            } else {
                self.mode = None;
            }
            return;
        }

        if is_left_mouse_down && self.mode.is_none() && self.drag_device.is_none() {
            if let Some(da) = dr.get(DeviceGetQuery::Pos(mouse_pos)) {
                self.mode = Some(GuiMode::Drag);
                self.drag_device = Some(da.id);
            }
            return;
        }
        // -----------------------------------

        // Ethernet Connect
        // -----------------------------------
        if let (Some((d1_id, d1_port)), Some((d2_id, d2_port))) = (self.connect_d1, self.connect_d2)
        {
            dr.set(d1_id, DeviceSetQuery::Connect(d2_id, d1_port, d2_port));
            self.reset_states();
        }
        // -----------------------------------

        if !is_left_mouse_clicked
            || self
                .ethernet_dropdown
                .as_ref()
                .is_some_and(|d| d.bounds.check_collision_point_rec(mouse_pos))
            || self
                .edit_dropdown
                .as_ref()
                .is_some_and(|d| d.bounds.check_collision_point_rec(mouse_pos))
        {
            return;
        }

        self.gui_consume_this_click = false; // RayGUI does click logic in render. If a click is consumed here, we don't want the render to consume it again.

        // Ethernet Connection Mode
        // -----------------------------------
        if self.selection == Some(GuiButtonClickKind::Ethernet) {
            if let Some(da) = dr.get(DeviceGetQuery::Pos(mouse_pos)) {
                self.ethernet_dropdown = Some(Dropdown::new(da.id));
            } else {
                self.reset_states();
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
