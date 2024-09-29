#![allow(non_snake_case)]

use crate::{data_link::{ethernet_frame::*, ethernet_interface::*}, mac_addr, mac_broadcast_addr, physical::physical_sim::PhysicalSimulator};

mod EthernetFrameTests {
    use super::*;

    
    #[test]
    fn EthernetFrame_ToBytes_ReturnsValidByteArray() {

        // Arrange
        let ethernet_frame = EthernetFrame::new(
            mac_broadcast_addr!(),
            [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
            ether_payload(1),
            EtherType::Debug
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

        assert_eq!(bytes[20..22], [0xFF, 0xFF]); // EtherType
        assert_eq!(bytes[22..50], ether_payload(1)); // Data
        assert_eq!(bytes[50..54], [0x00, 0x00, 0x00, 0x00]); // Frame Check Sequence
    }

    #[test]
    fn EthernetFrame_FromBytes_CreatesIdenticalEthernetFrame() {
        // Arrange
        let ethernet_frame = EthernetFrame::new(
            mac_broadcast_addr!(),
            [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
            ether_payload(1),
            EtherType::Debug
        );

        let bytes = ethernet_frame.to_bytes();

        // Act
        let result = EthernetFrame::from_bytes(&bytes);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ethernet_frame);
    }
}

mod EthernetInterfaceTests {
    use super::*;

    #[test]
    fn EthernetInterface_Receive_ReturnsEmptyVecWhenNoData() {
        // Arrange
        let mut i1 = EthernetInterface::new(mac_addr!(1));
    
        // Act
        let i1_data = i1.receive();
    
        // Assert
        assert!(i1_data.is_empty());
    }

    #[test]
    fn EthernetInterface_SendUni_ReceivesFrame() {
        // Arrange
        let mut sim = PhysicalSimulator::new();
        let mut i1 = EthernetInterface::new(mac_addr!(1));
        let mut i2 = EthernetInterface::new(mac_addr!(2));

        sim.add_ports(vec![
            i1.port(),
            i2.port(),
        ]);

        EthernetInterface::connect(&mut i1, &mut i2);

        // Act
        i1.send(mac_addr!(0), EtherType::Debug, &&ether_payload(1));
        sim.tick();

        let i1_data = i1.receive();
        let i2_data = i2.receive();

        // Assert
        assert!(i1_data.is_empty());
        assert!(i2_data.len() == 1);

        assert_eq!(i2_data[0], EthernetFrame::new(
            mac_addr!(0),
            i1.mac_address(),
            ether_payload(1),
            EtherType::Debug
        ));
    }

    #[test]
    fn EthernetInterface_SendBi_ReceivesFrames() {
        // Arrange
        let mut sim = PhysicalSimulator::new();
        let mut i1 = EthernetInterface::new(mac_addr!(1));
        let mut i2 = EthernetInterface::new(mac_addr!(2));

        sim.add_ports(vec![
            i1.port(),
            i2.port(),
        ]);

        EthernetInterface::connect(&mut i1, &mut i2);

        // Act
        i1.send(mac_addr!(0), EtherType::Debug, &ether_payload(1));
        i2.send(mac_addr!(0), EtherType::Debug, &ether_payload(2));
        sim.tick();

        let i1_data = i1.receive();
        let i2_data = i2.receive();

        // Assert
        assert!(i1_data.len() == 1);
        assert!(i2_data.len() == 1);

        assert_eq!(i1_data[0], EthernetFrame::new(
            mac_addr!(0),
            i2.mac_address(),
            ether_payload(2),
            EtherType::Debug
        ));

        assert_eq!(i2_data[0], EthernetFrame::new(
            mac_addr!(0),
            i1.mac_address(),
            ether_payload(1),
            EtherType::Debug
        ));

    }

    #[test]
    fn EthernetInterface_SendUniMult_ReceivesAllData() {
            // Arrange
            let mut sim = PhysicalSimulator::new();
            let mut i1 = EthernetInterface::new(mac_addr!(1));
            let mut i2 = EthernetInterface::new(mac_addr!(2));
        
            sim.add_ports(vec![
                i1.port(),
                i2.port(),
            ]);
        
            EthernetInterface::connect(&mut i1, &mut i2);

            // Act
            i1.send(mac_addr!(0), EtherType::Debug, &ether_payload(1));
            i1.send(mac_addr!(0), EtherType::Debug, &ether_payload(2));
            i1.send(mac_addr!(0), EtherType::Debug, &ether_payload(3));
            sim.tick();
            let received_data = i2.receive();

            // Assert
            assert!(received_data.len() == 3);
            assert_eq!(*received_data[0].data(), ether_payload(1));
            assert_eq!(*received_data[1].data(), ether_payload(2));
            assert_eq!(*received_data[2].data(), ether_payload(3));
    }

    #[test]
    fn EthernetInterface_SendBiMult_ReceivesAllData() {
        // Arrange
        let mut sim = PhysicalSimulator::new();
        let mut i1 = EthernetInterface::new(mac_addr!(1));
        let mut i2 = EthernetInterface::new(mac_addr!(2));

        sim.add_ports(vec![
            i1.port(),
            i2.port(),
        ]);

        EthernetInterface::connect(&mut i1, &mut i2);

        // Act
        i1.send(mac_addr!(0), EtherType::Debug, &ether_payload(1));
        i1.send(mac_addr!(0), EtherType::Debug, &ether_payload(2));
        i1.send(mac_addr!(0), EtherType::Debug, &ether_payload(3));
        
        i2.send(mac_addr!(0), EtherType::Debug, &ether_payload(4));
        i2.send(mac_addr!(0), EtherType::Debug, &ether_payload(5));
        i2.send(mac_addr!(0), EtherType::Debug, &ether_payload(6));
        sim.tick();
        let i1_data = i1.receive();
        let i2_data = i2.receive();

        // Assert
        assert!(i1_data.len() == 3);
        assert!(i2_data.len() == 3);

        assert_eq!(*i1_data[0].data(), ether_payload(4));
        assert_eq!(*i1_data[1].data(), ether_payload(5));
        assert_eq!(*i1_data[2].data(), ether_payload(6));

        assert_eq!(*i2_data[0].data(), ether_payload(1));
        assert_eq!(*i2_data[1].data(), ether_payload(2));
        assert_eq!(*i2_data[2].data(), ether_payload(3));
    }
}