# Dpaa2-Dpdk_Docs - Guides

**Pages:** 5

---

## 16. DPAA2 Poll Mode Driver — Data Plane Development Kit 26.07.0-rc2 documentation

**URL:** https://doc.dpdk.org/guides/nics/dpaa2.html

**Contents:**
- 16. DPAA2 Poll Mode Driver
- 16.1. NXP DPAA2 (Data Path Acceleration Architecture Gen2)
  - 16.1.1. DPAA2 Overview
  - 16.1.2. Overview of DPAA2 Objects
  - 16.1.3. DPAA2 Objects for an Ethernet Network Interface
  - 16.1.4. Object Connections
  - 16.1.5. Interrupts
- 16.2. DPAA2 DPDK - Poll Mode Driver Overview
  - 16.2.1. DPAA2 bus driver
  - 16.2.2. DPIO driver

The DPAA2 NIC PMD (librte_net_dpaa2) provides poll mode driver support for the inbuilt NIC found in the NXP DPAA2 SoC family.

More information can be found at NXP Official Website.

This section provides an overview of the NXP DPAA2 architecture and how it is integrated into the DPDK.

Overview of DPAA2 objects

DPAA2 driver architecture overview

Reference: FSL MC BUS in Linux Kernel.

DPAA2 is a hardware architecture designed for high-speed network packet processing. DPAA2 consists of sophisticated mechanisms for processing Ethernet packets, queue management, buffer management, autonomous L2 switching, virtual Ethernet bridging, and accelerator (e.g. crypto) sharing.

A DPAA2 hardware component called the Management Complex (or MC) manages the DPAA2 hardware resources. The MC provides an object-based abstraction for software drivers to use the DPAA2 hardware.

The MC uses DPAA2 hardware resources such as queues, buffer pools, and network ports to create functional objects/devices such as network interfaces, an L2 switch, or accelerator instances.

The MC provides memory-mapped I/O command interfaces (MC portals) which DPAA2 software drivers use to operate on DPAA2 objects:

The diagram below shows an overview of the DPAA2 resource management architecture:

The MC mediates operations such as create, discover, connect, configuration, and destroy. Fast-path operations on data, such as packet transmit/receive, are not mediated by the MC and are done directly using memory mapped regions in DPIO objects.

The section provides a brief overview of some key DPAA2 objects. A simple scenario is described illustrating the objects involved in creating a network interfaces.

DPRC (Datapath Resource Container)

A DPRC is a container object that holds all the other types of DPAA2 objects. In the example diagram below there are 8 objects of 5 types (DPMCP, DPIO, DPBP, DPNI, and DPMAC) in the container.

From the point of view of an OS, a DPRC behaves similar to a plug and play bus, like PCI. DPRC commands can be used to enumerate the contents of the DPRC, discover the hardware objects present (including mappable regions and interrupts).

Hardware objects can be created and destroyed dynamically, providing the ability to hot plug/unplug objects in and out of the DPRC.

A DPRC has a mappable MMIO region (an MC portal) that can be used to send MC commands. It has an interrupt for status events (like hotplug).

All objects in a container share the same hardware “isolation context”. This means that with respect to an IOMMU the isolation granularity is at the DPRC (container) level, not at the individual object level.

DPRCs can be defined statically and populated with objects via a config file passed to the MC when firmware starts it. There is also a Linux user space tool called “restool” that can be used to create/destroy containers and objects dynamically.

A typical Ethernet NIC is monolithic– the NIC device contains TX/RX queuing mechanisms, configuration mechanisms, buffer management, physical ports, and interrupts. DPAA2 uses a more granular approach utilizing multiple hardware objects. Each object provides specialized functions. Groups of these objects are used by software to provide Ethernet network interface functionality. This approach provides efficient use of finite hardware resources, flexibility, and performance advantages.

The diagram below shows the objects needed for a simple network interface configuration on a system with 2 CPUs.

Below the objects are described. For each object a brief description is provided along with a summary of the kinds of operations the object supports and a summary of key resources of the object (MMIO regions and IRQs).

DPMAC (Datapath Ethernet MAC): represents an Ethernet MAC, a hardware device that connects to an Ethernet PHY and allows physical transmission and reception of Ethernet frames.

IRQs: DPNI link change

commands: set link up/down, link config, get stats, IRQ config, enable, reset

DPNI (Datapath Network Interface): contains TX/RX queues, network interface configuration, and RX buffer pool configuration mechanisms. The TX/RX queues are in memory and are identified by queue number.

commands: port config, offload config, queue config, parse/classify config, IRQ config, enable, reset

