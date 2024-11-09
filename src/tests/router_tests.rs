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
fn Route_ToSameNetwork_ReceiveOwnMessage() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut r = Router::from_seed(1);
    let mut i1 = Ipv4Interface::new(mac_addr!(10), [192, 168, 1, 1], [255, 255, 255, 0], None);
    let mut i2 = Ipv4Interface::new(mac_addr!(20), [192, 168, 1, 2], [255, 255, 255, 0], None);

    r.enable_interface(0, [192, 168, 1, 0], [255, 255, 255, 0]);
    r.enable_interface(1, [192, 168, 2, 0], [255, 255, 255, 0]);

    r.connect(0, &mut i1);
    r.connect(1, &mut i2);

    sim.adds(vec![i1.ethernet.port(), i2.ethernet.port()]);
    sim.adds(r.ports());

    let data: Vec<u8> = "Hello, world!".as_bytes().into();

    // Act
    i1.send([192, 168, 1, 1], data.clone());
    sim.tick();

    r.route();
    sim.tick();

    let i1_data = i1.receive();
    let i2_data = i2.receive();

    // Assert
    assert!(i1_data.is_empty());
    assert_eq!(i2_data.len(), 1);
    assert_eq!(
        i2_data[0],
        Ipv4Frame::new([192, 168, 1, 1], [192, 168, 1, 1], data.clone())
    );

    // TODO: route to self
    // TODO: self should not be in arp table and should just send to self (test in ivp4 interface tests)
}
