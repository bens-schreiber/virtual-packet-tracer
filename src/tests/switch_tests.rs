#![allow(non_snake_case)]

use crate::device::cable::CableSimulator;
use crate::device::switch::{BpduFrame, Switch};
use crate::ethernet::{interface::*, ByteSerialize, EtherType, EthernetFrame};
use crate::{eth2, eth2_data, mac_addr, mac_bpdu_addr, mac_broadcast_addr};

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
        BpduFrame::new(
            mac_bpdu_addr!(),
            switch.mac_address,
            false,
            BpduFrame::flags(true, true, 4, false, false, false),
            switch.bid(),
            0,
            switch.bid(),
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
    sim.tick();

    let i1_data = i1.receive_eth2();

    // Assert
    assert!(i1_data.is_empty());
}

#[test]
fn SpanningTree_SingleSwitch_ElectsSelfAsRoot() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut switch = Switch::from_seed(1, 1);

    sim.adds(switch.ports());

    // Act
    switch.init_stp();
    sim.tick();
    switch.forward();
    sim.tick();
    switch.finish_stp();

    // Assert
    assert!(switch.is_root_bridge());
    assert!(switch.root_port.is_none());
    assert!(switch.designated_ports().len() == 32);
}

#[test]
fn SpanningTree_FinishInit_NoMoreBpdus() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut switch = Switch::from_seed(1, 1);

    sim.adds(switch.ports());

    // Act
    switch.init_stp();
    let has_outgoing1 = switch.ports()[0].borrow().has_outgoing();
    sim.tick();

    switch.forward();
    let has_outgoing2 = switch.ports()[0].borrow().has_outgoing();
    sim.tick();

    switch.finish_stp();
    let has_outgoing3 = switch.ports()[0].borrow().has_outgoing();
    sim.tick();

    // Assert
    assert!(has_outgoing1);
    assert!(!has_outgoing2);
    assert!(!has_outgoing3);
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
    i1.send(i1.mac_address, EtherType::Debug, eth2_data!(1)); // attempt to send data to self
    switch.finish_stp();

    sim.tick();
    switch.forward();
    sim.tick();

    let i1_data = i1.receive_eth2();

    // Assert
    assert!(i1_data.len() == 1);
}

#[test]
fn SpanningTree_TwoConnectedFinishStp_BpdusEnd() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut s1 = Switch::from_seed(1, 1);
    let mut s2 = Switch::from_seed(35, 2);

    let s1_s2_port = 0;
    let s2_s1_port = 1;
    s1.connect_switch(s1_s2_port, &mut s2, s2_s1_port);

    sim.adds(s1.ports());
    sim.adds(s2.ports());

    // Act
    s1.init_stp();
    s2.init_stp();
    let s1_has_outgoing1 = s1.ports()[0].borrow().has_outgoing();
    let s2_has_outgoing1 = s2.ports()[1].borrow().has_outgoing();

    sim.tick();
    s1.forward();
    s2.forward();
    let s1_has_outgoing2 = s1.ports()[0].borrow().has_outgoing();
    let s2_has_outgoing2 = s2.ports()[1].borrow().has_outgoing();

    sim.tick();
    s1.finish_stp();
    s2.finish_stp();
    let s1_has_outgoing3 = s1.ports()[0].borrow().has_outgoing();
    let s2_has_outgoing3 = s2.ports()[1].borrow().has_outgoing();

    // Assert
    assert!(s1_has_outgoing1);
    assert!(s2_has_outgoing1);

    assert!(s1_has_outgoing2);
    assert!(s2_has_outgoing2);

    assert!(!s1_has_outgoing3);
    assert!(!s2_has_outgoing3);
}

#[test]
fn SpanningTree_TwoConnectedFinishStp_ElectsRootPortAndDesignatedPort() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut s1 = Switch::from_seed(1, 1);
    let mut s2 = Switch::from_seed(35, 2);

    let s1_s2_port = 0;
    let s2_s1_port = 1;
    s1.connect_switch(s1_s2_port, &mut s2, s2_s1_port);

    sim.adds(s1.ports());
    sim.adds(s2.ports());

    // Act
    s1.init_stp();
    s2.init_stp();
    sim.tick();
    s1.forward();
    s2.forward();
    sim.tick();
    s1.finish_stp();
    s2.finish_stp();

    // Assert
    assert!(s1.root_port.is_none());
    assert!(s1.is_root_bridge());
    assert!(s1.root_cost == 0);
    assert!(s1.designated_ports().contains(&s1_s2_port));
    assert!(s1.discarding_ports().len() == 0);

    assert!(s2.root_bid == s1.bid());
    assert!(s2.root_port == Some(s2_s1_port));
    assert!(!s2.designated_ports().contains(&s2_s1_port));
    assert!(s2.discarding_ports().len() == 0);
}

