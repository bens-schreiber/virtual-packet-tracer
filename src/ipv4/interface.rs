use std::collections::HashMap;

use super::*;
use crate::ethernet::interface::EthernetInterface;
use crate::ethernet::*;

/// A layer 3 interface for IpV4 actions, sending and receiving Ipv4Frames through an EthernetInterface.
///
/// Contains an ARP table to map IP addresses to MAC addresses.
pub struct Ipv4Interface {
    pub ethernet: EthernetInterface,
    pub ip_address: Ipv4Address,
    arp_table: HashMap<Ipv4Address, MacAddress>,
}

impl Ipv4Interface {
    pub fn new(mac_address: MacAddress, ip_address: Ipv4Address) -> Ipv4Interface {
        Ipv4Interface {
            ethernet: EthernetInterface::new(mac_address),
            ip_address,
            arp_table: HashMap::new(),
        }
    }

    /// Attempts to send data to the destination IP address as an Ipv4Frame.
    ///
    /// If the MAC address of the destination IP address is not in the ARP table, an ARP request is sent.
    ///
    /// Returns true if the data was sent successfully.
    ///
    /// Returns false if the MAC address of the destination IP address is not in the ARP table.
    ///
    /// * `destination` - The destination IP address to send the data to.
    /// * `data` - Byte data to send in the frame.
    pub fn send(&mut self, destination: Ipv4Address, data: Vec<u8>) -> bool {
        if let Some(mac_address) = self.arp_table.get(&destination) {
            let bytes = Ipv4Frame::new(self.ip_address, destination, data).to_bytes();

            self.ethernet.send(*mac_address, EtherType::Ipv4, bytes);
            return true;
        }

        // Send an ARP request to find the MAC address of the target IP address
        self.ethernet.arp_request(self.ip_address, destination);

        false
    }

    /// Receives data from the ethernet interface. Processes ARP frames to the ARP table.
    /// Sends an ARP reply if this interface is the target.
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

        ipv4_frames
    }
}
