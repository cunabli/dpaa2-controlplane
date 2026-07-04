# Dpaa2-Linux-Kernel - Dpaa2

**Pages:** 4

---

## DPAA2 (Data Path Acceleration Architecture Gen2) Overview — The Linux Kernel documentation

**URL:** https://docs.kernel.org/networking/device_drivers/ethernet/freescale/dpaa2/overview.html

**Contents:**
- DPAA2 (Data Path Acceleration Architecture Gen2) Overview¶
- Introduction¶
- Overview of DPAA2 Objects¶
  - DPRC (Datapath Resource Container)¶
  - DPAA2 Objects for an Ethernet Network Interface¶
    - DPMAC (Datapath Ethernet MAC)¶
    - DPNI (Datapath Network Interface)¶
    - DPIO (Datapath I/O)¶
    - DPBP (Datapath Buffer Pool)¶
    - DPMCP (Datapath MC Portal)¶

© 2015 Freescale Semiconductor Inc.

This document provides an overview of the Freescale DPAA2 architecture and how it is integrated into the Linux kernel.

DPAA2 is a hardware architecture designed for high-speeed network packet processing. DPAA2 consists of sophisticated mechanisms for processing Ethernet packets, queue management, buffer management, autonomous L2 switching, virtual Ethernet bridging, and accelerator (e.g. crypto) sharing.

A DPAA2 hardware component called the Management Complex (or MC) manages the DPAA2 hardware resources. The MC provides an object-based abstraction for software drivers to use the DPAA2 hardware. The MC uses DPAA2 hardware resources such as queues, buffer pools, and network ports to create functional objects/devices such as network interfaces, an L2 switch, or accelerator instances. The MC provides memory-mapped I/O command interfaces (MC portals) which DPAA2 software drivers use to operate on DPAA2 objects.

The diagram below shows an overview of the DPAA2 resource management architecture:

The MC mediates operations such as create, discover, connect, configuration, and destroy. Fast-path operations on data, such as packet transmit/receive, are not mediated by the MC and are done directly using memory mapped regions in DPIO objects.

The section provides a brief overview of some key DPAA2 objects. A simple scenario is described illustrating the objects involved in creating a network interfaces.

A DPRC is a container object that holds all the other types of DPAA2 objects. In the example diagram below there are 8 objects of 5 types (DPMCP, DPIO, DPBP, DPNI, and DPMAC) in the container.

From the point of view of an OS, a DPRC behaves similar to a plug and play bus, like PCI. DPRC commands can be used to enumerate the contents of the DPRC, discover the hardware objects present (including mappable regions and interrupts).

Hardware objects can be created and destroyed dynamically, providing the ability to hot plug/unplug objects in and out of the DPRC.

A DPRC has a mappable MMIO region (an MC portal) that can be used to send MC commands. It has an interrupt for status events (like hotplug). All objects in a container share the same hardware “isolation context”. This means that with respect to an IOMMU the isolation granularity is at the DPRC (container) level, not at the individual object level.

DPRCs can be defined statically and populated with objects via a config file passed to the MC when firmware starts it.

A typical Ethernet NIC is monolithic-- the NIC device contains TX/RX queuing mechanisms, configuration mechanisms, buffer management, physical ports, and interrupts. DPAA2 uses a more granular approach utilizing multiple hardware objects. Each object provides specialized functions. Groups of these objects are used by software to provide Ethernet network interface functionality. This approach provides efficient use of finite hardware resources, flexibility, and performance advantages.

The diagram below shows the objects needed for a simple network interface configuration on a system with 2 CPUs.

Below the objects are described. For each object a brief description is provided along with a summary of the kinds of operations the object supports and a summary of key resources of the object (MMIO regions and IRQs).

Represents an Ethernet MAC, a hardware device that connects to an Ethernet PHY and allows physical transmission and reception of Ethernet frames.

IRQs: DPNI link change

commands: set link up/down, link config, get stats, IRQ config, enable, reset

Contains TX/RX queues, network interface configuration, and RX buffer pool configuration mechanisms. The TX/RX queues are in memory and are identified by queue number.

commands: port config, offload config, queue config, parse/classify config, IRQ config, enable, reset

