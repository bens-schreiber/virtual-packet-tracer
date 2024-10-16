#![allow(non_snake_case)]

use crate::device::cable::CableSimulator;
use crate::device::switch::{BpduFrame, Switch};
use crate::ethernet::{interface::*, ByteSerialize, EtherType, EthernetFrame};
use crate::{bridge_id, eth2, eth2_data, mac_addr, mac_bpdu_addr, mac_broadcast_addr};

#[test]
pub fn Forward_ReceiveNotInTable_FloodsFrame() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut i2 = EthernetInterface::new(mac_addr!(2));
    let mut i3 = EthernetInterface::new(mac_addr!(3));
    let mut switch = Switch::from_seed(4, 1);

    switch.connect(0, &mut i1);
    switch.connect(1, &mut i2);
    switch.connect(2, &mut i3);

    sim.adds(vec![i1.port(), i2.port(), i3.port()]);

    sim.adds(switch.ports());

    // Act
    i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
    sim.tick();
    switch.forward();
    sim.tick();

    let i1_data = i1.receive();
    let i2_data = i2.receive();
    let i3_data = i3.receive();

    // Assert
    assert!(i1_data.is_empty());

    assert!(i2_data.len() == 1);
    assert_eq!(
        i2_data[0],
        eth2!(
            i2.mac_address,
            i1.mac_address,
            eth2_data!(1),
            EtherType::Debug
        )
    );

    assert!(i3_data.len() == 1);
    assert_eq!(
        i3_data[0],
        eth2!(
            i2.mac_address,
            i1.mac_address,
            eth2_data!(1),
            EtherType::Debug
        )
    );
}

#[test]
pub fn Forward_ReceiveInTable_ForwardsFrame() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut i2 = EthernetInterface::new(mac_addr!(2));
    let mut i3 = EthernetInterface::new(mac_addr!(3));
    let mut switch = Switch::from_seed(4, 1);

    switch.connect(0, &mut i1);
    switch.connect(1, &mut i2);
    switch.connect(2, &mut i3);

    sim.adds(vec![i1.port(), i2.port(), i3.port()]);

    sim.adds(switch.ports());

    i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
    sim.tick();
    switch.forward(); // Switch learns MAC address of i1
    sim.tick();
    i2.receive(); // dump incoming data
    i3.receive(); // dump incoming data

    // Act
    i2.send(i1.mac_address, EtherType::Debug, eth2_data!(1));
    sim.tick();
    switch.forward();
    sim.tick();

    let i1_data = i1.receive();
    let received_data3 = i3.receive();

    // Assert
    assert!(i1_data.len() == 1);
    assert_eq!(
        i1_data[0],
        eth2!(
            i1.mac_address,
            i2.mac_address,
            eth2_data!(1),
            EtherType::Debug
        )
    );

    assert!(received_data3.is_empty());
}

#[test]
fn Forward_ReceiveBroadcastAddr_DoesNotUpdateTableAndFloodsFrame() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut i2 = EthernetInterface::new(mac_addr!(2));
    let mut i3 = EthernetInterface::new(mac_addr!(3));
    let mut switch = Switch::from_seed(4, 1);

    switch.connect(0, &mut i1);
    switch.connect(1, &mut i2);
    switch.connect(2, &mut i3);

    sim.adds(vec![i1.port(), i2.port(), i3.port()]);

    sim.adds(switch.ports());

    // Act
    i1.send(mac_broadcast_addr!(), EtherType::Debug, eth2_data!(1)); // Send broadcast
    sim.tick();
    switch.forward();
    sim.tick();

    let i2_data = i2.receive(); // Receive broadcast
    let i3_data = i3.receive(); // Receive broadcast

    i1.send(mac_broadcast_addr!(), EtherType::Debug, eth2_data!(2)); // Send broadcast
    sim.tick();
    switch.forward();
    sim.tick();

    let i2_data2 = i2.receive(); // Receive broadcast
    let i3_data2 = i3.receive(); // Receive broadcast

    // Assert
    let frame1 = eth2!(
        mac_broadcast_addr!(),
        i1.mac_address,
        eth2_data!(1),
        EtherType::Debug
    );

    assert!(i2_data.len() == 1);
    assert_eq!(i2_data[0], frame1);

    assert!(i3_data.len() == 1);
    assert_eq!(i3_data[0], frame1);

    let frame2 = eth2!(
        mac_broadcast_addr!(),
        i1.mac_address,
        eth2_data!(2),
        EtherType::Debug
    );

    assert!(i2_data2.len() == 1);
    assert_eq!(i2_data2[0], frame2);

    assert!(i3_data2.len() == 1);
    assert_eq!(i3_data2[0], frame2);
}

