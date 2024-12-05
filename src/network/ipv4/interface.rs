use std::collections::HashMap;

use super::*;
use crate::localhost;
use crate::network::ethernet::interface::EthernetInterface;
use crate::network::ethernet::*;

macro_rules! ipv4_multicast_addr {
    () => {
        [224, 0, 0, 0]
    };
}

macro_rules! mac_multicast_addr {
    () => {
        [0x01, 0x00, 0x5e, 0x00, 0x00, 0x00]
    };
}

/// Arp table from a list of key-value pairs.
#[macro_export]
macro_rules! arp_table {
    ($($key:expr => $value:expr),*) => {
        {
            let mut map = std::collections::HashMap::new();
            $(
                map.insert($key, $value);
            )*
            map
        }
    };
}

/// subnet & mask => network address
#[macro_export]
macro_rules! network_address {
    ($subnet:expr, $mask:expr) => {{
        let mut res = [0; 4];
        for i in 0..4 {
            res[i] = $subnet[i] & $mask[i];
        }
        res
    }};
}

#[derive(Debug)]
struct WaitForArpResolve {
    ip: Ipv4Address, // The address needed to resolve
    ttl: u8,         // ticks to live ; TODO: Use a tick timer here instead
    retry: u8,
    frame: Ipv4Frame,
}

impl WaitForArpResolve {
    fn new(ip: Ipv4Address, frame: Ipv4Frame) -> Self {
        Self {
            ip,
            ttl: 30, // roughly 1 second in a 30 tick per second simulation
            retry: 3,
            frame,
        }
    }
}

/// A layer 3 interface for Ipv4 actions, sending and receiving Ipv4Frames through an EthernetInterface.
///
/// Contains an ARP table to map IP addresses to MAC addresses.
#[derive(Debug)]
pub struct Ipv4Interface {
    pub ethernet: EthernetInterface,
    pub ip_address: Ipv4Address,
    pub subnet_mask: Ipv4Address,
    pub default_gateway: Option<Ipv4Address>,
    arp_buf: Vec<WaitForArpResolve>,
    arp_table: HashMap<Ipv4Address, MacAddress>,
    router_interface: bool,
}

impl Ipv4Interface {
    pub fn new(
        mac_address: MacAddress,
        ip_address: Ipv4Address,
        subnet_mask: Ipv4Address,
        default_gateway: Option<Ipv4Address>,
    ) -> Self {
        let arp_table = HashMap::new();
        let arp_buf = Vec::new();
        Self {
            ethernet: EthernetInterface::new(mac_address),
            ip_address,
            subnet_mask,
            default_gateway,
            arp_buf,
            arp_table,
            router_interface: false,
        }
    }

    /// Disables the default gateway, rerouting to itself.
    pub fn new_router_interface(
        mac_address: MacAddress,
        ip_address: Ipv4Address,
        subnet_mask: Ipv4Address,
    ) -> Self {
        Self {
            router_interface: true,
            ..Ipv4Interface::new(mac_address, ip_address, subnet_mask, None)
        }
    }

    #[cfg(test)]
    pub fn from_arp_table(
        mac_address: MacAddress,
        ip_address: Ipv4Address,
        subnet_mask: Ipv4Address,
        default_gateway: Option<Ipv4Address>,
        arp_table: HashMap<Ipv4Address, MacAddress>,
    ) -> Self {
        let arp_buf = Vec::new();
        Self {
            ethernet: EthernetInterface::new(mac_address),
            ip_address,
            subnet_mask,
            default_gateway,
            arp_buf,
            arp_table,
            router_interface: false,
        }
    }

    pub fn connect(&mut self, other: &mut Ipv4Interface) {
        self.ethernet.connect(&mut other.ethernet);
    }

    pub fn disconnect(&mut self) {
        self.ethernet.disconnect();
    }

