from scapy.all import IP, TCP, raw
ip = IP(src="10.0.2.15", dst="104.20.23.154")
tcp = TCP(sport=49153, dport=80, seq=12345679, ack=42, flags="PA", window=8192)
payload = b"GET / HTTP/1.1\r\nHost: www.example.com\r\nConnection: close\r\n\r\n"
pkt = ip/tcp/payload
pkt = IP(raw(pkt)) # force checksum calculation
print(f"Scapy TCP Checksum: {pkt[TCP].chksum:04x}")
