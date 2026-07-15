# 本地 HTTP API

服务由 kv-server 启动，默认监听 127.0.0.1:8080。接口只用于本地 React 工作台和演示，不包含用户认证、权限控制或跨用户会话隔离。

## GET /api/state

返回当前演示会话可见的表结构和记录快照：

    {
      "ok": true,
      "tables": [
        {
          "meta": {
            "tableId": 1,
            "tableName": "users",
            "primaryKeyIndex": 0,
            "columns": [
              {
                "id": 0,
                "name": "id",
                "dataType": "Int",
                "nullable": false,
                "primaryKey": true
              }
            ],
            "indexes": 1
          },
          "rows": [[1]]
        }
      ]
    }

字段说明：

- meta.columns 是列定义；dataType 为 Rust 数据类型名称。
- meta.indexes 是该表二级索引数量，不包含主键本身。
- rows 是存储引擎返回的当前记录快照，不是页面原始字节的可视化。

## POST /api/query

执行一条 SQL。请求体必须是 JSON，最大 64 KiB：

    { "sql": "SELECT * FROM users WHERE id = 1" }

成功响应：

    {
      "ok": true,
      "sql": "SELECT * FROM users WHERE id = 1",
      "durationMicros": 184,
      "result": {
        "columns": [],
        "rows": [[1]],
        "affectedRows": 0,
        "lastInsertId": null
      },
      "state": { "ok": true, "tables": [] }
    }

失败响应仍返回 HTTP 200，但 ok 为 false，并包含 error：

    {
      "ok": false,
      "sql": "SELECT missing FROM users",
      "durationMicros": 91,
      "error": "unknown column: missing",
      "state": { "ok": true, "tables": [] }
    }

durationMicros 是 Rust 执行器围绕 execute_sql 采集的微秒数，不包含 HTTP 传输和前端动画。

## POST /api/reset

清理演示会话：

1. 如果存在活动事务，先执行 ROLLBACK。
2. 删除当前数据库中的所有演示表。
3. 返回新的 /api/state 内容。

成功响应与空数据库状态相同：

    { "ok": true, "tables": [] }

工作台的“清空演示数据”按钮调用此接口，适合每次录制前建立可重复的初始状态。

## 错误与限制

- JSON 无法解析时返回 HTTP 400。
- 请求体超过 64 KiB 时返回 HTTP 413。
- 未知路径返回 HTTP 404。
- 服务默认只绑定回环地址；不要把演示 API 暴露到公网。
- 服务器使用单个共享 Session，因此接口面向单用户本地演示，不应作为通用 REST 数据库服务。

对应实现：crates/kv-server/src/demo_http.rs。