DPIO (Datapath I/O): provides interfaces to enqueue and dequeue packets and do hardware buffer pool management operations. The DPAA2 architecture separates the mechanism to access queues (the DPIO object) from the queues themselves. The DPIO provides an MMIO interface to enqueue/dequeue packets. To enqueue something a descriptor is written to the DPIO MMIO region, which includes the target queue number. There will typically be one DPIO assigned to each CPU. This allows all CPUs to simultaneously perform enqueue/dequeued operations. DPIOs are expected to be shared by different DPAA2 drivers.

MMIO regions: queue operations, buffer management

IRQs: data availability, congestion notification, buffer pool depletion

commands: IRQ config, enable, reset

DPBP (Datapath Buffer Pool): represents a hardware buffer pool.

commands: enable, reset

DPMCP (Datapath MC Portal): provides an MC command portal. Used by drivers to send commands to the MC to manage objects.

MMIO regions: MC command portal

IRQs: command completion

commands: IRQ config, enable, reset

Some objects have explicit relationships that must be configured:

DPNI <–> L2-switch-port

A DPNI must be connected to something such as a DPMAC, another DPNI, or L2 switch port. The DPNI connection is made via a DPRC command.

A network interface requires a ‘buffer pool’ (DPBP object) which provides a list of pointers to memory where received Ethernet data is to be copied. The Ethernet driver configures the DPBPs associated with the network interface.

All interrupts generated by DPAA2 objects are message interrupts. At the hardware level message interrupts generated by devices will normally have 3 components– 1) a non-spoofable ‘device-id’ expressed on the hardware bus, 2) an address, 3) a data value.

In the case of DPAA2 devices/objects, all objects in the same container/DPRC share the same ‘device-id’. For ARM-based SoC this is the same as the stream ID.

This section provides an overview of the drivers for DPAA2– 1) the bus driver and associated “DPAA2 infrastructure” drivers and 2) functional object drivers (such as Ethernet).

As described previously, a DPRC is a container that holds the other types of DPAA2 objects. It is functionally similar to a plug-and-play bus controller.

Each object in the DPRC is a Linux “device” and is bound to a driver. The diagram below shows the dpaa2 drivers involved in a networking scenario and the objects bound to each driver. A brief description of each driver follows.

A brief description of each driver is provided below.

The DPAA2 bus driver is a rte_bus driver which scans the fsl-mc bus. Key functions include:

Reading the container and setting up vfio group

Scanning and parsing the various MC objects and adding them to their respective device list.

Additionally, it also provides the object driver for generic MC objects.

The DPIO driver is bound to DPIO objects and provides services that allow other drivers such as the Ethernet driver to enqueue and dequeue data for their respective objects. Key services include:

Data availability notifications

Hardware queuing operations (enqueue and dequeue of data)

Hardware buffer pool management

To transmit a packet the Ethernet driver puts data on a queue and invokes a DPIO API. For receive, the Ethernet driver registers a data availability notification callback. To dequeue a packet a DPIO API is used.

There is typically one DPIO object per physical CPU for optimum performance, allowing different CPUs to simultaneously enqueue and dequeue data.

The DPIO driver operates on behalf of all DPAA2 drivers active – Ethernet, crypto, compression, etc.

The DPBP driver is bound to a DPBP objects and provides services to create a hardware offloaded packet buffer mempool.

The Ethernet driver is bound to a DPNI and implements the kernel interfaces needed to connect the DPAA2 network interface to the network stack.

Each DPNI corresponds to a DPDK network interface.

Features of the DPAA2 PMD are:

Multiple queues for TX and RX

Receive Side Scaling (RSS)

Packet type information

Port hardware statistics

Scattered and gather for TX and RX

Traffic Management API

See NXP QorIQ DPAA2 Board Support Package for setup information

Follow the DPDK Getting Started Guide for Linux to setup the basic DPDK environment.

Some part of fslmc bus code (mc flib - object library) routines are dual licensed (BSD & GPLv2), however they are used as BSD in DPDK in userspace.

Refer to the document compiling and testing a PMD for a NIC for details.

Follow instructions available in the document compiling and testing a PMD for a NIC to run testpmd.

Use dev arg option drv_loopback=1 to loopback packets at driver level. Any packet received will be reflected back by the driver on same port. e.g. fslmc:dpni.1,drv_loopback=1

Use dev arg option drv_no_prefetch=1 to disable prefetching of the packet pull command which is issued in the previous cycle. e.g. fslmc:dpni.1,drv_no_prefetch=1

