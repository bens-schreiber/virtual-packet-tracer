#![allow(non_snake_case)]

use crate::data_link::*;

#[test]
fn test() {
    assert_eq!(2 + 2, 4);
}

#[test]
fn EthernetFrame_ToBytes_ReturnsValidByteArray() {

    // Arrange
    let ethernet_frame = EthernetFrame::new(
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
        vec![0x00]
    );


    // Act
    let bytes = ethernet_frame.to_bytes();

    // Assert
    for i in 0..7 {
        assert_eq!(bytes[i], 0x55); // Preamble
    }

    assert_eq!(bytes[7], 0xD5); // Start Frame Delimiter

    for i in 0..6 {
        assert_eq!(bytes[8 + i], 0x00); // Destination Address
    }

    for i in 0..6 {
        assert_eq!(bytes[14 + i], 0x01); // Source Address
    }

    assert_eq!(bytes[20..22], [0x00, 0x01]); // Length
    assert_eq!(bytes[22], 0x00); // Data
    assert_eq!(bytes[23..27], [0x00, 0x00, 0x00, 0x00]); // Frame Check Sequence
}

#[test]
fn EthernetFrame_FromBytes_CreatesIdenticalEthernetFrame() {
    // Arrange
    let ethernet_frame = EthernetFrame::new(
        [0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        [0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
        vec![0x00]
    );

    let bytes = ethernet_frame.to_bytes();

    // Act
    let result = EthernetFrame::from_bytes(&bytes);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), ethernet_frame);

}
