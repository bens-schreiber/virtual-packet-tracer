pub mod desktop {
    use crate::{
        network::{
            device::desktop::Desktop,
            ethernet::ByteSerializable,
            ipv4::{IcmpFrame, IcmpType},
        },
        tick::{TickTimer, Tickable},
    };
    use std::{collections::HashMap, net::Ipv4Addr};

    #[derive(Debug, PartialEq, Hash, Eq, Clone)]
    enum DesktopDelayedAction {
        Ping,
    }

    fn help(t: &mut DesktopTerminal, _: &mut Desktop, _: &[&str]) {
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
                t.channel_command = Some(DesktopDelayedAction::Ping);
                t.timer.schedule(DesktopDelayedAction::Ping, 3, false);
            }
            Err(_) => {
                t.out_buf.push(String::from("err: invalid ip"));
            }
        }
    }

    // TODO: this should be static or lazy
    type DesktopTerminalCommand = fn(&mut DesktopTerminal, &mut Desktop, &[&str]) -> ();
    fn desktop_terminal_dict() -> HashMap<String, DesktopTerminalCommand> {
        let mut dict = HashMap::new();
        dict.insert(String::from("help"), help as DesktopTerminalCommand);
        dict.insert(String::from("ping"), ping as DesktopTerminalCommand);
        dict
    }

    pub struct DesktopTerminal {
        out_buf: Vec<String>, // Output buffer for terminal commands. Only read when channel is closed.
        pub channel_open: bool, // Channel is open when a command is processing, and awaiting some response (via `tick`)
        channel_command: Option<DesktopDelayedAction>,
        timer: TickTimer<DesktopDelayedAction>,
    }

    impl Default for DesktopTerminal {
        fn default() -> Self {
            DesktopTerminal {
                out_buf: Vec::new(),
                channel_open: false,
                channel_command: None,
                timer: TickTimer::default(),
            }
        }
    }

    impl DesktopTerminal {
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
                    DesktopDelayedAction::Ping => {
                        self.out_buf.push(String::from("Ping timeout!"));
                        self.channel_open = false;
                        self.channel_command = None;
                    }
                }
            }

            self.timer.tick();

            match self.channel_command {
                Some(DesktopDelayedAction::Ping) => {
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
}

pub mod switch {
    use crate::network::device::switch::Switch;
    use std::collections::HashMap;

    fn help(t: &mut SwitchTerminal, _: &mut Switch, _: &[&str]) {
        t.out_buf.push(String::from("Available commands:"));
        t.out_buf.push(String::from("stp <priority>"))
    }

    // stp <priority>
    fn stp_init(t: &mut SwitchTerminal, s: &mut Switch, args: &[&str]) {
        if args.len() != 1 {
            t.out_buf
                .push(String::from("err: stp requires 1 argument priority"));
            return;
        }

        match args[0].parse::<u16>() {
            Ok(priority) => {
                s.set_bridge_priority(priority);
                s.init_stp();
                t.out_buf.push(String::from("STP initialized"));
            }
            Err(_) => {
                t.out_buf.push(String::from("err: invalid priority"));
            }
        }
    }

    type SwitchTerminalCommand = fn(&mut SwitchTerminal, &mut Switch, &[&str]) -> ();
    fn switch_terminal_dict() -> HashMap<String, SwitchTerminalCommand> {
        let mut dict = HashMap::new();
        dict.insert(String::from("help"), help as SwitchTerminalCommand);
        dict.insert(String::from("stp"), stp_init as SwitchTerminalCommand);
        dict
    }

    #[derive(Default)]
    pub struct SwitchTerminal {
        out_buf: Vec<String>, // Output buffer for terminal commands. Only read when channel is closed.
        pub channel_open: bool, // Channel is open when a command is processing, and awaiting some response (via `tick`)
    }

    impl SwitchTerminal {
        /// Processes a command and puts the output in the output buffer.
        pub fn input(&mut self, input: String, s: &mut Switch) {
            let tokenize = input.split_whitespace().collect::<Vec<&str>>();
            if tokenize.len() == 0 {
                return;
            }

            let command = tokenize[0];
            let args = &tokenize[1..];
            let dict = switch_terminal_dict();
            match dict.get(command) {
                Some(func) => func(self, s, args),
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
}

pub mod router {
    use std::{collections::HashMap, net::Ipv4Addr};

    use crate::network::device::router::Router;

    fn help(t: &mut RouterTerminal, _: &mut Router, _: &[&str]) {
        t.out_buf.push(String::from("Available commands:"));
        t.out_buf.push(String::from("enable <port> <ip> <subnet>"));
        t.out_buf.push(String::from("rip <port>"));
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

    fn enable_rip(t: &mut RouterTerminal, r: &mut Router, args: &[&str]) {
        if args.len() != 1 {
            t.out_buf
                .push(String::from("err: enable requires 1 argument"));
            return;
        }

        match args[0].parse::<u8>() {
            Ok(port) => {
                let res = r.enable_rip(port.into());
                if res.is_err() {
                    t.out_buf
                        .push(String::from(format!("err: {}", res.err().unwrap())));
                    return;
                };

                t.out_buf
                    .push(String::from(format!("RIP enabled on port {}", port)));
            }
            Err(_) => {
                t.out_buf.push(String::from("err: invalid port"));
            }
        }
    }

    type RouterTerminalCommand = fn(&mut RouterTerminal, &mut Router, &[&str]) -> ();
    fn router_terminal_dict() -> HashMap<String, RouterTerminalCommand> {
        let mut dict = HashMap::new();
        dict.insert(String::from("help"), help as RouterTerminalCommand);
        dict.insert(
            String::from("enable"),
            enable_interface as RouterTerminalCommand,
        );
        dict.insert(String::from("rip"), enable_rip as RouterTerminalCommand);
        dict
    }

    #[derive(Default)]
    pub struct RouterTerminal {
        out_buf: Vec<String>, // Output buffer for terminal commands. Only read when channel is closed.
        pub channel_open: bool, // Channel is open when a command is processing, and awaiting some response (via `tick`)
    }

    impl RouterTerminal {
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
}