Use dev arg option drv_tx_conf=1 to enable TX confirmation mode. In this mode tx conf queues need to be polled to free the buffers. e.g. fslmc:dpni.1,drv_tx_conf=1

Use dev arg option drv_rx_parse_drop=1 to configure the system to start dropping the error packets in hardware (parse errors). e.g. fslmc:dpni.1,drv_rx_parse_drop=1

Use dev arg option drv_error_queue=1 to enable Packets in Error queue. DPAA2 hardware drops the error packet in hardware. This option enables the hardware to not drop the error packet and let the driver dump the error packets, so that user can check what is wrong with those packets. e.g. fslmc:dpni.1,drv_error_queue=1

For enabling logging for DPAA2 PMD, following log-level prefix can be used:

Using bus.fslmc as log matching criteria, all FSLMC bus logs can be enabled which are lower than logging level.

Using pmd.net.dpaa2 as log matching criteria, all PMD logs can be enabled which are lower than logging level.

For blocking a DPAA2 device, following commands can be used.

Where x is the device object id as configured in resource container.

dpaa2 hardware imposes limits on some H/W access devices like Management Control Port and H/W portal. This causes issue in their shared usages in case of multi-process applications. It can overcome by using allowlist/blocklist in primary and secondary applications.

In order to ease usage of standard debugging apps like dpdk-procinfo, dpaa2 driver reserves extra Management Control Port and H/W portal which can be used by debug application to debug any existing application without blocking these devices in primary process.

DPAA2 drivers for DPDK can only work on NXP SoCs as listed in the Supported DPAA2 SoCs.

The DPAA2 SoC family support a maximum of a 10240 jumbo frame. The value is fixed and cannot be changed. So, even when the rxmode.mtu member of struct rte_eth_conf is set to a value lower than 10240, frames up to 10240 bytes can still reach the host interface.

RSS hash key cannot be modified.

RSS RETA cannot be configured.

DPAA2 PMD supports generic DPDK Traffic Management API which allows to configure the following features:

Hierarchical scheduling

Internally TM is represented by a hierarchy (tree) of nodes. Node which has a parent is called a leaf whereas node without parent is called a non-leaf (root).

Nodes hold following types of settings:

for egress scheduler configuration: weight

for egress rate limiter: private shaper

Hierarchy is always constructed from the top, i.e first a root node is added then some number of leaf nodes. Number of leaf nodes cannot exceed number of configured tx queues.

After hierarchy is complete it can be committed.

For an additional description please refer to DPDK Traffic Management API.

The following capabilities are supported:

Level0 (root node), Level1 and Level2 are supported.

1 private shaper at root node (port level) is supported.

8 TX queues per port supported (1 channel per port)

Both SP and WFQ scheduling mechanisms are supported on all 8 queues.

the network, DPDK driver will not enqueue any packet (no taildrop or WRED)

User can also check node, level capabilities using testpmd commands.

For a detailed usage description please refer to “Traffic Management” section in DPDK Testpmd Runtime Functions.

Run testpmd as follows:

One port level shaper and strict priority on all 4 queues of port 0:

One port level shaper and WFQ on all 4 queues of port 0:

Create flows as per the source IP addresses:

Inject the traffic on port1 as per the configured flows, you will see shaped and scheduled forwarded traffic on port0

---

## 9. NXP DPAA2 CAAM (DPAA2_SEC) — Data Plane Development Kit 26.07.0-rc2 documentation

**URL:** https://doc.dpdk.org/guides/cryptodevs/dpaa2_sec.html

**Contents:**
- 9. NXP DPAA2 CAAM (DPAA2_SEC)
- 9.1. Architecture
- 9.2. Implementation
- 9.3. Features
- 9.4. Supported DPAA2 SoCs
- 9.5. Allowing & Blocking
- 9.6. Limitations
- 9.7. Prerequisites
- 9.8. Enabling logs
- 9.9. Enabling debug prints

The DPAA2_SEC PMD provides poll mode crypto driver support for NXP DPAA2 CAAM hardware accelerator.

SEC is the SOC’s security engine, which serves as NXP’s latest cryptographic acceleration and offloading hardware. It combines functions previously implemented in separate modules to create a modular and scalable acceleration and assurance engine. It also implements block encryption algorithms, stream cipher algorithms, hashing algorithms, public key algorithms, run-time integrity checking, and a hardware random number generator. SEC performs higher-level cryptographic operations than previous NXP cryptographic accelerators. This provides significant improvement to system level performance.

DPAA2_SEC is one of the hardware resource in DPAA2 Architecture. More information on DPAA2 Architecture is described in DPAA2 Overview.

