# Physical / Link-Layer Mesh Options for Offline Basecamps, and Feasibility of Carrying Logos Messaging (libp2p) and Zcash Ironwood Data

Scope: physical/network mesh layer and bandwidth feasibility only. Logos Messaging internals, Zcash internals, offline-payment prior art and the threat model are covered by other researchers. State as of 2026-10-24. All transfer-time numbers were computed by me (script logic and arithmetic shown below); they are inferences built on the cited constants.

---

## 1. Comparison of mesh link technologies (range, throughput, latency, MTU, duty cycle, IP support, cost, power, legal constraints)

### Takeaway
The technologies fall into two groups. (a) IP-capable "backbone" links: 802.11s/BATMAN-adv/Babel/OLSR over 2.4/5 GHz Wi-Fi, 5 GHz point-to-point (PtP) or point-to-multipoint (PtMP) bridges, and Wi-Fi HaLow. These give 1 to 450+ Mbps over hundreds of metres to tens of km, need line of sight (or near it), and draw watts. (b) Sub-GHz LoRa-class links: Meshtastic, MeshCore, Reticulum/RNode. These give about 0.2 to 20 kbps PHY over 2 to 10+ km (record 331 km), use 184 to 500-byte frames and draw milliwatts, but are throttled hard by duty-cycle and dwell rules. They do not carry IP natively. Amateur-radio options (AX.25, VARA, Winlink, AREDN, Meshtastic "licensed" mode) are legally unusable for this project: 47 CFR 97.113(a)(4) bans "messages encoded for the purpose of obscuring their meaning", and a Zcash shielded transaction is mostly ciphertext. Part 97.113(a)(3) also bars communications in which the operator has a pecuniary interest.

### Cited Findings

