# 3 分钟项目视频录制指南

本指南只用于录制视频，不是实验报告。成片建议控制在 **2 分 45 秒至 2 分 55 秒**，给平台
转码和片尾留出余量，绝对不要超过 3 分钟。

## 一、录制前必须填写

把下面信息写在一张简洁的标题页或 README 临时演示副本中，录制时不要口头说“稍后补充”。

- 项目名称：`KV Database - Rust 教学型关系数据库`
- 成员 1：`【姓名】 【学号】`，负责：`【实际负责模块】`
- 成员 2：`【姓名】 【学号】`，负责：`【实际负责模块】`
- 其他成员：没有则删除，不要保留占位符
- GitHub：`【仓库 URL；未公开则不要展示这一行】`

分工必须与提交记录和代码实际情况一致。建议按 `存储/事务`、`SQL/协议`、`Web/测试/文档`
描述，不要只写“前端”“后端”。

## 二、录制环境

### 画面设置

- 分辨率使用 1920x1080，帧率 30 FPS，浏览器缩放 90% 或 100%。
- 只录应用窗口或指定区域，关闭微信、QQ、邮件和系统通知。
- VS Code 字号调到 18-20，终端字号至少 16，隐藏无关侧栏和个人路径。
- 浏览器只保留工作台标签页，不展示收藏夹、账号头像或隐私信息。
- 麦克风先录 10 秒试听，确保没有爆音、键盘声和持续底噪。
- 提前打开 [源码对照录制卡](source-code-comparison.md) 中的 8 个固定标签页；不要现场搜索仓库。

### 启动干净的演示环境

打开两个 PowerShell 终端。第一个终端：

```powershell
$env:KV_DATA_DIR="target/video-demo"
cargo run -p kv-server
```

第二个终端：

```powershell
cd demo-client
npm run dev
```

打开 `http://127.0.0.1:5173`。正式录制前点击工具栏的垃圾桶按钮并确认，确保左上角显示
`0 数据表 / 0 记录 / 0 字段`。不要删除或替换个人的 `kv_data/kv.db`。

### 提前准备测试证据

在录制前运行一次：

```powershell
cargo fmt --check --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
python test_protocol.py
```

确认最后结果是 Rust 全量测试通过、Clippy 无警告、协议套件 `87 passed`。录制时可以展示已完成
的终端结果，不要在三分钟内等待重新编译。

## 三、提前放入剪贴板的 SQL

按顺序放进剪贴板历史（Windows 可用 `Win+V`），避免现场输入出错。

### SQL A：建表、数据和索引

```sql
CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(100), age INT);
INSERT INTO users VALUES (1, 'Ada', 28), (2, 'Grace', 31), (3, 'Linus', 25);
CREATE INDEX idx_users_name ON users (name);
CREATE TABLE orders (id INT PRIMARY KEY, uid INT, amount FLOAT);
INSERT INTO orders VALUES (101, 1, 299.0), (102, 2, 499.0), (103, 1, 129.0);
```

预期：左侧显示 2 张表、6 条记录；`users` schema 显示 `索引 1`。

### SQL B：JOIN 查询

```sql
SELECT * FROM users JOIN orders ON users.id = orders.uid;
```

预期：返回 3 行关联结果，右上角显示后端真实执行耗时。

### SQL C：事务内读己之写

```sql
BEGIN;
UPDATE users SET age = 99 WHERE id = 1;
SELECT * FROM users WHERE id = 1;
```

预期：查询结果中 Ada 的年龄为 99。

### SQL D：回滚验证

```sql
ROLLBACK;
SELECT * FROM users WHERE id = 1;
```

预期：Ada 的年龄恢复为 28。

## 四、成片时间轴

以下台词不需要逐字背诵，但每一项事实都应说到。正常语速约每分钟 220-260 个汉字。

