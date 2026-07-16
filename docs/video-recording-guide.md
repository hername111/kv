# 3 分钟项目视频录制与提交指导

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
- 提前按本指南“四、开源源码对照”打开 8 个固定标签页；不要现场搜索仓库。

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

录制前先在仓库根目录运行一次完整检查。不要在成片中等待编译，把已经通过的终端结果作为证据展示：

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
git diff --check
```

命令的含义和录制时应框出的证据：

| 命令 | 检查内容 | 画面应出现 |
| --- | --- | --- |
| `cargo fmt --check --all` | Rust 格式 | 命令无输出并返回成功 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 代码规范和潜在问题 | `Finished` 且没有 warning |
| `cargo test --workspace --all-targets` | crate 单元测试、集成测试和协议层 Rust 测试 | 各测试组 `test result: ok` |
| `cargo doc --workspace --no-deps` | 公共 API 文档可生成 | `Finished` |
| `npm run build` | React/Vite 生产构建 | `built in ...` |
| `python test_protocol.py` | MySQL TCP、DDL/DML、JOIN、事务、索引、错误处理和重启持久化 | 最后一行 `87 passed` |
| `git diff --check` | 提交内容没有空白错误 | 无输出并返回成功 |

当前基线是 **74 个 Rust 测试**和 **87 项协议/持久化测试**。课程视频只需展示 Rust 测试的最后一组、
前端构建成功和 `87 passed` 三个局部画面；完整命令和结果保留在提交前检查记录中。

### 一键测试并展示结果

仓库还提供 [scripts/run-and-show-tests.ps1](../scripts/run-and-show-tests.ps1)，用于把上述检查压缩成
一个可录制的终端画面。它按顺序执行 Rust 格式、Clippy、Rust 测试、文档生成、前端构建、协议/持久化
测试和差异检查；每个阶段只显示最后几行，最后给出统一摘要。运行前先停止占用 3307 端口的演示服务，
因为 `test_protocol.py` 会自行启动和重启测试服务：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-and-show-tests.ps1
```

预期结尾：

```text
==================================
passed: 7
failed: 0
RESULT: ALL CHECKS PASSED
```

阶段失败时脚本会继续执行剩余检查，终端会显示 `FAIL (exit N)`，最后以非零退出码结束。录制时只
展示 `Rust tests`、`Frontend build`、`Protocol and persistence tests` 三个阶段和最后摘要；排查问题时
再查看脚本提示的临时日志路径。该脚本不会删除数据库或构建产物。

### 终端展示 kv.db 的真实状态

仓库提供原生 PowerShell 只读脚本 [scripts/show-kv-db.ps1](../scripts/show-kv-db.ps1)。它解析本项目自定义文件格式的
superblock，不把 `kv.db` 当作 SQLite 打开，也不会修改文件。脚本展示：文件大小、4096 字节页大小、
物理页数、`KVDBPAGE` 魔数、格式版本、next page id、free-list head 和 catalog root。

服务使用本指南的演示目录时，在仓库根目录的 PowerShell 中执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\show-kv-db.ps1 -DbPath target/video-demo/kv.db
```

如果使用默认目录：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\show-kv-db.ps1 -DbPath kv_data/kv.db
```

建议在 SQL A 执行完成后录一次，画面应类似：

```text
KV Database file inspection (read-only)
--------------------------------------
path:             target/video-demo/kv.db
file size:        20480 bytes
page size:        4096 bytes
page count:       5
superblock magic: KVDBPAGE
format version:   1
next page id:     5
free-list head:   0
catalog root:     1
```

实际页数会随数据量变化，不要照读示例数字。台词应说：“这是我们自己的 4 KiB 页面文件；page 0
保存 superblock，catalog root 指向表元数据 B+Tree，free-list head 用于复用已释放页面。脚本是只读
检查，不是把文件伪装成 SQLite。”如果脚本出现 `WARNING`，不要继续录制，先清空 `target/video-demo`
并重启服务。

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

## 四、开源项目对比：按代码说，不只报项目名

源码镜头统一遵循“上游 6-8 行 -> 本项目对应实现 -> 差异和改进”三步。固定版本、许可证、
permalink 和台词全部以本节为准。