Provides interfaces to enqueue and dequeue packets and do hardware buffer pool management operations. The DPAA2 architecture separates the mechanism to access queues (the DPIO object) from the queues themselves. The DPIO provides an MMIO interface to enqueue/dequeue packets. To enqueue something a descriptor is written to the DPIO MMIO region, which includes the target queue number. There will typically be one DPIO assigned to each CPU. This allows all CPUs to simultaneously perform enqueue/dequeued operations. DPIOs are expected to be shared by different DPAA2 drivers.

MMIO regions: queue operations, buffer management

IRQs: data availability, congestion notification, buffer pool depletion

commands: IRQ config, enable, reset

Represents a hardware buffer pool.

commands: enable, reset

Provides an MC command portal. Used by drivers to send commands to the MC to manage objects.

MMIO regions: MC command portal

IRQs: command completion

commands: IRQ config, enable, reset

Some objects have explicit relationships that must be configured:

DPNI <--> L2-switch-port

A DPNI must be connected to something such as a DPMAC, another DPNI, or L2 switch port. The DPNI connection is made via a DPRC command.

A network interface requires a ‘buffer pool’ (DPBP object) which provides a list of pointers to memory where received Ethernet data is to be copied. The Ethernet driver configures the DPBPs associated with the network interface.

All interrupts generated by DPAA2 objects are message interrupts. At the hardware level message interrupts generated by devices will normally have 3 components-- 1) a non-spoofable ‘device-id’ expressed on the hardware bus, 2) an address, 3) a data value.

In the case of DPAA2 devices/objects, all objects in the same container/DPRC share the same ‘device-id’. For ARM-based SoC this is the same as the stream ID.

This section provides an overview of the Linux kernel drivers for DPAA2-- 1) the bus driver and associated “DPAA2 infrastructure” drivers and 2) functional object drivers (such as Ethernet).

As described previously, a DPRC is a container that holds the other types of DPAA2 objects. It is functionally similar to a plug-and-play bus controller. Each object in the DPRC is a Linux “device” and is bound to a driver. The diagram below shows the Linux drivers involved in a networking scenario and the objects bound to each driver. A brief description of each driver follows.

A brief description of each driver is provided below.

The MC-bus driver is a platform driver and is probed from a node in the device tree (compatible “fsl,qoriq-mc”) passed in by boot firmware. It is responsible for bootstrapping the DPAA2 kernel infrastructure. Key functions include:

registering a new bus type named “fsl-mc” with the kernel, and implementing bus call-backs (e.g. match/uevent/dev_groups)

implementing APIs for DPAA2 driver registration and for device add/remove

creates an MSI IRQ domain

doing a ‘device add’ to expose the ‘root’ DPRC, in turn triggering a bind of the root DPRC to the DPRC driver

The binding for the MC-bus device-tree node can be consulted at Documentation/devicetree/bindings/misc/fsl,qoriq-mc.yaml. The sysfs bind/unbind interfaces for the MC-bus can be consulted at ABI file testing/sysfs-bus-fsl-mc.

The DPRC driver is bound to DPRC objects and does runtime management of a bus instance. It performs the initial bus scan of the DPRC and handles interrupts for container events such as hot plug by re-scanning the DPRC.

Certain objects such as DPMCP and DPBP are generic and fungible, and are intended to be used by other drivers. For example, the DPAA2 Ethernet driver needs:

DPMCPs to send MC commands, to configure network interfaces

DPBPs for network buffer pools

The allocator driver registers for these allocatable object types and those objects are bound to the allocator when the bus is probed. The allocator maintains a pool of objects that are available for allocation by other DPAA2 drivers.

The DPIO driver is bound to DPIO objects and provides services that allow other drivers such as the Ethernet driver to enqueue and dequeue data for their respective objects. Key services include:

data availability notifications

hardware queuing operations (enqueue and dequeue of data)

hardware buffer pool management

To transmit a packet the Ethernet driver puts data on a queue and invokes a DPIO API. For receive, the Ethernet driver registers a data availability notification callback. To dequeue a packet a DPIO API is used. There is typically one DPIO object per physical CPU for optimum performance, allowing different CPUs to simultaneously enqueue and dequeue data.

The DPIO driver operates on behalf of all DPAA2 drivers active in the kernel-- Ethernet, crypto, compression, etc.

