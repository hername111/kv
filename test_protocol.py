#!/usr/bin/env python3
"""
KV 数据库全功能测试脚本
测试所有用户可见的 SQL 功能：DDL / DML / 查询 / 事务 / 索引 / 持久化

用法：
    python test_protocol.py [--no-persistence]

    --no-persistence  跳过需要启停服务进程的磁盘持久化测试
"""

import socket
import struct
import subprocess
import sys
import time
import os
import shutil
from pathlib import Path

# ============================================================================
# MySQL 协议工具函数
# ============================================================================

def _read_lenenc(data, offset):
    """读取 MySQL length-encoded integer。返回 (value, new_offset)。"""
    if offset >= len(data):
        return 0, offset
    first = data[offset]
    if first < 251:
        return first, offset + 1
    elif first == 0xFC:
        return struct.unpack('<H', data[offset + 1:offset + 3])[0], offset + 3
    elif first == 0xFD:
        return (data[offset + 1] | (data[offset + 2] << 8) | (data[offset + 3] << 16)), offset + 4
    elif first == 0xFE:
        return struct.unpack('<Q', data[offset + 1:offset + 9])[0], offset + 9
    else:
        return 0, offset + 1


def read_raw_packet(sock):
    """读取单个 MySQL 包，返回 (seq, payload) 或 None。"""
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
    """
    读取完整 MySQL 响应。
    返回 (seq, decoded_response_dict) 或 (None, error_string)。
    """
    first = read_raw_packet(sock)
    if first is None:
        return None, {"error": "NO_RESPONSE"}
    seq, data = first
    first_byte = data[0] if data else 0

    if first_byte == 0x00:
        # OK 包
        affected, pos = _read_lenenc(data, 1)
        last_id, pos = _read_lenenc(data, pos)
        return seq, {"type": "OK", "affected": affected, "last_id": last_id}

    elif first_byte == 0xff:
        # 错误包
        err_code = struct.unpack('<H', data[1:3])[0]
        sql_state = data[3:8].decode('ascii', errors='replace')
        err_msg = data[8:].decode('utf-8', errors='replace')
        return seq, {"type": "ERR", "code": err_code, "sql_state": sql_state, "msg": err_msg}

    else:
        # 结果集: first_byte = 列数
        col_count = first_byte

        # 读取列定义
        columns = []
        for _ in range(col_count):
            pkt = read_raw_packet(sock)
            if pkt is None:
                break
            columns.append(pkt[1])

        # 读取 EOF
        eof_pkt = read_raw_packet(sock)
        if eof_pkt is None:
            return seq, {"type": "RS", "columns": len(columns), "rows": [], "truncated": True}

        # 读取行数据直到 EOF
        rows = []
        while True:
            pkt = read_raw_packet(sock)
            if pkt is None:
                break
            payload = pkt[1]
            if len(payload) < 9 and (len(payload) == 0 or payload[0] == 0xfe):
                break
            rows.append(payload)

        return seq, {
            "type": "RS",
            "col_count": col_count,
            "col_defs": columns,
            "rows": rows,
            "row_count": len(rows),
        }


def send_command(sock, sql, seq=0):
    """发送 COM_QUERY 命令。"""
    payload = b'\x03' + sql.encode()
    packet = struct.pack('<I', len(payload))[0:3] + bytes([seq]) + payload
    sock.sendall(packet)


# ============================================================================
# 客户端封装
# ============================================================================