DPAA2_SEC PMD is one of DPAA2 drivers which interacts with Management Complex (MC) portal to access the hardware object - DPSECI. The MC provides access to create, discover, connect, configure and destroy dpseci objects in DPAA2_SEC PMD.

DPAA2_SEC PMD also uses some of the other hardware resources like buffer pools, queues, queue portals to store and to enqueue/dequeue data to the hardware SEC.

DPSECI objects are detected by PMD using a resource container called DPRC (like in DPAA2 Overview).

SEC provides platform assurance by working with SecMon, which is a companion logic block that tracks the security state of the SOC. SEC is programmed by means of descriptors (not to be confused with frame descriptors (FDs)) that indicate the operations to be performed and link to the message and associated data. SEC incorporates two DMA engines to fetch the descriptors, read the message data, and write the results of the operations. The DMA engine provides a scatter/gather capability so that SEC can read and write data scattered in memory. SEC may be configured by means of software for dynamic changes in byte ordering. The default configuration for this version of SEC is little-endian mode.

A block diagram similar to dpaa2 NIC is shown below to show where DPAA2_SEC fits in the DPAA2 Bus model

The DPAA2_SEC PMD has support for:

RTE_CRYPTO_CIPHER_NULL

RTE_CRYPTO_CIPHER_3DES_CBC

RTE_CRYPTO_CIPHER_AES128_CBC

RTE_CRYPTO_CIPHER_AES192_CBC

RTE_CRYPTO_CIPHER_AES256_CBC

RTE_CRYPTO_CIPHER_AES128_CTR

RTE_CRYPTO_CIPHER_AES192_CTR

RTE_CRYPTO_CIPHER_AES256_CTR

RTE_CRYPTO_AUTH_SHA1_HMAC

RTE_CRYPTO_AUTH_SHA224_HMAC

RTE_CRYPTO_AUTH_SHA256_HMAC

RTE_CRYPTO_AUTH_SHA384_HMAC

RTE_CRYPTO_AUTH_SHA512_HMAC

RTE_CRYPTO_AUTH_MD5_HMAC

RTE_CRYPTO_AUTH_AES_XCBC_MAC

RTE_CRYPTO_AUTH_AES_CMAC

RTE_CRYPTO_AEAD_AES_GCM

The DPAA2 SEC device can be blocked with the following:

Where x is the device object id as configured in resource container.

Hash followed by Cipher mode is not supported

Only supports the session-oriented API implementation (session-less APIs are not supported).

DPAA2_SEC driver has similar pre-requisites as described in DPAA2 Overview. The following dependencies are not part of DPDK and must be installed separately:

See NXP QorIQ DPAA2 Board Support Package for setup information

Follow the DPDK Getting Started Guide for Linux to setup the basic DPDK environment.

For enabling logs, use the following EAL parameter:

Using crypto.dpaa2 as log matching criteria, all Crypto PMD logs can be enabled which are lower than logging level.

Use dev arg option drv_dump_mode=x to dump useful debug prints on HW sec error. There are 3 dump modes available 0, 1 and 2. Mode 0 means no dump print on error, mode 1 means dump HW error code and mode 2 means dump HW error code along with other useful debugging information like session, queue, descriptor data. e.g. fslmc:dpseci.1,drv_dump_mode=1

Use dev arg option drv_strict_order=1 to enable strict ordering. By default, loose ordering is set for ordered schedule type event. e.g. fslmc:dpseci.1,drv_strict_order=1

---

## 5. NXP DPAA2 Eventdev Driver — Data Plane Development Kit 26.07.0-rc2 documentation

**URL:** https://doc.dpdk.org/guides/eventdevs/dpaa2.html

**Contents:**
- 5. NXP DPAA2 Eventdev Driver
- 5.1. Features
- 5.2. Supported DPAA2 SoCs
- 5.3. Prerequisites
- 5.4. Initialization
- 5.5. Enabling logs
- 5.6. Limitations
  - 5.6.1. Platform Requirement
  - 5.6.2. Port-core binding

The dpaa2 eventdev is an implementation of the eventdev API, that provides a wide range of the eventdev features. The eventdev relies on a dpaa2 hw to perform event scheduling.

More information can be found at NXP Official Website.

The DPAA2 EVENTDEV implements many features in the eventdev API;

Hardware based event scheduler

See NXP QorIQ DPAA2 Board Support Package for setup information

Follow the DPDK Getting Started Guide for Linux to setup the basic DPDK environment.

Some part of fslmc bus code (mc flib - object library) routines are dual licensed (BSD & GPLv2).

