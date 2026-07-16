# 项目文档

`docs` 只保存与当前代码一致、需要长期维护的项目资料。

| 文档 | 内容 | 适用对象 |
| --- | --- | --- |
| [架构与设计依据](architecture.md) | 请求路径、磁盘布局、设计参考、实现边界与后续优先级 | 开发者、报告编写者 |
| [本地 HTTP API](api-reference.md) | 工作台接口、请求响应格式、限制和错误码 | 前端开发、演示复现 |
| [开发与测试指南](development.md) | 模块边界、验证命令、测试原则与生成文件规则 | 开发与验收 |
| [开源参考与差异](open-source-references.md) | 参考来源、借鉴方式、差异、改进与可复核代码位置 | 评分说明、报告编写者 |
| [源码对照录制卡](source-code-comparison.md) | 固定版本、原码短摘录、许可证、对应实现和口头台词 | 视频、答辩 |
| [视频录制指南](video-recording-guide.md) | 三分钟时间轴、台词、操作、录制准备和故障预案 | 项目展示者 |
| [提交前检查清单](submission-checklist.md) | 源码、报告、视频、引用和最终压缩包的逐项验收 | 提交前 |

视频辅助脚本位于仓库根目录的 `scripts/`：

- `show-kv-db.sh`：只读展示自定义 `kv.db` 的 superblock 和页统计。
- `run-and-show-tests.sh`：执行质量门禁并输出适合录制的测试摘要。

两个脚本都不是数据库修复工具；测试脚本会调用 `python test_protocol.py` 完成协议和持久化验证。

## 维护约定

- 功能或文件格式发生变化时，同步更新 `architecture.md` 和 README 的功能范围。
- 构建、测试或运行方式发生变化时，同步更新 `development.md` 和 README 的命令。
- HTTP 路径、请求字段或响应格式发生变化时，同步更新 `api-reference.md` 和前端调用。
- 开源版本、许可证或对照代码位置发生变化时，同步更新 `open-source-references.md` 和 `source-code-comparison.md`。
- 已失效的计划稿、临时截图和工具生成资料不在此目录长期保留。
- 项目入口、安装和运行方式以仓库根目录的 [README](../README.md) 为准。
