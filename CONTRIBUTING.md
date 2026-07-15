# 贡献指南

## 开发流程

1. 从 `main` 创建短生命周期分支。
2. 修改前先确认模块边界，公共接口变更必须同步所有实现。
3. 功能改动必须包含测试；磁盘格式改动必须包含重新打开文件后的恢复测试。
4. 提交前运行完整质量门禁。

```powershell
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cd demo-client
npm ci
npm run build
```

协议、事务或持久化行为发生变化时，还需运行：

```powershell
python test_protocol.py
```

## 提交要求

- 每个提交只处理一个明确问题，提交信息使用动词说明行为变化。
- 不提交 `target/`、`node_modules/`、`dist/`、本地数据库或 IDE 配置。
- 不在日志、测试数据或截图中包含密码、令牌、个人路径和学号等隐私信息。
- 公共 API、运行方式或限制变化时同步更新 `README.md` 和 `docs/`。

## 评审重点

- 正确性：错误路径、边界输入、重启恢复和事务可见性是否有测试。
- 兼容性：是否破坏已有磁盘格式、MySQL 协议或 Web API。
- 可维护性：命名、错误传播、模块依赖和文档是否与代码一致。
- 安全性：是否引入无限制分配、任意文件访问或对公网开放的未认证接口。
