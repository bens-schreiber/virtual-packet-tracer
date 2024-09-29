#![allow(non_snake_case)]

use crate::{data_link::{device::switch::Switch, ethernet_frame::*, ethernet_interface::*}, mac_addr, physical::packet_sim::PacketSimulator};

#[test]
pub fn Switch_ReceiveNotInTable_FloodsFrame() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = EthernetInterface::new(mac_addr!(1));
    let mut interface2 = EthernetInterface::new(mac_addr!(2));
    let mut interface3 = EthernetInterface::new(mac_addr!(3));
    let mut switch = Switch::from_seed(4);

    switch.connect(0, &mut interface1);
    switch.connect(1, &mut interface2);
    switch.connect(2, &mut interface3);

    sim.add_ports(vec![
        interface1.port(),
        interface2.port(),
        interface3.port(),
    ]);

    sim.add_ports(switch.ports());

    // Act
    interface1.send(interface2.mac_address(), EtherType::Debug, &ether_payload(1));
    sim.tick();
    switch.receive();
    sim.tick();

    let received_data2 = interface2.receive();
    let received_data3 = interface3.receive();

    // Assert
    assert!(received_data2.len() == 1);
    assert_eq!(received_data2[0], EthernetFrame::new(
        interface2.mac_address(),
        interface1.mac_address(),
        ether_payload(1),
        EtherType::Debug
    ));

    assert!(received_data3.len() == 1);
    assert_eq!(received_data3[0], EthernetFrame::new(
        interface2.mac_address(),
        interface1.mac_address(),
        ether_payload(1),
        EtherType::Debug
    ));

}

#[test]
pub fn Switch_ReceiveInTable_ForwardsFrame() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = EthernetInterface::new(mac_addr!(1));
    let mut interface2 = EthernetInterface::new(mac_addr!(2));
    let mut interface3 = EthernetInterface::new(mac_addr!(3));
    let mut switch = Switch::from_seed(4);

    switch.connect(0, &mut interface1);
    switch.connect(1, &mut interface2);
    switch.connect(2, &mut interface3);

    sim.add_ports(vec![
        interface1.port(),
        interface2.port(),
        interface3.port(),
    ]);

    sim.add_ports(switch.ports());

    interface1.send(interface2.mac_address(), EtherType::Debug, &ether_payload(1));
    sim.tick();
    switch.receive();       // Switch learns MAC address of interface1
    sim.tick();
    interface2.receive();   // dump incoming data
    interface3.receive();   // dump incoming data

    // Act
    interface2.send(interface1.mac_address(), EtherType::Debug, &ether_payload(1));
    sim.tick();
    switch.receive();
    sim.tick();

    let received_data1 = interface1.receive();
    let received_data3 = interface3.receive();

    // Assert
    assert!(received_data1.len() == 1);
    assert_eq!(received_data1[0], EthernetFrame::new(
        interface1.mac_address(),
        interface2.mac_address(),
        ether_payload(1),
        EtherType::Debug
    ));

    assert!(received_data3.is_empty());

}