class KVClient:
    """KV 数据库客户端，封装 MySQL 协议连接和命令执行。"""

    def __init__(self, host='127.0.0.1', port=3307, timeout=10):
        self.host = host
        self.port = port
        self.timeout = timeout
        self.sock = None

    def connect(self):
        """连接并进行 MySQL 握手认证。"""
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect((self.host, self.port))

        # 读取握手包
        seq, data = read_raw_packet(self.sock)
        if data is None:
            raise ConnectionError("握手失败：无响应")

        # 发送认证响应（32 字节任意密码）
        resp = b'\x00' * 32
        packet = struct.pack('<I', len(resp))[0:3] + bytes([1]) + resp
        self.sock.sendall(packet)

        seq, result = read_full_response(self.sock)
        if result.get("type") == "ERR":
            raise ConnectionError(f"认证失败: {result}")
        return result

    def execute(self, sql):
        """执行 SQL 并返回解析后的响应 dict。"""
        send_command(self.sock, sql, 0)
        seq, result = read_full_response(self.sock)
        return result

    def close(self):
        """关闭连接。"""
        if self.sock:
            self.sock.close()
            self.sock = None


# ============================================================================
# 测试框架
# ============================================================================

PASS = 0
FAIL = 0
TEST_INDEX = 0


def test(name, condition, detail=""):
    """单个测试断言。"""
    global PASS, FAIL, TEST_INDEX
    TEST_INDEX += 1
    if condition:
        PASS += 1
        print(f"  [{TEST_INDEX}] \033[92mPASS\033[0m {name}")
    else:
        FAIL += 1
        print(f"  [{TEST_INDEX}] \033[91mFAIL\033[0m {name}  {detail}")


def test_raises(name, result, expected_msg_substring=""):
    """断言结果为 ERR 类型。"""
    is_err = result.get("type") == "ERR"
    ok = is_err
    detail = ""
    if not is_err:
        detail = f"expected error, got: {result}"
    elif expected_msg_substring and expected_msg_substring not in result.get("msg", ""):
        ok = False
        detail = f"error msg mismatch: {result.get('msg')}"
    test(name, ok, detail)


def test_ok(name, result, expected_affected=None):
    """断言结果为 OK 类型。"""
    ok = result.get("type") == "OK"
    detail = ""
    if not ok:
        detail = f"expected OK, got: {result}"
    elif expected_affected is not None and result.get("affected") != expected_affected:
        detail = f"expected affected={expected_affected}, got={result.get('affected')}"
        ok = False
    test(name, ok, detail)


def test_rs(name, result, expected_rows=None, expected_cols=None):
    """断言结果为结果集类型。"""
    ok = result.get("type") == "RS"
    detail = ""
    if not ok:
        detail = f"expected RS, got: {result}"
    else:
        if expected_rows is not None and result.get("row_count") != expected_rows:
            detail = f"expected {expected_rows} rows, got {result.get('row_count')}"
            ok = False
        if expected_cols is not None and result.get("col_count") != expected_cols:
            detail = f"expected {expected_cols} cols, got {result.get('col_count')}"
            ok = False
    test(name, ok, detail)


def section(title):
    """打印测试小节标题。"""
    print(f"\n{'─' * 60}")
    print(f"  {title}")
    print(f"{'─' * 60}")


def summary():
    """打印测试汇总。"""
    total = PASS + FAIL
    print(f"\n{'═' * 60}")
    print(f"  Total: {total}, \033[92m{PASS} passed\033[0m", end="")
    if FAIL > 0:
        print(f", \033[91m{FAIL} failed\033[0m")
    else:
        print("")
    print(f"{'═' * 60}")
    return FAIL == 0


# ============================================================================
# 服务进程管理（用于持久化测试）
# ============================================================================

