#![allow(non_snake_case)]

use std::time::Duration;

use crate::{
    eth2_data, mac_addr,
    network::{
        device::{cable::CableSimulator, router::Router, switch::Switch},
        ethernet::{interface::EthernetInterface, EtherType},
        ipv4::interface::Ipv4Interface,
    },
    tick::{Tickable, TimeProvider},
};

#[test]
fn Tick_SwitchRstpInit_FinishesAfter15Seconds() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut s = Switch::from_seed(1, 1);
    let mut i1 = EthernetInterface::new(mac_addr!(33));
    let mut i2 = EthernetInterface::new(mac_addr!(44));

    sim.adds(vec![i1.port(), i2.port()]);
    sim.adds(s.ports());

    s.connect(0, &mut i1);
    s.connect(1, &mut i2);

    s.init_stp();

    {
        let mut tp = TimeProvider::instance().lock().unwrap();
        tp.freeze();
    }

    // Act
    sim.tick();
    s.tick();
    sim.tick();
    s.tick();

    {
        let mut tp = TimeProvider::instance().lock().unwrap();
        tp.advance(Duration::from_secs(15));
    }

    s.tick();

    i1.send(i2.mac_address, EtherType::Debug, eth2_data!(0));
    sim.transmit();
    s.forward();
    sim.transmit();

    let i2_data = i2.receive_eth2();

    // Assert
    assert_eq!(i2_data.len(), 1);
}

#[test]
fn Tick_RouterRipMulticast_SendsEveryFiveSeconds() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut r = Router::from_seed(1);
    let mut i1 = Ipv4Interface::new(
        mac_addr!(9),
        [192, 168, 1, 2],
        [255, 255, 255, 0],
        Some([192, 168, 1, 1]),
    );

    sim.add(i1.ethernet.port());
    sim.adds(r.ports());

    r.connect(0, &mut i1);
    r.enable_interface(0, [192, 168, 1, 1], [255, 255, 255, 0]);
    r.enable_rip(0);

    {
        let mut tp = TimeProvider::instance().lock().unwrap();
        tp.freeze();
    }

    // Act
    for _ in 0..2 {
        sim.tick();
        r.tick();
        {
            let mut tp = TimeProvider::instance().lock().unwrap();
            tp.advance(Duration::from_secs(5));
        }
        sim.tick();
        r.tick();
    }

    // Assert
    assert_eq!(i1.receive().len(), 2);
}