    /// Attempts to send data to the destination IP address as an Ipv4Frame.
    /// * `source` - The source IP address to send the data from.
    /// * `destination` - The destination IP address to send the data to.
    /// * `proxied_destination` - The destination that the mac address will be resolved to, if different from the destination.
    /// * `ttl` - Time to live of the frame.
    /// * `data` - Byte data to send in the frame.
    /// * `protocol` - The Ipv4 protocol of the frame.
    ///
    /// # Remarks
    /// Will send an ARP request if the destination MAC address is not in the ARP table.
    /// The original packet is placed in a buffer to send after the ARP request is resolved within the next 30 ticks
    ///
    /// The key will be either:
    /// 1. The destination IP address if the destination is on the same subnet.
    /// 2. The default gateway if the destination is on a different subnet.
    ///
    /// Proxy destination is used for a router sending to a different subnet. Routers could know a destination is reachable via its
    /// routing table, and need to send a message destined for a different subnet to some interface. Normally, sending to a different subnet
    /// would require the default gateway, but a router port does not have a default gateway. Instead, proxied_destination is used to override the
    /// default gateway, resolving to the correct MAC address of the interface the router knows the destination is reachable through.
    ///
    /// # Returns
    /// True if the address was found in the ARP table and the frame was sent, false if buffering the frame.
    /// Err if the destination is unreachable (no default gateway).
    pub fn sendv(
        &mut self,
        source: Ipv4Address,
        destination: Ipv4Address,
        proxied_destination: Option<Ipv4Address>,
        ttl: u8,
        data: Vec<u8>,
        protocol: Ipv4Protocol,
    ) -> Result<bool, &'static str> {
        if self._ip_is_self(destination) {
            let frame = Ipv4Frame::new(source, destination, ttl, data, protocol);
            self.ethernet
                .send(self.ethernet.mac_address, EtherType::Ipv4, frame.to_bytes());
            return Ok(true);
        }

        let arp_key = if self._subnets_match(destination) {
            Some(destination)
        } else {
            proxied_destination.or(self.default_gateway)
        };

        let frame = Ipv4Frame::new(source, destination, ttl, data, protocol);

        if arp_key.is_none() {
            if self.router_interface {
                // No default gateway, but we are a router interface, so we can send to ourselves
                // for the router to check the routing table.
                self.ethernet
                    .send(self.ethernet.mac_address, EtherType::Ipv4, frame.to_bytes());
                return Ok(true);
            }
            return Err("Destination is unreachable. No default gateway.");
        }

        if let Some(mac_address) = self.arp_table.get(&arp_key.unwrap()) {
            self.ethernet
                .send(*mac_address, EtherType::Ipv4, frame.to_bytes());
            return Ok(true);
        }

        // Send an ARP request to find the MAC address of the target IP address
        // Buffer the frame to send after the ARP request is resolved
        self.arp_buf
            .push(WaitForArpResolve::new(arp_key.unwrap(), frame));
        self.ethernet.arp_request(self.ip_address, arp_key.unwrap());

