#![allow(non_snake_case)]

use crate::{data_link::{ethernet_frame::*, ethernet_interface::*}, ether_payload, mac_addr, mac_broadcast_addr, physical::packet_sim::PacketSimulator};

#[test]
fn EthernetFrame_ToBytes_ReturnsValidByteArray() {

    // Arrange
    let ethernet_frame = EthernetFrame::new(
        mac_broadcast_addr!(),
        [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
        ether_payload!(1),
        EtherType::Arp
    );


    // Act
    let bytes = ethernet_frame.to_bytes();

    // Assert
    for i in 0..7 {
        assert_eq!(bytes[i], 0x55); // Preamble
    }

    assert_eq!(bytes[7], 0xD5); // Start Frame Delimiter

    for i in 0..6 {
        assert_eq!(bytes[8 + i], 0xFF); // Destination Address
    }

    for i in 0..6 {
        assert_eq!(bytes[14 + i], 0x01); // Source Address
    }

    assert_eq!(bytes[20..22], [0x08, 0x06]); // EtherType
    assert_eq!(bytes[22..50], ether_payload!(1)); // Data
    assert_eq!(bytes[50..54], [0x00, 0x00, 0x00, 0x00]); // Frame Check Sequence
}

#[test]
fn EthernetFrame_FromBytes_CreatesIdenticalEthernetFrame() {
    // Arrange
    let ethernet_frame = EthernetFrame::new(
        mac_broadcast_addr!(),
        [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
        ether_payload!(1),
        EtherType::Arp
    );

    let bytes = ethernet_frame.to_bytes();

    // Act
    let result = EthernetFrame::from_bytes(&bytes);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ethernet_frame);

}

#[test]
fn EthernetInterface_Receive_ReturnsEmptyVecWhenNoData() {
    // Arrange
    let mut interface = EthernetInterface::new(mac_addr!(1));

    // Act
    let result = interface.receive_frames();

    // Assert
    assert!(result.is_empty());
}

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

    interface1.send_data(ether_payload!(1));
    interface2.send_data(ether_payload!(2));
    uc_interface.send_data(ether_payload!(3));

    // Act
    sim.tick();

    // Assert
    assert!(!interface1.port().borrow().has_outgoing());
    assert!(!interface2.port().borrow().has_outgoing());
    assert!(!uc_interface.port().borrow().has_outgoing());

}

#[test]
fn EthernetInterface_SendDataMonodirectional_ReceivesFrame() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = EthernetInterface::new(mac_addr!(1));
    let mut interface2 = EthernetInterface::new(mac_addr!(2));

    sim.add_port(interface1.port());
    sim.add_port(interface2.port());

    EthernetInterface::connect_port(&mut interface1, &mut interface2);

    // Act
    interface1.send_data(ether_payload!(1));
    sim.tick();

    let received_data1 = interface1.receive_frames();
    let received_data2 = interface2.receive_frames();

    // Assert
    assert!(received_data1.is_empty());
    assert!(received_data2.len() == 1);

    assert_eq!(received_data2[0], EthernetFrame::new(
        mac_broadcast_addr!(),
        interface1.mac_address(),
        ether_payload!(1),
        EtherType::Arp
    ));
}

#[test]
fn EthernetInterface_SendDataBidirectional_ReceivesFrames() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = EthernetInterface::new(mac_addr!(1));
    let mut interface2 = EthernetInterface::new(mac_addr!(2));

    sim.add_port(interface1.port());
    sim.add_port(interface2.port());

    EthernetInterface::connect_port(&mut interface1, &mut interface2);

    // Act
    interface1.send_data(ether_payload!(1));
    interface2.send_data(ether_payload!(2));
    sim.tick();

    let received_data1 = interface1.receive_frames();
    let received_data2 = interface2.receive_frames();

    // Assert
    assert!(received_data1.len() == 1);
    assert!(received_data2.len() == 1);

    assert_eq!(received_data1[0], EthernetFrame::new(
        mac_broadcast_addr!(),
        interface2.mac_address(),
        ether_payload!(2),
        EtherType::Arp
    ));

    assert_eq!(received_data2[0], EthernetFrame::new(
        mac_broadcast_addr!(),
        interface1.mac_address(),
        ether_payload!(1),
        EtherType::Arp
    ));

}

#[test]
fn EthernetInterface_SendMultipleOutgoingMonodirectionalData_ReceivesAllData() {
        // Arrange
        let mut sim = PacketSimulator::new();
        let mut interface1 = EthernetInterface::new(mac_addr!(1));
        let mut interface2 = EthernetInterface::new(mac_addr!(2));
    
        sim.add_port(interface1.port());
        sim.add_port(interface2.port());
    
        EthernetInterface::connect_port(&mut interface1, &mut interface2);

        // Act
        interface1.send_data(ether_payload!(1));
        interface1.send_data(ether_payload!(2));
        interface1.send_data(ether_payload!(3));
        sim.tick();
        let received_data = interface2.receive_frames();

        // Assert
        assert!(received_data.len() == 3);
        assert_eq!(*received_data[0].data(), ether_payload!(1));
        assert_eq!(*received_data[1].data(), ether_payload!(2));
        assert_eq!(*received_data[2].data(), ether_payload!(3));
}

#[test]
fn EthernetInterface_SendMultipleOutgoingBidirectional_ReceivesAllData() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = EthernetInterface::new(mac_addr!(1));
    let mut interface2 = EthernetInterface::new(mac_addr!(2));

    sim.add_port(interface1.port());
    sim.add_port(interface2.port());

    EthernetInterface::connect_port(&mut interface1, &mut interface2);

    // Act
    interface1.send_data(ether_payload!(1));
    interface1.send_data(ether_payload!(2));
    interface1.send_data(ether_payload!(3));
    
    interface2.send_data(ether_payload!(4));
    interface2.send_data(ether_payload!(5));
    interface2.send_data(ether_payload!(6));
    sim.tick();
    let received_data1 = interface1.receive_frames();
    let received_data2 = interface2.receive_frames();

    // Assert
    assert!(received_data1.len() == 3);
    assert!(received_data2.len() == 3);

    assert_eq!(*received_data1[0].data(), ether_payload!(4));
    assert_eq!(*received_data1[1].data(), ether_payload!(5));
    assert_eq!(*received_data1[2].data(), ether_payload!(6));

    assert_eq!(*received_data2[0].data(), ether_payload!(1));
    assert_eq!(*received_data2[1].data(), ether_payload!(2));
    assert_eq!(*received_data2[2].data(), ether_payload!(3));
}