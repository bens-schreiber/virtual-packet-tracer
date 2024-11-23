#![allow(non_snake_case)]

use crate::{
    device::{cable::CableSimulator, router::Router},
    ipv4::{interface::Ipv4Interface, Ipv4Frame},
    mac_addr,
};

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
    sim.tick();

    r.route();
    sim.tick();

    i1.receive();
    sim.tick();

    // Assert
    let r_p0_receive = r.receive_port(0);
    assert_eq!(r_p0_receive.len(), 1);
    assert_eq!(
        r_p0_receive[0],
        Ipv4Frame::new(i1.ip_address, i1.default_gateway.unwrap(), 64, data.clone())
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
    sim.tick();

    r.route();
    sim.tick();

    i1.receive(); // ---- i1 -> r send frame
    sim.tick();

    r.route(); // ---- r -> i2 Resolve mac addresses
    sim.tick();

    i2.receive();
    sim.tick();

    r.route(); // ---- r -> i2 send frame
    sim.tick();

    let i2_data = i2.receive();

    // Assert
    assert_eq!(i2_data.len(), 1);
    assert_eq!(
        i2_data[0],
        Ipv4Frame::new(i1.ip_address, i2.ip_address, 63, data.clone())
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
    sim.tick();

    r1.route();
    r2.route();
    sim.tick();

    i1.send(i2.ip_address, data.clone());

    for _ in 0..6 {
        sim.tick();
        i1.receive();
        i2.receive();
        r1.route();
        r2.route();
    }

    sim.tick();
    let i2_data = i2.receive();

    // Assert
    assert_eq!(i2_data.len(), 1);
    assert_eq!(
        i2_data[0],
        Ipv4Frame::new(i1.ip_address, i2.ip_address, 62, data.clone())
    );
}