#[test]
fn SpanningTree_Init_SendsBpdus() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut switch = Switch::from_seed(2, 1);

    let switch_port = 0;
    switch.connect(switch_port, &mut i1);

    sim.add(i1.port());
    sim.adds(switch.ports());

    // Act
    switch.init_stp();
    sim.tick();
    switch.finish_init_stp();

    let i1_data = i1.receive();

    // Assert
    assert!(i1_data.len() == 1);
    let e802_3 = match &i1_data[0] {
        EthernetFrame::Ethernet802_3(frame) => frame,
        _ => panic!("Expected Ethernet802_3 frame"),
    };

    assert!(e802_3.destination_address == mac_bpdu_addr!());

    let bpdu = match BpduFrame::from_bytes(e802_3.data.clone()) {
        Ok(bpdu) => bpdu,
        Err(_) => panic!("Expected BpduFrame"),
    };

    assert_eq!(
        bpdu,
        BpduFrame::hello(
            switch.mac_address,
            bridge_id!(switch.mac_address, switch.bridge_priority),
            0,
            bridge_id!(switch.mac_address, switch.bridge_priority),
            switch_port as u16
        )
    )
}

#[test]
fn SpanningTree_Init_DiscardsEndDevices() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut switch = Switch::from_seed(2, 1);

    let switch_port = 0;
    switch.connect(switch_port, &mut i1);

    sim.add(i1.port());
    sim.adds(switch.ports());

    // Act
    i1.send(i1.mac_address, EtherType::Debug, eth2_data!(1)); // attempt to send data to self
    switch.init_stp();
    sim.tick();
    switch.forward();
    switch.finish_init_stp();
    sim.tick();

    let i1_data = i1.receive_eth2();

    // Assert
    assert!(i1_data.is_empty());
}

#[test]
fn SpanningTree_FinishInit_ForwardsEndDevices() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut i1 = EthernetInterface::new(mac_addr!(1));
    let mut switch = Switch::from_seed(2, 1);

    let switch_port = 0;
    switch.connect(switch_port, &mut i1);

    sim.add(i1.port());
    sim.adds(switch.ports());

    // Act
    switch.init_stp();
    sim.tick();
    switch.forward();
    switch.finish_init_stp();
    i1.send(i1.mac_address, EtherType::Debug, eth2_data!(1)); // attempt to send data to self
    sim.tick();
    switch.forward();
    sim.tick();

    let i1_data = i1.receive_eth2();

    // Assert
    assert!(i1_data.len() == 1);
}

#[test]
fn SpanningTree_BiConnect_ElectsRootPortAndDesignatedPort() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut s1 = Switch::from_seed(1, 1);
    let mut s2 = Switch::from_seed(35, 2);

    let s1_port = 0;
    let s2_port = 1;
    s1.connect_switch(s1_port, &mut s2, s2_port);

    sim.adds(s1.ports());
    sim.adds(s2.ports());

    // Act
    s1.init_stp();
    s2.init_stp();
    sim.tick();
    s1.forward();
    s2.forward();
    sim.tick();
    s1.finish_init_stp();
    s2.finish_init_stp();

    // Assert
    assert!(s1.root_port.is_none());
    assert!(s1.root_bid == s1.bid());
    // assert!(s1.designated_ports().len() == 1);
    assert!(s1.designated_ports().contains(&s1_port));
    assert!(s1.disabled_ports().len() == 0);

    assert!(s2.root_bid == s1.bid());
    assert!(s2.root_port == Some(s2_port));
    // assert!(s2.designated_ports().len() == 1);
    assert!(!s2.designated_ports().contains(&s2_port));
}

#[test]
fn SpanningTree_CompleteGraph_ElectsRootPortAndDesignatedPortsAndDisabledPorts() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut s1 = Switch::from_seed(1, 1);
    let mut s2 = Switch::from_seed(33, 2);
    let mut s3 = Switch::from_seed(65, 3);

    let s1_s2_port = 0;
    let s1_s3_port = 1;

    let s2_s1_port = 0;
    let s2_s3_port = 1;

    let s3_s1_port = 0;
    let s3_s2_port = 1;

    s1.connect_switch(s1_s2_port, &mut s2, s2_s1_port);
    s1.connect_switch(s1_s3_port, &mut s3, s3_s1_port);
    s2.connect_switch(s2_s3_port, &mut s3, s3_s2_port);

    sim.adds(s1.ports());
    sim.adds(s2.ports());
    sim.adds(s3.ports());

    // Act
    s1.init_stp();
    s2.init_stp();
    s3.init_stp();

    for _ in 0..10 {
        // unscientifically determined number of iterations to converge
        sim.tick();
        s1.forward();
        s2.forward();
        s3.forward();
    }

    s1.finish_init_stp();
    s2.finish_init_stp();
    s3.finish_init_stp();

    // Assert
    assert!(s1.root_bid == s1.bid());
    assert!(s1.root_port.is_none());
    assert!(
        s1.designated_ports().contains(&s1_s2_port) && s1.designated_ports().contains(&s1_s3_port)
    );
    assert!(s1.disabled_ports().len() == 0);

    assert!(s2.root_bid == s1.bid());
    assert!(s2.root_port == Some(s2_s1_port));
    assert!(s2.designated_ports().contains(&s2_s3_port));
    assert!(s2.disabled_ports().len() == 0);

    assert!(s3.root_bid == s1.bid());
    assert!(s3.root_port == Some(s3_s1_port));
    assert!(!s3.designated_ports().contains(&s3_s2_port));
    assert!(s3.disabled_ports().contains(&s3_s2_port));
}