#[test]
fn SpanningTree_BiConnectEquivalentPriorities_ElectsWithBidTiebreaker() {
    // Arrange
    let mut sim = CableSimulator::new();
    let mut s1 = Switch::from_seed(100, 1); // Same priority, higher mac address
    let mut s2 = Switch::from_seed(35, 1); // Same priority, lower mac address    => S2 should win

    let s1_s2_port = 0;
    let s2_s1_port = 1;
    s1.connect_switch(s1_s2_port, &mut s2, s2_s1_port);

    sim.adds(s1.ports());
    sim.adds(s2.ports());

    // Act
    s1.init_stp();
    s2.init_stp();
    sim.tick();
    s1.forward();
    s2.forward();
    sim.tick();
    s1.finish_stp();
    s2.finish_stp();

    // Assert
    assert!(s1.root_port == Some(s1_s2_port));
    assert!(s1.root_bid == s2.bid());
    assert!(s1.root_cost == 1);
    assert!(!s1.designated_ports().contains(&s1_s2_port));
    assert!(s1.discarding_ports().len() == 0);

    assert!(s2.is_root_bridge());
    assert!(s2.root_port == None);
    assert!(s2.designated_ports().contains(&s2_s1_port));
    assert!(s2.discarding_ports().len() == 0);
}

// Helper for creating a complete graph topology (3 switches and 2 end devices
//      s1
//     /  \
// i2-s2---s3-i1
//
// Initializes and finishes the spanning tree protocol
fn complete_network() -> (
    CableSimulator,
    Switch,
    Switch,
    Switch,
    EthernetInterface,
    EthernetInterface,
    (usize, usize, usize, usize, usize, usize),
) {
    let mut sim = CableSimulator::new();

    let mut s1 = Switch::from_seed(1, 1);
    s1.set_debug_tag(1);
    let mut s2 = Switch::from_seed(33, 2);
    s2.set_debug_tag(2);
    let mut s3 = Switch::from_seed(65, 3);
    s3.set_debug_tag(3);

    let mut i1 = EthernetInterface::new(mac_addr!(100));
    let mut i2 = EthernetInterface::new(mac_addr!(200));

    let s1_s2_port = 0;
    let s1_s3_port = 1;

    let s2_s1_port = 0;
    let s2_s3_port = 1;

    let s3_s1_port = 0;
    let s3_s2_port = 1;

    s1.connect_switch(s1_s2_port, &mut s2, s2_s1_port);
    s1.connect_switch(s1_s3_port, &mut s3, s3_s1_port);
    s2.connect_switch(s2_s3_port, &mut s3, s3_s2_port);

    s3.connect(3, &mut i1);
    s2.connect(2, &mut i2);

    sim.adds(s1.ports());
    sim.adds(s2.ports());
    sim.adds(s3.ports());
    sim.adds(vec![i1.port(), i2.port()]);
    (
        sim,
        s1,
        s2,
        s3,
        i1,
        i2,
        (
            s1_s2_port, s1_s3_port, s2_s1_port, s2_s3_port, s3_s1_port, s3_s2_port,
        ),
    )
}

