use std::collections::HashMap;

use crate::data_link::{ethernet_frame::MacAddress, ethernet_interface::EthernetInterface};
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
            
            self.ethernet.send(*mac_address, bytes);
            return true;
        }

        // Send an ARP request to find the MAC address of the target IP address
        self.ethernet.send_arp(self.ip_address, destination);

        false
    }
}