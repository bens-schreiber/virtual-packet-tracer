#![allow(non_snake_case)]

use crate::{
    mac_addr,
    network::{
        device::{cable::CableSimulator, router::Router},
        ethernet::ByteSerialize,
        ipv4::{interface::Ipv4Interface, IcmpFrame, Ipv4Frame},
    },
};

#[test]
fn Route_DoesNotExist_ReceiveDestinationUnreachable() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(
        mac_addr!(1),
        [192, 168, 1, 2],
        [255, 255, 255, 0],
        Some([192, 168, 1, 1]),
    );
    let mut r = Router::from_seed(2);

    r.enable_interface(0, [192, 168, 1, 1], [255, 255, 255, 0]);
    r.connect(0, &mut i1);

    sim.add(i1.ethernet.port());
    sim.adds(r.ports());

    let data: Vec<u8> = "Hello, world!".as_bytes().into();

    // Act
    i1.send([192, 168, 2, 1], data.clone()); // ---- i1 -> r ARP
    sim.transmit();

    r.route();
    sim.transmit();

    i1.receive(); // ---- i1 -> r send frame
    sim.transmit();

    r.route();
    sim.transmit();

    let i1_data = i1.receive();

    // Assert
    assert_eq!(i1_data.len(), 1);
    assert_eq!(
        i1_data[0],
        Ipv4Frame::new(
            i1.default_gateway.unwrap(),
            [192, 168, 1, 2],
            64,
            IcmpFrame::destination_unreachable(0, vec![]).to_bytes(),
            true
        )
    );
}

#[test]
fn Route_ConnectedInterfaceCanResolveDefaultGateway_ReceiveFrame() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(
        mac_addr!(10),
        [192, 168, 1, 1],
        [255, 255, 255, 0],
        Some([192, 168, 1, 0]),
    );
    let mut r = Router::from_seed(1);

    r.enable_interface(0, i1.default_gateway.unwrap(), [255, 255, 255, 0]);
    r.connect(0, &mut i1);

    sim.add(i1.ethernet.port());
    sim.adds(r.ports());

    let data: Vec<u8> = "Hello, world!".as_bytes().into();

    // Act
    i1.send(i1.default_gateway.unwrap(), data.clone());
    sim.transmit();

    r.route();
    sim.transmit();

    i1.receive();
    sim.transmit();

    // Assert
    let r_p0_receive = r.receive_port(0);
    assert_eq!(r_p0_receive.len(), 1);
    assert_eq!(
        r_p0_receive[0],
        Ipv4Frame::new(
            i1.ip_address,
            i1.default_gateway.unwrap(),
            64,
            data.clone(),
            false
        )
    );
}

#[test]
fn Route_SendAcrossSubnetworks_ReceiveFrame() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(
        mac_addr!(1),
        [192, 168, 1, 2],
        [255, 255, 255, 0],
        Some([192, 168, 1, 1]),
    );
    let mut i2 = Ipv4Interface::new(
        mac_addr!(2),
        [192, 168, 2, 2],
        [255, 255, 255, 0],
        Some([192, 168, 2, 1]),
    );
    let mut r = Router::from_seed(3);

    r.enable_interface(0, i1.default_gateway.unwrap(), [255, 255, 255, 0]);
    r.connect(0, &mut i1);

    r.enable_interface(1, i2.default_gateway.unwrap(), [255, 255, 255, 0]);
    r.connect(1, &mut i2);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    sim.adds(r.ports());

    let data: Vec<u8> = "Hello, world!".as_bytes().into();

    // Act
    i1.send(i2.ip_address, data.clone()); // ---- i1 -> r Resolve mac addresses
    sim.transmit();

    r.route();
    sim.transmit();

    i1.receive(); // ---- i1 -> r send frame
    sim.transmit();

    r.route(); // ---- r -> i2 Resolve mac addresses
    sim.transmit();

    i2.receive();
    sim.transmit();

    r.route(); // ---- r -> i2 send frame
    sim.transmit();

    let i2_data = i2.receive();

    // Assert
    assert_eq!(i2_data.len(), 1);
    assert_eq!(
        i2_data[0],
        Ipv4Frame::new(i1.ip_address, i2.ip_address, 63, data.clone(), false)
    );
}

#[test]
fn Route_SendAcrossRoutersWithRipConfig_ReceiveFrame() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(
        mac_addr!(1),
        [192, 168, 1, 2],
        [255, 255, 255, 0],
        Some([192, 168, 1, 1]),
    );
    let mut i2 = Ipv4Interface::new(
        mac_addr!(2),
        [192, 168, 2, 2],
        [255, 255, 255, 0],
        Some([192, 168, 2, 1]),
    );
    let mut r1 = Router::from_seed(3);
    let mut r2 = Router::from_seed(12);

    r1.enable_interface(0, i1.default_gateway.unwrap(), [255, 255, 255, 0]);
    r1.connect(0, &mut i1);

    r2.enable_interface(0, i2.default_gateway.unwrap(), [255, 255, 255, 0]);
    r2.connect(0, &mut i2);

    r1.enable_interface(1, [10, 0, 0, 1], [255, 255, 255, 252]);
    r2.enable_interface(1, [10, 0, 0, 2], [255, 255, 255, 252]);
    r1.enable_rip(1);
    r2.enable_rip(1);
    r1.connect_router(1, &mut r2, 1);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    sim.adds(r1.ports());
    sim.adds(r2.ports());

    let data: Vec<u8> = "Hello, world!".as_bytes().into();

    // Act
    sim.transmit();

    r1.route();
    r2.route();
    sim.transmit();

    i1.send(i2.ip_address, data.clone());

    for _ in 0..6 {
        sim.transmit();
        i1.receive();
        i2.receive();
        r1.route();
        r2.route();
    }

    sim.transmit();
    let i2_data = i2.receive();

    // Assert
    assert_eq!(i2_data.len(), 1);
    assert_eq!(
        i2_data[0],
        Ipv4Frame::new(i1.ip_address, i2.ip_address, 62, data.clone(), false)
    );
}