| 来源 | 上游代码要指出的具体内容 | 本项目要指出的代码 | 准确的差异与改进台词 |
| --- | --- | --- | --- |
| PostgreSQL `REL_18_0` | `PageHeaderData` 中的 `pd_lower`、`pd_upper`、`pd_special` 和 `pd_linp` | `crates/kv-storage/src/page.rs` 的 `PageHeader { free_start, free_end }`、`SlotEntry` 和 `SlottedPage::validate_layout` | “两者都把 slot 目录和 tuple 数据分开管理。PostgreSQL 还保存 LSN、checksum、special space 和 line pointer；本项目只保留教学所需边界，并在 `validate_layout` 中拒绝 slot 越界。我们借鉴页面布局，不复制页面头。” |
| SQLite `version-3.50.4` | `src/btree.c` 的 `freePage2` 先检查 `iPage` 范围，再更新 page 1 的 free-page 计数 | `crates/kv-storage/src/pager.rs` 的 `DiskPager::free_page`、`open` 和 `free_list_head` | “SQLite 用 page 1 的 trunk/leaf freelist；本项目用每个空闲页前 8 字节串成单链表。共同点是先拒绝非法页号；本项目额外校验文件长度、魔数、版本和 freelist 环，并测试重启后的页面复用。” |
| MySQL InnoDB `mysql-8.4.0` | `page0types.h` 的 `PAGE_N_DIR_SLOTS`、`PAGE_N_RECS`、`PAGE_LEVEL`、`PAGE_INDEX_ID` | `crates/kv-storage/src/btree.rs` 的 `FLAG_INTERNAL`、`FLAG_LEAF`、键数量和叶节点 `next` | “InnoDB 页面头持久化目录槽数、记录数、层级和索引 ID，并配合聚簇索引和恢复机制；本项目只保留固定阶 B+Tree 必需字段，叶节点用 `next` 支持范围扫描，超出 4KB 或字段损坏直接返回错误。” |
| BusTub 固定 commit | `BufferPoolManager` 的 `NewPage`、`DeletePage`、`CheckedReadPage`、`CheckedWritePage`、`FlushPage` | `crates/kv-storage/src/buffer.rs` 的 `BufferPool`/`BufferedPager`，以及 `kv-common` 的 `Pager` trait | “BusTub 用 page guard、pin count、读写锁和 replacer 管理并发页；本项目用 Rust trait 拆出读写、分配、释放和刷盘，并用 Mutex 保护 LRU 缓存。释放页面时还会显式删除缓存，测试页号复用不会读到旧页。” |

不要说“我们的实现等同于 PostgreSQL/SQLite/InnoDB/BusTub”。应明确：参考的是数据结构和职责
边界；本项目的改进是格式校验、错误传播、缓存失效和可重复测试，生产数据库的 WAL、并发控制和
恢复能力不在本课程项目范围内。

### 四组固定源码镜头

录制前按下表依次打开“上游链接 + 本项目文件”，共 8 个标签页。上游画面只展示指定的 6-12 行，
不要下载或复制整个文件。

