import socket
import struct
import sys

def _read_lenenc(data, offset):
    """Read a MySQL length-encoded integer from data at offset. Returns (value, new_offset)."""
    if offset >= len(data):
        return 0, offset
    first = data[offset]
    if first < 251:
        return first, offset + 1
    elif first == 0xFC:
        return struct.unpack('<H', data[offset+1:offset+3])[0], offset + 3
    elif first == 0xFD:
        return (data[offset+1] | (data[offset+2] << 8) | (data[offset+3] << 16)), offset + 4
    elif first == 0xFE:
        return struct.unpack('<Q', data[offset+1:offset+9])[0], offset + 9
    else:
        return 0, offset + 1

def read_raw_packet(sock):
    """Read a single MySQL packet, returning (seq, payload)."""
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

def read_full_response(sock):
    """Read a complete MySQL response (OK, ERR, or full result set with EOF-terminated rows)."""
    first = read_raw_packet(sock)
    if first is None:
        return None, "NO_RESPONSE"
    seq, data = first
    first_byte = data[0] if data else 0

    if first_byte == 0x00:
        # OK packet: decode length-encoded integers
        affected, pos = _read_lenenc(data, 1)
        last_id, pos = _read_lenenc(data, pos)
        return seq, f"OK(affected={affected}, last_id={last_id})"
    elif first_byte == 0xff:
        # Error packet — single packet
        err_code = struct.unpack('<H', data[1:3])[0]
        sql_state = data[3:8].decode('ascii', errors='replace')
        err_msg = data[8:].decode('utf-8', errors='replace')
        return seq, f"ERR(code={err_code}, sql_state={sql_state}, msg={err_msg})"
    elif first_byte == 0xfe and len(data) < 9:
        # EOF packet (length < 9 to distinguish from OK with 0xfe header flag)
        return seq, "EOF"
    else:
        # Result set: first byte = column count
        col_count = first_byte
        parts = [f"col_count={col_count}"]

        # Read column definitions (col_count packets)
        columns = []
        for _ in range(col_count):
            pkt = read_raw_packet(sock)
            if pkt is None:
                break
            # Column definition: starts with "def" catalog/schema/table/name
            columns.append(pkt)

        # Read EOF after column defs
        eof_pkt = read_raw_packet(sock)
        if eof_pkt is None:
            return seq, f"RS({', '.join(parts)}, TRUNCATED)"

        # Read row data until EOF
        rows = []
        while True:
            pkt = read_raw_packet(sock)
            if pkt is None:
                break
            payload = pkt[1]
            if len(payload) < 9 and (len(payload) == 0 or payload[0] == 0xfe):
                # EOF — end of result set
                break
            # Row data
            rows.append(payload)

        parts.append(f"rows={len(rows)}")
        return seq, f"RS({', '.join(parts)})"


def send_command(sock, sql, seq=0):
    payload = b'\x03' + sql.encode()
    packet = struct.pack('<I', len(payload))[0:3] + bytes([seq]) + payload
    sock.sendall(packet)

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.settimeout(10)
try:
    sock.connect(('127.0.0.1', 3307))

    seq, data = read_raw_packet(sock)
    print(f"1. Handshake: {len(data)} bytes, seq={seq}")

    # Send auth response (dummy 32 bytes)
    resp = b'\x00' * 32
    packet = struct.pack('<I', len(resp))[0:3] + bytes([1]) + resp
    sock.sendall(packet)

    seq, result = read_full_response(sock)
    print(f"2. Auth: {result}")

    tests = [
        "CREATE TABLE t (id INT PRIMARY KEY, name VARCHAR(100))",
        "INSERT INTO t VALUES (1, 'hello')",
        "INSERT INTO t VALUES (2, 'world')",
        "SELECT * FROM t",
        "SELECT id, name FROM t WHERE id = 1",
        "UPDATE t SET name = 'updated' WHERE id = 1",
        "SELECT * FROM t WHERE id = 1",
        "DELETE FROM t WHERE id = 2",
        "SELECT * FROM t",
    ]

    for i, sql in enumerate(tests):
        send_command(sock, sql, 0)
        seq, result = read_full_response(sock)
        print(f"{i+3}. {sql[:55]:<55s} => {result}")

except Exception as e:
    import traceback
    traceback.print_exc()
finally:
    sock.close()