The Ethernet driver is bound to a DPNI and implements the kernel interfaces needed to connect the DPAA2 network interface to the network stack. Each DPNI corresponds to a Linux network interface.

An Ethernet PHY is an off-chip, board specific component and is managed by the appropriate PHY driver via an mdio bus. The MAC driver plays a role of being a proxy between the PHY driver and the MC. It does this proxy via the MC commands to a DPMAC object. If the PHY driver signals a link change, the MAC driver notifies the MC via a DPMAC command. If a network interface is brought up or down, the MC notifies the DPMAC driver via an interrupt and the driver can take appropriate action.

---

## DPAA2 DPIO (Data Path I/O) Overview — The Linux Kernel documentation

**URL:** https://docs.kernel.org/networking/device_drivers/ethernet/freescale/dpaa2/dpio-driver.html

**Contents:**
- DPAA2 DPIO (Data Path I/O) Overview¶
- Introduction¶
  - Driver Overview¶
  - DPIO Object Driver (dpio-driver.c)¶
  - DPIO service (dpio-service.c, dpaa2-io.h)¶
  - QBman portal interface (qbman-portal.c)¶
  - Other (dpaa2-fd.h, dpaa2-global.h)¶

This document provides an overview of the Freescale DPAA2 DPIO drivers

A DPAA2 DPIO (Data Path I/O) is a hardware object that provides interfaces to enqueue and dequeue frames to/from network interfaces and other accelerators. A DPIO also provides hardware buffer pool management for network interfaces.

This document provides an overview the Linux DPIO driver, its subcomponents, and its APIs.

See DPAA2 (Data Path Acceleration Architecture Gen2) Overview for a general overview of DPAA2 and the general DPAA2 driver architecture in Linux.

The DPIO driver is bound to DPIO objects discovered on the fsl-mc bus and provides services that:

allow other drivers, such as the Ethernet driver, to enqueue and dequeue frames for their respective objects

allow drivers to register callbacks for data availability notifications when data becomes available on a queue or channel

allow drivers to manage hardware buffer pools

DPIO object driver-- fsl-mc driver that manages the DPIO object

DPIO service-- provides APIs to other Linux drivers for services

QBman portal interface-- sends portal commands, gets responses:

The diagram below shows how the DPIO driver components fit with the other DPAA2 Linux driver components:

The dpio-driver component registers with the fsl-mc bus to handle objects of type “dpio”. The implementation of probe() handles basic initialization of the DPIO including mapping of the DPIO regions (the QBman SW portal) and initializing interrupts and registering irq handlers. The dpio-driver registers the probed DPIO with dpio-service.

The dpio service component provides queuing, notification, and buffers management services to DPAA2 drivers, such as the Ethernet driver. A system will typically allocate 1 DPIO object per CPU to allow queuing operations to happen simultaneously across all CPUs.

dpaa2_io_service_register()

dpaa2_io_service_deregister()

dpaa2_io_service_rearm()

dpaa2_io_service_pull_fq()

dpaa2_io_service_pull_channel()

dpaa2_io_service_enqueue_fq()

dpaa2_io_service_enqueue_qd()

dpaa2_io_store_create()

dpaa2_io_store_destroy()

dpaa2_io_store_next()

dpaa2_io_service_release()

dpaa2_io_service_acquire()

The qbman-portal component provides APIs to do the low level hardware bit twiddling for operations such as:

initializing Qman software portals

building and sending portal commands

portal interrupt configuration and processing

The qbman-portal APIs are not public to other drivers, and are only used by dpio-service.

Frame descriptor and scatter-gather definitions and the APIs used to manipulate them are defined in dpaa2-fd.h.

Dequeue result struct and parsing APIs are defined in dpaa2-global.h.

---

## DPAA2 MAC / PHY support — The Linux Kernel documentation

**URL:** https://docs.kernel.org/networking/device_drivers/ethernet/freescale/dpaa2/mac-phy-support.html

**Contents:**
- DPAA2 MAC / PHY support¶
- Overview¶
- DPAA2 Software Architecture¶
- Implementation¶
- Exported API¶

The DPAA2 MAC / PHY support consists of a set of APIs that help DPAA2 network drivers (dpaa2-eth, dpaa2-ethsw) interact with the PHY library.

