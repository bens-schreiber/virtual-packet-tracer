# Virtual Packet Tracer by Benjamin Schreiber

Live WASM compiled demo https://bschr.dev/vpt

VPT is a Cisco Packet Tracer inspired simulation tool which allows you to create virtual network environments, test communication between devices, trace packets, and inspect input and output.
Packets are fully serialized to byte level before being transmitted across devices. Currently simulates layers 1 2 and 3 of the OSI model and stays true to their IEEE standards.

https://github.com/user-attachments/assets/9105ace3-b319-4b42-a408-ab7a0ed28a1a

The network features:

1. Physical
- Ethernet ports
- Ethernet connections (via `CableSimulator`)

2. Data Link
- Mac Addresses
- Ethernet II communication
- Ethernet 802.3 communication (reserved for Switch RSTP)
- Address Resolution Protocol
- Layer 2 Switches
- Rapid Spanning Tree Protocol, BPDUs

3. Network Layer
- Ipv4 Addresses
- Subnetting
- Ipv4 Communication
- ICMP Communication
- ARP Tables, ARP Packet Buffer
- Layer 3 Desktop
- Layer 3 Router
- RIP Protocol

This was originally semester project for WSU CPTS 327, but quickly became very large. Because of my limited time to work on it, theres a couple TODOs that became out of scope I'd like to resolve before considering this a complete sim of the first 3 layers of the OSI model:
- [ ] Ethernet II and 802.3 Frame Check Sequence
- [ ] Ipv4 Checksums
- [ ] ICMP Checksums
- [ ] Prefix tries for router routing table
- [ ] Creation and handling of custom data outside of the standard protocols.
- [x] ~~Better UI for packet inspection~~
- [x] ~~Advanced packet inspection (see each field from eth2 up to ipv4 and its data)~~
- [x]~~ More commands for routers, desktops and switches such as `arp`~~
- [x] ~~Cleanup and reduce GUI code~~

There are several bugs I've noticed when testing I'd also like to fix:
- [x] ~~Creating multiple unique devices and deleting them can sometimes crash the sim~~
- [x] ~~RSTP fails to block redundancies in advanced graphs under elapsed time (unit tests prove the alg works programmically, realtime simulation produces interesting edge cases)~~
- [x] ~~Either change the default font or make all fonts a multiple of the default font size (10) to get rid of blur on WASM build~~
- [ ] ...many small UI bugs



