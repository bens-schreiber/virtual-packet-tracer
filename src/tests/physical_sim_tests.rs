#![allow(non_snake_case)]

use crate::{data_link::{ethernet_interface::EthernetInterface, frame::ethernet_ii::EtherType}, eth2_data, mac_addr, physical::physical_sim::PhysicalSimulator};

#[test]
fn PhysicalSimulator_Tick_ConsumesAllOutgoing() {
    // Arrange
    let mut sim = PhysicalSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut i2 = EthernetInterface::new(mac_addr!(2));
    let mut uc_interface = EthernetInterface::new(mac_addr!(3));

    sim.adds(vec![
        i1.port(),
        i2.port(),
        uc_interface.port(),
    ]);

    EthernetInterface::connect(&mut i1, &mut i2);

    i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
    i2.send(i1.mac_address, EtherType::Debug, eth2_data!(2));
    uc_interface.send(mac_addr!(0), EtherType::Debug, eth2_data!(3));

    // Act
    sim.tick();

    // Assert
    assert!(!i1.port().borrow().has_outgoing());
    assert!(!i2.port().borrow().has_outgoing());
    assert!(!uc_interface.port().borrow().has_outgoing());

}