Among other DPAA2 objects, the fsl-mc bus exports DPNI objects (abstracting a network interface) and DPMAC objects (abstracting a MAC). The dpaa2-eth driver probes on the DPNI object and connects to and configures a DPMAC object with the help of phylink.

Data connections may be established between a DPNI and a DPMAC, or between two DPNIs. Depending on the connection type, the netif_carrier_[on/off] is handled directly by the dpaa2-eth driver or by phylink.

Depending on an MC firmware configuration setting, each MAC may be in one of two modes:

DPMAC_LINK_TYPE_FIXED: the link state management is handled exclusively by the MC firmware by polling the MAC PCS. Without the need to register a phylink instance, the dpaa2-eth driver will not bind to the connected dpmac object at all.

DPMAC_LINK_TYPE_PHY: The MC firmware is left waiting for link state update events, but those are in fact passed strictly between the dpaa2-mac (based on phylink) and its attached net_device driver (dpaa2-eth, dpaa2-ethsw), effectively bypassing the firmware.

At probe time or when a DPNI’s endpoint is dynamically changed, the dpaa2-eth is responsible to find out if the peer object is a DPMAC and if this is the case, to integrate it with PHYLINK using the dpaa2_mac_connect() API, which will do the following:

look up the device tree for PHYLINK-compatible of binding (phy-handle)

will create a PHYLINK instance associated with the received net_device

connect to the PHY using phylink_of_phy_connect()

The following phylink_mac_ops callback are implemented:

.validate() will populate the supported linkmodes with the MAC capabilities only when the phy_interface_t is RGMII_* (at the moment, this is the only link type supported by the driver).

.mac_config() will configure the MAC in the new configuration using the dpmac_set_link_state() MC firmware API.

.mac_link_up() / .mac_link_down() will update the MAC link using the same API described above.

At driver unbind() or when the DPNI object is disconnected from the DPMAC, the dpaa2-eth driver calls dpaa2_mac_disconnect() which will, in turn, disconnect from the PHY and destroy the PHYLINK instance.

In case of a DPNI-DPMAC connection, an ‘ip link set dev eth0 up’ would start the following sequence of operations:

phylink_start() called from .dev_open().

The .mac_config() and .mac_link_up() callbacks are called by PHYLINK.

In order to configure the HW MAC, the MC Firmware API dpmac_set_link_state() is called.

The firmware will eventually setup the HW MAC in the new configuration.

A netif_carrier_on() call is made directly from PHYLINK on the associated net_device.

The dpaa2-eth driver handles the LINK_STATE_CHANGE irq in order to enable/disable Rx taildrop based on the pause frame settings.

In case of a DPNI-DPNI connection, a usual sequence of operations looks like the following:

ip link set dev eth0 up

The dpni_enable() MC API called on the associated fsl_mc_device.

ip link set dev eth1 up

The dpni_enable() MC API called on the associated fsl_mc_device.

The LINK_STATE_CHANGED irq is received by both instances of the dpaa2-eth driver because now the operational link state is up.

The netif_carrier_on() is called on the exported net_device from link_state_update().

Any DPAA2 driver that drivers endpoints of DPMAC objects should service its _EVENT_ENDPOINT_CHANGED irq and connect/disconnect from the associated DPMAC when necessary using the below listed API:

A phylink integration is necessary only when the partner DPMAC is not of TYPE_FIXED. This means it is either of TYPE_PHY, or of TYPE_BACKPLANE (the difference being the two that in the TYPE_BACKPLANE mode, the MC firmware does not access the PCS registers). One can check for this condition using the following helper:

Before connection to a MAC, the caller must allocate and populate the dpaa2_mac structure with the associated net_device, a pointer to the MC portal to be used and the actual fsl_mc_device structure of the DPMAC.

---

## DPAA2 Switch driver — The Linux Kernel documentation

**URL:** https://docs.kernel.org/networking/device_drivers/ethernet/freescale/dpaa2/switch-driver.html

**Contents:**
- DPAA2 Switch driver¶
- Creating an Ethernet Switch¶
- Switching features¶
- Offloads¶
  - Routing actions (redirect, trap, drop)¶
    - Mirroring¶

