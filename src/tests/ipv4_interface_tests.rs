#![allow(non_snake_case)]

use crate::device::cable::CableSimulator;
use crate::ethernet::ByteSerialize;
use crate::ethernet::{interface::*, EtherType};
use crate::ipv4::interface::*;
use crate::ipv4::*;
use crate::{eth2, eth2_data, mac_addr, mac_broadcast_addr};

#[test]
fn Send_UnknownIpV4_ReceiveArpRequest() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1]);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2]);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);

    EthernetInterface::connect(&mut i1.ethernet, &mut i2.ethernet);

    // Act
    let i1_sent = i1.send(i2.ip_address, eth2_data!(1));
    sim.tick();

    let i1_data = i1.ethernet.receive();
    let i2_data = i2.ethernet.receive();

    // Assert
    assert!(i1_data.is_empty());
    assert!(i2_data.len() == 1);
    assert!(!i1_sent);

    assert_eq!(
        i2_data[0],
        eth2!(
            mac_broadcast_addr!(),
            i1.ethernet.mac_address,
            ArpFrame::new(
                ArpOperation::Request,
                i1.ethernet.mac_address,
                i1.ip_address,
                mac_addr!(0),
                i2.ip_address
            )
            .to_bytes(),
            EtherType::Arp
        )
    );
}

#[test]
fn Send_UnknownIpV4_ReceiveArpReply() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1]);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2]);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);

    EthernetInterface::connect(&mut i1.ethernet, &mut i2.ethernet);

    // Act
    i1.send(i2.ip_address, eth2_data!(1)); // Fails, sends ARP request
    sim.tick();

    i2.receive(); // Sends ARP reply

    sim.tick();

    let i1_data = i1.ethernet.receive();

    // Assert
    assert!(i1_data.len() == 1);
    assert_eq!(
        i1_data[0],
        eth2!(
            i1.ethernet.mac_address,
            i2.ethernet.mac_address,
            ArpFrame::new(
                ArpOperation::Reply,
                i2.ethernet.mac_address,
                i2.ip_address,
                i1.ethernet.mac_address,
                i1.ip_address
            )
            .to_bytes(),
            EtherType::Arp
        )
    );
}

#[test]
fn Send_Uni_ReceivesIpv4Frame() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1]);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2]);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);

    EthernetInterface::connect(&mut i1.ethernet, &mut i2.ethernet);

    i1.send(i2.ip_address, eth2_data!(1)); // Fails, sends ARP request
    sim.tick();
    i2.receive(); // Sends ARP reply
    sim.tick();
    i1.receive(); // Process ARP reply

    // Act
    let i1_sent = i1.send(i2.ip_address, eth2_data!(1)); // Sends Ipv4 frame
    sim.tick();

    let i2_data = i2.receive();

    // Assert
    assert!(i1_sent);
    assert!(i2_data.len() == 1);
    assert_eq!(
        i2_data[0],
        Ipv4Frame::new(i1.ip_address, i2.ip_address, eth2_data!(1))
    );
}

#[test]
fn Arp_TwoInterfaces_BothInterfacesFillArpTable() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1]);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2]);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);

    EthernetInterface::connect(&mut i1.ethernet, &mut i2.ethernet);

    i1.send(i2.ip_address, eth2_data!(1)); // Fails, sends ARP request
    sim.tick();
    i2.receive(); // Sends ARP reply
    sim.tick();
    i1.receive(); // Process ARP reply

    // Act
    let i1_sent = i1.send(i2.ip_address, eth2_data!(1)); // Sends Ipv4 frame
    let i2_sent = i2.send(i1.ip_address, eth2_data!(2)); // Sends Ipv4 frame

    // Assert
    assert!(i1_sent);
    assert!(i2_sent);
}
