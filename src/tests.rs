#![allow(non_snake_case)]

use crate::{data_link::*, mac_addr, mac_broadcast_addr, physical::PacketSimulator};

#[test]
fn EthernetFrame_ToBytes_ReturnsValidByteArray() {

    // Arrange
    let ethernet_frame = EthernetFrame::new(
        mac_broadcast_addr!(),
        [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
        vec![0x00],
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
    assert_eq!(bytes[22], 0x00); // Data
    assert_eq!(bytes[23..27], [0x00, 0x00, 0x00, 0x00]); // Frame Check Sequence
}

#[test]
fn EthernetFrame_FromBytes_CreatesIdenticalEthernetFrame() {
    // Arrange
    let ethernet_frame = EthernetFrame::new(
        mac_broadcast_addr!(),
        [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
        vec![0x00],
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
fn EthernetInterface_ConnectedPorts_CanShareData() {
    // Arrange
    let mut sim = PacketSimulator::new();
    let mut interface1 = EthernetInterface::new(mac_addr!(1));
    let mut interface2 = EthernetInterface::new(mac_addr!(2));

    sim.add_port(interface1.port());
    sim.add_port(interface2.port());

    EthernetInterface::connect(&mut interface1, &mut interface2);

    let data = vec![0x00, 0x01, 0x02, 0x03];

    // Act
    interface1.send(data.clone());
    interface2.send(data.clone());
    sim.tick();

    let received_data1 = interface1.receive();
    let received_data2 = interface2.receive();

    // Assert
    assert!(received_data1.is_some());
    assert!(received_data2.is_some());

    assert_eq!(received_data1.unwrap(), EthernetFrame::new(
        mac_broadcast_addr!(),
        interface2.mac_address(),
        data.clone(),
        EtherType::Arp
    ));

    assert_eq!(received_data2.unwrap(), EthernetFrame::new(
        mac_broadcast_addr!(),
        interface1.mac_address(),
        data.clone(),
        EtherType::Arp
    ));

}