The dpaa2 eventdev is exposed as a vdev device which consists of a set of dpcon devices and dpci devices. On EAL initialization, dpcon and dpci devices will be probed and then vdev device can be created from the application code by

Invoking rte_vdev_init("event_dpaa2") from the application

Using --vdev="event_dpaa2" in the EAL options, which will call rte_vdev_init() internally

For enabling logs, use the following EAL parameter:

Using eventdev.dpaa2 as log matching criteria, all Event PMD logs can be enabled which are lower than logging level.

DPAA2 drivers for DPDK can only work on NXP SoCs as listed in the Supported DPAA2 SoCs.

DPAA2 EVENTDEV can support only one eventport per core.

---

## 4. NXP DPAA2 CMDIF Driver — Data Plane Development Kit 26.07.0-rc2 documentation

**URL:** https://doc.dpdk.org/guides/rawdevs/dpaa2_cmdif.html

**Contents:**
- 4. NXP DPAA2 CMDIF Driver
- 4.1. Features
- 4.2. Supported DPAA2 SoCs
- 4.3. Prerequisites
- 4.4. Enabling logs
- 4.5. Initialization
  - 4.5.1. Platform Requirement

The DPAA2 CMDIF is an implementation of the rawdev API, that provides communication between the GPP and AIOP (Firmware). This is achieved via using the DPCI devices exposed by MC for GPP <–> AIOP interaction.

More information can be found at NXP Official Website.

The DPAA2 CMDIF implements following features in the rawdev API;

Getting the object ID of the device (DPCI) using attributes

I/O to and from the AIOP device using DPCI

See NXP QorIQ DPAA2 Board Support Package for setup information

Follow the DPDK Getting Started Guide for Linux to setup the basic DPDK environment.

Some part of fslmc bus code (mc flib - object library) routines are dual licensed (BSD & GPLv2).

For enabling logs, use the following EAL parameter:

Using pmd.raw.dpaa2.cmdif as log matching criteria, all Event PMD logs can be enabled which are lower than logging level.

The DPAA2 CMDIF is exposed as a vdev device which consists of dpci devices. On EAL initialization, dpci devices will be probed and then vdev device can be created from the application code by

Invoking rte_vdev_init("dpaa2_dpci") from the application

Using --vdev="dpaa2_dpci" in the EAL options, which will call rte_vdev_init() internally

DPAA2 drivers for DPDK can only work on NXP SoCs as listed in the Supported DPAA2 SoCs.

---

## 3. NXP DPAA2 QDMA Driver — Data Plane Development Kit 26.07.0-rc2 documentation

**URL:** https://doc.dpdk.org/guides/dmadevs/dpaa2.html

**Contents:**
- 3. NXP DPAA2 QDMA Driver
- 3.1. Features
- 3.2. Supported DPAA2 SoCs
- 3.3. Prerequisites
- 3.4. Enabling logs
- 3.5. Initialization
  - 3.5.1. Platform Requirement
- 3.6. Device Arguments

The DPAA2 QDMA is an implementation of the dmadev API, that provide means to initiate a DMA transaction from CPU. The initiated DMA is performed without CPU being involved in the actual DMA transaction. This is achieved via using the DPDMAI device exposed by MC.

More information can be found at NXP Official Website.

The DPAA2 QDMA implements following features in the dmadev API;

Supports issuing DMA of data within memory without hogging CPU while performing DMA operation.

Supports configuring to optionally get status of the DMA translation on per DMA operation basis.

See NXP QorIQ DPAA2 Board Support Package for setup information

Follow the DPDK Getting Started Guide for Linux to setup the basic DPDK environment.

Some part of fslmc bus code (mc flib - object library) routines are dual licensed (BSD & GPLv2).

For enabling logs, use the following EAL parameter:

Using pmd.dma.dpaa2.qdma as log matching criteria, all Event PMD logs can be enabled which are lower than logging level.

The DPAA2 QDMA is exposed as a dma device which consists of dpdmai devices. On EAL initialization, dpdmai devices will be probed and populated into the dmadevices. The dmadev ID of the device can be obtained using

Invoking rte_dma_get_dev_id_by_name("dpdmai.x") from the application where x is the object ID of the DPDMAI object created by MC. Use can use this index for further rawdev function calls.

DPAA2 drivers for DPDK can only work on NXP SoCs as listed in the Supported DPAA2 SoCs.

Pre-populate all DMA descriptors with pre-initialized values. Usage example: fslmc:dpdmai.1,fle_pre_populate=1

Enable descriptor debugs. Usage example: fslmc:dpdmai.1,desc_debug=1

Enable short FDs. Usage example: fslmc:dpdmai.1,short_fd=1

---
