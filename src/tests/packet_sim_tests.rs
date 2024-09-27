#![allow(non_snake_case)]

use crate::{data_link::{ethernet_frame::*, ethernet_interface::*}, ether_payload, mac_addr, physical::packet_sim::PacketSimulator};

#[test]
fn PacketSimulator_Tick_ConsumesAllOutgoing() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = EthernetInterface::new(mac_addr!(1));
    let mut interface2 = EthernetInterface::new(mac_addr!(2));
    let mut uc_interface = EthernetInterface::new(mac_addr!(3));

    sim.add_port(interface1.port());
    sim.add_port(interface2.port());
    sim.add_port(uc_interface.port());

    EthernetInterface::connect_port(&mut interface1, &mut interface2);

    interface1.send(mac_addr!(0), EtherType::Debug, ether_payload!(1));
    interface2.send(mac_addr!(0), EtherType::Debug, ether_payload!(2));
    uc_interface.send(mac_addr!(0), EtherType::Debug, ether_payload!(3));

    // Act
    sim.tick();

    // Assert
    assert!(!interface1.port().borrow().has_outgoing());
    assert!(!interface2.port().borrow().has_outgoing());
    assert!(!uc_interface.port().borrow().has_outgoing());

}