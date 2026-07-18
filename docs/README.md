# KV Database 文档

本目录面向使用者、评审者和后续维护者，保留项目运行、架构、测试和开源参考相关文档。

## 文档索引

- [快速上手](getting-started.md)：环境要求、启动服务、运行 Web 工作台、执行 SQL。
- [系统架构](architecture.md)：workspace 模块划分、请求链路、HTTP API 和存储层边界。
- [测试与验证](testing.md)：本地质量门禁、端到端协议测试、数据库文件检查脚本。
- [开源参考说明](open-source-references.md)：参考来源、固定版本、许可证、借鉴点与本项目差异。

## 项目边界

KV Database 是课程实践规模的教学型关系数据库系统原型。当前版本覆盖 SQL 执行、事务基础、B+Tree、持久化、MySQL 协议入口和 Web 工作台；WAL、崩溃原子恢复、认证、权限控制、行级锁和完整 MySQL 兼容性属于后续扩展方向。
