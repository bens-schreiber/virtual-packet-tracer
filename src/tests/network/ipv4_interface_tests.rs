#![allow(non_snake_case)]

use crate::network::device::cable::CableSimulator;
use crate::network::ethernet::ByteSerializable;
use crate::network::ethernet::{interface::*, EtherType};
use crate::network::ipv4::interface::*;
use crate::{arp_table, network::ipv4::*};
use crate::{eth2, eth2_data, mac_addr, mac_broadcast_addr};

fn same_subnet_filled_arp_tables() -> (CableSimulator, Ipv4Interface, Ipv4Interface) {
    let mut sim = CableSimulator::default();

    let i1_ip = [192, 168, 1, 1];
    let i1_mac_addr = mac_addr!(1);

    let i2_ip = [192, 168, 1, 2];
    let i2_mac_addr = mac_addr!(2);

    let mut i1 = Ipv4Interface::from_arp_table(
        i1_mac_addr,
        i1_ip,
        [255, 255, 255, 0],
        None,
        arp_table!(i2_ip => i2_mac_addr),
    );

    let mut i2 = Ipv4Interface::from_arp_table(
        i2_mac_addr,
        i2_ip,
        [255, 255, 255, 0],
        None,
        arp_table!(i1_ip => i1_mac_addr),
    );

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);

    EthernetInterface::connect(&mut i1.ethernet, &mut i2.ethernet);

    (sim, i1, i2)
}

#[test]
fn Ipv4_EncapsulateIcmpFrame_GetOriginalFrameAfterSerialization() {
    // Arrange
    let frame = IcmpFrame::echo_request(0, 0, vec![1, 2, 3]);
    let ipv4_frame = Ipv4Frame::new(
        [192, 168, 1, 1],
        [192, 168, 1, 2],
        64,
        frame.to_bytes(),
        Ipv4Protocol::Icmp,
    );

    // Act
    let serialized = ipv4_frame.to_bytes();
    let deserialized = Ipv4Frame::from_bytes(serialized).unwrap();

    // Assert
    assert_eq!(frame, IcmpFrame::from_bytes(deserialized.data).unwrap());
}

#[test]
fn Send_UnknownIpV4_ReceiveArpRequest() {
    // Arrange
    let mut sim = CableSimulator::default();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1], [255, 255, 255, 0], None);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2], [255, 255, 255, 0], None);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    i1.connect(&mut i2);

    // Act
    let i1_sent_arp = i1.send(i2.ip_address, vec![1], Ipv4Protocol::Test) == Ok(false);
    sim.transmit();

    let i1_data = i1.ethernet.receive();
    let i2_data = i2.ethernet.receive();

    // Assert
    assert!(i1_sent_arp);
    assert!(i1_data.is_empty());
    assert_eq!(i2_data.len(), 1);

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
    let mut sim = CableSimulator::default();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1], [255, 255, 255, 0], None);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2], [255, 255, 255, 0], None);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    i1.connect(&mut i2);

    // Act
    i1.send_t(i2.ip_address, 1); // Fails, sends ARP request
    sim.transmit();

    i2.receive(); // Sends ARP reply

    sim.transmit();

    let i1_data = i1.ethernet.receive();

    // Assert
    assert_eq!(i1_data.len(), 1);
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
fn Send_UnknownIpV4AfterMultipleRetries_ReceiveMultipleArpRequests() {
    // Arrange
    let mut sim = CableSimulator::default();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1], [255, 255, 255, 0], None);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2], [255, 255, 255, 0], None);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    i1.connect(&mut i2);

    // Act
    i1.send_t(i2.ip_address, 1); // Fails, places in buffer
    sim.transmit();

    for _ in 0..90 {
        i1.receive();
        sim.transmit(); // 30 ticks to retry
    }

    let i2_data = i2.ethernet.receive();

    // Assert
    assert_eq!(i2_data.len(), 4); // 1 + (90 / 30) = 4
}

#[test]
fn Send_SameSubnet_ReceivesIpv4Frame() {
    // Arrange
    let (mut sim, mut i1, mut i2) = same_subnet_filled_arp_tables();

    // Act
    let i1_sent = i1.send(i2.ip_address, vec![1], Ipv4Protocol::Test); // Sends Ipv4 frame
    sim.transmit();

    let i2_data = i2.receive();

    // Assert
    assert!(i1_sent.is_ok_and(|b| b));
    assert_eq!(i2_data.len(), 1);
    assert_eq!(
        i2_data[0],
        Ipv4Frame::test(i1.ip_address, i2.ip_address, 64, 1)
    );
}

#[test]
fn Send_UnknownIpv4AfterMultipleRetries_ReturnsOriginalRequest() {
    // Arrange
    let mut sim = CableSimulator::default();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1], [255, 255, 255, 0], None);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2], [255, 255, 255, 0], None);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    i1.connect(&mut i2);

    // Act
    i1.send_t(i2.ip_address, 1); // Fails, places in buffer
    sim.transmit();

    for _ in 0..60 {
        i1.receive();
        sim.transmit(); // 30 ticks to retry
    }

    i2.receive(); // Sends ARP reply(s)

    sim.transmit();
    i1.receive(); // Receives arp reply, now table is filled, sends original request
    sim.transmit();

    let i2_data = i2.receive();

    // Assert
    assert_eq!(i2_data.len(), 1);
    assert_eq!(
        i2_data[0],
        Ipv4Frame::test(i1.ip_address, i2.ip_address, 64, 1)
    );
}

