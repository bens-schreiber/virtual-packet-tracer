#![allow(non_snake_case)]

use crate::{data_link::{device::switch::Switch, ethernet_interface::EthernetInterface, frame::ethernet_ii::{EtherType, Ethernet2Frame}}, eth2, eth2_data, mac_addr, mac_broadcast_addr, physical::physical_sim::PhysicalSimulator};

#[test]
pub fn Switch_ReceiveNotInTable_FloodsFrame() {
    // Arrange
    let mut sim = PhysicalSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut i2 = EthernetInterface::new(mac_addr!(2));
    let mut i3 = EthernetInterface::new(mac_addr!(3));
    let mut switch = Switch::from_seed(4);

    switch.connect(0, &mut i1);
    switch.connect(1, &mut i2);
    switch.connect(2, &mut i3);

    sim.adds(vec![
        i1.port(),
        i2.port(),
        i3.port(),
    ]);

    sim.adds(switch.ports());

    // Act
    i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
    sim.tick();
    switch.receive();
    sim.tick();

    let i2_data = i2.receive();
    let received_data3 = i3.receive();

    // Assert
    assert!(i2_data.len() == 1);
    assert_eq!(i2_data[0], eth2!(
        i2.mac_address,
        i1.mac_address,
        eth2_data!(1),
        EtherType::Debug
    ));

    assert!(received_data3.len() == 1);
    assert_eq!(received_data3[0], eth2!(
        i2.mac_address,
        i1.mac_address,
        eth2_data!(1),
        EtherType::Debug
    ));

}

#[test]
pub fn Switch_ReceiveInTable_ForwardsFrame() {
    // Arrange
    let mut sim = PhysicalSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut i2 = EthernetInterface::new(mac_addr!(2));
    let mut i3 = EthernetInterface::new(mac_addr!(3));
    let mut switch = Switch::from_seed(4);

    switch.connect(0, &mut i1);
    switch.connect(1, &mut i2);
    switch.connect(2, &mut i3);

    sim.adds(vec![
        i1.port(),
        i2.port(),
        i3.port(),
    ]);

    sim.adds(switch.ports());

    i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
    sim.tick();
    switch.receive();       // Switch learns MAC address of i1
    sim.tick();
    i2.receive();   // dump incoming data
    i3.receive();   // dump incoming data

    // Act
    i2.send(i1.mac_address, EtherType::Debug, eth2_data!(1));
    sim.tick();
    switch.receive();
    sim.tick();

    let i1_data = i1.receive();
    let received_data3 = i3.receive();

    // Assert
    assert!(i1_data.len() == 1);
    assert_eq!(i1_data[0], eth2!(
        i1.mac_address,
        i2.mac_address,
        eth2_data!(1),
        EtherType::Debug
    ));

    assert!(received_data3.is_empty());

}

#[test]
fn Switch_ReceiveBroadcastAddr_DoesNotUpdateTableAndFloodsFrame() {
    // Arrange
    let mut sim = PhysicalSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut i2 = EthernetInterface::new(mac_addr!(2));
    let mut i3 = EthernetInterface::new(mac_addr!(3));
    let mut switch = Switch::from_seed(4);

    switch.connect(0, &mut i1);
    switch.connect(1, &mut i2);
    switch.connect(2, &mut i3);

    sim.adds(vec![
        i1.port(),
        i2.port(),
        i3.port(),
    ]);

    sim.adds(switch.ports());

    // Act
    i1.send(mac_broadcast_addr!(), EtherType::Debug, eth2_data!(1)); // Send broadcast
    sim.tick();
    switch.receive();
    sim.tick();

    let i2_data = i2.receive(); // Receive broadcast
    let i3_data = i3.receive(); // Receive broadcast

    i1.send(mac_broadcast_addr!(), EtherType::Debug, eth2_data!(2)); // Send broadcast
    sim.tick();
    switch.receive();
    sim.tick();

    let i2_data2 = i2.receive(); // Receive broadcast
    let i3_data2 = i3.receive(); // Receive broadcast

    // Assert
    let frame1 = eth2!(
        mac_broadcast_addr!(),
        i1.mac_address,
        eth2_data!(1),
        EtherType::Debug
    );

    assert!(i2_data.len() == 1);
    assert_eq!(i2_data[0], frame1);

    assert!(i3_data.len() == 1);
    assert_eq!(i3_data[0], frame1);

    let frame2 = eth2!(
        mac_broadcast_addr!(),
        i1.mac_address,
        eth2_data!(2),
        EtherType::Debug
    );

    assert!(i2_data2.len() == 1);
    assert_eq!(i2_data2[0], frame2);

    assert!(i3_data2.len() == 1);
    assert_eq!(i3_data2[0], frame2);
}