#![allow(non_snake_case)]

use crate::{data_link::{arp_frame::{ArpFrame, ArpOperation}, ethernet_frame::*, ethernet_interface::*}, ether_payload, mac_addr, mac_broadcast_addr, network::{ipv4::Ipv4Frame, network_interface::NetworkInterface}, physical::packet_sim::PacketSimulator};

#[test]
fn NetworkInterface_SendToUnknownIpV4_ReceiveArpRequest() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = NetworkInterface::new(mac_addr!(1), [192, 168, 1, 1]);
    let mut interface2 = NetworkInterface::new(mac_addr!(2), [192, 168, 1, 2]);

    sim.add_port(interface1.ethernet.port());
    sim.add_port(interface2.ethernet.port());

    EthernetInterface::connect_port(&mut interface1.ethernet, &mut interface2.ethernet);

    // Act
    let result = interface1.send(interface2.ip_address(), ether_payload!(1));
    sim.tick();

    let received_data1 = interface1.ethernet.receive();
    let received_data2 = interface2.ethernet.receive();

    // Assert
    assert!(received_data1.is_empty());
    assert!(received_data2.len() == 1);
    assert!(!result);

    assert_eq!(received_data2[0], EthernetFrame::new(
        mac_broadcast_addr!(),
        interface1.ethernet.mac_address(),
        ArpFrame::new(
            ArpOperation::Request,
            interface1.ethernet.mac_address(),
            interface1.ip_address(),
            mac_addr!(0),
            interface2.ip_address()
        ).to_bytes(),
        EtherType::Arp
    ));

}

#[test]
fn NetworkInterface_SendToUnknownIpV4_ReceiveArpReply() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = NetworkInterface::new(mac_addr!(1), [192, 168, 1, 1]);
    let mut interface2 = NetworkInterface::new(mac_addr!(2), [192, 168, 1, 2]);

    sim.add_port(interface1.ethernet.port());
    sim.add_port(interface2.ethernet.port());

    EthernetInterface::connect_port(&mut interface1.ethernet, &mut interface2.ethernet);

    // Act
    interface1.send(interface2.ip_address(), ether_payload!(1));   // Fails, sends ARP request
    sim.tick();
    
    interface2.receive(); // Sends ARP reply

    sim.tick();

    let received_data = interface1.ethernet.receive();

    // Assert
    assert!(received_data.len() == 1);
    assert_eq!(received_data[0], EthernetFrame::new(
        mac_broadcast_addr!(),
        interface2.ethernet.mac_address(),
        ArpFrame::new(
            ArpOperation::Reply,
            interface2.ethernet.mac_address(),
            interface2.ip_address(),
            interface2.ethernet.mac_address(),
            interface1.ip_address()
        ).to_bytes(),
        EtherType::Arp
    ));

}

#[test]
fn NetworkInterface_SendUni_ReceivesIpv4Frame() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = NetworkInterface::new(mac_addr!(1), [192, 168, 1, 1]);
    let mut interface2 = NetworkInterface::new(mac_addr!(2), [192, 168, 1, 2]);

    sim.add_port(interface1.ethernet.port());
    sim.add_port(interface2.ethernet.port());

    EthernetInterface::connect_port(&mut interface1.ethernet, &mut interface2.ethernet);

    interface1.send(interface2.ip_address(), ether_payload!(1));   // Fails, sends ARP request
    sim.tick();
    interface2.receive(); // Sends ARP reply
    sim.tick();
    interface1.receive(); // Process ARP reply

    // Act
    let result = interface1.send(interface2.ip_address(), ether_payload!(1));  // Sends Ipv4 frame
    sim.tick();

    let received_data = interface2.receive();

    // Assert
    assert!(result);
    assert!(received_data.len() == 1);
    assert_eq!(received_data[0], Ipv4Frame::new(
        interface1.ip_address(),
        interface2.ip_address(),
        ether_payload!(1)
    ));


}