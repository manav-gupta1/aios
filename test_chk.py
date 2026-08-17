import struct, socket
def checksum(data):
    if len(data) % 2 != 0:
        data += b'\x00'
    s = sum(struct.unpack('!%dH' % (len(data)//2), data))
    s = (s >> 16) + (s & 0xffff)
    s += s >> 16
    return ~s & 0xffff

ip_src = socket.inet_aton('10.0.2.15')
ip_dst = socket.inet_aton('172.66.147.243')
payload = b"GET / HTTP/1.1\r\nHost: www.example.com\r\nConnection: close\r\n\r\n"
tcp_len = 20 + len(payload)
pseudo_hdr = struct.pack('!4s4sBBH', ip_src, ip_dst, 0, 6, tcp_len)
tcp_hdr = struct.pack('!HHIIHHHH', 49153, 80, 12345679, 576002, 0x5018, 8192, 0, 0)
print(f"{checksum(pseudo_hdr + tcp_hdr + payload):04x}")