#[test]
fn Send_DifferentSubnet_SendsToDefaultGateway() {
    // Arrange
    let mut sim = CableSimulator::default();

    let mut default_gateway =
        Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 254], [255, 255, 255, 0], None);
    let mut i1 = Ipv4Interface::from_arp_table(
        mac_addr!(2),
        [192, 168, 1, 1],
        [255, 255, 255, 0],
        Some(default_gateway.ip_address),
        arp_table!([192, 168, 1, 1] => mac_addr!(1)),
    );

    i1.connect(&mut default_gateway);

    sim.adds(vec![i1.ethernet.port(), default_gateway.ethernet.port()]);

    // Act
    let i1_sent = i1.send([192, 168, 2, 1], vec![1], Ipv4Protocol::Test);
    sim.transmit();
    let dg_received = default_gateway.ethernet.receive();

    // Assert
    assert!(i1_sent.is_ok_and(|b| !b));
    assert_eq!(dg_received.len(), 1);
}

#[test]
fn Send_FillsArpTableOnReceive_SendsWithoutArp() {
    // Arrange
    let mut sim = CableSimulator::default();
    let mut i1 = Ipv4Interface::from_arp_table(
        mac_addr!(1),
        [192, 168, 1, 1],
        [255, 255, 255, 0],
        None,
        arp_table!(
            [192, 168, 1, 2] => mac_addr!(2) // Prefill with i2
        ),
    );
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2], [255, 255, 255, 0], None);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    i1.connect(&mut i2);

    // Act
    let i1_sends = i1.send(i2.ip_address, vec![1], Ipv4Protocol::Test);
    sim.transmit();

    let i2_data = i2.receive(); // Should fill ARP table passively
    sim.transmit();

    let i2_sends = i2.send(i1.ip_address, eth2_data!(2), Ipv4Protocol::Test);
    sim.transmit();

    let i1_data = i1.receive();

    // Assert
    assert!(i1_sends.is_ok_and(|b| b));
    assert!(i2_sends.is_ok_and(|b| b));
    assert_eq!(i2_data.len(), 1);
    assert_eq!(i1_data.len(), 1);
}

#[test]
fn Send_DifferentSubnetsWithoutDefaultGateway_DropsFrame() {
    // Arrange
    let mut sim = CableSimulator::default();
    let mut i1 = Ipv4Interface::from_arp_table(
        mac_addr!(1),
        [192, 168, 1, 1],
        [255, 255, 255, 0],
        None,
        arp_table!([192,168,1,2] => mac_addr!(2)),
    );
    let mut i2 = Ipv4Interface::from_arp_table(
        mac_addr!(2),
        [192, 168, 2, 1],
        [255, 255, 255, 0],
        None,
        arp_table!(i1.ip_address => i1.ethernet.mac_address),
    );

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    i1.connect(&mut i2);

    // Act
    let i1_sent = i1.send(i2.ip_address, vec![1], Ipv4Protocol::Test);
    sim.transmit();

    let i2_data = i2.receive();

    // Assert
    assert!(i1_sent.is_err());
    assert!(i2_data.is_empty());
}

#[test]
fn Arp_TwoInterfaces_BothInterfacesFillArpTable() {
    // Arrange
    let (_, mut i1, mut i2) = same_subnet_filled_arp_tables();

    // Act
    let i1_sent = i1.send(i2.ip_address, vec![1], Ipv4Protocol::Test); // Sends Ipv4 frame
    let i2_sent = i2.send(i1.ip_address, eth2_data!(2), Ipv4Protocol::Test); // Sends Ipv4 frame

    // Assert
    assert!(i1_sent.is_ok_and(|b| b));
    assert!(i2_sent.is_ok_and(|b| b));
}

#[test]
fn Ping_TwoInterfaces_BothInterfacesReceiveIcmp() {
    // Arrange
    let mut sim = CableSimulator::default();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1], [255, 255, 255, 0], None);
    let mut i2 = Ipv4Interface::new(mac_addr!(2), [192, 168, 1, 2], [255, 255, 255, 0], None);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);

    i1.connect(&mut i2);

    // Act
    let _ = i1.send_icmp(i2.ip_address, IcmpType::EchoRequest);
    sim.transmit();

    i2.receive(); // Sends ARP reply
    sim.transmit();

    i1.receive(); // Receives ARP reply, sends ICMP request
    sim.transmit();

    i2.receive(); // Receives ICMP request, sends ICMP reply
    sim.transmit();

    let i1_frames = i1.receive(); // Receives ICMP reply

    // Assert
    assert_eq!(i1_frames.len(), 1);
    assert_eq!(
        i1_frames[0],
        Ipv4Frame::new(
            i2.ip_address,
            i1.ip_address,
            64,
            IcmpFrame::echo_reply(0, 0, vec![]).to_bytes(),
            Ipv4Protocol::Icmp
        )
    );
}

#[test]
fn Ping_Self_ReceiveIcmpEchoReply() {
    // Arrange
    let mut sim = CableSimulator::default();
    let mut i1 = Ipv4Interface::new(mac_addr!(1), [192, 168, 1, 1], [255, 255, 255, 0], None);

    sim.add(i1.ethernet.port());

    // Act
    let _ = i1.send_icmp(i1.ip_address, IcmpType::EchoRequest);
    sim.transmit();

    let i1_frames = i1.receive();

    // Assert
    assert_eq!(i1_frames.len(), 1);
    assert_eq!(
        i1_frames[0],
        Ipv4Frame::new(
            i1.ip_address,
            i1.ip_address,
            64,
            IcmpFrame::echo_reply(0, 0, vec![]).to_bytes(),
            Ipv4Protocol::Icmp
        )
    );
}
