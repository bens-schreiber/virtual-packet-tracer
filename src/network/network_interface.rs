use std::collections::HashMap;

use crate::data_link::{arp_frame::{ArpFrame, ArpOperation}, ethernet_frame::{EtherType, MacAddress}, ethernet_interface::EthernetInterface};
use super::ipv4::{IPv4Address, IPv4Frame};


pub struct NetworkInterface {
    pub ethernet: EthernetInterface,
    ip_address: IPv4Address,
    arp_table: HashMap<IPv4Address, MacAddress>,
}

impl NetworkInterface {
    pub fn new(mac_address: MacAddress, ip_address: IPv4Address) -> NetworkInterface {
        NetworkInterface {
            ethernet: EthernetInterface::new(mac_address),
            ip_address,
            arp_table: HashMap::new()
        }
    }

    pub fn ip_address(&self) -> IPv4Address {
        self.ip_address
    }

    /// Attempts to send data to the destination IP address as an IPv4Frame.
    /// 
    /// If the MAC address of the destination IP address is not in the ARP table, an ARP request is sent.
    /// 
    /// Returns true if the data was sent successfully.
    /// 
    /// Returns false if the MAC address of the destination IP address is not in the ARP table.
    pub fn send(&mut self, destination: IPv4Address, data: Vec<u8>) -> bool {
        if let Some(mac_address) = self.arp_table.get(&destination) {

            let bytes = IPv4Frame::new(self.ip_address, destination, data).to_bytes();
            
            self.ethernet.send(*mac_address, EtherType::IPv4, bytes);
            return true;
        }

        // Send an ARP request to find the MAC address of the target IP address
        self.ethernet.send_arp_request( self.ip_address, destination);

        false
    }

    /// Receives data from the ethernet interface. Processes ARP frames to the ARP table.
    /// Sends an ARP reply if this interface is the target.
    pub fn receive(&mut self) -> Vec<IPv4Frame> {
        let mut ipv4_frames = Vec::new();
        let frames = self.ethernet.receive();

        for frame in frames {

            if frame.ether_type == EtherType::IPv4 {
                let ipv4_frame = match IPv4Frame::from_bytes(frame.data()) {
                    Ok(ipv4_frame) => ipv4_frame,
                    Err(_) => continue  // Discard invalid IPv4 frames
                };

                ipv4_frames.push(ipv4_frame);
                continue;
            }

            if frame.ether_type == EtherType::Arp {
                
                let arp_frame = match ArpFrame::from_bytes(frame.data()) {
                    Ok(arp_frame) => arp_frame,
                    Err(_) => continue  // Discard invalid ARP frames
                };

                // Update the ARP table with the sender's MAC address
                self.arp_table.insert(arp_frame.sender_ip, arp_frame.sender_mac);

                // Update the ARP table with the target's MAC address
                if arp_frame.opcode == ArpOperation::Reply {
                    self.arp_table.insert(arp_frame.target_ip, arp_frame.target_mac);
                }

                // Send an ARP reply if we are the target
                else if arp_frame.target_ip == self.ip_address {
                    self.ethernet.send_arp_reply(self.ip_address, arp_frame.sender_ip);
                }
            }
        }

        ipv4_frames
    }
}