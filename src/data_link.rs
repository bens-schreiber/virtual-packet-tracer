pub type MacAddress = [u8; 6];


/// IEEE 802.3 Ethernet Frame
#[derive(Debug, PartialEq)]
pub struct EthernetFrame {
    preamble: [u8; 7],
    start_frame_delimiter: u8,
    destination_address: MacAddress,
    source_address: MacAddress,
    length: u16,
    data: Vec<u8>,
    frame_check_sequence: u32,
}

impl EthernetFrame {
    pub fn new(destination_address: MacAddress, source_address: MacAddress, data: Vec<u8>) -> EthernetFrame {
        EthernetFrame {
            preamble: [0x55; 7],            // 7 bytes of 0x55
            start_frame_delimiter: 0xD5,    // 1 byte of 0xD5
            destination_address,
            source_address,
            length: data.len() as u16,
            data,
            frame_check_sequence: 0,
        }
    }

    pub fn from_bytes(bytes: &Vec<u8>) -> Result<EthernetFrame, std::io::Error>  {

        if bytes.len() < 26 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Insufficient bytes for Ethernet frame"));
        }

        // Ignore the preamble and start frame delimiter. Unnecessary for virtual simulation.
        let preamble = [0x55; 7];
        let start_frame_delimiter = 0xD5;

        
        let destination_address = [bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13]];
        let source_address = [bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19]];

        let length = u16::from_be_bytes([bytes[20], bytes[21]]);

        let data = bytes[22..22 + length as usize].to_vec();

        let frame_check_sequence = u32::from_be_bytes([bytes[22 + length as usize], bytes[23 + length as usize], bytes[24 + length as usize], bytes[25 + length as usize]]);

        Ok(EthernetFrame {
            preamble,
            start_frame_delimiter,
            destination_address,
            source_address,
            length,
            data,
            frame_check_sequence,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.preamble);
        bytes.push(self.start_frame_delimiter);
        bytes.extend_from_slice(&self.destination_address);
        bytes.extend_from_slice(&self.source_address);
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.extend_from_slice(&self.data);
        bytes.extend_from_slice(&self.frame_check_sequence.to_be_bytes());
        bytes
    }
}