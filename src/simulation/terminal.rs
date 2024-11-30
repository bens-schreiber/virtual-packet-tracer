use std::{collections::HashMap, net::Ipv4Addr};

use crate::network::{
    device::desktop::Desktop,
    ethernet::ByteSerialize,
    ipv4::{IcmpFrame, IcmpType},
};

use super::tick::{TickTimer, Tickable};

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
enum TerminalCommand {
    Ping,
}

fn ping(t: &mut DesktopTerminal, d: &mut Desktop, args: &[&str]) {
    if args.len() != 1 {
        t.out_buf
            .push(String::from("err: ping requires 1 argument ip"));
        return;
    }

    match args[0].parse::<Ipv4Addr>() {
        Ok(ip) => {
            t.channel_open = true;
            t.out_buf.push(String::from(format!("Pinging {}", ip)));
            t.channel_command = Some(TerminalCommand::Ping);
            t.timer.schedule(TerminalCommand::Ping, 3, false);
            d.interface.send_icmp(ip.octets(), IcmpType::EchoRequest);
        }
        Err(_) => {
            t.out_buf.push(String::from("err: invalid ip"));
        }
    }
}

// TODO: this should most certainly be lazy, but my rust is limited atm
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
            }
        }

        self.timer.tick();

        match self.channel_command {
            Some(TerminalCommand::Ping) => {
                // Manually tick a desktop device. Find an ICMP reply frame to close the channel.
                for frame in desktop.interface.receive() {
                    print!("{:?}\n", frame);
                    if frame.protocol == 1 {
                        let icmp = match IcmpFrame::from_bytes(frame.data) {
                            Ok(icmp) => icmp,
                            Err(_) => {
                                print!("err: parse icmp frame\n");
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
            None => {
                self.channel_open = false;
            }
        }
    }
}
