use std::{collections::HashMap, net::Ipv4Addr};

use crate::{
    network::{
        device::{desktop::Desktop, router::Router},
        ethernet::ByteSerialize,
        ipv4::{IcmpFrame, IcmpType},
    },
    tick::{TickTimer, Tickable},
};

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
enum TerminalCommand {
    Ping,
    EnableInterface,
}

// Desktop
//------------------------------------------------------

fn dhelp(t: &mut DesktopTerminal, _: &mut Desktop, _: &[&str]) {
    t.out_buf.push(String::from("Available commands:"));
    t.out_buf.push(String::from("ping <ip>"));
}

fn ping(t: &mut DesktopTerminal, d: &mut Desktop, args: &[&str]) {
    if args.len() != 1 {
        t.out_buf
            .push(String::from("err: ping requires 1 argument ip"));
        return;
    }

    match args[0].parse::<Ipv4Addr>() {
        Ok(ip) => {
            if d.interface
                .send_icmp(ip.octets(), IcmpType::EchoRequest)
                .is_err()
            {
                t.out_buf.push(String::from(
                    "err: failed to send ICMP: Destination unreachable",
                ));
                return;
            }
            t.channel_open = true;
            t.out_buf.push(String::from(format!("Pinging {}", ip)));
            t.channel_command = Some(TerminalCommand::Ping);
            t.timer.schedule(TerminalCommand::Ping, 3, false);
        }
        Err(_) => {
            t.out_buf.push(String::from("err: invalid ip"));
        }
    }
}

// TODO: this should most certainly be lazy
type DesktopTerminalCommand = fn(&mut DesktopTerminal, &mut Desktop, &[&str]) -> ();
fn desktop_terminal_dict() -> HashMap<String, DesktopTerminalCommand> {
    let mut dict = HashMap::new();
    dict.insert(String::from("ping"), ping as DesktopTerminalCommand);
    dict
}

pub struct DesktopTerminal {
    out_buf: Vec<String>, // Output buffer for terminal commands. Only read when channel is closed.
    pub channel_open: bool, // Channel is open when a command is processing, and awaiting some response (via `tick`)
    channel_command: Option<TerminalCommand>,
    timer: TickTimer<TerminalCommand>,
}

impl DesktopTerminal {
    pub fn new() -> DesktopTerminal {
        DesktopTerminal {
            out_buf: Vec::new(),
            channel_open: false,
            channel_command: None,
            timer: TickTimer::new(),
        }
    }

    /// Processes a command and puts the output in the output buffer.
    pub fn input(&mut self, input: String, desktop: &mut Desktop) {
        let tokenize = input.split_whitespace().collect::<Vec<&str>>();
        if tokenize.len() == 0 {
            return;
        }

        let command = tokenize[0];
        let args = &tokenize[1..];
        let dict = desktop_terminal_dict();
        match dict.get(command) {
            Some(func) => func(self, desktop, args),
            None => {
                self.out_buf.push(String::from("err: command not found"));
            }
        }
    }

    /// Returns the first output in the output buffer.
    pub fn out(&mut self) -> Option<String> {
        if self.out_buf.len() == 0 {
            return None;
        }

        Some(self.out_buf.remove(0))
    }

    /// When the terminal channel is open, intercepts frames before it reaches the desktop buffer.
    pub fn tick(&mut self, desktop: &mut Desktop) {
        if !self.channel_open {
            self.timer.tick();
            return;
        }

        for action in self.timer.ready() {
            match action {
                TerminalCommand::Ping => {
                    self.out_buf.push(String::from("Ping timeout!"));
                    self.channel_open = false;
                    self.channel_command = None;
                }
                _ => {}
            }
        }

        self.timer.tick();

        match self.channel_command {
            Some(TerminalCommand::Ping) => {
                // Manually tick a desktop device. Find an ICMP reply frame to close the channel.
                for frame in desktop.interface.receive() {
                    if frame.protocol == 1 {
                        let icmp = match IcmpFrame::from_bytes(frame.data) {
                            Ok(icmp) => icmp,
                            Err(_) => {
                                self.channel_open = false;
                                return;
                            }
                        };

                        if icmp.icmp_type == IcmpType::EchoReply as u8 {
                            self.out_buf.push(String::from("Pong!"));
                            self.channel_open = false;
                            return;
                        }
                    } else {
                        desktop.received.push(frame);
                    }
                }
            }
            _ => {
                self.channel_open = false;
            }
        }
    }
}
//------------------------------------------------------

// Router
//------------------------------------------------------

fn rhelp(t: &mut RouterTerminal, _: &mut Router, _: &[&str]) {
    t.out_buf.push(String::from("Available commands:"));
    t.out_buf.push(String::from("enable <port> <ip> <subnet>"));
}

fn enable_interface(t: &mut RouterTerminal, r: &mut Router, args: &[&str]) {
    if args.len() != 3 {
        t.out_buf
            .push(String::from("err: enable requires 3 arguments"));
        return;
    }

    match args[0].parse::<u8>() {
        Ok(port) => match args[1].parse::<Ipv4Addr>() {
            Ok(ip) => match args[2].parse::<Ipv4Addr>() {
                Ok(subnet) => {
                    t.out_buf
                        .push(String::from(format!("Port {} enabled.", port)));
                    r.enable_interface(port.into(), ip.octets(), subnet.octets());
                }
                Err(_) => {
                    t.out_buf.push(String::from("err: invalid subnet"));
                }
            },
            Err(_) => {
                t.out_buf.push(String::from("err: invalid ip"));
            }
        },
        Err(_) => {
            t.out_buf.push(String::from("err: invalid port"));
        }
    }
}

type RouterTerminalCommand = fn(&mut RouterTerminal, &mut Router, &[&str]) -> ();
fn router_terminal_dict() -> HashMap<String, RouterTerminalCommand> {
    let mut dict = HashMap::new();
    dict.insert(String::from("help"), rhelp as RouterTerminalCommand);
    dict.insert(
        String::from("enable"),
        enable_interface as RouterTerminalCommand,
    );
    dict
}

pub struct RouterTerminal {
    out_buf: Vec<String>, // Output buffer for terminal commands. Only read when channel is closed.
    pub channel_open: bool, // Channel is open when a command is processing, and awaiting some response (via `tick`)
    channel_command: Option<TerminalCommand>,
    timer: TickTimer<TerminalCommand>,
}

impl RouterTerminal {
    pub fn new() -> RouterTerminal {
        RouterTerminal {
            out_buf: Vec::new(),
            channel_open: false,
            channel_command: None,
            timer: TickTimer::new(),
        }
    }

    /// Processes a command and puts the output in the output buffer.
    pub fn input(&mut self, input: String, r: &mut Router) {
        let tokenize = input.split_whitespace().collect::<Vec<&str>>();
        if tokenize.len() == 0 {
            return;
        }

        let command = tokenize[0];
        let args = &tokenize[1..];
        let dict = router_terminal_dict();
        match dict.get(command) {
            Some(func) => func(self, r, args),
            None => {
                self.out_buf.push(String::from("err: command not found"));
            }
        }
    }

    /// Returns the first output in the output buffer.
    pub fn out(&mut self) -> Option<String> {
        if self.out_buf.len() == 0 {
            return None;
        }

        Some(self.out_buf.remove(0))
    }
}
