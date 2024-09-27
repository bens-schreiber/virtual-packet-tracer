pub type IPv4Address = [u8; 4];

pub struct IPv4Frame {
    version_hlen : u8,              // 4 bits version, 4 bits header length
    tos : u8,                       // Type of service
    total_length : u16,             // Total length of the frame
    id : u16,
    flags_fragment_offset : u16,    // 3 bits flags, 13 bits fragment offset
    ttl : u8,                       // Time to live
    protocol : u8,
    checksum : u16,
    source : IPv4Address,
    destination : IPv4Address,
    option: Vec<u8>,
    data : Vec<u8>
}

impl IPv4Frame {
    pub fn new(source: IPv4Address, destination: IPv4Address, data: Vec<u8>) -> IPv4Frame {
        IPv4Frame {
            version_hlen: 0x45, // IPv4, 5 words
            tos: 0,
            total_length: 20 + data.len() as u16,
            id: 0,
            flags_fragment_offset: 0,
            ttl: 64,            // Default TTL
            protocol: 0,
            checksum: 0,        // TODO: Calculate checksum
            source,
            destination,
            option: Vec::new(),
            data
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(self.version_hlen);
        bytes.push(self.tos);
        bytes.extend_from_slice(&self.total_length.to_be_bytes());
        bytes.extend_from_slice(&self.id.to_be_bytes());
        bytes.extend_from_slice(&self.flags_fragment_offset.to_be_bytes());
        bytes.push(self.ttl);
        bytes.push(self.protocol);
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes.extend_from_slice(&self.source);
        bytes.extend_from_slice(&self.destination);
        bytes.extend_from_slice(&self.option);
        bytes.extend_from_slice(&self.data);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<IPv4Frame, &'static str> {
        if bytes.len() < 20 {
            return Err("IPv4 frame does not have 20 bytes");
        }

        if bytes.len() > 65535 {
            return Err("IPv4 frame is too large");
        }

        let version_hlen = bytes[0];
        let tos = bytes[1];
        let total_length = u16::from_be_bytes([bytes[2], bytes[3]]);
        let id = u16::from_be_bytes([bytes[4], bytes[5]]);
        let flags_fragment_offset = u16::from_be_bytes([bytes[6], bytes[7]]);
        let ttl = bytes[8];
        let protocol = bytes[9];
        let checksum = u16::from_be_bytes([bytes[10], bytes[11]]);
        let source = [bytes[12], bytes[13], bytes[14], bytes[15]];
        let destination = [bytes[16], bytes[17], bytes[18], bytes[19]];
        let option = bytes[20..(version_hlen & 0x0F) as usize * 4].to_vec();
        let data = bytes[(version_hlen & 0x0F) as usize * 4..].to_vec();
        
        Ok(IPv4Frame {
            version_hlen,
            tos,
            total_length,
            id,
            flags_fragment_offset,
            ttl,
            protocol,
            checksum,
            source,
            destination,
            option,
            data,
        })
    }
}