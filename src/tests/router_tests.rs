#![allow(non_snake_case)]

use crate::{
    device::{cable::CableSimulator, router::Router},
    ipv4::{interface::Ipv4Interface, Ipv4Frame},
    mac_addr,
};

#[test]
fn FromSeed_ConnectPortOnDisabledInterface_Panics() {
    // Arrange
    let mut router = Router::from_seed(1);
    let mut i1 = Ipv4Interface::new(mac_addr!(10), [192, 168, 1, 1], [255, 255, 255, 0], None);

    // Act
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        router.connect(0, &mut i1);
    }));

    // Assert
    assert!(result.is_err());
}

#[test]
fn FromSeed_ConnectPortOnEnabledInterface_DoesNotPanic() {
    // Arrange
    let mut router = Router::from_seed(1);
    let mut i1 = Ipv4Interface::new(mac_addr!(10), [192, 168, 1, 1], [255, 255, 255, 0], None);

    // Act
    router.enable_interface(0, [192, 168, 1, 0], [255, 255, 255, 0]);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        router.connect(0, &mut i1);
    }));

    // Assert
    assert!(result.is_ok());
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
        Ipv4Frame::new(i1.ip_address, i1.default_gateway.unwrap(), data.clone())
    );
}

#[test]
fn Route_SendAcrossSubnetworks_ReceiveFrame() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = Ipv4Interface::new(
        mac_addr!(1),
        [192, 168, 1, 1],
        [255, 255, 255, 0],
        Some([192, 168, 1, 0]),
    );
    let mut i2 = Ipv4Interface::new(
        mac_addr!(2),
        [192, 168, 2, 1],
        [255, 255, 255, 0],
        Some([192, 168, 2, 0]),
    );
    let mut r = Router::from_seed(3);

    r.enable_interface(0, i1.default_gateway.unwrap(), [255, 255, 255, 0]);
    r.enable_interface(1, i2.default_gateway.unwrap(), [255, 255, 255, 0]);

    r.connect(0, &mut i1);
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
        Ipv4Frame::new(i1.ip_address, i2.ip_address, data.clone())
    );
}