#[test]
fn SpanningTree_CompleteNetwork_BpdusEnd() {
    // Arrange
    let (
        sim,
        mut s1,
        mut s2,
        mut s3,
        _,
        _,
        (s1_s2_port, s1_s3_port, s2_s1_port, s2_s3_port, s3_s1_port, s3_s2_port),
    ) = complete_network();

    // Act
    s1.init_stp();
    s2.init_stp();
    s3.init_stp();
    let s1_has_outgoing_s2_1 = s1.ports()[s1_s2_port].borrow().has_outgoing();
    let s1_has_outgoing_s3_1 = s1.ports()[s1_s3_port].borrow().has_outgoing();
    let s2_has_outgoing_s1_1 = s2.ports()[s2_s1_port].borrow().has_outgoing();
    let s2_has_outgoing_s3_1 = s2.ports()[s2_s3_port].borrow().has_outgoing();
    let s3_has_outgoing_s1_1 = s3.ports()[s3_s1_port].borrow().has_outgoing();
    let s3_has_outgoing_s2_1 = s3.ports()[s3_s2_port].borrow().has_outgoing();

    sim.tick();
    s1.forward();
    s2.forward();
    s3.forward();
    let s1_has_outgoing_s2_2 = s1.ports()[s1_s2_port].borrow().has_outgoing();
    let s1_has_outgoing_s3_2 = s1.ports()[s1_s3_port].borrow().has_outgoing();
    let s2_has_outgoing_s1_2 = s2.ports()[s2_s1_port].borrow().has_outgoing();
    let s2_has_outgoing_s3_2 = s2.ports()[s2_s3_port].borrow().has_outgoing();
    let s3_has_outgoing_s1_2 = s3.ports()[s3_s1_port].borrow().has_outgoing();
    let s3_has_outgoing_s2_2 = s3.ports()[s3_s2_port].borrow().has_outgoing();

    sim.tick();
    s1.forward();
    s2.forward();
    s3.forward();
    let s1_has_outgoing_s2_3 = s1.ports()[s1_s2_port].borrow().has_outgoing();
    let s1_has_outgoing_s3_3 = s1.ports()[s1_s3_port].borrow().has_outgoing();
    let s2_has_outgoing_s1_3 = s2.ports()[s2_s1_port].borrow().has_outgoing();
    let s2_has_outgoing_s3_3 = s2.ports()[s2_s3_port].borrow().has_outgoing();
    let s3_has_outgoing_s1_3 = s3.ports()[s3_s1_port].borrow().has_outgoing();
    let s3_has_outgoing_s2_3 = s3.ports()[s3_s2_port].borrow().has_outgoing();

    // Assert
    assert!(s1_has_outgoing_s2_1);
    assert!(s1_has_outgoing_s3_1);
    assert!(s2_has_outgoing_s1_1);
    assert!(s2_has_outgoing_s3_1);
    assert!(s3_has_outgoing_s1_1);
    assert!(s3_has_outgoing_s2_1);

    assert!(s1_has_outgoing_s2_2);
    assert!(s1_has_outgoing_s3_2);
    assert!(s2_has_outgoing_s1_2);
    assert!(s2_has_outgoing_s3_2);
    assert!(s3_has_outgoing_s1_2);
    assert!(s3_has_outgoing_s2_2);

    assert!(!s1_has_outgoing_s2_3); // Everything settled
    assert!(!s1_has_outgoing_s3_3);
    assert!(!s2_has_outgoing_s1_3);
    assert!(!s2_has_outgoing_s3_3);
    assert!(!s3_has_outgoing_s1_3);
    assert!(!s3_has_outgoing_s2_3);
}

fn stp_complete_network() -> (
    CableSimulator,
    Switch,
    Switch,
    Switch,
    EthernetInterface,
    EthernetInterface,
    (usize, usize, usize, usize, usize, usize),
) {
    let (
        sim,
        mut s1,
        mut s2,
        mut s3,
        mut i1,
        mut i2,
        (s1_s2_port, s1_s3_port, s2_s1_port, s2_s3_port, s3_s1_port, s3_s2_port),
    ) = complete_network();

    s1.init_stp();
    s2.init_stp();
    s3.init_stp();

    for _ in 0..3 {
        sim.tick();
        s1.forward();
        s2.forward();
        s3.forward();
    }
    i1.receive(); // dump incoming data, just bpdu frames we don't care about
    i2.receive(); // dump incoming data, just bpdu frames we don't care about
    s1.finish_stp();
    s2.finish_stp();
    s3.finish_stp();
    (
        sim,
        s1,
        s2,
        s3,
        i1,
        i2,
        (
            s1_s2_port, s1_s3_port, s2_s1_port, s2_s3_port, s3_s1_port, s3_s2_port,
        ),
    )
}

#[test]
fn SpanningTree_CompleteGraph_ElectsRootPortAndDesignatedPortsAndDisabledPorts() {
    // Arrange
    let (
        _,
        s1,
        s2,
        s3,
        _,
        _,
        (s1_s2_port, s1_s3_port, s2_s1_port, s2_s3_port, s3_s1_port, s3_s2_port),
    ) = stp_complete_network();

    // Assert
    assert!(s1.is_root_bridge());
    assert!(s1.root_port.is_none());
    assert!(
        s1.designated_ports().contains(&s1_s2_port) && s1.designated_ports().contains(&s1_s3_port)
    );
    assert!(s1.discarding_ports().len() == 0);

    assert!(s2.root_bid == s1.bid());
    assert!(s2.root_port == Some(s2_s1_port));
    assert!(s2.designated_ports().contains(&s2_s3_port));
    assert!(s2.discarding_ports().len() == 0);

    assert!(s3.root_bid == s1.bid());
    assert!(s3.root_port == Some(s3_s1_port));
    assert!(!s3.designated_ports().contains(&s3_s2_port));
    assert!(s3.discarding_ports().contains(&s3_s2_port));
}

