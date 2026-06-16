import socket
import struct
import sys

def read_packet(sock):
    header = sock.recv(4)
    if len(header) < 4:
        return None
    length = header[0] | (header[1] << 8) | (header[2] << 16)
    seq = header[3]
    data = b''
    while len(data) < length:
        chunk = sock.recv(length - len(data))
        if not chunk:
            break
        data += chunk
    return seq, data

def send_command(sock, sql, seq=0):
    payload = b'\x03' + sql.encode()
    packet = struct.pack('<I', len(payload))[0:3] + bytes([seq]) + payload
    sock.sendall(packet)

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.settimeout(5)
try:
    sock.connect(('127.0.0.1', 3307))

    seq, data = read_packet(sock)
    print(f"1. Handshake: {len(data)} bytes, seq={seq}")

    resp = b'\x00' * 32
    packet = struct.pack('<I', len(resp))[0:3] + bytes([1]) + resp
    sock.sendall(packet)

    seq, data = read_packet(sock)
    print(f"2. Auth OK: {data[0]==0x00}, first_byte=0x{data[0]:02x}")

    send_command(sock, "CREATE TABLE t (id INT PRIMARY KEY, name VARCHAR(100))", 0)
    seq, data = read_packet(sock)
    print(f"3. CREATE TABLE: OK={data[0]==0x00}, len={len(data)}")

    send_command(sock, "INSERT INTO t VALUES (1, 'hello')", 0)
    seq, data = read_packet(sock)
    affected = struct.unpack('<Q', data[1:9])[0] if data[0] == 0x00 else 0
    print(f"4. INSERT 1: OK={data[0]==0x00}, affected_rows={affected}")

    send_command(sock, "INSERT INTO t VALUES (2, 'world')", 0)
    seq, data = read_packet(sock)
    print(f"5. INSERT 2: OK={data[0]==0x00}")

    send_command(sock, "SELECT * FROM t", 0)
    seq, data = read_packet(sock)
    print(f"6. SELECT: first_byte=0x{data[0]:02x}, len={len(data)}")
    if data[0] == 0x00:
        print("   (OK packet)")
    elif data[0] == 0xff:
        err_code = struct.unpack('<H', data[1:3])[0]
        err_msg = data[3:].decode('utf-8', errors='replace')
        print(f"   ERROR: code={err_code}, msg={err_msg}")
    else:
        print(f"   Column count: {data[0]}")

except Exception as e:
    import traceback
    traceback.print_exc()
finally:
    sock.close()
