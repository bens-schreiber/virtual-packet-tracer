use std::collections::HashMap;

use super::*;
use crate::ethernet::interface::EthernetInterface;
use crate::{ethernet::*, localhost};

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
struct WaitForArpResolveFrame {
    ip: Ipv4Address, // The address needed to resolve
    ttl: u8,
    retry: u8,
    frame: Ipv4Frame,
}

impl WaitForArpResolveFrame {
    fn new(ip: Ipv4Address, frame: Ipv4Frame) -> WaitForArpResolveFrame {
        WaitForArpResolveFrame {
            ip,
            ttl: 30,
            retry: 3,
            frame,
        } // 30 ticks ~ 1 second
    }
}

/// A layer 3 interface for IpV4 actions, sending and receiving Ipv4Frames through an EthernetInterface.
///
/// Contains an ARP table to map IP addresses to MAC addresses.
#[derive(Debug)]
pub struct Ipv4Interface {
    pub ethernet: EthernetInterface,
    pub ip_address: Ipv4Address,
    pub subnet_mask: Ipv4Address,
    pub default_gateway: Option<Ipv4Address>,
    arp_buf: Vec<WaitForArpResolveFrame>,
    arp_table: HashMap<Ipv4Address, MacAddress>,
}

impl Ipv4Interface {
    pub fn new(
        mac_address: MacAddress,
        ip_address: Ipv4Address,
        subnet_mask: Ipv4Address,
        default_gateway: Option<Ipv4Address>,
    ) -> Ipv4Interface {
        let arp_table = arp_table!(localhost!() => mac_address, ip_address => mac_address);
        let arp_buf = Vec::new();
        Ipv4Interface {
            ethernet: EthernetInterface::new(mac_address),
            ip_address,
            subnet_mask,
            default_gateway,
            arp_buf,
            arp_table,
        }
    }

    #[cfg(test)]
    pub fn from_arp_table(
        mac_address: MacAddress,
        ip_address: Ipv4Address,
        subnet_mask: Ipv4Address,
        default_gateway: Option<Ipv4Address>,
        mut arp_table: HashMap<Ipv4Address, MacAddress>,
    ) -> Ipv4Interface {
        arp_table.insert(localhost!(), mac_address);
        arp_table.insert(ip_address, mac_address);
        let arp_buf = Vec::new();
        Ipv4Interface {
            ethernet: EthernetInterface::new(mac_address),
            ip_address,
            subnet_mask,
            default_gateway,
            arp_buf,
            arp_table,
        }
    }

    /// Connects this interface to another interface.
    /// * `other` - The other interface to connect to.
    pub fn connect(&mut self, other: &mut Ipv4Interface) {
        self.ethernet.connect(&mut other.ethernet);
    }

    /// Mutually disconnects this interface from the ethernet interface.
    pub fn disconnect(&mut self) {
        self.ethernet.disconnect();
    }

    /// Attempts to send data to the destination IP address as an Ipv4Frame.
    /// * `destination` - The destination IP address to send the data to.
    /// * `data` - Byte data to send in the frame.
    /// * `source` - The source IP address to send the data from.
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
    /// True if the address was found in the ARP table and the frame was sent, false otherwise.
    pub fn sendv(&mut self, source: Ipv4Address, destination: Ipv4Address, data: Vec<u8>) -> bool {
        // Check if the destination is on the same subnet
        let subnets_match = destination == localhost!()
            || (network_address!(destination, self.subnet_mask)
                == network_address!(self.ip_address, self.subnet_mask));

        let table_key = if subnets_match {
            destination
        } else {
            if self.default_gateway.is_none() {
                return false;
            }
            self.default_gateway.unwrap()
        };

        let frame = Ipv4Frame::new(source, destination, data);
        if let Some(mac_address) = self.arp_table.get(&frame.destination) {
            self.ethernet
                .send(*mac_address, EtherType::Ipv4, frame.to_bytes());
            return true;
        }

        // Send an ARP request to find the MAC address of the target IP address
        // Buffer the frame to send after the ARP request is resolved
        self.arp_buf
            .push(WaitForArpResolveFrame::new(table_key, frame));
        self.ethernet.arp_request(self.ip_address, table_key);

        false
    }

    /// Attempts to send data to the destination IP address as an Ipv4Frame.
    /// * `destination` - The destination IP address to send the data to.
    /// * `data` - Byte data to send in the frame.
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
    /// True if the address was found in the ARP table and the frame was sent, false otherwise.
    pub fn send(&mut self, destination: Ipv4Address, data: Vec<u8>) -> bool {
        self.sendv(self.ip_address, destination, data)
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
                let ipv4_frame = match Ipv4Frame::from_bytes(f.data) {
                    Ok(ipv4_frame) => ipv4_frame,
                    Err(_) => continue, // Discard invalid Ipv4 frames
                };

                // Passive arp table filling: Update the ARP table with the sender's MAC address
                self.arp_table.insert(ipv4_frame.source, f.source_address);
                ipv4_frames.push(ipv4_frame);
                continue;
            }

            if f.ether_type == EtherType::Arp {
                let arp_frame = match ArpFrame::from_bytes(f.data) {
                    Ok(arp_frame) => arp_frame,
                    Err(_) => continue, // Discard invalid ARP frames
                };

                // Update the ARP table with the sender's MAC address
                self.arp_table
                    .insert(arp_frame.sender_ip, arp_frame.sender_mac);

                // Update the ARP table with the target's MAC address
                if arp_frame.opcode == ArpOperation::Reply {
                    self.arp_table
                        .insert(arp_frame.sender_ip, arp_frame.sender_mac);
                }
                // Send an ARP reply if we are the target
                else if arp_frame.target_ip == self.ip_address {
                    self.ethernet.arp_reply(
                        self.ip_address,
                        arp_frame.sender_mac,
                        arp_frame.sender_ip,
                    );
                }
            }
        }

        // Resolve ARP frames in the buffer
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

        ipv4_frames
    }
}