| 时间 | 画面与操作 | 建议讲述 | 对应评分点 |
| --- | --- | --- | --- |
| 0:00-0:18 | 工作台全景；标题页可覆盖左下角，显示项目名、成员、学号和分工 | “大家好，我们是【成员信息】。项目是 KV Database，一个用 Rust 从零实现的教学型关系数据库。我们的目标不是封装现有数据库，而是打通 SQL、事务、B+Tree 磁盘存储和 MySQL 协议的完整链路。” | 背景目标、团队分工、非照搬 |
| 0:18-0:38 | 切到 VS Code，展示 `docs/architecture.md` 顶部请求路径，再快速展开六个 crate | “系统按职责拆成六个 crate：common 定义 trait 和类型，SQL 层完成词法、语法、计划和执行，txn 管理事务、MVCC 与锁，storage 实现 4KB 页面、缓冲池和 B+Tree，network 与 server 提供 MySQL 和 Web 双入口。” | 架构清晰、Rust 工程结构 |
| 0:38-1:08 | 回到工作台，粘贴 SQL A，点击执行；执行时指向六阶段链路，完成后点击 users/orders 标签 | “这里一次执行建表、批量插入和二级索引。请求真实经过 Lexer、Parser、Planner、事务层和 B+Tree。右侧可以直接检查 schema、主键、索引数量和当前记录，数据不是前端 mock，而是来自 Rust 服务的状态快照。” | 主体功能、索引、可观察性 |
| 1:08-1:28 | 粘贴 SQL B 执行，停留在三行结果和耗时 | “查询层支持条件、投影、排序和等值 JOIN。这个例子把 users 与 orders 关联，结果由 MySQL 风格类型编码返回；右上角的微秒或毫秒数是后端执行器实测，不包含页面动画。” | 查询能力、真实结果、性能证据 |
| 1:28-1:52 | 粘贴 SQL C，执行后指向 Ada=99；随后粘贴 SQL D，执行后指向 Ada=28 | “事务中更新后的 99 对当前会话立即可见，说明实现了读己之写；执行 ROLLBACK 后恢复为 28。底层由事务状态机、写缓冲、MVCC 可见性和表级锁协作，而不是在前端撤销。” | 事务核心功能、正确性演示 |
| 1:52-2:08 | 切到测试终端，先框出 Rust 测试通过，再框出 `87 passed` | “除了界面演示，我们还有 crate 单元测试、集成测试和 87 项真实 TCP 协议测试，覆盖 DDL、DML、JOIN、事务、错误处理，以及关闭服务后重新打开数据库的持久化恢复；Clippy 以 warnings as errors 通过。” | 功能完善、代码规范、测试充分 |
| 2:08-2:40 | 按源码对照录制卡快速切换四组标签：PostgreSQL page header、SQLite free-list、InnoDB page constants、BusTub buffer API；每组随后切到本项目对应文件 | “这里是四个固定版本的源码对照。PostgreSQL 用 page header 和 line pointer 组织槽页，我们用更小的 free_start/free_end；SQLite 先检查释放页范围，我们实现了 superblock 加空闲页链；InnoDB 的页面头包含目录、层级和索引身份，我们只保留教学 B+Tree 必需字段；BusTub 暴露 New/Delete/Read/Write/Flush，我们用 Rust Pager trait 和写穿缓存完成同样边界。所有摘录都保留官方许可证和链接，没有复制上游文件。” | 开源引用、区别、特色、改进、许可证 |
| 2:40-2:55 | 回到工作台全景，停在表状态与存储记录 | “最终项目形成了可连接、可持久化、可测试、可视化的 Rust 数据库最小闭环。源码、设计依据和复现命令都已整理在仓库中。谢谢观看。” | 总结完整、画面收束 |

## 五、必须拍到的 A 档证据

录完后逐项回看，少任何一项都建议重录：

- [ ] 前 18 秒出现所有成员姓名、学号和真实分工。
- [ ] 说清楚“自行实现”以及项目目标，不只展示页面。
- [ ] 画面出现六个 Rust crate 或架构请求路径。
- [ ] 实际执行建表、插入、二级索引和 JOIN，不使用静态截图冒充。
- [ ] 事务画面先出现 99，再通过 `ROLLBACK` 恢复为 28。
- [ ] 出现真实后端耗时，口头说明不包含动画时间。
- [ ] 出现 Rust 测试/Clippy 和 `87 passed` 证据。
- [ ] 四个开源项目都出现固定版本、源码短摘录、许可证、官方 URL 和本项目对应文件。
- [ ] 明确说出每个开源项目的共同点、差异和本项目的一个改进/取舍。
- [ ] 说清楚“只引用短摘录，不复制源码”，并在画面中保留许可证信息。
- [ ] 主动说明没有 WAL 等边界，避免被追问时显得夸大。
- [ ] 总时长不超过 3:00，声音清楚，所有终端文字可读。

## 六、表达禁区

- 不要说“完全实现 MySQL”“生产级数据库”或“完整 ACID”。当前没有 WAL 和崩溃原子恢复。
- 不要把工作台中的“存储记录”称作真实 B+Tree 页面可视化；它展示的是存储引擎返回的记录快照。
- 不要说二级索引已具备 InnoDB 的全部能力；非唯一键、增量维护和重启恢复仍有限制。
- 不要花时间逐行读代码。视频评分看核心功能、亮点、逻辑和开源差异。
- 不要展示长时间安装、编译、滚动日志或输入错误的过程。
- 不要把 GPLv2 的 InnoDB 源码复制到仓库或视频素材包；只展示官方链接中的短摘录。

## 七、故障预案

### 页面显示服务离线

检查后端终端是否出现 `Demo HTTP API listening on http://127.0.0.1:8080`，再点击刷新按钮。

### 提示表已经存在

点击垃圾桶按钮清空演示数据；仍失败时停止后端，删除 `target/video-demo` 后重新启动。不要删除
`kv_data`。

### 端口被占用

```powershell
Get-NetTCPConnection -LocalPort 3307,8080,5173 -State Listen
```

关闭之前启动的项目进程后重试。录制当天不要临时修改端口和 Vite 代理。

### SQL 执行失败

不要现场修 SQL。直接停止录制，清空演示数据，重新使用本指南提供的四段 SQL。

### 成片超过三分钟

优先删掉页面等待和鼠标移动，不要删团队信息、事务结果、测试证据或开源差异。目标成片控制在
2:50 左右。
