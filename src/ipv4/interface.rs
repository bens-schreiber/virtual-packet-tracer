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
    pub subnet_mask: Ipv4Address,
    pub default_gateway: Option<Ipv4Address>,
    arp_table: HashMap<Ipv4Address, MacAddress>,
}

impl Ipv4Interface {
    pub fn new(
        mac_address: MacAddress,
        ip_address: Ipv4Address,
        subnet_mask: Ipv4Address,
        default_gateway: Option<Ipv4Address>,
    ) -> Ipv4Interface {
        Ipv4Interface {
            ethernet: EthernetInterface::new(mac_address),
            ip_address,
            subnet_mask,
            arp_table: HashMap::new(),
            default_gateway,
        }
    }

    /// Attempts to send data to the destination IP address as an Ipv4Frame.
    /// * `destination` - The destination IP address to send the data to.
    /// * `data` - Byte data to send in the frame.
    ///
    /// # Remarks
    /// Will send an ARP request if the key is not in the ARP table.
    ///
    /// The key will be either:
    /// 1. The destination IP address if the destination is on the same subnet.
    /// 2. The default gateway if the destination is on a different subnet.
    ///
    /// # Returns
    ///  * `true` - If the data was sent successfully.
    /// * `false` - If the data was not sent.
    pub fn send(&mut self, destination: Ipv4Address, data: Vec<u8>) -> bool {
        // Check if the destination is on the same subnet
        let subnets_match = {
            let mut destination_subnet = destination.clone();
            let mut source_subnet = self.ip_address.clone();
            for i in 0..4 {
                destination_subnet[i] = destination[i] & self.subnet_mask[i];
                source_subnet[i] = self.ip_address[i] & self.subnet_mask[i];
            }
            destination_subnet == source_subnet
        };

        let table_key = if subnets_match {
            destination
        } else {
            if self.default_gateway.is_none() {
                return false; // A gateway is required to send data to a different subnet
            }
            self.default_gateway.unwrap()
        };

        if let Some(mac_address) = self.arp_table.get(&table_key) {
            let bytes = Ipv4Frame::new(self.ip_address, destination, data).to_bytes();

            self.ethernet.send(*mac_address, EtherType::Ipv4, bytes);
            return true;
        }

        // Send an ARP request to find the MAC address of the target IP address
        self.ethernet.arp_request(self.ip_address, table_key);

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