**Wi-Fi mesh (802.11s, BATMAN-adv, Babel, OLSR)**
- batman-adv encapsulates Ethernet frames. Users raise the underlying MTU to 1532 so batman-adv stops warning about fragmentation, but without jumbo-frame support the usual hardware limit is 1500 (some hardware caps lower). — [OpenWrt forum](https://forum.openwrt.org/t/tl-wr1043nd-v4-eth0-mtu-max-1516-openwrt-18-06-4/43062)
- A batman-adv maintainer: a single half-duplex radio "will be cut to 50% with the first hop, and will decrease further with more hops". — [b.a.t.m.a.n mailing list](https://lists.open-mesh.org/mailman3/hyperkitty/list/b.a.t.m.a.n@lists.open-mesh.org/thread/HRSUFGGQ36RAYYJZAHASQILFRF3WYYWX)
- OpenWrt field reports (anecdotal): about 100 Mbps through one 802.11s hop on Linksys EA8300s, where a dedicated AP/station link gave about 170 Mbps. On D-Link routers, about 18 Mbps over 802.11s versus about 90 Mbps in plain AP-client mode. — [OpenWrt forum: mesh hops](https://forum.openwrt.org/t/mesh-hops-bandwidth-reduction/66095); [OpenWrt forum: 802.11s throughput](https://forum.openwrt.org/t/802-11s-mesh-throughput/8378)
- A 2021 multi-channel BATMAN-adv mesh with 802.11ac radios reached 5 hops with about 30% throughput loss (multi-radio design). — [researching.cn (Chinese-language study)](https://www.researching.cn/articles/OJ3d61511be0676afd)
- Analytical bound: 802.11 many-to-one throughput is capped at 3/4 of link capacity when sources are 2+ hops from the sink. — [arXiv 0704.0528](https://arxiv.org/pdf/0704.0528)
- Babel is standardized as RFC 8966 and OLSRv2 as RFC 7181. Both are IP (L3) routing protocols. — [RFC 8966](https://www.rfc-editor.org/rfc/rfc8966); [RFC 7181](https://www.rfc-editor.org/rfc/rfc7181)

**Long-range Wi-Fi PtP/PtMP (5 GHz airMAX / MikroTik class)**
- Ubiquiti LiteBeam 5AC Gen2: 23 dBi, 450+ Mbps, 7 W max power consumption (24 V passive PoE), -40 to 70 °C. Reseller range claims run from "15+ km" to 25+ km (LR variant) to 30 km, and the official datasheet figure is unconfirmed. Price is about £55 to €73 incl. VAT. — [NetXL](https://www.netxl.com/point-to-point-wireless/ubiquiti-lbe-5ac-gem2-litebeam-ac-gen-2-5ghz-23dbi-radio.md); [Connectec](https://connectec.uk/networking/network-devices/wireless-bridges/ubiquiti-airmax-lbe-5ac-gen2-litebeam-ac-wireless-bridge); [wlan-shop24](https://www.wlan-shop24.de/ubiquti-litebeam-5ac-gen2)

**Wi-Fi HaLow (802.11ah)**
- Bands: 902–928 MHz in North America (26 channels), 863–868 MHz in Europe. Channel widths are 1/2/4/8/16 MHz. The lowest rate is MCS10 on 1 MHz, about 0.15 Mbit/s. The top theoretical rate is 433.3 Mbit/s with 4 streams on 16 MHz. The standard nominally covers a 1 km radius. The relay function is limited to two hops. TWT and RAW provide power saving and contention control. — [Wikipedia: IEEE 802.11ah](https://en.wikipedia.org/wiki/IEEE_802.11ah)
- Morse Micro range record: 15.9 km line of sight in Joshua Tree with the MM6108-EKH01 kit and stock 1 dBi antenna, about 2 Mbps UDP. Rethink Research reports a 3.6 Mbps peak and 1.31 Mbps average. Morse notes that 15.9 km is also the theoretical maximum derived from 802.11ah timing parameters. — [eeNews Europe](https://www.eenewseurope.com/en/halow-hits-record-16km-wifi-distance-at-900mhz); [Rethink Research](https://rethinkresearch.biz/articles/morse-micro-nets-wifi-halow-range-record/)
- An earlier urban demo reached 3 km in San Francisco, where throughput fell to about 1 Mbps. — [ISPreview](https://www.ispreview.co.uk/index.php/2024/02/long-range-halow-wifi-network-test-pushes-signal-to-3km.html)
- MM6108-EKH01: "data rates up to 32.5 Mbps". — [Mouser](https://www.mouser.in/new/morse-micro/morse-micro-mm6108-ekh01-platform/)
- GL.iNet HaLowLink 2 (Morse Micro MM8108 plus MT7621): 1/2/4/8 MHz channels, up to 43.3 Mbit/s net (optimal), about 1 km range claim, up to 26 dBm TX (non-EU only), WPA3, USB-C 5 V, about €126 each or about €217 for a 2-pack. — [Igor'sLAB](https://www.igorslab.de/en/wi-fi-halow-with-kilometer-range-why-the-new-gl-inet-halowlink-2-is-reaching-its-limits-in-europe/)
- EU HaLow limits (863–868 MHz): five 1 MHz channels with no bundling, 25 mW ERP max (14 dBm), and a duty cycle of 10% per hour for access points (360 s/h) and 2.8% for stations (about 100.8 s/h), after which the device must stay silent for the rest of the hour. LBT is satisfied by 802.11 CSMA/CA. — [Igor'sLAB](https://www.igorslab.de/en/wi-fi-halow-with-kilometer-range-why-the-new-gl-inet-halowlink-2-is-reaching-its-limits-in-europe/); [heise](https://heise.de/-11152896); [ZHAW / Swiss BAKOM table](https://www.zhaw.ch/storage/engineering/institute-zentren/isc/Dateienablage_News/ZHAW_WiFi_HaLow_ISC_InEs_22.01.2025_V2.pdf)
- Morse Micro's OpenWrt-based EKH01 user guide documents a mesh operating mode. A non-official LinkedIn post claims 802.11s mesh is an alternate mode on HaLowLink 2. No measured HaLow 802.11s throughput found. — [Morse Micro (search result)](https://www.morsemicro.com/?p=251203); see Gaps

**LoRa (common to Meshtastic, MeshCore, RNode/Reticulum)**
- Meshtastic modem presets (theoretical PHY data rate; link budget assumes 22 dBm and 0 dBi): ShortTurbo SF7/500 kHz 21.88 kbps (140 dB); ShortFast SF7/250 10.94 kbps (143 dB); ShortSlow SF8/250 6.25 kbps; MediumTurbo SF9/500 7.03 kbps; MediumFast SF9/250 3.52 kbps (148 dB); MediumSlow SF10/250 1.95 kbps; LongTurbo SF11/500 CR4/8 1.34 kbps; LongFast SF11/250 1.07 kbps (153 dB); LongModerate SF11/125 CR4/8 0.34 kbps (156 dB). "Turbo presets are unavailable in EU_868." The rates "don't account for packet headers, hops and re-transmissions." — [Meshtastic radio settings](https://meshtastic.org/docs/overview/radio-settings/)
- Meshtastic packet: 256-byte max LoRa packet with a 16-byte header. Usable payload is variously cited as 233/237/240 bytes, and one user saw failures above about 223 bytes. — [Meshtastic forum](https://meshtastic.discourse.group/t/lora-packet-size-in-meshtastic/3288); [things-nyc/lwom](https://github.com/things-nyc/lwom)
- The Meshtastic hop limit is a 3-bit field (max 7). — [Meshtastic overview](https://meshtastic.org/docs/overview/)
- Meshtastic range: typical 2–5 km. Community record 331 km. Documented balloon record 206 km on 915 MHz LongFast. A ground-to-ground report of 76 km used a 1 W DIY module. — [Wikipedia: Meshtastic](https://en.wikipedia.org/wiki/Meshtastic); [Meshtastic range tests](https://meshtastic.org/docs/overview/range-tests/); [Meshtastic forum](https://meshtastic.discourse.group/t/practical-range-test-results/692/130)
- Time-on-air formula (SX127x datasheet §4.1.1.6, reproduced by third-party calculators): T_sym = 2^SF/BW; T_preamble = (N_preamble + 4.25)·T_sym; N_payload = 8 + max(ceil((8·PL − 4·SF + 28 + 16·CRC − 20·IH)/(4·(SF − 2·DE)))·(CR+4), 0). Low-data-rate optimization (DE) is mandatory when T_sym > 16 ms. Reference check: SF11/250 kHz/4/5 with a 16-byte payload gives T_sym = 8.192 ms, 23 payload symbols and ToA = 354.304 ms (with a 16-symbol preamble). — [lora-air-time (GitHub)](https://github.com/ifTNT/lora-air-time); [d-central LoRa airtime calculator](https://d-central.tech/lora-airtime-calculator/)

**Duty-cycle / dwell regulation**
- EU 863–870 MHz SRD sub-bands per ETSI EN 300 220 V3.2.1: K 863–865 0.1%/25 mW; L 865–868 1%/25 mW; M 868.0–868.6 1%/25 mW ERP; N 868.7–869.2 0.1%/25 mW; P 869.4–869.65 10%/500 mW ERP; Q 869.7–870 1%/25 mW. — [TTN: regional limitations](https://www.thethingsnetwork.org/docs/lorawan/regional-limitations-of-rf-use/)
- Meshtastic notes that the 869.40–869.65 MHz band allows "a higher ERP and a duty cycle of 10%." — [Meshtastic radio settings](https://meshtastic.org/docs/overview/radio-settings/)
- US 47 CFR 15.247(a)(1)(i), 902–928 MHz frequency hopping: if the 20 dB BW is under 250 kHz, at least 50 hop frequencies with average occupancy ≤0.4 s in any 20 s; if 250 kHz or more, at least 25 frequencies with ≤0.4 s in 10 s; max 20 dB BW 500 kHz. 15.247(a)(2): digital modulation requires a 6 dB BW of at least 500 kHz. 15.247(b): 1 W peak for 50+ hop channels, 0.25 W for 25–49. 15.247(e): digital modulation PSD ≤8 dBm per 3 kHz. 15.247(f) covers hybrid systems. — [47 CFR 15.247 (LII)](https://www.law.cornell.edu/cfr/text/47/15.247)
- US 47 CFR 15.249: fundamental field strength limit in 902–928 MHz is 50 mV/m at 3 m (applies to non-15.247 devices). — [47 CFR 15.249 (LII)](https://www.law.cornell.edu/cfr/text/47/15.249)
- LoRaWAN US915: 64×125 kHz uplink channels, 8×500 kHz uplink, 8×500 kHz downlink, 400 ms max dwell per transmission, +30 dBm EIRP max. — [TTN: regional limitations](https://www.thethingsnetwork.org/docs/lorawan/regional-limitations-of-rf-use/)

**Meshtastic / MeshCore specifics**
- On amateur bands Meshtastic's licensed mode is "Plain-Text Only". The FAQ states "On amateur radio bands, encryption is illegal" (citing FCC 97.113(a)(4)). — [Meshtastic FAQ](https://meshtastic.org/docs/faq/)
- MeshCore: LoRa mesh with companion, repeater and standalone node classes, AES-128 E2E (relays cannot read content), "10km+ line-of-sight", 50+ supported devices (Heltec V3, RAK WisMesh, SenseCAP Solar P1 Pro, T-Deck etc.). — [MeshCore](https://meshcore.co.uk/)
- MeshCore packet: 255-byte max on-wire, 184-byte payload cap, path field up to 64 bytes (1-byte header, optional 4-byte transport code, path-length byte, path, payload). Typical practical max is 3–7 hops, with firmware support up to 64. Room servers act as store-and-forward. (Community wiki, not official docs.) — [eastmesh wiki: packet structure](https://wiki.eastmesh.au/meshcore/packet-structure/); [eastmesh wiki: overview](https://wiki.eastmesh.au/meshcore/overview/); [PacoX/MeshCore README](https://github.com/PacoX/MeshCore)

**Reticulum (RNS) + LXMF**
- Reticulum requires a physical MTU of 500 bytes (packet payload 0–465 bytes) and a minimum average throughput of 5 bit/s. Link establishment costs 3 packets totalling 297 bytes (83 + 115 + 99). Link keepalives are 20 bytes, about 0.45 bit/s. The header is 2 bytes plus one or two 16-byte addresses and a 1-byte context. Announces are 167 bytes and path requests 51 bytes. Crypto: X25519 ECDH, Ed25519 signatures, Fernet-style AES-256-CBC + HMAC-SHA256 with HKDF. Links give forward secrecy. "Resources" do compression, segmentation, sequencing, integrity checking and automatic retransmission over a Link. IP is not required ("medium agnostic"). — [Reticulum manual: Understanding](https://reticulum.network/manual/understanding.html)
- Interfaces: AutoInterface (IPv6 link-local discovery over Ethernet/Wi-Fi, UDP ports 29716/42671), Backbone, TCP server/client, UDP, I2P, RNodeInterface (LoRa over serial/TCP/BLE), RNodeMultiInterface, Serial, Pipe, KISS (TNC with optional `id_callsign` beacons), AX25KISS. RNode has `airtime_limit_short` (about 15 s window) and `airtime_limit_long` (rolling 60 min) settings for duty-cycle compliance. — [Reticulum manual: Interfaces](https://reticulum.network/manual/interfaces.html)
- LXMF: "The complete message overhead for LXMF is only 111 bytes". Delivery is direct over a Link, opportunistic in a single packet, or propagated (store-and-forward through peering, synchronizing Propagation Nodes). Encrypted "paper" messages can travel as QR codes or `lxm://` URIs. — [LXMF README](https://github.com/markqvist/LXMF)
- Field evidence for RNode/LXMF range or throughput is anecdotal only (e.g., a drone-relay writeup with a ground LoRa link lost at about 2.5 miles under canopy). — [yage.ai writeup (low-quality source)](https://yage.ai/share/reticulum-drone-offline-network-en-20260811.html)

**Yggdrasil / cjdns overlays**
- Yggdrasil: decentralised mesh routing that transports IPv6 only, with all traffic end-to-end encrypted. Addresses come from 0200::/7. It uses the largest MTU the platform supports (up to 65535). Multicast discovery of peers on the same subnet is on by default. Peerings run over IPv4 or IPv6. "Official stance is that it is still alpha software" and it has not been externally audited. — [Yggdrasil FAQ](https://yggdrasil-network.github.io/faq.html)

**Bluetooth LE mesh / Briar**
- Bluetooth Mesh: most messages fit one 11-byte segment. SAR allows up to 32 segments, giving a 384-octet max upper-transport PDU (about 377 bytes of app data after MIC and a vendor opcode). — [Zephyr: Bluetooth Mesh SAR](https://docs.zephyrproject.org/latest/connectivity/bluetooth/api/mesh/sar_cfg.html); [Nordic DevZone](https://devzone.nordicsemi.com/f/nordic-q-a/32854/max-size-of-data-to-send-from-one-node-to-other-ble-mesh/127111)
- Briar: transports are Bluetooth, Wi-Fi, Tor and removable storage (USB). It works without internet. Platforms are Android plus desktop Windows/macOS/Linux, with no iOS. It uses the Bramble protocol (delay-tolerant) and interoperates only with Briar. Cure53 audited it in 2017. No general third-party data API is documented in the source. — [Wikipedia: Briar](https://en.wikipedia.org/wiki/Briar_(software))

**goTenna Pro**
- goTenna Pro X2/X2m: vendor claims 15–55 mi (24–88.5 km) line of sight depending on terrain, and AES-256 encryption. The product page does not list bands, TX power or data rate. — [goTenna Pro X2m](https://gotenna.com/products/gotenna-pro-x2m)
- CISA advisory ICSA-24-270-04 lists goTenna Pro vulnerabilities, including weak handling of encrypted-broadcast keys and insecure key storage (app ≤1.6.1). — [CISA ICSA-24-270-04](https://cisa.gov/news-events/ics-advisories/icsa-24-270-04)

**HF/VHF/UHF amateur packet (AX.25, VARA, Winlink, AREDN)**
- AX.25 on VHF/UHF is almost exclusively 1200 baud Bell 202 AFSK or 9600 baud G3RUH. — [Wikipedia: AX.25](https://en.wikipedia.org/wiki/AX.25)
- Dire Wolf author WB2OSZ: moving from 1200 to 9600 bps can turn out to be "only twice as fast" in practice because of protocol overhead and channel timing. — [Dire Wolf doc](https://cloud.xlxbra.net/direwolf/direwolf_git_v1.6/doc/Why-is-9600-only-twice-as-fast-as-1200.pdf)
- VARA HF: reported up to about 7,050 bps in a 2,300 Hz SSB passband and about 1,500 bps in 500 Hz. VARA FM: up to about 13 kbps narrow and about 25 kbps wide via the 9600-baud data port. (Third-party articles, not developer docs.) — [hamradio.my](https://hamradio.my/varac-hf-digital-chat-emcomm/)
- A Winlink VARA gateway example runs a 2,750 Hz wide channel. — [Winlink: VK4UI gateway](https://winlink.org/content/vk4ui_hf_vara_gateway)
- AREDN (amateur Wi-Fi mesh) tells operators to avoid encryption or proprietary compression because these "may be interpreted as encoding messages for the purpose of obscuring their meaning" (97.113(a)(4)). — [AREDN services guide](https://arednmesh.readthedocs.io/en/latest/arednServicesGuide/services_overview.html)

**Amateur-radio legal constraint**
- 47 CFR 97.113(a): amateur stations shall not transmit (a)(2) "communications for hire or for material compensation", (a)(3) "communications in which the station licensee or control operator has a pecuniary interest", or (a)(4) "messages encoded for the purpose of obscuring their meaning" (except as otherwise provided). — [47 CFR 97.113 (LII)](https://www.law.cornell.edu/cfr/text/47/97.113)
- The US rule mirrors the international treaty provision ITU RR Article 25 (RR25.2A). The FCC dismissed a 2013 petition for an emergency encryption exception. — [ARRL: FCC dismisses encryption petition](https://www.arrl.org/news/fcc-dismisses-encryption-petition); [FCC DA-13-1918](https://docs.fcc.gov/public/attachments/DA-13-1918A1_Rcd.pdf)
- A 2019 NYU petition asked the FCC to clarify that the ban covers "effectively encrypted" messages. ARRL urged dismissal, and I found no outcome. — [ARRL](https://www.arrl.org/news/arrl-asks-fcc-to-dismiss-petition-seeking-declaratory-ruling-on-encoded-message-rule)
- Community view (informal, not an FCC ruling): digital signing without encryption does not violate the rule. — [Lemmy thread](https://lemmy.klein.ruhr/post/188849/792880)

### Inferences

**Synthesis table.** Values come from the cited findings above. Cells marked * are my estimates or unverified.

| Link | Realistic range | Throughput | Latency | Frame/MTU | Duty/dwell | Native IP | Unit cost | Power |
|---|---|---|---|---|---|---|---|---|
| 802.11s / BATMAN-adv / Babel / OLSR (2.4/5 GHz) | 50–300 m/hop* omni; km with directional | 18–170 Mbps observed per hop; roughly halves per single-radio hop | ms per hop* | 1500 (1532 for batman-adv w/o frag) | none (5 GHz DFS*) | Yes (L2 or L3) | $50–150/router* | 3–10 W* |
| 5 GHz PtP/PtMP (airMAX/MikroTik) | 15–30 km LoS (reseller claims) | 450+ Mbps PHY (LiteBeam 5AC Gen2) | ms | 1500 | none* | Yes (bridged Ethernet) | ~€55–75 per end | 7 W max |
| Wi-Fi HaLow US (902–928) | 1 km nominal; 3 km urban; 15.9 km LoS record | 1–43 Mbps; 1.3–2 Mbps at 15.9 km | ms–tens of ms* | 1500* | none beyond Part 15 (DTS) | Yes | ~€126 (HaLowLink 2) | ~2–5 W* (USB-C 5 V) |
| Wi-Fi HaLow EU (863–868) | similar physics, 25 mW ERP | 1 MHz channels only; AP 10%/h, STA 2.8%/h airtime | ms | 1500* | **10%/2.8% per hour** | Yes | same | same |
| LoRa / Meshtastic | 2–5 km typical; 76 km ground and 206–331 km records | 0.34–21.9 kbps PHY (preset table) | seconds per frame at SF11–12 | ~231–237 B app | EU 1% (868.0–868.6) or 10% (869.4–869.65); US 0.4 s dwell if hopping | No | $20–60* (Heltec/RAK/LilyGO) | 10–100 mA* |
| MeshCore | "10 km+ LoS" | same LoRa PHY | same | 255 B on-wire / 184 B payload | same | No | same | same |
| Reticulum (RNode) + LXMF | same LoRa PHY | same; works down to 5 bit/s | same | 500 B RNS MTU | RNode airtime limits configurable | No (own addressing; can ride IP) | RNode $30–100* | same |
| Yggdrasil | overlay | link-limited | +crypto | up to 65535 | n/a | IPv6 overlay | free | CPU only |
| BLE mesh | tens of m/hop* | very low* | high (managed flood)* | 11 B segment / 384 B PDU | n/a | No | $5–20* | mW |
| Briar | BT ~10 m*, Wi-Fi LAN | n/a | n/a | n/a | n/a | No (app-only) | free (Android) | phone |
| goTenna Pro X2 | 24–88 km LoS (vendor) | not published | n/a | n/a | n/a (likely licensed land-mobile*) | No* | expensive* | n/a |
| AX.25 1200/9600 | VHF/UHF LoS tens of km* | 1200/9600 baud; 9600 ≈ 2× 1200 in practice | seconds | AX.25 frame ~256 B* | n/a | IP over AX.25 possible* | radio + TNC | 5–50 W TX* |
| VARA HF / FM / Winlink | HF: regional-continental*; FM: LoS | HF ≤ ~7 kbps; FM ≤ ~25 kbps | seconds to minutes | n/a | n/a | No (Winlink email) | VARA licence + radio | 50–100 W TX* |

- **Amateur radio implications.** Every Zcash shielded action carries a 580-byte `encCiphertext` and an 80-byte `outCiphertext` (see §3), and Logos Messaging relays traffic over Noise-encrypted libp2p connections. Transmitting either over Part 97 bands is therefore prohibited by 97.113(a)(4). A payment network also plausibly falls foul of 97.113(a)(3) (pecuniary interest) and (a)(2). The prohibition applies even if you only sign and do not add encryption yourself, because the payload is already encrypted by the Zcash protocol. That rules out AX.25/VARA/Winlink, AREDN, Meshtastic licensed mode, and Reticulum over AX.25/KISS on ham frequencies as carriers for this project. Ham bands remain usable only for unencrypted coordination (e.g., "gateway up at 14:00"). This is legal interpretation, not legal advice; other countries implement the same ITU RR 25.2A text.
- **Usable spectrum.** Encrypted traffic belongs on licence-exempt ISM/SRD bands (US Part 15; EU EN 300 220 at 868 MHz; 2.4/5 GHz Wi-Fi), which carry no content restrictions, or on licensed commercial land-mobile spectrum (an assumption about goTenna-class products, unverified).
- **US single-channel LoRa.** Under 15.247, LoRa at 125/250 kHz gets up to 1 W only as a frequency-hopping system with ≤0.4 s occupancy per channel. A 255-byte frame at SF ≥9/250 kHz lasts 0.64–2.16 s (my computation in §3), which exceeds 0.4 s. SF7/500 kHz plausibly qualifies as "digital modulation" (6 dB BW ≥500 kHz). Single-channel 125/250 kHz operation otherwise falls under 15.249's 50 mV/m at 3 m. Converting: EIRP = (E·d)²/30 = (0.05·3)²/30 ≈ 0.75 mW ≈ −1.2 dBm. How Meshtastic/RNode firmware and certified products justify single-channel 250 kHz operation is a grey area to verify with counsel or the product's FCC grant.
- **EU HaLow.** HaLow is much weaker in the EU than in the US. Five 1 MHz channels, 25 mW ERP and a 2.8% station duty cycle mean a client can send for only about 100 s per hour. HaLow's EU band (863–868 MHz) also overlaps LoRa SRD sub-bands K/L, so co-located HaLow and LoRa 865–868 MHz equipment will interfere. In the EU, prefer 5 GHz PtP for the backbone and keep LoRa on 869.4–869.65 MHz.
- **BLE mesh and Briar.** BLE mesh is effectively unusable for Zcash payloads: a 9.2 KB transaction would need at least 25 maximum-size (384 B) segmented messages, each of up to 32 advertising segments, flooded hop by hop. Briar is a good human-messaging fallback but exposes no transport API, so it cannot carry Logos traffic.
- **goTenna.** goTenna Pro has strong range but no published data rate, a proprietary stack and a CISA-documented key-handling vulnerability history. It is a poor foundation for a privacy-critical design.

### Gaps
- No measured 802.11s throughput over HaLow (EKH01/HaLowLink 2) was found, and Morse Micro's user guide PDF could not be read.
- No official Ubiquiti datasheet range number was found (only reseller claims). MikroTik equivalents were not researched.
- goTenna Pro X2 frequency bands, TX power, data rate and licensing basis are not published on the product page (the datasheet was not retrieved).
- BLE mesh real-world throughput: no measured number found.
- cjdns: no current (2026) source consulted. Its status and maintenance level are unknown.
- No authoritative source found on how single-channel 250 kHz LoRa firmware (Meshtastic/RNode) complies with 15.247/15.249 in the US.
- RNode firmware handling of the 500-byte Reticulum MTU over 255-byte LoRa frames (split-packet behaviour) was not verified from source.
- VARA throughput figures come from third-party articles, not the developer.

---

## 2. Can libp2p / Logos Messaging run over each link? IP-native links, libp2p over LoRa, bridging approaches

### Takeaway
Logos Messaging runs unmodified only over IP links: 802.11s/BATMAN-adv/Babel/OLSR meshes, 5 GHz bridges, HaLow, and optionally a Yggdrasil overlay on top. Its documented transports are TCP (default) and secure WebSocket, with QUIC/WebRTC/WebTransport still "under research". libp2p over LoRa is impractical. Connection setup alone (TCP, multistream-select, Noise XX, muxer, identify of "a couple of kilobytes") costs about 3 KB, which is roughly 32 s of airtime at Meshtastic LongFast and about 54 min wall-clock under the EU 1% duty cycle. GossipSub then runs a 1-second heartbeat and D=6 mesh fan-out. On top of that, RLN's `max_epoch_gap` and per-block Merkle-root window are designed to discard messages that arrive late, which store-and-forward delivery does by nature. The workable architecture is an application-level gateway. It runs Logos Messaging on the IP backbone and, for the non-IP fallback, re-encodes only the Zcash payload (about 9.2 KB) into Reticulum/LXMF, which handles segmentation, retransmission and store-and-forward. Fountain-coded broadcast suits one-to-many bulk data (compact blocks), and QR or USB sneakernet covers the zero-RF case.

### Cited Findings
- Logos Messaging is built on libp2p. Relay is GossipSub pub/sub, and RLN Relay adds rate limiting. Filter, Light Push and Store serve bandwidth-constrained/light nodes (Filter trades some privacy for less bandwidth). Store keeps messages briefly for offline peers with no availability guarantee. Discovery uses static peers, DNS, Discv5 and Peer Exchange. "TCP is the default for service nodes"; secure WebSocket serves browsers. "WebRTC, WebTransport, and QUIC are under research and not yet supported." The page does not address mesh or no-internet operation. — [Logos docs: Messaging](https://docs.logos.co/messaging)
- GossipSub v1.0 defaults: D=6, D_low=4, D_high=12, D_lazy=D, heartbeat_interval 1 s, fanout_ttl 60 s, mcache_len 5, mcache_gossip 3, seen_ttl 2 min. IHAVE gossip is emitted every heartbeat to up to D non-mesh peers per topic. — [libp2p gossipsub v1.0 spec](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md)
- RLN Relay (17/WAKU2-RLN-RELAY, now hosted as a Logos LIP): RateLimitProof carries `proof` (256 bytes uncompressed or 128 compressed), `merkle_root` (32), `epoch`, `share_x` (32), `share_y` (32) and `nullifier` (32). Messages whose epoch differs from the router's current epoch by more than `max_epoch_gap` "MUST be discarded". Routers keep only `acceptable_root_window_size` past Merkle roots, "updated on a per-block basis". Registration "MAY be moderated through a smart contract deployed on the Ethereum blockchain" and requires a staking transaction. `max_epoch_gap` is sized from network transmission delay plus clock asynchrony. — [Logos LIP 17/RLN-RELAY](https://lip.logos.co/messaging/core/draft/17/rln-relay.html)
- RLN proof generation needs a Merkle witness. Users either index the membership-contract events locally or rely on a blockchain RPC. Generation takes "several hundred milliseconds on a modern laptop" and longer on mobile. — [Vac RLN overview](https://vac.dev/rln); [RLN v3 proposal (HackMD)](https://hackmd.io/@rymnc/rln-v3-proposal)
- Design commentary (opinion, not measurement): libp2p "prices connection setup in round trips because on IP they are free": multistream-select, Noise handshake, muxer negotiation, identify ("a couple of kilobytes by itself"), then "GossipSub heartbeats every second, forever". — [funkpost issue #1](https://github.com/NiKrause/funkpost/issues/1)
- An existing Go `libp2p-lora-transport` is WIP and its README states it has "absolutely no security". — [RTradeLtd/libp2p-lora-transport](https://github.com/RTradeLtd/libp2p-lora-transport)
- Reticulum needs no IP and can run over LoRa, serial, packet radio, TCP/UDP/I2P and Ethernet/Wi-Fi (AutoInterface). Its Resources provide chunking, compression, sequencing, integrity checks and retransmission. — [Reticulum manual: Understanding](https://reticulum.network/manual/understanding.html); [Reticulum manual: Interfaces](https://reticulum.network/manual/interfaces.html)
- LXMF supports store-and-forward via Propagation Nodes that peer and synchronize, plus QR/`lxm://` paper messages, with 111 bytes of overhead per message. — [LXMF README](https://github.com/markqvist/LXMF)
- Blockchain Commons animated QR (UR) uses Luby-transform fountain codes, so a receiver can start at any frame and recover from missed frames. A single QR code holds at most 2,953 bytes of binary data. — [Blockchain Commons: animated QRs](https://developer.blockchaincommons.com/animated-qrs/); [Learning Bitcoin: animated QR](https://learningbitcoin.blockchaincommons.com/08_4_Creating_Animated_QR_Codes/)
- ZIP 374 defines a Partially Created Zcash Transaction (PCZT) format, which is useful for splitting build/sign/prove across devices. — [ZIP index (ZIP 374)](https://zips.z.cash/)

### Inferences

**IP-native carriers.**
- 802.11s (L2), BATMAN-adv (L2 over any interface), Babel/OLSR (L3), 5 GHz bridges and HaLow (US: full rate; EU: duty-limited) all carry IPv4/IPv6. nwaku/Logos Delivery nodes can peer over them via static peers or local discovery with no internet.
- Yggdrasil can add stable, encrypted IPv6 addressing across a multi-segment backbone, so libp2p multiaddrs stay valid as the topology changes. The costs are double encryption (Yggdrasil plus Noise) and Yggdrasil's alpha, unaudited status.
- Reticulum can also run over the same IP backbone (AutoInterface/TCP), which makes it a single overlay spanning both IP and LoRa segments.

**Why libp2p over LoRa fails.** My estimates use LongFast (SF11/250 kHz), a 255-byte frame (2.157 s airtime) and about 200 useful bytes per frame, giving about 742 bit/s effective:
- *Header tax.* IPv4+TCP headers are 40 B and IPv6+TCP 60 B, i.e. 16–24% of a 255-byte frame before any libp2p framing (Noise adds a 16-byte MAC per frame; muxer headers add more). Header compression (SCHC/ROHC) would be required, and nobody ships it for libp2p.
- *Connection setup.* TCP 3-way handshake (~3×40–60 B), multistream-select for security and for the muxer (~20–60 B per round), and Noise XX (~32 B + ~230 B + ~200 B with identity payloads) come to about 0.5 KB. Identify (~1–2 KB, consistent with the "couple of kilobytes" claim), Waku metadata and per-protocol negotiations, and GossipSub SUBSCRIBE/GRAFT bring the total to about 3 KB. That is about 32 s of airtime and about 54 min wall-clock at 1% duty (SF12: about 2.3 min airtime, about 3.9 h wall-clock). TCP retransmission timers and multi-second LoRa RTTs (each 255-byte frame takes 2.2 s one way) would cause spurious retransmits and collapse.
- *Steady state.* Each published message is forwarded to up to D=6 mesh peers as separate unicasts on what is physically one shared broadcast channel, which multiplies airtime about 6× per hop. IHAVE gossip (message IDs) goes out every 1 s heartbeat while messages sit in the cache. Discv5/peer-exchange chatter, plus libp2p ping or keepalive traffic, consumes the 1% duty budget (36 s of airtime per hour) even with no payments flowing.
- *RLN timing.* A message delayed by hours of store-and-forward will exceed `max_epoch_gap` and the Merkle-root window, so relays will drop it. Offline basecamps also cannot register new RLN memberships or follow membership-tree updates without Ethereum chain access. RLN must therefore terminate at the gateway: the far gateway re-publishes with a fresh proof, or the basecamp cluster runs its own non-RLN or long-epoch network. The Logos Messaging researcher should confirm the exact parameters.

**Recommended bridging pattern (application gateway).**
1. Inside a basecamp and across the IP backbone, wallets/agents publish to a Logos Messaging content topic (Relay or Light Push), keeping Logos privacy properties where IP is available.
2. A gateway service on each basecamp's edge node subscribes (Filter or Relay) to the "outbound-to-remote" topic. It extracts only the application payload: a signed Ironwood transaction (~9.2 KB), or a payment notice or PCZT fragment. It wraps that in an LXMF message to the destination basecamp's LXMF destination, or to the internet gateway's. LXMF/RNS supplies E2E encryption (X25519/AES), Resource segmentation and retransmission, and Propagation-Node store-and-forward.
3. On arrival, the receiving gateway re-injects the payload into its local Logos Messaging network (fresh RLN proof, current epoch) or hands it directly to zebrad/Zaino for broadcast when it has an uplink.
4. For one-to-many bulk data (compact-block bundles going to several basecamps), use a rateless fountain code (LT, as in BC-UR, or RaptorQ) over broadcast frames, so receivers only need *any* sufficient subset of frames and per-receiver ACK storms are avoided. For unicast transaction delivery, Reticulum Resources (ARQ) are adequate.
5. For the zero-RF fallback, the same payload goes out as LXMF paper messages (QR or `lxm://`), animated BC-UR QR, or files on USB. A 9,166-byte transaction needs ⌈9166/2953⌉ = 4 maximum-capacity static QR codes, or a short animated-UR loop.
6. Do not tunnel IP or libp2p over RNS/LoRa. Bridging at the application layer avoids the TCP/Noise/GossipSub overheads and the RLN timing failures.

**Which links can carry what.**
- *Logos Messaging native:* 802.11s/BATMAN/Babel/OLSR meshes, 5 GHz PtP/PtMP and HaLow, with Yggdrasil optional on top.
- *Bridged only (no IP):* Meshtastic, MeshCore, RNode/Reticulum, BLE mesh and Briar.
- *Not legally usable for encrypted payloads:* Part 97 amateur links.
- *Meshtastic specifics:* it has an API and "store and forward" plugins, but its ~233-byte payloads, 7-hop flood and lack of segmentation/ARQ make it worse than Reticulum for multi-KB transactions.
- *MeshCore specifics:* 184-byte payloads with path routing and room servers. Better than Meshtastic for unicast, but still no bulk-transfer layer was found.

### Gaps
- Exact byte counts of a libp2p/nwaku connection setup (identify payload size, Waku metadata protocol, multistream strings) were not measured. The ~3 KB figure is my estimate and should be confirmed with a packet capture of a nwaku-to-nwaku connection.
- I found no Logos Messaging statement on a supported maximum message size or on running an isolated cluster without RLN or chain access (deferred to the Logos Messaging researcher).
- No published Reticulum Resource throughput measurements over RNode at given SF/BW were found.
- No existing Logos-to-Reticulum bridge implementation was found.

---

## 3. Data budget: Zcash (Ironwood) payload sizes and transfer times over each link

### Takeaway
Ironwood reuses Orchard's encoding unchanged: 820-byte actions, a Halo 2 proof of 2,720 + 2,272·n bytes, and 64-byte signatures (ZIP 229). So the authoritative size of a typical 1-input/2-output Ironwood-only payment (2 actions) is **9,166 bytes** (3 actions: 12,322 B; 4 actions: 15,478 B). Moving one transaction is easy on every IP link (under 1 s) and feasible on LoRa:
- about 99 s of airtime at LongFast (SF11/250), about 16.5 min wall-clock in the EU 10% band, or about 2.8 h in the 1% band;
- about 7 min of airtime at SF12/125 (about 12 h at 1%).

Daily light-client sync is the real problem. With the same lightwalletd/Zaino compact-block format (148 raw bytes per Ironwood compact action, about 159 B on the wire), a day of compact blocks is about 0.15 MB (quiet testnet) to about 4 MB (mainnet Aug/Sep-2026 peak). Full block headers add 1.71 MB/day (1,152 × 1,487 B). At LongFast that is about 0.5–12 h of airtime per day, which is impossible under EU 1% and marginal at 10%. Chain-sync data should therefore travel over HaLow or Wi-Fi or by courier, and LoRa should carry only transactions and short notices.

### Cited Findings
- **Ironwood status.** NU6.3 introduced the Ironwood shielded pool. Activation heights are **Testnet 4,134,000** and **Mainnet 3,428,143**, with MIN_NETWORK_PROTOCOL_VERSION 170160. ZIP 258 status is Final. After NU6.3 the Orchard pool can only be spent from, not added to. — [ZIP 258](https://zips.z.cash/zip-0258)
- Mainnet activation was July 28, 2026, following a late-May 2026 Orchard circuit flaw, an emergency soft fork on June 2 and a corrective hard fork. Ironwood "reuses the fixed Orchard zero-knowledge circuit" with minor modifications and Halo 2. — [The Block](https://theblock.co/post/409934/zcash-activates-ironwood-upgrade-launching-new-shielded-pool-after-orchard-vulnerability)
- **v6 transaction format (ZIP 229, Draft, created 2026-06-13).** "The Orchard Action encoding and the Halo 2 proof system are reused unchanged." The Ironwood fields are: nActionsIronwood (compactSize); vActionsIronwood at 820·n bytes ("same encoding as Orchard Actions"); flagsIronwood (1); valueBalanceIronwood (8); anchorIronwood (32); sizeProofsIronwood (compactSize) with proofsIronwood of **2720 + 2272·nActionsIronwood** bytes; vSpendAuthSigsIronwood at 64·n; bindingSigIronwood (64). Common fields are header, nVersionGroupId, nConsensusBranchId, lock_time and nExpiryHeight, 4 bytes each. — [ZIP 229](https://zips.z.cash/zip-0229)
- **Orchard/v5 (ZIP 225).** An OrchardAction is cv, nullifier, rk, cmx and ephemeralKey (32 bytes each), plus encCiphertext (580) and outCiphertext (80), totalling 820 bytes. sizeProofsOrchard is 2720 + 2272·nActionsOrchard. A Sapling SpendDescriptionV5 is 96 bytes plus a 192-byte proof plus a 64-byte sig. A Sapling OutputDescriptionV5 is 756 bytes plus a 192-byte proof. — [ZIP 225](https://zips.z.cash/zip-0225)
- ZIP 2005 (Ironwood quantum recoverability) changes note commitment/key derivation and the note-plaintext lead byte (0x03). It states no change in ciphertext, action or proof size. — [ZIP 2005](https://zips.z.cash/zip-2005)
- **Compact blocks (lightwallet-protocol).** CompactOrchardAction has nullifier [32], cmx [32], ephemeralKey [32] and ciphertext [52] (the first 52 bytes of encCiphertext). CompactTx has `repeated CompactOrchardAction ironwoodActions = 9`, so Ironwood uses the same compact format. CompactSaplingOutput totals 116 bytes and CompactSaplingSpend carries only a 32-byte nf. ChainMetadata includes `ironwoodCommitmentTreeSize`. The CompactBlock `header` field "should always be unset (empty)". — [lightwallet-protocol compact_formats.proto](https://github.com/zcash/lightwallet-protocol/blob/main/walletrpc/compact_formats.proto)
- **Header size.** The serialized block header is 1,487 bytes: 108-byte Equihash input + 32-byte nonce + 3-byte compactSize + 1,344-byte Equihash solution (Mainnet/Testnet). — [Zebra: BLOCK_HEADER_LENGTH](https://doc-internal.zebra.zfnd.org/zebra_chain/block/header/constant.BLOCK_HEADER_LENGTH.html); [Zebra: equihash::Solution](https://zebra.zfnd.org/internal/zebra_chain/work/equihash/enum.Solution.html)
- NU6.3 adds Ironwood fields (hashEarliestIronwoodRoot, hashLatestIronwoodRoot, nIronwoodTxCount) to the history-tree (ZIP 221 FlyClient) node structure. — [ZIP 258](https://zips.z.cash/zip-0258)
- **Block time.** PostBlossomPoWTargetSpacing is 75 s. — [ZIP 208](https://zips.z.cash/zip-0208)
- ZIP 218 (Status: Proposed, created 2026-03-13) would cut the target spacing to 25 s at NU7. — [ZIP 218](https://zips.z.cash/zip-0218)
- **Transaction expiry.** The default expiry delta is 40 blocks after Blossom (about 50 min). `nExpiryHeight = 0` disables expiry, and the maximum is 499,999,999. ZIP 218 recommends 120 blocks at NU7. — [ZIP 203](https://zips.z.cash/zip-0203)
- **Volume.** Q1 2026: 118,484 shielded transactions on mainnet (about 1,300/day). Weekly counts ran 8,000–12,000 from Jan to Jul 2026, and an Aug–Sep 2026 weekly high of 62,375 (about 8,900/day) is reported. These are secondary figures not checked against chain data. — [Blockworks Q1 2026 report](https://blockworks.com/api/investor-report/zcash-token-holder-report-q1-2026/pdf); [Cryptopolitan](https://www.cryptopolitan.com/?p=313646); [Blockworks Q3 2026 report](https://blockworks.com/api/investor-report/zcash-token-holder-report-q3-2026/pdf)
- **Link constants used below:** Meshtastic presets and ToA formula ([Meshtastic](https://meshtastic.org/docs/overview/radio-settings/); [lora-air-time](https://github.com/ifTNT/lora-air-time)); EU duty cycles ([TTN](https://www.thethingsnetwork.org/docs/lorawan/regional-limitations-of-rf-use/)); HaLow EU 10%/2.8% ([Igor'sLAB](https://www.igorslab.de/en/wi-fi-halow-with-kilometer-range-why-the-new-gl-inet-halowlink-2-is-reaching-its-limits-in-europe/)); HaLow 1.31 Mbps average at 15.9 km ([Rethink](https://rethinkresearch.biz/articles/morse-micro-nets-wifi-halow-range-record/)); VARA ([hamradio.my](https://hamradio.my/varac-hf-digital-chat-emcomm/)); Reticulum link cost 297 B ([RNS manual](https://reticulum.network/manual/understanding.html)); LXMF overhead 111 B ([LXMF](https://github.com/markqvist/LXMF)); RLN proof fields ([LIP 17](https://lip.logos.co/messaging/core/draft/17/rln-relay.html)).

### Inferences (with arithmetic)

**A. Ironwood-only 1-in/2-out transaction (2 actions), from the ZIP 229 field table:**
- Common fields: 5 × 4 = 20 B.
- Empty transparent counts: 2 B. Empty Sapling counts: 2 B. nActionsOrchard = 0: 1 B.
- Ironwood bundle:
  - nActions 1 + actions 2×820 = 1,640 + flags 1 + valueBalance 8 + anchor 32 = 1,682 B
  - proof: compactSize 3 + proof bytes 2,720 + 2×2,272 = 7,264, so 7,267 B
  - signatures: spendAuthSigs 2×64 = 128 + bindingSig 64 = 192 B
  - Bundle total: 1,682 + 7,267 + 192 = **9,141 B**
- **Total = 20 + 2 + 2 + 1 + 9,141 = 9,166 bytes.** The proof is 79% of it, and the actions' encrypted ciphertexts (2 × 660 = 1,320 B) another 14%. All of it is high-entropy, so compression gains nothing. Each extra action adds 820 + 2,272 + 64 = 3,156 B: 3 actions = 12,322 B, 4 actions = 15,478 B. A 2-action transaction is the floor for a 1-in/2-out payment with change. That builder-side padding to at least 2 actions is my assumption from Orchard practice; the Zcash researcher should confirm it.
- Wrapping overheads (estimates): LXMF +111 B; a Reticulum Link setup 297 B per session plus per-packet headers (~19–35 B of 500 B) and Fernet token overhead; a Waku message carrying the transaction adds about 288 B of RLN proof fields (128 + 5×32) plus envelope/protobuf (about 9.5 KB total). I use about +25% for LoRa transport overhead (headers, ACKs, path requests).

**B. Compact-block (light-client) daily volume.** Assumptions: protobuf on-wire sizes, 1,152 blocks/day (86,400/75), no header bytes (per the proto), and coinbase/transparent data ignored.
- Per Ironwood compact action: 3×(1+1+32) + (1+1+52) = 156 B, plus a 3-byte embedding, so about **159 B**.
- Per CompactTx overhead: about 45 B (index, 32-byte txid, fee, framing).
- Per CompactBlock overhead: about 102 B (height, 32-byte hash, 32-byte prevHash, time, chainMetadata, gRPC framing).
- Results:
  - Quiet testnet (100 shielded tx/day × 2 actions): 1,152×102 + 100×45 + 200×159 ≈ **154 kB/day**
  - Busier testnet (1,000 tx/day × 2 actions): ≈ **481 kB/day**
  - Mainnet Q1-2026 average (~1,300 tx/day × 2.5 actions, the action count assumed): ≈ **693 kB/day**
  - Mainnet Aug/Sep-2026 peak (~8,900 tx/day × 2.5 actions): ≈ **4.06 MB/day**
- **Full headers** (to verify PoW yourself): 1,152 × 1,487 = **1.71 MB/day**. If ZIP 218's 25 s spacing ever activates, this triples to 3,456 × 1,487 ≈ 5.1 MB/day, and the per-block compact overhead triples with it.
- To verify that a payment was mined (a confirmation proof), a basecamp needs one header (1,487 B), a Merkle branch (~32 B × log2(tx count), about 160–320 B) and the transaction, so about 1.7 KB beyond the transaction. Without the header chain, that proof is only as trustworthy as the gateway that supplies it. A FlyClient/history-tree proof (ZIP 221, extended by NU6.3) could compress chain verification to logarithmic size; I did not find a deployed implementation.

**C. LoRa per-frame airtime.** Computed with the SX127x formula: 255-byte PHY payload, 16-symbol preamble, CRC on, explicit header, CR4/5 unless noted.
- Worked example, SF11/250 kHz: T_sym = 2048/250,000 = 8.192 ms. Preamble = (16 + 4.25) × 8.192 = 165.9 ms. Payload symbols = 8 + ⌈(2040 − 44 + 28 + 16)/44⌉ × 5 = 8 + 47×5 = 243, i.e. 1,990.7 ms. **ToA = 2,157 ms.** PHY goodput = 255×8/2.157 = 946 bit/s.
- SF12/125 kHz (DE = 1): T_sym = 32.768 ms. Payload symbols = 8 + ⌈2036/40⌉ × 5 = 263. ToA = 663.6 + 8,618 = **9,282 ms**, giving 220 bit/s.
- Every row assumes 200 useful bytes per 255-byte frame (≈22% for mesh/RNS headers and crypto), a single hop and no losses.

| Preset | ToA per 255 B | PHY goodput | Over 0.4 s US dwell? |
|---|---|---|---|
| SF12/BW125 | 9,282 ms | 220 bps | yes |
| SF11/BW125 CR4/8 (LongModerate) | 7,934 ms | 257 bps | yes |
| SF11/BW250 (LongFast) | 2,157 ms | 946 bps | yes |
| SF10/BW250 (MediumSlow) | 1,181 ms | 1,728 bps | yes |
| SF9/BW250 (MediumFast) | 642 ms | 3,180 bps | yes |
| SF8/BW250 (ShortSlow) | 362 ms | 5,640 bps | no |
| SF7/BW250 (ShortFast) | 204 ms | 10,005 bps | no |
| SF7/BW500 (ShortTurbo, not EU) | 102 ms | 20,009 bps | no |

**D. Transfer times on LoRa.** Wall-clock = airtime ÷ duty cycle. 1 transaction = 9,166 B = 46 frames; 1 day of headers = 8,566 frames; compact blocks/day (mainnet Q1 average, 693 kB) = 3,464 frames. "Back-to-back" means continuous transmission and ignores US hopping/dwell rules. The 10% EU band (869.4–869.65 MHz) is only 250 kHz wide, so BW500 is not allowed there.

| Preset | 1 tx airtime | 1 tx at EU 10% | 1 tx at EU 1% | Compact blocks/day (693 kB) airtime | at EU 10% | at EU 1% | Headers/day (1.71 MB) airtime |
|---|---|---|---|---|---|---|---|
| SF12/125 | 7.1 min | 71 min | 11.9 h | 8.9 h | 3.7 d | 37 d | 22 h |
| SF11/250 LongFast | 99 s | 16.5 min | 2.8 h | 2.1 h | 20.8 h | 8.6 d | 5.1 h |
| SF10/250 | 54 s | 9.1 min | 90 min | 68 min | 11.4 h | 4.7 d | 2.8 h |
| SF9/250 | 30 s | 4.9 min | 49 min | 37 min | 6.2 h | 2.6 d | 92 min |
| SF8/250 | 17 s | 2.8 min | 28 min | 21 min | 3.5 h | 35 h | 52 min |
| SF7/250 | 9 s | 94 s | 16 min | 12 min | 118 min | 19.6 h | 29 min |
| SF7/500 (US only) | 5 s | n/a | n/a | 6 min | n/a | n/a | 15 min |

Other compact-block scenarios at LongFast airtime: quiet testnet (154 kB) about 28 min/day; peak mainnet (4.06 MB) about 12.1 h/day (5.1 days of wall-clock per day of data at 10%).

- *Multi-hop.* On a single shared channel, each relay hop re-spends the same airtime, so an n-hop path has at most about 1/n of these rates. Meshtastic-style managed flooding multiplies airtime by the number of rebroadcasting nodes. Every relay also draws on its own duty-cycle budget. With 25% transport overhead and 2 hops at LongFast, one transaction takes about 2 × 2.1 min = 4.2 min of airtime, about 41 min at 10% or about 7 h at 1%.
- *Expiry interaction.* A transaction built with the default 40-block (~50 min) expiry will **expire before it can cross a multi-hop EU 1% LoRa path or a courier link**. Wallets must set a large expiry delta or `nExpiryHeight = 0`. Trade-offs: no expiry means a stuck transaction lingers, and the anchor's age may be a fingerprint, so the Zcash researcher should assess this.

**E. IP-class links.** Effective throughputs used; transfer time = bytes × 8 / rate.

| Link (effective rate) | 1 tx (9.2 kB) | Compact blocks/day, Q1 mainnet (693 kB) | Peak (4.06 MB) | Headers/day (1.71 MB) |
|---|---|---|---|---|
| AX.25 1200 bd (~600 bps)* (illegal for this payload) | 2.0 min | 2.6 h | 15 h | 6.3 h |
| VARA HF (7,050 bps peak) (illegal for this payload) | 10 s | 13 min | 77 min | 32 min |
| VARA FM (25 kbps peak) (illegal for this payload) | 3 s | 3.7 min | 22 min | 9 min |
| HaLow EU station (2.8% × assumed 1 Mbps = 28 kbps avg) | 3 s | 3.3 min | 19 min | 8.2 min |
| HaLow EU AP (10% × 1 Mbps = 100 kbps avg) | 0.7 s | 55 s | 5.4 min | 2.3 min |
| HaLow US at 15.9 km (1.31 Mbps avg) | <0.1 s | 4 s | 25 s | 10 s |
| HaLow US short range (~10 Mbps assumed) | <0.1 s | <1 s | 3 s | 1.4 s |
| 802.11s, 3 hops (~20 Mbps assumed) | <0.1 s | <1 s | 2 s | <1 s |
| 5 GHz PtP (~100 Mbps assumed) | <0.1 s | <0.1 s | <1 s | <0.2 s |

- **Conclusion.** Any HaLow or Wi-Fi backbone (even EU duty-limited HaLow) moves a full day of chain data in minutes. LoRa should carry only transactions (minutes of airtime), payment notices (one 200-byte frame) and, at most, targeted data: a single compact block, a Merkle proof or tree state of a few kB. Bulk sync over LoRa is viable only on US single-hop SF7–SF8 links with quiet chains, and even then it consumes 15–60 min of airtime per day per hop.

### Gaps
- No direct measurement of actual compact-block bytes/day on testnet or mainnet in 2026 was found. My numbers are modelled from the proto and secondary transaction counts, and assume about 2–2.5 actions per transaction. Coinbase/transparent data (`vin`/`vout` in CompactTx) is excluded. A Zaino/lightwalletd GetBlockRange byte count over 1,152 blocks would settle it.
- Ironwood testnet transaction volume after activation at height 4,134,000 is unknown.
- I found no statement on whether wallets pad Ironwood bundles to a minimum of 2 actions (Orchard-builder practice). It should be confirmed with librustzcash.
- ZIP 229 is still marked Draft (ZIP 258 is Final). The v6 field sizes could theoretically change, but ZIP 258 says NU6.3 is already active.
- No source gives real 2022-spam-era compact-block volumes, which would be a useful worst case.

---

## 4. Recommended basecamp topology (backbone, fallback, gateways, store-and-forward)

### Takeaway
Use a three-tier design:
1. **IP backbone between basecamps.** 5 GHz PtP/PtMP where line of sight exists (15–30 km class); Wi-Fi HaLow where foliage or terrain blocks 5 GHz (US: km-scale at Mbps; EU: duty-limited); 802.11s plus BATMAN-adv (or Babel at L3) for path redundancy. Logos Messaging runs natively here.
2. **Fallback for when the backbone is down.** A LoRa/Reticulum layer: RNode transport nodes on high points, an LXMF propagation node at every basecamp, and EU operation in the 869.4–869.65 MHz 10%/500 mW sub-band. It carries only transactions and notices via an application gateway.
3. **Internet gateways.** One or more gateway nodes with intermittent uplink (Starlink Mini, cellular, or a courier carrying USB/QR). They run the full node and light-wallet server when connected, act as the store-and-forward hub for Logos Store, LXMF propagation and transaction queues, and export signed compact-block bundles.

### Cited Findings
- 802.11ah relays are limited to two hops, so HaLow multi-hop needs 802.11s or L3 routing on top. — [Wikipedia: IEEE 802.11ah](https://en.wikipedia.org/wiki/IEEE_802.11ah)
- A single-radio mesh roughly halves throughput per hop. Dual or triple radios, wired backhaul, or separating backhaul (5 GHz) from client access (2.4 GHz) are the standard workarounds. — [b.a.t.m.a.n list](https://lists.open-mesh.org/mailman3/hyperkitty/list/b.a.t.m.a.n@lists.open-mesh.org/thread/HRSUFGGQ36RAYYJZAHASQILFRF3WYYWX); [OpenWrt forum](https://forum.openwrt.org/t/mesh-hops-bandwidth-reduction/66095)
- An informal AREDN-context estimate holds that half-duplex mesh halves bandwidth per hop, so 6 hops leave about 1.5% (estimate, not measured). — [Hackaday: AREDN under test](https://hackaday.io/project/187126-aredn-under-test)
- LXMF Propagation Nodes peer and sync to form a distributed encrypted message store, and users can retrieve from any node. — [LXMF README](https://github.com/markqvist/LXMF)
- MeshCore room servers store and forward for offline recipients. Repeaters extend range from high sites. — [eastmesh wiki](https://wiki.eastmesh.au/meshcore/overview/); [MeshCore](https://meshcore.co.uk/)
- Logos Store keeps messages briefly for offline peers with no availability guarantee. — [Logos docs](https://docs.logos.co/messaging)
- Starlink Mini: 20–40 W average and about 15 W idle (attributed to Starlink support by a vendor blog). One user measured about 22 W idle and a 45 W startup peak, and installers report around 100 W instantaneous at startup. — [Outcamp](https://outcamp.com.au/blogs/starlink-education-guide/starlink-mini-power-consumption-12v-batteries-runtime); [Airforums measurement](https://www.airforums.com/threads/starlink-mini.1441539/post-16188063); [Anker SOLIX blog](https://www.ankersolix.com/blogs/smart-home/starlink-mini-power-consumption)
- Default Zcash transaction expiry is 40 blocks (~50 min). `nExpiryHeight = 0` disables it. — [ZIP 203](https://zips.z.cash/zip-0203)

### Inferences

**Topology sketch.** Each basecamp:
- **Basecamp core.** Raspberry Pi 5 (NVMe) running:
  - wallet daemon (Ironwood-capable light wallet)
  - Logos Messaging node (Relay + Store) with a local RLN membership if required
  - Reticulum `rnsd` (AutoInterface on LAN + RNodeInterface on LoRa) plus `lxmd` propagation node
  - the Logos-to-LXMF gateway service
- **Basecamp LAN.** An OpenWrt router (GL.iNet class) giving a local Wi-Fi AP for phones/laptops. Its 5 GHz or HaLow uplink radio joins the inter-camp backbone via 802.11s with batman-adv over it (or Babel).
- **Inter-camp backbone, by region:**
  - *Line of sight under ~15 km:* 5 GHz PtP pairs (LiteBeam class, 7 W) using dedicated radios, so camp A to B to C does not suffer single-radio halving.
  - *No line of sight, US:* HaLow 2–8 MHz channels (1–10 Mbps at km range).
  - *EU:* HaLow is only a short-range filler because of the 2.8%/10% duty cycles and 25 mW ERP. Rely on 5 GHz.
- **LoRa fallback.** RNode at each camp plus 1–3 solar RNode transport nodes on ridgelines, configured with Reticulum `airtime_limit_long` set below the legal duty cycle:
  - *US 915:* SF7–SF9/BW250–500 for speed, SF11–12 only for long, sparse links.
  - *EU:* 869.525 MHz, BW250, SF7–SF9, 10%. Keep the 868.0–868.6 MHz 1% band for announces only.
- **Gateway site(s).**
  - zebrad full node plus Zaino/lightwalletd, which needs an uplink to stay in sync. The uplink is Starlink Mini or cellular, duty-cycled (e.g., 1 h/day costs about 20–40 Wh).
  - When online: broadcast queued transactions, pull new blocks, then export a signed bundle (compact blocks plus headers or a FlyClient proof, tree states, mempool status of known txids).
  - When offline: hold that bundle for courier or USB delivery.
  - Also runs a Logos Store node and an LXMF propagation node, so remote camps can deposit transactions at any time and collect results later.
  - Put two gateways in different locations to avoid a single point of failure and a single observation point.

**Store-and-forward placement.**
1. At every basecamp (LXMF propagation node plus Logos Store), so a camp can receive while its users are offline.
2. At ridge or relay nodes, *only* as RNS transport. Do not store payloads there, because stolen relays should hold nothing useful.
3. At gateways (the transaction outbox queue plus inbound bundle cache).
4. Courier media (USB) as a physical store-and-forward hop: signed and encrypted bundle files, plus QR for single transactions.

**Data flows (outbound payment from camp A):**
1. The wallet builds and signs an Ironwood transaction (9.2 KB) with a large expiry delta.
2. The wallet publishes it on Logos (backbone up) or LXMF (backbone down) to the gateway outbox.
3. The gateway broadcasts it when its uplink is available.
4. The gateway returns a "mined in block N" notice plus a header and Merkle proof (~1.7 KB) to A and to the payee's camp B.
5. Optionally, A sends B the full transaction directly over the mesh so B can trial-decrypt immediately ("payment pending"). It is not final until mined.

**Why not LoRa-only.** About 693 kB/day of compact blocks for mainnet-like volume needs about 2.1 h/day of LongFast airtime per hop, which no EU duty cycle allows. Wallets that cannot sync cannot update witnesses or spend safely. LoRa-only is acceptable only if each camp's wallet gets chain state by courier and LoRa carries transactions only.

**Latency budget (single transaction, camp to gateway):**
- 5 GHz/HaLow backbone: under 1 s per hop.
- LoRa, 2 hops: about 4 min of airtime at LongFast; wall-clock about 40 min at EU 10% or about 7 h at EU 1%; US about 4 min.
- Courier: hours to days.

### Gaps
- No field reports were found of a combined HaLow + Reticulum + Logos deployment. The topology is my synthesis.
- Gateway-to-camp bundle formats (signed compact-block bundles, FlyClient proofs) have no standard. Whether Zaino or other tools can export or import offline bundles was not researched.
- The privacy cost of gateways learning which transactions or notes a camp cares about is deferred to the threat-model researcher.

---

## 5. Deployment considerations: hardware, power, antennas, interference, tamper resistance, RF/direction-finding exposure

### Takeaway
Plan a basecamp core at about 10–15 W continuous (Pi 5 at about 3–9 W, a 5 GHz radio at 7 W max, a router or HaLow unit at a few watts, a LoRa node under 0.2 W). That is about 250–360 Wh/day, served by roughly a 200 W panel and a 100 Ah 12.8 V LiFePO4 battery for about 2 days of autonomy at mid-latitude insolation. LoRa ridge relays (RAK4631 class, about 2–11 mA idle and about 27 mA at 20 dBm TX) run on a 5–10 W panel and 18650s. Gateways with Starlink Mini need an extra 20–40 W whenever the uplink is up, plus startup surges of 45–100 W. Line of sight and Fresnel clearance dominate range. Any transmitter can be direction-found, so minimize airtime, beacons and announces, and prefer directional backbone links. Keep keys off relays and assume field hardware will be stolen.

### Cited Findings
- Raspberry Pi 5: about 2.7–3.0 W headless idle, about 3.6 W with peripherals, about 8.8 W with all four CPU cores loaded, up to about 16 W in extreme load (CPU + USB SSD + video), and about 1.7 W in soft-off. — [raspberry.tips 2026 comparison](https://raspberry.tips/en/faq/raspberry-pi-power-consumption-update-2026-all-models-compared)
- A vendor guide (methodology unverified) reports a Pi 5 with NVMe and active cooler at about 12 W sustained. — [fast.io](https://fast.io/resources/openclaw-raspberry-pi-power-supply-always-on-agent-guide/)
- Ubiquiti LiteBeam 5AC Gen2: 7 W max, 24 V passive PoE. — [NetXL](https://www.netxl.com/point-to-point-wireless/ubiquiti-lbe-5ac-gem2-litebeam-ac-gen-2-5ghz-23dbi-radio.md)
- GL.iNet HaLowLink 2 is powered by USB-C 5 V and has an external SMA HaLow antenna. — [Igor'sLAB](https://www.igorslab.de/en/wi-fi-halow-with-kilometer-range-why-the-new-gl-inet-halowlink-2-is-reaching-its-limits-in-europe/)
- RAK4631 (anecdotal): about 2–11 mA in Meshtastic router mode with power saving; about 2.2 mA idle at 13.1 V through a step-down converter and about 27 mA transmitting at 20 dBm. Heltec V3 reports range from about 12 mA to about 100 mA (ESP32). A RAK19007 with MPPT and 2×18650 lost about 12% overnight versus about 20% for a Heltec V3. — [Meshtastic forum: WisBlock power](https://meshtastic.discourse.group/t/power-consumption-of-wisblock-seems-too-high/13543); [kaspars.net](https://kaspars.net/8783); [Meshtastic forum: universal solar node](https://meshtastic.discourse.group/t/universal-meshtastic-solar-node-with-mppt/14766); [d-central solar build guide](https://d-central.tech/solar-meshtastic-node-build/)
- Meshtastic documents a method for measuring power consumption of solar nodes. — [Meshtastic: measure power consumption](https://meshtastic.org/docs/hardware/solar-powered/measure-power-consumption/)
- Starlink Mini is about 20–40 W average and about 15–22 W idle, with startup peaks of 45 W measured and about 100 W reported. — [Outcamp](https://outcamp.com.au/blogs/starlink-education-guide/starlink-mini-power-consumption-12v-batteries-runtime); [Airforums](https://www.airforums.com/threads/starlink-mini.1441539/post-16188063)
- EU ERP limits include antenna gain: 25 mW ERP in most 868 sub-bands, 500 mW ERP in 869.4–869.65 MHz. US LoRaWAN is limited to +30 dBm EIRP. — [TTN](https://www.thethingsnetwork.org/docs/lorawan/regional-limitations-of-rf-use/)
- Direction finding (community discussion, anecdotal): any transmitter can be located by RSSI vectoring or TDOA. Some claim two SDRs and a few packet bursts can localize a node closely, while others note that DF on 868/915 at range needs sensitive receivers and line of sight. The public Meshtastic MQTT broker can leak node IDs and positions, and operators reduce position precision and self-host. — [r/meshtastic (mirror)](https://redlib.hbubli.cc/r/meshtastic/comments/1jb4cai/avoid_triangulation); [Meshtastic forum: hiding node location](https://meshtastic.discourse.group/t/hiding-node-location/9313); [d-central MQTT guide](https://d-central.tech/meshtastic-mqtt-guide/)
- A 2025 Hackfest talk covered Meshtastic privacy leaks, key management and jamming. — [Hackfest 2025](https://cfp.hackfest.ca/hf2025/talk/RUNCYK/)
- Reticulum announces are 167 bytes and the optional `id_callsign` beacons are periodic. LXMF supports `lxm://` and QR paper transport. — [RNS manual](https://reticulum.network/manual/understanding.html); [RNS interfaces](https://reticulum.network/manual/interfaces.html); [LXMF](https://github.com/markqvist/LXMF)

### Inferences

**Hardware bill of materials per role.** Costs are approximate street prices and unverified unless cited.
- **Basecamp core:** Raspberry Pi 5 (8 GB) + NVMe HAT and SSD, in a sealed IP65 enclosure with a conformal-coated board; full-disk encryption (LUKS); and a hardware wallet or secure element for keys if available. Spending keys should never sit unencrypted on an always-on node.
- **Router:** GL.iNet/OpenWrt (802.11s plus batman-adv packages); HaLowLink 2 (€126) in US deployments.
- **Backbone:** LiteBeam 5AC Gen2 pairs (€55–75 each, 7 W) or MikroTik equivalents, on masts.
- **LoRa:** RAK WisBlock (RAK4631 nRF52840 + SX1262) for lowest power; Heltec V3/V4 or LilyGO T-Beam/T3S3 for cheapness or GPS. Flash RNode firmware for Reticulum, or MeshCore/Meshtastic firmware if those stacks are chosen. Antennas: 868/915 MHz collinear omni, 3–6 dBi, at basecamps and ridges; Yagi or panel for dedicated long hops.
- **Gateway:** Pi 5 or mini-PC running zebrad plus Zaino. Chain storage needs a large SSD; the size was not researched. Starlink Mini or a cellular modem on a switched power rail. Optional QR camera and printer, and USB courier kit.

**Power budget example (my arithmetic).**
- Core: Pi 5 at 5 W average + LiteBeam at 5 W average (7 W max) + router/HaLow at 3 W + RAK RNode at 0.1 W ≈ 13 W, so 13 × 24 ≈ **312 Wh/day**.
- Solar: at 4 peak-sun-hours and 75% system efficiency, the panel needs 312 / (4 × 0.75) ≈ 104 W. Specify **150–200 W** for margin and winter.
- Battery: 2 days × 312 Wh = 624 Wh; at 80% usable LiFePO4 depth of discharge, 780 Wh. A **100 Ah × 12.8 V = 1.28 kWh** pack gives about 3 days.
- Gateway: add Starlink at about 30 W × uplink hours (1 h/day is about 30 Wh) and size the DC converter for a 100 W surge. A zebrad node on a Pi 5 with NVMe at about 12 W is about 290 Wh/day by itself.
- LoRa ridge relay: about 10 mA average at 3.7 V is about 0.9 Wh/day. A 5–6 W panel plus 2×18650 (about 25 Wh) gives days of autonomy. Use RAK/nRF52 rather than ESP32 boards for relays.

**Antennas and line of sight.**
- First-Fresnel-zone radius at mid-path is r = 17.32·√(d_km / (4·f_GHz)) metres.
  - 10 km at 0.915 GHz: about 28.6 m.
  - 10 km at 5.8 GHz: about 11.4 m.
- So 900 MHz links need *taller* masts for the same clearance even though they tolerate foliage better. Ridge placement matters more than TX power.
- EU ERP caps include antenna gain. At 869.4–869.65 MHz (500 mW ERP), a 6 dBi antenna requires backing TX power down.
- 5 GHz bridges should use narrow-beam dishes or panels. This raises link margin and also reduces off-axis emissions (lower DF exposure) and interference.

**Interference.**
- 868/915 MHz is shared with LoRaWAN, community Meshtastic/MeshCore meshes, alarms and meters. Use a private Reticulum or LoRa channel and sync word, and avoid popular Meshtastic default frequencies.
- In the EU, HaLow (863–868) overlaps the LoRa 865–868 sub-bands, so do not co-locate both there.
- 5 GHz has DFS radar-avoidance channels (general regulatory knowledge, not sourced here). Plan channels and test.

**Tamper resistance.**
- Relays and transport nodes should hold no spending keys and no plaintext payloads: Reticulum transport needs only its own node identity, and LXMF/Logos payloads are end-to-end encrypted.
- Enclosures should be locked and tamper-evident (seals, chassis-intrusion switch on Pi GPIO triggering key wipe on the core).
- Disable debug and USB-boot interfaces where possible, and use signed firmware.
- Rotate node identities if a relay is recovered after loss.
- Courier USB bundles should be encrypted and signed (e.g., age + minisign), because a lost stick otherwise reveals transaction graph metadata.

**RF emission and direction-finding exposure (brief; the threat-model researcher covers depth).**
- Every transmission, including HaLow, LoRa and 5 GHz, reveals that a node exists and roughly where it is, and LoRa's long, regular frames are easy to detect.
- SF12 frames of about 9 s are much easier to DF than SF7 frames of about 0.1–0.2 s.
- Periodic Reticulum announces (167 B), Meshtastic position/telemetry beacons and callsign IDs create a recognisable activity rhythm. Raise announce intervals, disable position sharing, and never bridge to public MQTT.
- Mitigations:
  - Prefer directional, high-gain backbone links (5 GHz dishes) over omni broadcast.
  - Batch transaction traffic into scheduled bursts, perhaps with cover traffic so payment timing does not correlate with RF bursts.
  - Use the lowest SF that closes the link.
  - Place emitters (ridge relays) away from people and wallets, so DF locates a relay rather than a basecamp.
- Ciphertext does not hide traffic analysis: frame counts betray payload size (a 46-frame burst is about 9.2 KB, i.e. one 2-action Zcash transaction). Pad transactions to fixed sizes or always send fixed-size bundles.

### Gaps
- No measured power draw was found for GL.iNet HaLowLink 2 or Morse Micro HaLow modules under load.
- No authoritative, quantified study was found on LoRa/HaLow direction-finding accuracy (only anecdotal forum claims).
- Zcash full-node (zebrad) storage size and Pi 5 suitability in 2026 were not researched (outside scope and could affect gateway hardware choice).
- Street prices for RAK/Heltec/LilyGO boards and MikroTik gear were not verified.
- Regional solar insolation for the actual basecamp locations is unknown. The 4 peak-sun-hours figure is an assumption.