| 顺序 | 固定来源与许可证 | 上游画面 | 本项目画面 |
| --- | --- | --- | --- |
| 1 | [PostgreSQL `REL_18_0`, PostgreSQL License](https://github.com/postgres/postgres/blob/REL_18_0/src/include/storage/bufpage.h#L159-L174) | `PageHeaderData` 的 `pd_lower`、`pd_upper`、`pd_special`、`pd_linp` | `crates/kv-storage/src/page.rs` 的 `PageHeader`、`SlotEntry`、`validate_layout` |
| 2 | [SQLite `version-3.50.4`, Public Domain](https://github.com/sqlite/sqlite/blob/version-3.50.4/src/btree.c#L6769-L6795) | `freePage2` 的页号范围检查和 free-page 计数 | `crates/kv-storage/src/pager.rs` 的 `free_page`、`open`、`free_list_head` |
| 3 | [MySQL `mysql-8.4.0`, GPLv2](https://github.com/mysql/mysql-server/blob/mysql-8.4.0/storage/innobase/include/page0types.h#L53-L105) | `PAGE_N_DIR_SLOTS`、`PAGE_N_RECS`、`PAGE_LEVEL`、`PAGE_INDEX_ID` | `crates/kv-storage/src/btree.rs` 的节点标记、键数量和叶节点 `next` |
| 4 | [BusTub commit `f0d9e375...b48508`, MIT](https://github.com/cmu-db/bustub/blob/f0d9e3753482d45f2b5919da1873684600b48508/src/include/buffer/buffer_pool_manager.h#L110-L127) | `NewPage`、`DeletePage`、`CheckedReadPage`、`CheckedWritePage`、`FlushPage` | `crates/kv-storage/src/buffer.rs` 的 `BufferPool`/`BufferedPager` 和 `kv-common` 的 `Pager` trait |

每组画面按同一节奏：上游文件名、版本和许可证 2 秒，关键行 3 秒，本项目对应实现 3 秒。四组
都只说“共同问题、不同实现、增加的检查或限制”，不要逐行念代码。

## 五、成片时间轴

以下台词不需要逐字背诵，但每一项事实都应说到。正常语速约每分钟 220-260 个汉字。

| 时间 | 画面与操作 | 建议讲述 | 对应评分点 |
| --- | --- | --- | --- |
| 0:00-0:18 | 工作台全景；标题页可覆盖左下角，显示项目名、成员、学号和分工 | “大家好，我们是【成员信息】。项目是 KV Database，一个用 Rust 从零实现的教学型关系数据库。我们的目标不是封装现有数据库，而是打通 SQL、事务、B+Tree 磁盘存储和 MySQL 协议的完整链路。” | 背景目标、团队分工、非照搬 |
| 0:18-0:38 | 切到根 `README.md` 的“架构与接口”，再快速展开六个 crate | “系统按职责拆成六个 crate：common 定义 trait 和类型，SQL 层完成词法、语法、计划和执行，txn 管理事务、MVCC 与锁，storage 实现 4KB 页面、缓冲池和 B+Tree，network 与 server 提供 MySQL 和 Web 双入口。” | 架构清晰、Rust 工程结构 |
| 0:38-1:08 | 回到工作台，粘贴 SQL A，点击执行；执行时指向六阶段链路，完成后点击 users/orders 标签 | “这里一次执行建表、批量插入和二级索引。请求真实经过 Lexer、Parser、Planner、事务层和 B+Tree。右侧可以直接检查 schema、主键、索引数量和当前记录，数据不是前端 mock，而是来自 Rust 服务的状态快照。” | 主体功能、索引、可观察性 |
| 1:08-1:28 | 粘贴 SQL B 执行，停留在三行结果和耗时 | “查询层支持条件、投影、排序和等值 JOIN。这个例子把 users 与 orders 关联，结果由 MySQL 风格类型编码返回；右上角的微秒或毫秒数是后端执行器实测，不包含页面动画。” | 查询能力、真实结果、性能证据 |
| 1:28-1:52 | 粘贴 SQL C，执行后指向 Ada=99；随后粘贴 SQL D，执行后指向 Ada=28 | “事务中更新后的 99 对当前会话立即可见，说明实现了读己之写；执行 ROLLBACK 后恢复为 28。底层由事务状态机、写缓冲、MVCC 可见性和表级锁协作，而不是在前端撤销。” | 事务核心功能、正确性演示 |
| 1:52-2:08 | 切到测试终端，先框出 Rust 测试通过，再运行 `show-kv-db.ps1` 检查 `target/video-demo/kv.db`，最后框出 `87 passed` | “测试覆盖 crate 单元和集成逻辑；协议脚本再通过真实 TCP 验证 DDL、DML、事务、错误处理和重启持久化。这里是数据库文件本身：4096 字节页、page 0 的 `KVDBPAGE` superblock、catalog root 和 free-list head 都来自磁盘，而不是前端模拟。” | 功能完善、代码规范、持久化证据 |
| 2:08-2:40 | 按本指南“四组固定源码镜头”切换 PostgreSQL、SQLite、InnoDB、BusTub 及本项目对应文件 | “PostgreSQL 的 `pd_lower/pd_upper/pd_linp` 对应我们的 `free_start/free_end/SlotEntry`，但我们没有 LSN 和 checksum；SQLite 的 `freePage2` 先做页号检查，我们在 `DiskPager::free_page` 之外还检查 superblock 和 freelist 环；InnoDB 的 `PAGE_LEVEL/PAGE_INDEX_ID` 对应页面元数据，而我们用 `FLAG_INTERNAL/FLAG_LEAF` 和叶节点 `next` 保留 B+Tree 最小闭环；BusTub 的五个 buffer API 被我们拆成 Rust `Pager` trait，并补了释放页后的缓存失效测试。所有摘录都保留许可证和官方链接，没有复制上游文件。” | 开源引用、区别、特色、改进、许可证 |
| 2:40-2:55 | 回到工作台全景，停在表状态与存储记录 | “最终项目形成了可连接、可持久化、可测试、可视化的 Rust 数据库最小闭环。源码、设计依据和复现命令都已整理在仓库中。谢谢观看。” | 总结完整、画面收束 |

## 六、必须拍到的 A 档证据

录完后逐项回看，少任何一项都建议重录：

- [ ] 前 18 秒出现所有成员姓名、学号和真实分工。
- [ ] 说清楚“自行实现”以及项目目标，不只展示页面。
- [ ] 画面出现六个 Rust crate 或架构请求路径。
- [ ] 实际执行建表、插入、二级索引和 JOIN，不使用静态截图冒充。
- [ ] 事务画面先出现 99，再通过 `ROLLBACK` 恢复为 28。
- [ ] 出现真实后端耗时，口头说明不包含动画时间。
- [ ] 出现 Rust 测试/Clippy 和 `87 passed` 证据。
- [ ] 终端出现 `show-kv-db.ps1` 的只读检查结果，并能看到 `KVDBPAGE`、页大小和 catalog root。
- [ ] 四个开源项目都出现固定版本、源码短摘录、许可证、官方 URL 和本项目对应文件。
- [ ] 明确说出每个开源项目的具体字段、函数或 API，共同点、差异和本项目的一个改进/取舍。
- [ ] 说清楚“只引用短摘录，不复制源码”，并在画面中保留许可证信息。
- [ ] 主动说明没有 WAL 等边界，避免被追问时显得夸大。
- [ ] 总时长不超过 3:00，声音清楚，所有终端文字可读。

## 七、表达禁区

- 不要说“完全实现 MySQL”“生产级数据库”或“完整 ACID”。当前没有 WAL 和崩溃原子恢复。
- 不要把工作台中的“存储记录”称作真实 B+Tree 页面可视化；它展示的是存储引擎返回的记录快照。
- 不要说二级索引已具备 InnoDB 的全部能力；非唯一键、增量维护和重启恢复仍有限制。
- 不要花时间逐行读代码。视频评分看核心功能、亮点、逻辑和开源差异。
- 不要展示长时间安装、编译、滚动日志或输入错误的过程。
- 不要把 GPLv2 的 InnoDB 源码复制到仓库或视频素材包；只展示官方链接中的短摘录。

## 八、故障预案

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

## 九、最终提交检查

### 提交物

- [ ] 实验报告使用 `report/main.pdf`，不以 Word 作为唯一报告文件。
- [ ] 视频时长不超过 3 分钟，声音清楚，终端和代码可读。
- [ ] 源码可按根 `README.md` 从干净环境启动。
- [ ] 仓库包含 `README.md`、本指南、`Cargo.toml`、`Cargo.lock`、前端锁文件和源代码。
- [ ] 提交包不包含 `target/`、`node_modules/`、`dist/`、`kv_data/`、临时截图、录屏或日志。

### 报告与视频事实一致性

- [ ] 姓名、学号和真实分工已同时写入报告与视频标题页。
- [ ] 报告、视频和代码使用相同的测试数字、端口、模块名称和功能边界。
- [ ] 明确说明没有 WAL、完整崩溃恢复、认证和完整 MySQL 兼容性。
- [ ] 四个开源项目的固定版本、许可证、官方链接和差异均已展示。

### 最终命令

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
git diff --check
```

- [ ] Rust 格式、Clippy、测试和文档生成全部通过。
- [ ] 前端生产构建成功。
- [ ] 协议脚本显示 `Total: 87, 87 passed`，并包含重启持久化测试。
- [ ] 全仓库不存在密码、令牌、个人绝对路径、无关截图或临时日志。
- [ ] 最终压缩包解压到新目录后仍能按 README 启动。
