# 提交前检查清单

这份清单用于课程大作业最终验收，不是实验报告正文。建议在提交前按顺序执行，并把结果保存到本地备查。

## 一、提交物

- [ ] 实验报告已导出为 PDF，不提交 Word 作为唯一报告文件。
- [ ] 视频时长不超过 3 分钟，画面和声音清晰。
- [ ] 源码仓库可以通过 README 的命令从干净环境启动。
- [ ] 如果提交 GitHub 链接，链接无需登录即可访问；否则准备源码压缩包。
- [ ] 提交包中包含 `README.md`、`docs/`、`Cargo.toml`、`Cargo.lock`、`demo-client/package-lock.json` 和源代码。
- [ ] 如果视频展示数据库文件状态，提交包中包含 `scripts/show-kv-db.sh`，并确认它只读运行。
- [ ] 提交前已运行 `bash scripts/run-and-show-tests.sh`，最终摘要为 `RESULT: ALL CHECKS PASSED`。
- [ ] 提交包中不包含 `target/`、`node_modules/`、`dist/`、`kv_data/` 或临时截图/录屏文件。

## 二、报告事实核对

报告中的下列内容必须与源码和视频一致：

- [ ] 项目名称、成员姓名、学号和实际分工已填写。
- [ ] 目标明确说明是 Rust 教学型关系数据库，并说明选择该题目的原因。
- [ ] 架构图能够对应 `kv-common`、`kv-storage`、`kv-txn`、`kv-sql`、`kv-network`、`kv-server` 六个 crate。
- [ ] 关键数据结构至少覆盖 4 KiB 页面、superblock、slotted page、B+Tree、缓冲池、catalog 和事务状态。
- [ ] 实验结果包含建表、插入、查询、JOIN、索引和事务回滚的真实结果或截图。
- [ ] 明确写出当前边界：没有 WAL、完整崩溃恢复、认证和完整 MySQL 兼容性。
- [ ] SQLite、PostgreSQL、MySQL InnoDB、BusTub 的来源、许可证、固定版本和差异均有说明。
- [ ] 没有把“借鉴设计思想”写成“复制源代码”，也没有把本项目描述为生产级数据库。

## 三、视频验收

完整操作顺序见[视频录制指南](video-recording-guide.md)，录完后逐项回看：

- [ ] 开头出现项目名、成员、学号和分工。
- [ ] 实际执行建表、批量插入、创建索引和 JOIN。
- [ ] 事务演示先显示更新后的值，再用 `ROLLBACK` 恢复。
- [ ] 画面出现 Web 工作台、后端真实耗时和测试通过结果。
- [ ] 源码对照镜头包含四个项目的固定版本、官方链接、许可证和本项目对应文件。
- [ ] 口头说明每个参考项目的共同点、差异和本项目的取舍。
- [ ] 片尾说明源码、测试和文档已整理；总时长控制在 2:45-2:55。

## 四、最终命令

在仓库根目录执行：

```powershell
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo doc --workspace --no-deps

cd demo-client
npm ci
npm run build
cd ..

python test_protocol.py
```

必须确认：

- [ ] fmt 无差异。
- [ ] Clippy 无 warning。
- [ ] Rust 测试全部通过。
- [ ] 前端生产构建成功。
- [ ] 协议脚本通过，并包含持久化重启测试。
- [ ] `git diff --check` 无空白错误。

## 五、提交前人工检查

- [ ] 全仓库搜索并删除个人路径、密码、令牌、无关截图和临时日志。
- [ ] `README.md` 中的端口、命令和功能与当前代码一致。
- [ ] 所有 Markdown 链接都能在仓库内找到目标文件。
- [ ] 视频中的姓名、学号、分工和 GitHub 地址已经替换占位符。
- [ ] 最终压缩包解压到新目录后，仍能按 README 启动。