//                      (DP) s1 (DP)
//                      /         \
//                  (RP)           (RP)
//      i2 --- (DP) s2 (DP)----(BP) s3 (DP) --- i1
//
// i1 to i2 = i1 -> s3 -> s1 -> s2 -> i2
// i2 to i1 = i2 -> s2 -> s1 -> s3 -> i1
#[test]
fn SpanningTree_CompleteGraphFinishedStp_Ethernet2FramesDoNotUseBlockedPort() {
    // Arrange
    let (sim, mut s1, mut s2, mut s3, mut i1, mut i2, _) = stp_complete_network();

    // Act
    i1.send(i2.mac_address, EtherType::Debug, eth2_data!(1));
    i2.send(i1.mac_address, EtherType::Debug, eth2_data!(2));

    for _ in 0..3 {
        sim.tick();
        s1.forward();
        s2.forward();
        s3.forward();

        assert!(i1.receive().is_empty());
        assert!(i2.receive().is_empty());
    }

    sim.tick();

    let i1_data = i1.receive();
    let i2_data = i2.receive();

    // Assert
    assert!(i1_data.len() == 1);
    assert_eq!(
        i1_data[0],
        eth2!(
            i1.mac_address,
            i2.mac_address,
            eth2_data!(2),
            EtherType::Debug
        )
    );

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
}

#[test]
fn SpanningTree_ExistingNetworkReceiveTcnBpdu_UpdateDesignatedPorts() {
    // Arrange
    //
    //      s1
    //     /  \
    // i2-s2---s3-i1
    //          |
    //          s4
    //
    let (mut sim, mut s1, mut s2, mut s3, _, _, _) = stp_complete_network();
    let mut s4 = Switch::from_seed(97, 4);
    let s3_s4_port = 2;
    let s4_s3_port = 0;

    s3.connect_switch(s3_s4_port, &mut s4, s4_s3_port);
    sim.adds(s4.ports());

    // Act
    s4.init_stp();
    for _ in 0..10 {
        sim.tick();
        s1.forward();
        s2.forward();
        s3.forward();
        s4.forward();
    }

    // Assert
    assert!(s3.designated_ports().contains(&s3_s4_port));

    assert!(s4.root_bid == s1.bid());
    assert!(s4.root_port == Some(s4_s3_port));
}

#[test]
fn SpanningTree_ExistingNetworkRecieveTcnBpdu_UpdateRoot() {
    // Arrange
    //
    //      s1
    //     /  \
    // i2-s2---s3-i1
    //          |
    //          s4
    //
    let (
        mut sim,
        mut s1,
        mut s2,
        mut s3,
        _,
        _,
        (s1_s2_port, s1_s3_port, s2_s1_port, s2_s3_port, s3_s1_port, s3_s2_port),
    ) = stp_complete_network();
    let mut s4 = Switch::from_seed(97, 0);
    let s3_s4_port = 2;
    let s4_s3_port = 0;

    s3.connect_switch(s3_s4_port, &mut s4, s4_s3_port);
    sim.adds(s4.ports());

    // Act
    s4.init_stp();
    for _ in 0..100 {
        sim.tick();
        s1.forward();
        s2.forward();
        s3.forward();
        s4.forward();
    }
    s4.finish_stp();

    // Assert
    assert!(s4.root_bid == s4.bid());
    assert!(s4.root_port.is_none());
    assert!(s4.root_cost == 0);
    assert!(s4.discarding_ports().len() == 0);
    assert!(s4.designated_ports().contains(&s4_s3_port));

    assert!(s3.root_bid == s4.bid());
    assert!(s3.root_port == Some(s3_s4_port));
    assert!(s3.root_cost == 1);
    assert!(s3.discarding_ports().contains(&s3_s2_port));
    assert!(s3.designated_ports().contains(&s3_s1_port));

    assert!(s2.root_bid == s4.bid());
    assert!(s2.root_port == Some(s2_s1_port));
    assert!(s2.root_cost == 3);
    assert!(s2.discarding_ports().len() == 0);
    assert!(s2.designated_ports().contains(&s2_s3_port));

    assert!(s1.root_bid == s4.bid());
    assert!(s1.root_port == Some(s1_s3_port));
    assert!(s1.root_cost == 2);
    assert!(s1.discarding_ports().len() == 0);
    assert!(s1.designated_ports().contains(&s1_s2_port));
    assert!(s1.designated_ports().contains(&s1_s3_port));
}
