# Virtual Packet Tracer by Benjamin Schreiber

Live WASM compiled demo https://bschr.dev/vpt

VPT is a Cisco Packet Tracer inspired simulation tool which allows you to create virtual network environments, test communication between devices, trace packets, and inspect input and output.
Packets are fully serialized to byte level before being transmitted across devices. Currently simulates layers 1 2 and 3 of the OSI model and stays true to their IEEE standards.

Router example (RIP allows routers to know where outside networks are!)

https://github.com/user-attachments/assets/150a37f1-4128-45ca-8f9e-4f3ef637528d

RSTP Example (Notice real time topology changes!):

https://github.com/user-attachments/assets/1884d4c1-2c33-481e-8955-481e2ddbc143



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

This project was originally created as a semester project for WSU CPTS 327, and quickly became very large. Because of my limited time to work on it, theres a couple TODOs that became out of scope I'd like to resolve before considering this a complete tool:
- [ ] Ethernet II and 802.3 Frame Check Sequence
- [ ] Ipv4 Checksums
- [ ] ICMP Checksums
- [ ] Prefix tries for router routing table
- [ ] Creation and handling of custom data outside of the standard protocols.
- [ ] Allow dragging in Ethernet Connect mode
- [ ] Better UI for packet inspection
- [ ] Advanced packet inspection (see each field from eth2 up to ipv4 and its data)
- [ ] More commands for routers, desktops and switches such as `arp`
- [ ] Cleanup and reduce GUI code

Theres several bugs I've noticed when testing I'd also like to fix:
- [x] ~~Creating multiple unique devices and deleting them can sometimes crash the sim~~
- [x] ~~RSTP fails to block redundancies in advanced graphs under elapsed time (unit tests prove the alg works programmically, realtime simulation produces interesting edge cases)~~
- [ ] Either change the default font or make all fonts a multiple of the default font size (10) to get rid of blur on WASM build
- [ ] ...many small UI bugs



