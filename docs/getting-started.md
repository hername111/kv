# 快速上手

## 环境要求

- Rust 1.94.0 或更新版本
- Node.js 20+ 与 npm 10+
- Python 3.10+
- 可选：MySQL CLI

## 启动后端服务

在仓库根目录运行：

```powershell
cargo run -p kv-server
```

默认地址：

| 服务 | 地址 |
| --- | --- |
| MySQL Wire Protocol | `127.0.0.1:3307` |
| HTTP API | `127.0.0.1:8080` |
| 数据目录 | `kv_data/kv.db` |

可以通过环境变量指定隔离数据目录：

```powershell
$env:KV_DATA_DIR = "target/demo"
cargo run -p kv-server
```

## 启动 Web 工作台

另开一个终端：

```powershell
cd demo-client
npm ci
npm run dev
```

打开 <http://127.0.0.1:5173>。Vite 会把 `/api` 请求代理到后端 HTTP API。

## 使用 MySQL 客户端

```powershell
mysql -h 127.0.0.1 -P 3307 -u root
```

示例 SQL：

```sql
CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(100), age INT);
INSERT INTO users VALUES (1, 'Ada', 28), (2, 'Grace', 31);
SELECT id, name FROM users WHERE age > 28 ORDER BY id;
BEGIN;
UPDATE users SET age = 32 WHERE id = 2;
ROLLBACK;
```

## 清理演示数据

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\show-kv-db.ps1 -ResetDemoData
```

默认清理 `target/demo`。如果使用了自定义目录：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\show-kv-db.ps1 -ResetDemoData -DemoDir target/my-demo
```