        Ok(false)
    }

    /// Attempts to send data to the destination IP address as an Ipv4Frame.
    /// Defaults to a Ipv4 TTL of 64.
    /// * `destination` - The destination IP address to send the data to.
    /// * `data` - Byte data to send in the frame.
    /// * `protocol` - The Ipv4 protocol of the frame.
    ///
    /// # Remarks
    /// Will send an ARP request if the destination MAC address is not in the ARP table.
    /// The original packet will be sent if the ARP request is successful in the next 30 tickets (~1 second in full simulation).
    ///
    /// The key will be either:
    /// 1. The destination IP address if the destination is on the same subnet.
    /// 2. The default gateway if the destination is on a different subnet.
    ///
    /// # Returns
    /// True if the address was found in the ARP table and the frame was sent, false if buffering the frame.
    /// Err if the destination is unreachable (no default gateway).
    pub fn send(
        &mut self,
        destination: Ipv4Address,
        data: Vec<u8>,
        protocol: Ipv4Protocol,
    ) -> Result<bool, &'static str> {
        self.sendv(self.ip_address, destination, None, 64, data, protocol)
    }

    #[cfg(test)]
    pub fn send_t(&mut self, destination: Ipv4Address, data: u8) {
        self.send(destination, vec![data], Ipv4Protocol::Test)
            .unwrap();
    }

    /// Sends an ICMP frame to the destination IP address.
    /// * `destination` - The destination IP address to send the ICMP frame to.
    /// * `kind` - The type of ICMP frame to send.
    pub fn send_icmp(
        &mut self,
        destination: Ipv4Address,
        kind: IcmpType,
    ) -> Result<bool, &'static str> {
        let mut k = kind;
        if self._ip_is_self(destination) {
            k = IcmpType::EchoReply; // Ping self? Reply.
        }

        let icmp_frame = match k {
            IcmpType::EchoRequest => IcmpFrame::echo_request(0, 0, vec![]),
            IcmpType::EchoReply => IcmpFrame::echo_reply(0, 0, vec![]),
            IcmpType::Unreachable => IcmpFrame::destination_unreachable(0, vec![]),
        };

        self.sendv(
            self.ip_address,
            destination,
            None,
            64,
            icmp_frame.to_bytes(),
            Ipv4Protocol::Icmp,
        )
    }

    /// Sends data to the multicast address.
    pub fn multicast(&mut self, data: Vec<u8>, protocol: Ipv4Protocol) {
        let frame = Ipv4Frame::new(
            self.ip_address,
            ipv4_multicast_addr!(),
            64,
            data.clone(),
            protocol,
        );
        self.ethernet
            .send(mac_multicast_addr!(), EtherType::Ipv4, frame.to_bytes());
    }

    /// Receives data from the ethernet interface. Processes ARP frames to the ARP table.
    ///
    /// Sends an ARP reply if this interface is the target.
    ///
    /// Resolves ARP frames in the buffer.
    ///
    /// # Returns
    /// A vector of Ipv4Frames received from the ethernet interface.
    pub fn receive(&mut self) -> Vec<Ipv4Frame> {
        let mut ipv4_frames = Vec::new();
        let frames = self.ethernet.receive();

        for frame in frames {
            let f = match frame {
                EthernetFrame::Ethernet2(frame) => frame,
                _ => continue, // Discard non-Ethernet2 frames
            };

            if f.ether_type == EtherType::Ipv4 {
                if let Ok(ipv4_frame) = Ipv4Frame::from_bytes(f.data) {
                    self._receive_ipv4(ipv4_frame, f.source_address, &mut ipv4_frames);
                }
                continue;
            }

            if f.ether_type == EtherType::Arp {
                if let Ok(arp_frame) = ArpFrame::from_bytes(f.data) {
                    self._receive_arp(arp_frame);
                }
            }
        }

        self._process_arp_buf();

        ipv4_frames
    }

    fn _ip_is_self(&self, ip: Ipv4Address) -> bool {
        ip == self.ip_address || ip == localhost!()
    }

    fn _add_arp_entry(&mut self, ip: Ipv4Address, mac: MacAddress) {
        if self._ip_is_self(ip) {
            return; // Don't add self to ARP table.
        }
        self.arp_table.insert(ip, mac);
    }

    fn _subnets_match(&self, destination: Ipv4Address) -> bool {
        self._ip_is_self(destination)
            || (network_address!(destination, self.subnet_mask)
                == network_address!(self.ip_address, self.subnet_mask))
    }

    fn _receive_ipv4(
        &mut self,
        frame: Ipv4Frame,
        source_mac: MacAddress,
        ipv4_frames: &mut Vec<Ipv4Frame>,
    ) {
        self._add_arp_entry(frame.source, source_mac);

        // On ICMP echo request, reply with an echo reply if we are the intended target. Don't reply to self.
        if frame.destination == self.ip_address
            && frame.source != self.ip_address
            && frame.protocol == 1
        {
            if let Ok(icmp_frame) = IcmpFrame::from_bytes(frame.data.clone()) {
                if icmp_frame.icmp_type == 8 {
                    let _ = self.send_icmp(frame.source, IcmpType::EchoReply);
                    return;
                }
            }
        }
        ipv4_frames.push(frame);
    }

    fn _receive_arp(&mut self, frame: ArpFrame) {
        self._add_arp_entry(frame.sender_ip, frame.sender_mac);

        // Update the ARP table with the target's MAC address
        if frame.opcode == ArpOperation::Reply {
            self._add_arp_entry(frame.target_ip, frame.target_mac);
            return;
        }

        let destination_mac: Option<MacAddress> = {
            if frame.target_ip == self.ip_address {
                Some(frame.sender_mac)
            } else {
                self.arp_table.get(&frame.target_ip).copied()
            }
        };

        // Reply if this interface has the value
        if let Some(destination_mac) = destination_mac {
            self.ethernet
                .arp_reply(self.ip_address, destination_mac, frame.sender_ip);
        }
    }

    fn _process_arp_buf(&mut self) {
        for i in 0..self.arp_buf.len() {
            let w = &mut self.arp_buf[i];

            w.ttl = w.ttl.saturating_sub(1);
            if w.ttl == 0 && w.retry == 0 {
                continue;
            }

            if w.ttl == 0 && w.retry > 0 {
                w.retry = w.retry.saturating_sub(1);
                w.ttl = 30;

                // Retry ARP request
                self.ethernet
                    .arp_request(self.ip_address, w.frame.destination);
            }

            if let Some(mac_address) = self.arp_table.get(&w.ip) {
                self.ethernet
                    .send(*mac_address, EtherType::Ipv4, w.frame.to_bytes());
                w.retry = 0;
                w.ttl = 0;
                continue;
            }
        }

        // Pop resolved ARP frames
        while self
            .arp_buf
            .last()
            .is_some_and(|w| w.ttl <= 0 && w.retry <= 0)
        {
            self.arp_buf.pop();
        }
    }
}