The DPAA2 Switch driver probes on the Datapath Switch (DPSW) object which can be instantiated on the following DPAA2 SoCs and their variants: LS2088A and LX2160A.

The driver uses the switch device driver model and exposes each switch port as a network interface, which can be included in a bridge or used as a standalone interface. Traffic switched between ports is offloaded into the hardware.

The DPSW can have ports connected to DPNIs or to DPMACs for external access.

The dpaa2-switch driver probes on DPSW devices found on the fsl-mc bus. These devices can be either created statically through the boot time configuration file - DataPath Layout (DPL) - or at runtime using the DPAA2 object APIs (incorporated already into the restool userspace tool).

At the moment, the dpaa2-switch driver imposes the following restrictions on the DPSW object that it will probe:

The minimum number of FDBs should be at least equal to the number of switch interfaces. This is necessary so that separation of switch ports can be done, ie when not under a bridge, each switch port will have its own FDB.

Both the broadcast and flooding configuration should be per FDB. This enables the driver to restrict the broadcast and flooding domains of each FDB depending on the switch ports that are sharing it (aka are under the same bridge).

The control interface of the switch should not be disabled (DPSW_OPT_CTRL_IF_DIS not passed as a create time option). Without the control interface, the driver is not capable to provide proper Rx/Tx traffic support on the switch port netdevices.

Besides the configuration of the actual DPSW object, the dpaa2-switch driver will need the following DPAA2 objects:

1 DPMCP - A Management Command Portal object is needed for any interaction with the MC firmware.

1 DPBP - A Buffer Pool is used for seeding buffers intended for the Rx path on the control interface.

Access to at least one DPIO object (Software Portal) is needed for any enqueue/dequeue operation to be performed on the control interface queues. The DPIO object will be shared, no need for a private one.

The driver supports the configuration of L2 forwarding rules in hardware for port bridging as well as standalone usage of the independent switch interfaces.

The hardware is not configurable with respect to VLAN awareness, thus any DPAA2 switch port should be used only in usecases with a VLAN aware bridge:

Topology and loop detection through STP is supported when stp_state 1 is used at bridge create

L2 FDB manipulation (add/delete/dump) is supported.

HW FDB learning can be configured on each switch port independently through bridge commands. When the HW learning is disabled, a fast age procedure will be run and any previously learnt addresses will be removed.

Restricting the unknown unicast and multicast flooding domain is supported, but not independently of each other:

Broadcast flooding on a switch port can be disabled/enabled through the brport sysfs:

The DPAA2 switch is able to offload flow-based redirection of packets making use of ACL tables. Shared filter blocks are supported by sharing a single ACL table between multiple ports.

The following flow keys are supported:

Ethernet: dst_mac/src_mac

IPv4: dst_ip/src_ip/ip_proto/tos

VLAN: vlan_id/vlan_prio/vlan_tpid/vlan_dei

L4: dst_port/src_port

Also, the matchall filter can be used to redirect the entire traffic received on a port.

As per flow actions, the following are supported:

mirred egress redirect

Each ACL entry (filter) can be setup with only one of the listed actions.

Example 1: send frames received on eth4 with a SA of 00:01:02:03:04:05 to the CPU:

Example 2: drop frames received on eth4 with VID 100 and PCP of 3:

Example 3: redirect all frames received on eth4 to eth1:

Example 4: Use a single shared filter block on both eth5 and eth6:

The DPAA2 switch supports only per port mirroring and per VLAN mirroring. Adding mirroring filters in shared blocks is also supported.

When using the tc-flower classifier with the 802.1q protocol, only the ‘’vlan_id’’ key will be accepted. Mirroring based on any other fields from the 802.1q protocol will be rejected:

If a mirroring VLAN filter is requested on a port, the VLAN must to be installed on the switch port in question either using ‘’bridge’’ or by creating a VLAN upper device if the switch port is used as a standalone interface:

Also, it should be noted that the mirrored traffic will be subject to the same egress restrictions as any other traffic. This means that when a mirrored packet will reach the mirror port, if the VLAN found in the packet is not installed on the port it will get dropped.

The DPAA2 switch supports only a single mirroring destination, thus multiple mirror rules can be installed but their ‘’to’’ port has to be the same:

---
