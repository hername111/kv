# 测试与验证

## 一键检查

推荐在提交前运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-and-show-tests.ps1
```

该脚本会执行：

1. `cargo fmt --check --all`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace --all-targets`
4. `cargo doc --workspace --no-deps`
5. `npm run build`
6. `python test_protocol.py`
7. `git diff --check`

预期结尾：

```text
passed: 7
failed: 0
RESULT: ALL CHECKS PASSED
```

## 协议与持久化测试

```powershell
python test_protocol.py
```

默认完整模式会自动选择临时本地端口，启动独立测试服务和隔离数据目录。测试覆盖：

- MySQL 握手与认证
- DDL / DML
- `WHERE`、`ORDER BY`、`JOIN`
- `BEGIN`、`COMMIT`、`ROLLBACK`
- 索引创建
- 错误输入
- 服务重启后的表结构和数据持久化

当前基线：87 项协议/持久化测试通过。

## 直连已有服务

如果已经手动启动了 `kv-server`，可以跳过持久化重启测试：

```powershell
python test_protocol.py --no-persistence --port 3307
```

## 数据库文件检查

查看数据库 superblock 和 Web 当前状态：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\show-kv-db.ps1 -DbPath target/demo/kv.db
```

脚本以只读方式读取数据库文件，并输出：

- 文件大小
- 页面大小
- 页面数量
- `KVDBPAGE` 魔数
- 格式版本
- `next page id`
- `free-list head`
- `catalog root`
- 后端 HTTP API 当前表数据
