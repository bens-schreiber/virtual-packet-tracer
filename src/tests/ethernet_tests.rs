#![allow(non_snake_case)]

use crate::ethernet::*;
use crate::ethernet::interface::*;
use crate::ethernet::ByteSerialize;
use crate::{mac_addr, mac_broadcast_addr, eth2_data, eth2, eth802_3_data};

mod EthernetFrameTests {

    use super::*;

    
    #[test]
    fn Ethernet2Frame_ToBytes_ReturnsValidByteArray() {

        // Arrange
        let ethernet_frame = Ethernet2Frame::new(
            mac_broadcast_addr!(),
            [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
            eth2_data!(1),
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
        assert_eq!(bytes[22..50], eth2_data!(1)); // Data
        assert_eq!(bytes[50..54], [0x00, 0x00, 0x00, 0x00]); // Frame Check Sequence
    }

    #[test]
    fn Ethernet2Frame_FromBytes_CreatesIdenticalEthernetFrame() {
        // Arrange
        let ethernet_frame = Ethernet2Frame::new(
            mac_broadcast_addr!(),
            [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
            eth2_data!(1),
            EtherType::Debug
        );

        let bytes = ethernet_frame.to_bytes();

        // Act
        let result = Ethernet2Frame::from_bytes(bytes);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ethernet_frame);
    }

    #[test]
    fn Ethernet802_3Frame_ToBytes_ReturnsValidByteArray() {
        // Arrange
        let ethernet_frame = Ethernet2Frame::new(
            mac_broadcast_addr!(),
            [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
            eth2_data!(1),
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
        assert_eq!(bytes[22..50], eth2_data!(1)); // Data
        assert_eq!(bytes[50..54], [0x00, 0x00, 0x00, 0x00]); // Frame Check Sequence
    }

    #[test]
    fn Ethernet802_3Frame_FromBytes_CreatesIdenticalEthernetFrame() {
        // Arrange
        let ethernet_frame = Ethernet2Frame::new(
            mac_broadcast_addr!(),
            [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
            eth2_data!(1),
            EtherType::Debug
        );

        let bytes = ethernet_frame.to_bytes();

        // Act
        let result = Ethernet2Frame::from_bytes(bytes);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ethernet_frame);
    }
}

mod EthernetInterfaceTests {
    use crate::device::cable::CableSimulator;

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
        let mut sim = CableSimulator::new();
        let mut i1 = EthernetInterface::new(mac_addr!(1));
        let mut i2 = EthernetInterface::new(mac_addr!(2));

        sim.adds(vec![
            i1.port(),
            i2.port(),
        ]);

        EthernetInterface::connect(&mut i1, &mut i2);

        // Act
        i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
        sim.tick();

        let i1_data = i1.receive();
        let i2_data = i2.receive();

        // Assert
        assert!(i1_data.is_empty());
        assert!(i2_data.len() == 1);

        assert_eq!(i2_data[0], eth2!(
            i2.mac_address,
            i1.mac_address,
            eth2_data!(1),
            EtherType::Debug
        ));
    }

    #[test]
    fn EthernetInterface_SendBi_ReceivesFrames() {
        // Arrange
        let mut sim = CableSimulator::new();
        let mut i1 = EthernetInterface::new(mac_addr!(1));
        let mut i2 = EthernetInterface::new(mac_addr!(2));

        sim.adds(vec![
            i1.port(),
            i2.port(),
        ]);

        EthernetInterface::connect(&mut i1, &mut i2);

        // Act
        i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
        i2.send(i1.mac_address, EtherType::Debug, eth2_data!(2));
        sim.tick();

        let i1_data = i1.receive();
        let i2_data = i2.receive();

        // Assert
        assert!(i1_data.len() == 1);
        assert!(i2_data.len() == 1);

        assert_eq!(i1_data[0], eth2!(
            i1.mac_address,
            i2.mac_address,
            eth2_data!(2),
            EtherType::Debug
        ));

        assert_eq!(i2_data[0], eth2!(
            i2.mac_address,
            i1.mac_address,
            eth2_data!(1),
            EtherType::Debug
        ));

    }

    #[test]
    fn EthernetInterface_SendUniMult_ReceivesAllData() {
            // Arrange
            let mut sim = CableSimulator::new();
            let mut i1 = EthernetInterface::new(mac_addr!(1));
            let mut i2 = EthernetInterface::new(mac_addr!(2));
        
            sim.adds(vec![
                i1.port(),
                i2.port(),
            ]);
        
            EthernetInterface::connect(&mut i1, &mut i2);

            // Act
            i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
            i1.send(i2.mac_address, EtherType::Debug, eth2_data!(2));
            i1.send(i2.mac_address, EtherType::Debug, eth2_data!(3));
            sim.tick();
            let received_data = i2.receive_eth2();

            // Assert
            assert!(received_data.len() == 3);
            assert_eq!(*received_data[0].data, eth2_data!(1));
            assert_eq!(*received_data[1].data, eth2_data!(2));
            assert_eq!(*received_data[2].data, eth2_data!(3));
    }

    #[test]
    fn EthernetInterface_SendBiMult_ReceivesAllData() {
        // Arrange
        let mut sim = CableSimulator::new();
        let mut i1 = EthernetInterface::new(mac_addr!(1));
        let mut i2 = EthernetInterface::new(mac_addr!(2));

        sim.adds(vec![
            i1.port(),
            i2.port(),
        ]);

        EthernetInterface::connect(&mut i1, &mut i2);

        // Act
        i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
        i1.send(i2.mac_address, EtherType::Debug, eth2_data!(2));
        i1.send(i2.mac_address, EtherType::Debug, eth2_data!(3));
        
        i2.send(i1.mac_address, EtherType::Debug, eth2_data!(4));
        i2.send(i1.mac_address, EtherType::Debug, eth2_data!(5));
        i2.send(i1.mac_address, EtherType::Debug, eth2_data!(6));
        sim.tick();
        let i1_data = i1.receive_eth2();
        let i2_data = i2.receive_eth2();

        // Assert
        assert!(i1_data.len() == 3);
        assert!(i2_data.len() == 3);

        assert_eq!(*i1_data[0].data, eth2_data!(4));
        assert_eq!(*i1_data[1].data, eth2_data!(5));
        assert_eq!(*i1_data[2].data, eth2_data!(6));

        assert_eq!(*i2_data[0].data, eth2_data!(1));
        assert_eq!(*i2_data[1].data, eth2_data!(2));
        assert_eq!(*i2_data[2].data, eth2_data!(3));
    }

    #[test]
    fn EthernetInterface_ReceiveEthernet2AndEthernet8023_ReturnsBothFrames() {
        // Arrange
        let mut sim = CableSimulator::new();
        let mut i1 = EthernetInterface::new(mac_addr!(1));
        let mut i2 = EthernetInterface::new(mac_addr!(2));

        sim.adds(vec![
            i1.port(),
            i2.port(),
        ]);

        EthernetInterface::connect(&mut i1, &mut i2);

        // Act
        i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
        i1.send802_3(i2.mac_address, eth802_3_data!(2));
        sim.tick();

        let i2_data = i2.receive();

        // Assert
        assert!(i2_data.len() == 2);

        assert!(matches!(i2_data[0], EthernetFrame::Ethernet2(_)));
        assert!(matches!(i2_data[1], EthernetFrame::Ethernet802_3(_)));
    }
}