class ServerProcess:
    """管理 KV 服务器的启停。"""

    def __init__(self, data_dir="target/kv-test-data", port=3307):
        self.data_dir = data_dir
        self.port = port
        self.process = None
        self.binary = None

    def _find_binary(self):
        """查找编译好的服务端二进制。"""
        candidates = [
            "target/debug/kv-server.exe",
            "target/debug/kv-server",
            "target/release/kv-server.exe",
            "target/release/kv-server",
        ]
        for c in candidates:
            if os.path.exists(c):
                return c
        return None

    def start(self):
        """编译并启动服务器。"""
        print("  Building server...")
        result = subprocess.run(
            ["cargo", "build", "--bin", "kv-server"],
            capture_output=True, text=True, timeout=120
        )
        if result.returncode != 0:
            print(f"  Build failed:\n{result.stderr}")
            return False

        self.binary = self._find_binary()
        if not self.binary:
            print(f"  Binary not found")
            return False

        # 清理旧数据目录，确保干净起点
        if os.path.exists(self.data_dir):
            shutil.rmtree(self.data_dir)

        self.process = subprocess.Popen(
            [self.binary],
            env={
                **os.environ,
                "KV_DATA_DIR": self.data_dir,
                "KV_ADDR": f"127.0.0.1:{self.port}",
                "KV_DEMO_ADDR": "127.0.0.1:18080",
            },
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        time.sleep(2)
        if self.process.poll() is not None:
            stdout, stderr = self.process.communicate()
            print(f"  Server start failed:\nSTDOUT:\n{stdout.decode()}\nSTDERR:\n{stderr.decode()}")
            return False
        print(f"  Server started (pid={self.process.pid})")
        return True

    def stop(self):
        """停止服务器。"""
        if self.process:
            # 先检查是否已经崩溃
            if self.process.poll() is not None:
                stdout, stderr = self.process.communicate()
                print(f"  Server died (rc={self.process.returncode})")
                if stdout:
                    print(f"  STDOUT:\n{stdout.decode()[:500]}")
                if stderr:
                    print(f"  STDERR:\n{stderr.decode()[:500]}")
                self.process = None
                return
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
            print(f"  Server stopped")
            self.process = None

    def restart(self):
        """重启服务器（保留数据目录）。"""
        self.stop()
        self.process = subprocess.Popen(
            [self.binary],
            env={
                **os.environ,
                "KV_DATA_DIR": self.data_dir,
                "KV_ADDR": f"127.0.0.1:{self.port}",
                "KV_DEMO_ADDR": "127.0.0.1:18080",
            },
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        time.sleep(2)
        if self.process.poll() is not None:
            stdout, stderr = self.process.communicate()
            print(f"  Server restart failed:\n{stdout.decode()}\n{stderr.decode()}")
            return False
        print(f"  Server restarted (pid={self.process.pid})")
        return True


# ============================================================================
# 测试套件
# ============================================================================

def test_connection(client):
    """连接测试。"""
    section("Connection & Auth")
    try:
        result = client.connect()
        test("MySQL handshake + auth", result.get("type") == "OK", str(result))
    except Exception as e:
        test("MySQL handshake + auth", False, str(e))
        return False
    return True


def test_ddl(client):
    """DDL: CREATE TABLE, DROP TABLE。"""
    section("DDL: CREATE TABLE / DROP TABLE")

    result = client.execute(
        "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(100), age INT, status INT)"
    )
    test_ok("CREATE TABLE with INT, VARCHAR types", result)

    # 注意: "desc" 是 DESC 关键字，不能用作列名
    result = client.execute(
        "CREATE TABLE products (pid INT PRIMARY KEY, price FLOAT, description TEXT)"
    )
    test_ok("CREATE TABLE with FLOAT, TEXT types", result)

    result = client.execute("DROP TABLE products")
    test_ok("DROP TABLE", result)

    result = client.execute("DROP TABLE nonexistent")
    test_ok("DROP TABLE non-existent (no-op)", result)


def test_insert(client):
    """INSERT。"""
    section("DML: INSERT")

    result = client.execute("INSERT INTO users VALUES (1, 'Alice', 30, 1)")
    test_ok("INSERT single row", result, expected_affected=1)

    result = client.execute("INSERT INTO users VALUES (2, 'Bob', 25, 0)")
    test_ok("INSERT second row", result, expected_affected=1)

    result = client.execute("INSERT INTO users VALUES (3, 'Charlie', 35, 1)")
    test_ok("INSERT third row", result, expected_affected=1)

    result = client.execute("INSERT INTO users (id, name) VALUES (4, 'Diana')")
    test_ok("INSERT with column list", result, expected_affected=1)

    result = client.execute("INSERT INTO users VALUES (5, 'Eve', 28, 1), (6, 'Frank', 22, 0)")
    test_ok("INSERT multi-row (2 rows)", result, expected_affected=2)


def test_select_basic(client):
    """SELECT 基本查询。"""
    section("DML: SELECT Basics")

    result = client.execute("SELECT * FROM users")
    test_rs("SELECT * all rows", result, expected_rows=6, expected_cols=4)

    result = client.execute("SELECT id, name FROM users")
    test_rs("SELECT specific columns", result, expected_rows=6, expected_cols=2)

    result = client.execute("SELECT name AS n, age AS a FROM users")
    test_rs("SELECT with AS aliases", result, expected_rows=6)


def test_select_where(client):
    """SELECT WHERE 条件。"""
    section("DML: SELECT WHERE")

    result = client.execute("SELECT * FROM users WHERE id = 1")
    test_rs("WHERE id = 1 (eq)", result, expected_rows=1)

    result = client.execute("SELECT * FROM users WHERE id > 3")
    test_rs("WHERE id > 3 (gt)", result, expected_rows=3)

    result = client.execute("SELECT * FROM users WHERE id >= 5")
    test_rs("WHERE id >= 5 (gte)", result, expected_rows=2)

    result = client.execute("SELECT * FROM users WHERE id < 3")
    test_rs("WHERE id < 3 (lt)", result, expected_rows=2)

    result = client.execute("SELECT * FROM users WHERE id <= 2")
    test_rs("WHERE id <= 2 (lte)", result, expected_rows=2)

    result = client.execute("SELECT * FROM users WHERE id <> 3")
    test_rs("WHERE id <> 3 (neq)", result, expected_rows=5)

    result = client.execute("SELECT * FROM users WHERE id != 4")
    test_rs("WHERE id != 4 (neq)", result, expected_rows=5)

    result = client.execute("SELECT * FROM users WHERE (id > 3) AND (status = 1)")
    test_rs("WHERE (cond) AND (cond) [with parens]", result, expected_rows=1)

    result = client.execute("SELECT * FROM users WHERE (id = 1) OR (id = 6)")
    test_rs("WHERE (cond) OR (cond) [with parens]", result, expected_rows=2)

    # 验证 AND/OR 与比较运算符的优先级修复
    result = client.execute("SELECT * FROM users WHERE id > 3 AND status = 1")
    test_rs("WHERE id > 3 AND status = 1 (no parens, precedence)", result, expected_rows=1)

    result = client.execute("SELECT * FROM users WHERE id = 1 OR id = 6")
    test_rs("WHERE id = 1 OR id = 6 (no parens, precedence)", result, expected_rows=2)

    result = client.execute("SELECT * FROM users WHERE id = 99")
    test_rs("WHERE no match (empty)", result, expected_rows=0)


def test_select_order_by(client):
    """SELECT ORDER BY。"""
    section("DML: SELECT ORDER BY")

    result = client.execute("SELECT id, name FROM users ORDER BY id ASC")
    test_rs("ORDER BY id ASC", result, expected_rows=6)

    result = client.execute("SELECT id, name FROM users ORDER BY id DESC")
    test_rs("ORDER BY id DESC", result, expected_rows=6)

    result = client.execute("SELECT id, name FROM users ORDER BY name ASC")
    test_rs("ORDER BY string column ASC", result, expected_rows=6)

    result = client.execute("SELECT * FROM users WHERE (id >= 3) ORDER BY id DESC")
    test_rs("WHERE + ORDER BY combined", result, expected_rows=4)


def test_select_join(client):
    """SELECT JOIN。"""
    section("DML: SELECT JOIN")

    result = client.execute(
        "CREATE TABLE orders (oid INT PRIMARY KEY, uid INT, amount FLOAT)"
    )
    test_ok("CREATE TABLE orders", result)

    client.execute("INSERT INTO orders VALUES (1, 1, 99.9)")
    client.execute("INSERT INTO orders VALUES (2, 2, 199.9)")

    result = client.execute(
        "SELECT * FROM users JOIN orders ON users.id = orders.uid"
    )
    test_rs("JOIN ... ON table.col = table.col", result, expected_rows=2)

    client.execute("DROP TABLE orders")


def test_update(client):
    """UPDATE 测试。"""
    section("DML: UPDATE")

    result = client.execute("UPDATE users SET name = 'Updated' WHERE id = 1")
    test_ok("UPDATE single column WHERE id=1", result, expected_affected=1)

    result = client.execute("SELECT name FROM users WHERE id = 1")
    test_rs("Verify UPDATE effect", result, expected_rows=1)

    result = client.execute("UPDATE users SET name = 'Multi', age = 99 WHERE id = 2")
    test_ok("UPDATE multiple columns", result, expected_affected=1)

    result = client.execute("UPDATE users SET age = 100 WHERE id > 10")
    test_ok("UPDATE no match (affected=0)", result, expected_affected=0)

    result = client.execute("UPDATE users SET age = 0")
    test_ok("UPDATE all rows (no WHERE)", result, expected_affected=6)


def test_delete(client):
    """DELETE 测试。"""
    section("DML: DELETE")

    client.execute("INSERT INTO users VALUES (10, 'Del1', 20, 1)")
    client.execute("INSERT INTO users VALUES (11, 'Del2', 21, 0)")

    result = client.execute("DELETE FROM users WHERE id = 10")
    test_ok("DELETE WHERE id=10", result, expected_affected=1)

    result = client.execute("SELECT * FROM users WHERE id = 10")
    test_rs("Verify DELETE (id=10 gone)", result, expected_rows=0)

    result = client.execute("DELETE FROM users WHERE id = 999")
    test_ok("DELETE no match (affected=0)", result, expected_affected=0)


def test_null_values(client):
    """NULL 值。"""
    section("NULL Values")

    result = client.execute("INSERT INTO users VALUES (20, 'NullTest', NULL, NULL)")
    test_ok("INSERT with NULL values", result, expected_affected=1)

    result = client.execute("SELECT * FROM users WHERE id = 20")
    test_rs("NULL row readable", result, expected_rows=1)


def test_create_index(client):
    """CREATE INDEX。"""
    section("Index: CREATE INDEX")

    result = client.execute("CREATE INDEX idx_users_name ON users (name)")
    test_ok("CREATE INDEX ON users(name)", result)

    result = client.execute("SELECT * FROM users WHERE name = 'Updated'")
    test_rs("SELECT still works after index creation", result, expected_rows=1)


def test_transactions(client):
    """事务: BEGIN / COMMIT / ROLLBACK。"""
    section("Transactions: BEGIN / COMMIT / ROLLBACK")

    # === ROLLBACK 撤销 INSERT ===
    result = client.execute("BEGIN")
    test_ok("BEGIN transaction", result)

    before = client.execute("SELECT * FROM users")
    before_count = before.get("row_count", 0)

    result = client.execute("INSERT INTO users VALUES (100, 'TxnRollback', 50, 1)")
    test_ok("INSERT inside txn", result, expected_affected=1)

    result = client.execute("SELECT * FROM users WHERE id = 100")
    test_rs("Row visible inside txn (write buffer)", result, expected_rows=1)

    result = client.execute("ROLLBACK")
    test_ok("ROLLBACK", result)

    result = client.execute("SELECT * FROM users WHERE id = 100")
    test_rs("Row gone after ROLLBACK", result, expected_rows=0)

    result = client.execute("SELECT * FROM users")
    test_rs(f"Row count restored after ROLLBACK (was {before_count})",
             result, expected_rows=before_count)

    # === COMMIT 持久化 INSERT ===
    result = client.execute("BEGIN")
    test_ok("BEGIN second txn", result)

    result = client.execute("INSERT INTO users VALUES (200, 'TxnCommit', 60, 0)")
    test_ok("INSERT inside txn (COMMIT path)", result, expected_affected=1)

    result = client.execute("COMMIT")
    test_ok("COMMIT", result)

    result = client.execute("SELECT * FROM users WHERE id = 200")
    test_rs("Row persists after COMMIT", result, expected_rows=1)

    # === DELETE in txn, then ROLLBACK (单独测试 DELETE) ===
    result = client.execute("BEGIN")
    test_ok("BEGIN (DELETE txn)", result)

    result = client.execute("DELETE FROM users WHERE id = 200")
    test_ok("DELETE inside txn", result, expected_affected=1)

    result = client.execute("SELECT * FROM users WHERE id = 200")
    test_rs("Gone after DELETE inside txn", result, expected_rows=0)

    result = client.execute("ROLLBACK")
    test_ok("ROLLBACK (undo DELETE)", result)

    result = client.execute("SELECT * FROM users WHERE id = 200")
    test_rs("Row restored after ROLLBACK", result, expected_rows=1)

    # === UPDATE in txn, then ROLLBACK ===
    result = client.execute("BEGIN")
    test_ok("BEGIN (UPDATE txn)", result)

    result = client.execute("UPDATE users SET name = 'UX', age = 999 WHERE id = 200")
    test_ok("UPDATE inside txn", result, expected_affected=1)

    result = client.execute("ROLLBACK")
    test_ok("ROLLBACK (undo UPDATE)", result)

    # 验证回滚后旧值还在
    result = client.execute("SELECT * FROM users WHERE id = 200")
    test_rs("UPDATE undone after ROLLBACK", result, expected_rows=1)

    # === UPDATE + DELETE same row in same txn ===
    result = client.execute("BEGIN")
    test_ok("BEGIN (UPDATE+DELETE txn)", result)

    result = client.execute("INSERT INTO users VALUES (300, 'UpdDel', 50, 1)")
    test_ok("INSERT setup row", result, expected_affected=1)

    result = client.execute("COMMIT")
    test_ok("COMMIT setup row", result)

    result = client.execute("BEGIN")
    test_ok("BEGIN (UPDATE+DELETE txn)", result)

    result = client.execute("UPDATE users SET name = 'Changed' WHERE id = 300")
    test_ok("UPDATE inside txn", result, expected_affected=1)

    result = client.execute("DELETE FROM users WHERE id = 300")
    test_ok("DELETE same row inside txn", result, expected_affected=1)

    result = client.execute("SELECT * FROM users WHERE id = 300")
    test_rs("Row gone after UPDATE+DELETE in same txn", result, expected_rows=0)

    result = client.execute("COMMIT")
    test_ok("COMMIT (UPDATE+DELETE)", result)

    result = client.execute("SELECT * FROM users WHERE id = 300")
    test_rs("Row still gone after COMMIT", result, expected_rows=0)

    # === Error: COMMIT/ROLLBACK without active txn ===
    result = client.execute("COMMIT")
    test_raises("COMMIT without active txn", result, "no active transaction")

    result = client.execute("ROLLBACK")
    test_raises("ROLLBACK without active txn", result, "no active transaction")


def test_error_handling(client):
    """错误处理。"""
    section("Error Handling")

    result = client.execute("SELECT * FROM nonexistent_table")
    test_raises("SELECT non-existent table", result, "not found")

    result = client.execute("INSERT INTO nonexistent VALUES (1)")
    test_raises("INSERT non-existent table", result, "not found")

    result = client.execute("UPDATE nonexistent SET x=1")
    test_raises("UPDATE non-existent table", result, "not found")

    result = client.execute("DELETE FROM nonexistent")
    test_raises("DELETE non-existent table", result, "not found")

    result = client.execute("CREATE INDEX bad_idx ON nonexistent (col)")
    test_raises("CREATE INDEX on non-existent table", result, "not found")

    result = client.execute("GARBAGE SQL !!!")
    test_raises("Invalid SQL syntax", result)


def test_drop_and_recreate(client):
    """DROP TABLE 后重建。"""
    section("DROP TABLE + Recreate")

    result = client.execute("DROP TABLE users")
    test_ok("DROP TABLE users", result)

    result = client.execute(
        "CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(100))"
    )
    test_ok("Recreate users table", result)

    result = client.execute("INSERT INTO users VALUES (1, 'Recreated')")
    test_ok("INSERT after recreate", result, expected_affected=1)

    result = client.execute("SELECT * FROM users")
    test_rs("SELECT after recreate", result, expected_rows=1)


def test_persistence(server):
    """磁盘持久化测试：验证 B+Tree 数据 + 表元数据跨重启存活。"""
    section("Disk Persistence: Data + Metadata survives restart")

    client = KVClient(port=3307)
    try:
        client.connect()

        client.execute(
            "CREATE TABLE persist_test (id INT PRIMARY KEY, val VARCHAR(200))"
        )
        for i in range(5):
            client.execute(f"INSERT INTO persist_test VALUES ({i}, 'data_{i}')")

        result = client.execute("SELECT * FROM persist_test")
        test_rs("Before restart: 5 rows", result, expected_rows=5)

        client.close()

        # 重启服务器
        if not server.restart():
            test("Server restart", False, "restart failed")
            return

        # 重新连接 - 元数据已持久化，无需重建表
        client2 = KVClient(port=3307)
        client2.connect()

        result = client2.execute("SELECT * FROM persist_test")
        test_rs("After restart: data + metadata persisted", result, expected_rows=5)

        # 验证可以继续写入
        client2.execute("INSERT INTO persist_test VALUES (99, 'after_restart')")
        result = client2.execute("SELECT * FROM persist_test")
        test_rs("After restart: INSERT works", result, expected_rows=6)

        # DROP TABLE 后重启 - 验证 DROP 也持久化了
        client2.execute("DROP TABLE persist_test")
        client2.close()

        if not server.restart():
            test("Server re-restart", False, "restart failed")
            return

        client3 = KVClient(port=3307)
        client3.connect()

        result = client3.execute("SELECT * FROM persist_test")
        test_raises("After restart: dropped table stays dropped", result, "not found")

        client3.close()

    except Exception as e:
        test("Persistence test", False, str(e))
    finally:
        try:
            client.close()
        except Exception:
            pass


# ============================================================================
# 主入口
# ============================================================================

def run_tests(skip_persistence=False):
    """运行所有测试。"""
    print("=" * 60)
    print("  KV Database - Full Feature Test Suite")
    print("=" * 60)

    server = None

    if skip_persistence:
        # 直连模式：假定服务器已在运行
        client = KVClient(port=3307)
        try:
            client.connect()
        except Exception as e:
            print(f"\nCannot connect to 127.0.0.1:3307: {e}")
            print("Start the server first: cargo run --bin kv-server")
            return False

        test_ddl(client)
        test_insert(client)
        test_select_basic(client)
        test_select_where(client)
        test_select_order_by(client)
        test_select_join(client)
        test_update(client)
        test_delete(client)
        test_null_values(client)
        test_create_index(client)
        test_transactions(client)
        test_error_handling(client)
        test_drop_and_recreate(client)

        client.close()
        return summary()

    else:
        # 完整模式：自行管理服务器生命周期
        global PASS, FAIL, TEST_INDEX
        server = ServerProcess(port=3307)

        if not server.start():
            print("Server failed to start")
            return False

        try:
            client = KVClient(port=3307)
            if not test_connection(client):
                return False

            test_ddl(client)
            test_insert(client)
            test_select_basic(client)
            test_select_where(client)
            test_select_order_by(client)
            test_select_join(client)
            test_update(client)
            test_delete(client)
            test_null_values(client)
            test_create_index(client)
            test_transactions(client)
            test_error_handling(client)
            test_drop_and_recreate(client)
            client.close()

            test_persistence(server)

            return summary()

        finally:
            server.stop()
            if os.path.exists(server.data_dir):
                shutil.rmtree(server.data_dir)


if __name__ == '__main__':
    skip_persistence = '--no-persistence' in sys.argv
    success = run_tests(skip_persistence=skip_persistence)
    sys.exit(0 if success else 1)
