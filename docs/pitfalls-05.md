# 踩坑分卷5（4.206–4.264）

> 本卷为 `docs/pitfalls.md`（主索引）分卷之一，只收正文；查阅先看主索引，编号 `§4.N` 全仓唯一。

### 4.206 服务端流控/计时三面：pause 事件、dump 机制、headers 计时归属（2026-09-25，http 尾巴轮）

- 坑一（服务端无体背压）：req 缓冲超 HWM 不停读、socket 不发 'pause'——
  no-read-no-dump 套件（handler 借 'pause' 触发 res.end + 客户端才续发体）
  整链挂死。修法四件联动：①体泵 backpressured 旗（push 返 false）；②__feed
  停读 + sock.pause()；③net Socket pause/resume 发事件（转换沿守卫、异步，
  node emitPauseStreamEvent 口径）；④req._read 钩消费即解暂停。
- 坑二（res 完成清 framing）：__onDone 无条件 st.req/st.framing 双清——体
  在途时后续体字节被当新请求头解析（HPE_INVALID_METHOD → 断连）。修法：
  体未完保留 framing 到体完（node dump 口径按帧处置）。**已试并回退**：
  node _dump 机制（removeAllListeners('data') + resume() + 续推）——本仓流
  端口 flowing 排空节奏与 node 有差（push 在 flowing 态仍缓冲累积、返
  false），dump 后二次背压 → 二次 'pause' → 二次 res.end → write-after-end；
  msg=null 直通丢弃泵又致泵停摆体完不了。**结论：dump 机制需先修流端口
  flowing 排空语义（readable_flow push/flow），独立轮另做**；维持孤儿弃收
  （st.req=null）+ framing 存活口径，dump-req-when-res-ends 维持挂死定级。
  同场钉死：`_consuming` 必须在 `_read` 钩置位（node 258 行），放公共
  read() 会被流机构内部 read(0) 污染（resume_ → read(0)）。
- 坑三（计时归属三混）：①Host 校验在升级检测之前——node parserOnIncoming
  头部对 upgrade 请求 return 0，host 400 只属 pipeline 路径（Host-less
  GET+Upgrade 形不得 400）；②劫持（CONNECT/upgrade）不撤 request/headers
  计时——408 打进已劫持 socket；③headersTimeout 当空闲计时用——node 模型：
  开于连接建立（首消息未启，408 可先于首字节）+ 新消息首字节（残头未齐），
  请求完成即撤；空闲 keep-alive 归 keepAliveTimeout 专管（headers-timeout-
  keepalive 套件：空闲 1.5×headersTimeout 无 408、残头超时 408 照发）。
- 附：killed/中断后台任务后必 `pkill -f winterjs` 清孤儿再跑基线（§4.205
  pgrep 预警的动版）；grep -c 退出码 1（计数 0）会断 `&&` 链——构建检查用
  `|| true` 收尾。
- 坑四（背压两形态不可兼得的解法）：初版"停读+续喂"在两套件间必顾此失
  彼——no-read 要停读事件，flush-drain 要泵永不停（体一次性到齐形，残段/
  终结段扣 fr.buf 等再喂而包不会再有）。终解：**泵不停读不中断，事件改状
  态驱动**（缓冲 ≥HWM 发 'pause' 转换沿、落回 HWM 内发 'resume'，缓冲有界
  =体长）；chunked 泵背压 early-return 撤销（8c 版二分三段实锤元凶）。
  推广：流控事件与流停读是两个正交面，事件可状态驱动，停读必须回答"残段
  谁再喂"。
- 坑五（分离 HEAD 提交）：二分 checkout 后直接 commit 落在 detached HEAD
  （父=旧提交，缺后续修复）——cherry-pick 回 master 解。推广：bisect/checkout
  后先 `git status` 看 HEAD 归属，提交前必 `git log --oneline -1` 核父。
- 本轮战果：no-read-no-dump / server-request-timeout-upgrade / server-
  headers-timeout-keepalive / outgoing-flush-drain / upgrade-large-body-unread
  五件转绿 + should-keep-alive / outgoing-message-capture-rejection 两件
  （见各自提交）；黑盒四件新增，node 域 288 全绿（fifo 按 §4.175 剔除），
  冒烟 5/5，sweep8 终局 379/16/8（五件零红）。
### 4.207 `TMPDIR` 放 U+F8FF 卷即黑盒假红：`URL.pathname` 是百分号编码（2026-09-25，D1 轮）

- 症状：`TMPDIR=/Volumes//wjs-data/tmp cargo test --test node child::` 挂 2 件
  （`envself/fileself false`、`fork-exit false`），默认 `TMPDIR` 全绿。
- 根因：外置盘卷名含 U+F8FF；探针用 `new URL("x.mjs", import.meta.url).pathname`
  取路径——pathname 是百分号编码（`/Volumes/%EF%A3%BF/...`），文件找不到。
  软链 `~/wjs-data` 也躲不过（入口 URL 经 canonicalize，§4.12）。node 同款语义，非实现 bug。
- 修法：`cargo test` 维持系统默认 `TMPDIR`（临时文件小且即删）；只有 node 套件检出 /
  sweep 工件 / 探针等大数据放外置盘（plan3 §0.5 已改）。
- 推广铁律：换数据盘/临时目录前先跑一次含 `import.meta.url` 的黑盒域；路径含非 ASCII
  时 URL→路径一律 `fileURLToPath`。
### 4.208 require 把用户异常转串重抛：位置恒 prelude 424:53、NodeError 空文案、原对象丢失（2026-09-25）

- 症状：`--run x.js`（node 套件几乎全是 CJS）任何未捕获错误都报 `Error: x.js:424:53: …`
  （行列是 prelude `__wjs_require_main` 的调用点）；NodeError 报 `Error: x.js:424:53: `
  空文案；`try { require('./m') } catch (e)` 拿到的是新 `Error`（丢类/code/stack）。
- 根因：`require_cjs_file` 用户代码失败时 `pending_message` 消费异常取文案，native 入口
  `report_error` 新抛一个 Error——异常栈变成重抛点；NodeError 的 message 是 `super()` 后
  defineProperty 的属性，引擎报告里的 message 槽为空。另：CJS 包装头占第 1 行，栈行号恒 +1。
- 修法：用户代码异常留 pending（`KEEP_PENDING` 哨兵 → native 直接 `return false`，吞错
  调用方显式 `take_pending_exception` 清场）；包装编译起始行 0（行号 = 物理行，mapper 去 +1）；
  报错点 `jsapi_glue::fill_message` 从 `message` 属性回填；入口报错抛点不在入口文件时如实
  报其文件名。`__wjs_cjs_compile` 链顺带 §4.80 rooting 修复（cur 裸 JSVal 跨调用）。
- 复现：`tests/node/require.rs::phase11_require_rethrows_original_exception`。
- 推广铁律：宿主转发用户异常一律"留 pending 原样透传"，禁转串再抛；需要文案时读属性
  而非只信引擎报告槽。
### 4.209 node 旗前缀放行 = 自 spawn 无限递归，fork 链吃光内核致系统 panic（2026-09-25，D1 回归）

- 症状：全域基线 sweep（3 并发）跑到 70% 时整机卡死、WindowServer 看门狗超时，
  随后内核 panic 重启（`userspace watchdog timeout: no successful checkins from configd`）。
  panic 快照：323 个 `winterjs` 进程，其中 321 个是**单链**（每个父进程只等一个子进程），
  链根 12:50:12 起、ppid=1（孤儿）；内核 zone `VM map entries 10G`，winterjs 常驻合计 7.4GB。
- 根因：D1 把 `--unhandled-rejections=` 等**前缀族**当 node 旗放行剥除。
  `test-promise-unhandled-flag.js` 用 `spawnSync(execPath, ['--unhandled-rejections=foobar', __filename])`
  验证非法值启动即拒（node exit 9）；本仓剥除后照跑**同一文件**——该文件再 spawnSync 自己，
  无限递归。sweep 超时只杀直接子进程，孙辈成孤儿继续繁殖约 20 分钟，直到内核 VM 耗尽。
- 修法（三道防线）：① 旗名单改**精确表**（`src/cli_node_flags.rs`，取自 `node --help`，剔除
  改执行模式的旗与 winterjs 自有同名旗如 `--allow-ffi`），必须带值的旗只认 `--k=v`，值非法即
  node 同款 exit 9；② 自 spawn 深度闸：起自身时子进程环境 `WINTERJS_SPAWN_DEPTH` 逐层 +1，
  超 32 即拒（变量对 JS 的 `process.env` 不可见）；③ sweep 每件独立进程组，超时/收尾 `killpg`
  连孙辈一起收，daemon 设 `RLIMIT_NPROC = 现有进程数 + 200`。
- 复现：`tests/cli.rs::self_spawn_depth_guard_stops_recursion`（链止于 33 层）+
  `node_runtime_flags_self_spawn_forms`（非法值 exit 9）；原套件现 rc=0。
- 推广铁律：**会让"同一文件再跑一次"的兼容翻译，必须同时回答"非法输入时会不会自递归"**；
  跑任何会 spawn 自身的批量任务前，先给进程数封顶（`ulimit -u` / `RLIMIT_NPROC`），超时一律杀进程组。
### 4.210 rustls/webpki 拒收 X.509 v1 证书：服务端绕 keys_match，客户端验签兜底（2026-09-25，P1）

- 症状：node 套件的 `agent*-cert.pem`（v1）在 `tls/https/http2.createServer` 即报
  `bad key/cert pair (… UnsupportedCertVersion)`；客户端 `ca:` 校验同码失败。基线 37 件同因。
- 根因：webpki 只解析 v3；`ServerConfig::with_single_cert` 经 `CertifiedKey::from_der` 做
  keys_match 时解析终端证书即拒。OpenSSL 照收 v1。
- 修法：服务端 provider 严格加载私钥后以固定解析器直出（`FixedCert`，不做 v1 解析）；
  客户端 `ca:` 路径包一层 `V1FallbackVerifier`——标准 WebPki 报 `UnsupportedCertVersion`
  时手工校验（issuer Name 全等 + 复用 `x509_verify_impl` 验签 + 有效期 + CN 主机名），
  TLS 1.3 握手签名经 SPKI 走 `verify_tls13_signature_with_raw_key`（1.2 无原始公钥变体，记档）。
- 复现：`tests/node/tls.rs::phase11_tls_x509_v1_certificates`（fixtures 用 LibreSSL
  `/usr/bin/openssl x509 -req` 生成——OpenSSL 3 缺省出 v3）。
- 推广铁律：轮子"拒收合法旧格式"时，先确认拒收点是**解析**还是**校验**；解析拒收可在
  出示侧绕开，校验侧兜底必须保住签名/有效期/主机名三件，不许退化成空校验。
### 4.211 修一个结算点会放出一串假绿：beforeExit 缺失 + process.emit 吞错 + 致命错不发 exit（2026-09-26，P2）

- 症状：base13 较 base12 净 −75——`exit` 事件修好后 mustCall 核对首次生效，143 件旧假绿翻红；
  其中 `beforeExit` 监听 9 件整簇不触发、`beforeExit` 抛错被静默吞掉、未捕获异常后 `exit` 监听不跑。
- 根因：① 事件循环排空即退，从未派发 `beforeExit`；② 公开 `process.emit` 与宿主内部派发
  共用吞错的 `__wjs_emit`，监听抛错全被 `catch {}`；③ 致命错直接 `?` 上抛到 main 渲染，
  node `triggerUncaughtException` 尾段（exitCode=1 → 派发 exit → 按 exitCode 退出）缺失。
- 修法：idle 点投递 `beforeExit`（经 nextTick，抛错走 uncaughtException/fatal 同一路由；
  每次排空只发一次，监听排新任务才复位）；公开 `emit` 改 EventEmitter 口径（抛错上抛、
  无监听 `'error'` 即抛），内部派发仍走 `__wjs_emit`；runner 各致命出口经 `fatal_exit`：
  先就地渲染错误（main 登记配色后才启用，testrun/worker 原错透传）再发 exit、按 exitCode 退出。
- 复现：`tests/node/process_.rs::phase11_before_exit_and_fatal_exit_event`。
- 推广铁律：**基线数字下跌先分"假绿现形"与"真退化"**（按红因聚类：`MUSTCALL` 计数不符 vs
  `HANG` 挂死自报）；修结算点类 bug 后必须全量重跑，旧绿数不可信。宿主内部派发与用户可见
  `emit` 必须分开，前者可吞错，后者照 node 上抛。
### 4.212 "同步底座 + 特判"的流实现一改就碎：fs 流改逐字移植，底座时序补两处（fs 回调微任务、setImmediate 钳 1ms）（2026-09-26，P2）

- 症状：fs 流 18/39 件红（mustCall 簇）：mock `fs.read/fs.close`、`options.fs` 自定义、
  `ReadStream.prototype.open` 补丁、destroy(err) 的 error/close 顺序全不对。旧实现是"快照读 +
  live-follow + 续命计数"的特判堆（933 行里 500 行），每条补一个套件。
- 根因：流不经 `this[kFs]` 调 fs（用户 mock 不可见），自带 open/读/关生命周期而非 node 的
  `_construct/_read/_destroy` + kIsPerformingIO/kIoDone；为弥补同步底座又加了续命计数器。
- 修法：`lib/internal/fs/streams.js` 逐字移植（kFs 缺省 = `node:fs` 默认导出对象），删续命
  natives。移植后暴露两处底座时序偏差并修正：① fs 回调 API 在微任务里完成——整条读链在一个
  检查点内跑到 EOF，定时器插不进（node 在后续轮次 poll 相送达）→ 改 `setImmediate` 派发；
  ② `setImmediate` 走 setTimeout 的 1ms 钳——每个 immediate ≥1ms，链式读被 1ms 写端甩开 →
  delay 恰 0 的非 interval 定时器不钳（`fire_due` 快照到期集，回调内新排的顺延一轮 = check 相）。
- 复现：`test-fs-read-stream-pos.js`（写端 1ms 追加，读端跟随）；`tests/builtins.rs` immediate 吞吐用例。
- 推广铁律：**有 node 原文的模块，优先逐字移植 + 修底座，而不是在自写实现上逐套件打补丁**；
  移植后新红多半是底座时序（微任务 vs 宏任务、定时器钳制）偏差，按 node 事件循环相位去对。

### 4.213 io_code 漏 ECONNRESET：拆链收尾错全落 UNKNOWN（2026-09-26，P2-tls-b）

- **症状**：wrap-econnreset 断言 `e.code === 'ECONNRESET'` 拿到 `UNKNOWN`；net 错误形状
  （errno -54、`connect ECONNRESET <target>`）全错。
- **根因**：`fs::io_code` 按 errno 原值映射，只录了常见文件/连接错误——macOS ECONNRESET=54、
  Linux=104 未录（ENOTCONN 57/107 同漏）。RST 类错误是 net/tls 收尾常态，不是边缘。
- **修法**：补 `54 | 104 => "ECONNRESET"`、`57 | 107 => "ENOTCONN"`。
- **复现**：net server `c.end()` + 客户端半关读，读端报错落 UNKNOWN。
- **铁律**：新增 io 错误映射必须**双侧平台 errno**（macOS/Linux）成对录入；新增网络
  断言面（e.code/errno）前先 grep io_code 是否覆盖目标错误。

### 4.214 listen 字符串参误判 UDS path：address().port 全 undefined（2026-09-26，P2-tls-b）

- **症状**：`tls.Server.listen(0, "127.0.0.1")` 后 `address().port === undefined`，全量
  tls/https 黑盒与套件 listen 形崩（连接拨到 `localhost:NaN`）。
- **根因**：给 listen 加 UDS path 支持时，把"非纯数字串"一律当 path（`udsPath = a`）——
  字符串 host（`"127.0.0.1"`、`"localhost"`）被误判；`__udsPath` 残留使 address() 回串。
- **修法**：node isPipeName 口径——**含 "/" 的串才是 path**，否则按 host；
  `address()` 的 UDS 分支只在 `listen(path)` 显式给出时激活。
- **复现**：`server.listen(0, "127.0.0.1", cb); server.address().port` → undefined。
- **铁律**：JS 层多形态参数判别（host vs path vs port）必须引 node `isPipeName` 原文
  判据，禁用"看起来像不像"的宽松启发；改动后须即跑既有黑盒（同域）再继续。

### 4.215 rustls 对 FIN-无-close_notify 严格报错，node/OpenSSL 视为干净 EOF（2026-09-26，P2-tls-b）

- **症状**：对端裸 destroy（RST/FIN 无 TLS close_notify）后，读端泵报
  `Error [UNKNOWN]: peer closed connection without sending TLS close_notify`，
  无监听即崩（test-tls-socket-close / -on-empty）。
- **根因**：tokio-rustls 读端把 bare FIN 映射为 `UnexpectedEof`（io_code 不识 →
  UNKNOWN）；node/OpenSSL 同场景按 TCP EOF 处理——'end' 无 error。
- **修法**：`TlsCleanEof` 读端适配器（tls.rs）——`UnexpectedEof` 且消息含
  `close_notify` → 映射 `Ok(0)`（干净 EOF）；三处 spawn_pumps 读端统一包。
- **复现**：net server `c.end()` 后客户端继续读 TLS 流。
- **铁律**：rustls 严格性与 OpenSSL/node 的宽容语义相悖处（close_notify、X.509 v1、
  hostname 大小写）必须逐一适配层收敛，禁让引擎差异漏到可观察面。

### 4.216 watch 过滤复用 test 表：serve 改 html/css 不触发重启（2026-09-27，CLI --watch 轮）

- 症状：`--serve pub --watch` 跑起后改 `index.html`，无 `restarting` 行、同端口一直回旧内容；
  改 `app.js` 才触发重启。
- 根因：`--serve --watch` 直接复用了 test watch 的 `watchable`（只认代码/JSON 后缀）——
  静态 serve 的被监视物恰恰是 html/css/图等非代码资源，过滤表与监视目标错配。
- 修法：`watch.rs` 拆两层——`ignored()`（node_modules/.git/target + 点文件，三处共用）+
  `watchable()`（test/run 用，后缀表不变）/ `watch_any()`（serve 用，只去噪音不卡后缀）；
  `watch_with(roots, filter)` 可注入，serve 传 `watch_any`。
- 复现：`tests/cli.rs::serve_watch_restarts_child_on_static_change`（改 html 断同端口新内容）。
- 推广铁律：**监视过滤表必须按"被监视物的语言"选，不按"已有表的语言"复用**；新增 watch
  调用点先问"目标目录里什么文件会变"，再定过滤函数。

### 4.217 按调用编译 Regex::new 是启动慢放：CJS 发现 8 正则现场编译烧 1.7s（2026-09-27，F2 轮）

- 症状：vite build-only（179 模块）`cjs_static_names` 累计 1688ms/48 调用（35ms/次），
  `nearest_pkg_type` 45ms/650 调用（逐级读 `package.json` + JSON 解析）；release 同构
  仍是除 prepare/compile 外最大单项（cjs-names 100ms）。
- 根因：① `Regex::new` 在函数内每次调用编译 8 个 pattern，递归每层再编
  （debug 放大，release 亦然）；② `nearest_pkg_type` 无缓存，每文件每轮都 walk。
  XDR 字节码同轮证伪：sm-compile 上限仅 118ms（release）且需新 unsafe，不如修这里。
- 修法：8 正则 `OnceLock` 进程级预编译（`precompiled!`）；pkgtype 按 `package.json`
  路径缓存（mtime 失效，Miss 也缓存 + 存在性校验，watch 安全，原语义逐字保留）；
  `require.rs` 990→1062 行超限，按算法族拆 `require_cjs.rs`（原位重导出）。
  memchr 锚点预检试过——大文件锚点全中、慢文件逐项耗时分毫不差，总量差落噪声带，
  整段回退（不留投机优化）。
- 复现：探针 `WINTERJS_TIMING=1 … --run build-only`（临时，已删；计数见 F2 commit）；
  单测 `nearest_pkg_type_table`（改包文件即验 mtime 失效）；vite dist 哈希断行为一致。
- 推广铁律：**热路径禁现场 `Regex::new`（一律进程级预编译）；纯 fs 判定函数配 mtime
  目录缓存；投机优化必须有同负载前后计数，无 measurable 差即回退**。

### 4.218 concat ESM 具名导出≠默认导出：新类只挂具名即用户面 undefined（2026-09-27，P2-crypto）

- 症状：`test-crypto-classes.js` 报 `invalid 'instanceof' operand crypto[clazz]`、
  `test-crypto-sign-verify.js` 报 `Sign is not a function`——`typeof crypto.Sign`
  实测 `undefined`，而 `Hash/Cipheriv` 正常。
- 根因：本仓 `node:crypto` 由 9 片 JS `concat!` 成同一模块——`crypto_sign.js` 的
  `class Sign` 只存在于模块作用域，`export { Sign }` 挂在 `crypto_pqx509.js`，
  但用户面 `require("crypto")` 拿到的是 `__api` **默认导出表**，该表漏了 `Sign/Verify`
  两门。具名导出绿了，默认表没跟上。
- 修法：`__api` 补 `Sign, Verify`（一行）；`Sign/Verify` 同步改 legacy 函数形
  （无 new 直调，Cipheriv 同款，4.129 坑一再进宫）。附带 `verify-failure` 同根转绿。
- 复现：`tests/node/crypto/asym.rs::p2_crypto_sign_verify_nonew`；
  对拍三件 `getcipherinfo`/`classes`/`verify-failure` 转绿 0。
- 推广铁律：**concat 模块新增可导出的类/函数必须双挂（具名 export + `__api`
  默认表），落地前用真机 `TEST_CASES` 键表逐项 `typeof` 点名**，缺一即此症。

### 4.219 原型污染 setter 探针：native 内部写 JS 层 grep 不到即结构性偏离（2026-09-27，P2-crypto）

- 症状：`test-crypto-sign-verify.js:57` 在 `Object.prototype` 挂 `library` setter
  （写即抛），`createSign('sha1').sign(badPem)` 真机抛 `bye, bye, library`，
  我方抛 `Invalid PKCS#1 key`。
- 根因：真机探针（setter 内打栈）定位写入点在 `node:internal/crypto/sig:147`
  即 `this[kHandle].sign(...)` **native 调用内部**；`grep library internal/crypto/*`
  零命中——写发生在 C++ 层（OpenSSL provider 语境），JS 移植面无此概念，
  RustCrypto 底座亦无对应物，逐字复刻等于伪造实现细节。
- 修法：不修，记档（本条）；该套件后续另需 `Sign` 真流式（`s.end()`），与
  STREAM-PIPE 4 件并案记档 P2-stream。
- 复现：`node -e 'Object.defineProperty(Object.prototype,"library",{set(){...}}); …'`
  逐段二分（create/update 不触发、sign 触发）。
- 推广铁律：**"真机抛 X"先问"写 X 的主体在 JS 还是 native"**——setter 探针打栈，
  栈底落 native 且 JS 全仓 grep 不到同名写，即判结构性偏离（记档不追），
  禁在 JS 层硬塞 dummy 写去"骗过"断言。

### 4.220 自家抛错与真机逐字同形时禁"修正"，修上游误喂（2026-09-27，P2-repl）

- 症状：10 件 `test-repl-*` 报 `input.on is not a function`，第一反应是改自家
  `createInterface` 的抛错分支（改成 ERR_INVALID_ARG_TYPE 等"规范"码）。
- 根因：真机实测 `createInterface({})` 抛的正是无码 `TypeError: input.on is not a function`
  ——逐字同形。错不在抛错，在上游把坏 input 喂了进来（位置形吞参、options 直构、
  双缺未缺省 stdio 三形态）。
- 修法：抛错分支一字不动；修三处上游（Interface 构造器 options 归一、legacy 位置形、
  双缺 stdio）。复验 8 件自绿。
- 复现：任何"自家文案可疑"处，先跑真机同输入再动。
- 推广铁律：**改抛错文案/码前必须真机同输入对照；逐字同形即无罪，往调用链上游找**。

### 4.221 同流自回显即真机亦无限递归：repl 黑盒入出分离（2026-09-27，P2-repl）

- 症状：新黑盒 `p2_repl_legacy_positional` 用同一 PassThrough 既当 input 又当 output，
  输出回显又被当输入求值（每轮添引号），`cargo test` 挂死 150s+（4.140 家族）。
- 根因：输出写入同流即触发 data→求值→输出正反馈；真机同构亦然（recoverable 套件
  靠 noop-write 的 ArrayStream 避开）。
- 修法：黑盒入出分离（位置形 duplex 走 `{ stdin, stdout }` 映射）；判 hang 只认退出码，
  长驻探针输出落盘（4.45/4.93 重申）。
- 复现：`tests/node/repl.rs::p2_repl_legacy_positional` 初版（已改）。
- 推广铁律：**REPL/流黑盒入与出恒分离；同 duplex 必须 noop-write 或断言侧单向**。

### 4.222 补全分支劫持含引号成员行：成员→路径→等号段→拒答→bare（2026-09-27，P2-repl）

- 症状：`obj["one"].toFi` 一直回空，而无引号形正常；`nonExisting.f` 回出
  `nonExisting.fetch`（bare 穿透）。
- 根因：① 未闭合串分支的正则在成员行末引号处误命中（`["one"]` 的关引号被当
  开引号），劫持整行走 fs；② 成员求值失败后穿透到 bare，残段当前缀乱配全局键。
- 修法：分支重排——成员（成形求值失败即拒，不穿透）→路径→等号段→拒答集→bare；
  bare 仅无点行（有点即拒）。
- 复现：`test-repl-tab-complete-computed-props` 全红转全绿；`nonExisting.*` 回空。
- 推广铁律：**多分支派发按"结构确定性"降序排；失败分"解析不成"（另寻他路）与
  "求值不成"（即拒），后者禁穿透**。

### 4.223 allowBlockingCompletions 是 fs 补全面开关，无之回空（2026-09-27，P2-repl）

- 症状：本机探针 `complete('fs.readFileSync("../fixtures/x')` 回空，套件内同调用却有值。
- 根因：套件经 helper 传了 `allowBlockingCompletions: true`；无此旗真机 fs 面回
  `[[], null]`（defer）。另：既存目录如内列子项裸名且 completeOn 置空（反直觉，
  实测为准）。
- 修法：fs 分支镜像真机表（目录→子项裸名+空 completeOn；同级前缀过滤；坏径空）。
- 复现：`test-repl-tab-complete-files`。
- 推广铁律：**探针复刻套件必须连 helper 的透传 options 一起抄**（`startNewREPLServer`
  的 `terminal: true` + `allowBlockingCompletions: true` 皆是行为开关）。

### 4.224 TUI 行编辑替换三坑：管道分流/prompt 拼接/Display 单行（2026-09-28，REPL C 档）

- 症状：rustyline→reedline 后，① 管道黑盒（`tests/repl.rs`）行为漂移；
  ② 提示符打出 `❄ ❄>` 双份；③ 有栈错误仍只打一行。
- 根因：① 旧代码靠 `Editor::new` 失败才退化逐行，新底座 `create()` 常成功，
  不显式分流即把管道当 TTY；② reedline 渲染 `left + indicator` 拼接，
  两边各写一份 `❄> ` 即翻倍；③ `Error` 的 `Display` 是一行格式，
  node 形多行只在 `render()` 里（非 TTY 走 `render_script_node_style`）。
- 修法：`stdin().is_terminal()` 为 false 直接 stdin 逐行（无 ANSI，黑盒原断言不动）；
  `left="❄"` + `indicator="> "` 拼出 `❄> `；REPL 出错走
  `Error::script_with_kind(...).render(color)` 并丢弃退出码（只打印不退出）。
- 复现：`tests/repl.rs::repl_error_prints_stack`（`at f (repl.js:` 帧）；
  `printf ... | winterjs --repl` 管道对照。
- 推广铁律：**换行编辑底座必须三查**：非 TTY 显式 `is_terminal` 分流（禁依赖构造失败退化）、
  prompt 按"左+指示器"拼接规则拼（禁两边各写全形）、报错走 `render()` 不走 `Display`。

### 4.225 读行线程持 raw mode 时他线程直写终端：多行 LF 阶梯 + prompt 竞争（2026-09-27，REPL 渲染修复）

- 症状：TTY REPL 里 ① 打函数体/多行返回值时每行阶梯状右移；② miette 错误框
  碎片散布（框线行首错位、尾部悬空 `┌──`）；③ 报错文本贴在下一轮 prompt 后
  （`❄> winterjs::js::uncaught_exception`）。pty 抓字节证实：多行输出行尾裸 LF 无 CR。
- 根因：读行线程 `read_line` 阻塞时终端处 crossterm raw mode（OPOST/ONLCR 关），
  主循环（求值/console/错误渲染）此时直写终端——LF 只下移不回车即阶梯；且
  readline 线程 send 行后立即回环渲染下一轮 prompt（还发 DSR 光标查询），与慢
  一拍的求值输出竞争。
- 修法：两层。① **哨兵协议**：readline 线程发行后 drain 积压旧哨兵再
  `blocking_recv` 等"本轮输出完毕"哨兵，期间不进 `read_line`（raw 已退，ONLCR
  正常 + prompt 不再抢先）；主循环在行处理轮的 pump/rejection 收尾后发哨兵
  （`pending_flush` 旗，tick 轮不发）。② **CRLF 化**：REPL TTY 会话置
  `REPL_TTY_OUTPUT` 旗，用户输出（console emit）与 REPL 自有打印（`repl_out`/
  `print_completion`/`render_string`）统一裸 `\n`→`\r\n`——覆盖哨兵之后的
  异步窗口（timer 回调里 console.log 多行），ONLCR 开时多出的 `\r` 视觉无害。
  附带：SIGINT 置忽略（哨兵窗口 ISIG 开会直接杀进程，reedline 读时才捕获）。
- 复现：`~/wjs-data/probe/repl_pty.py`（pty 驱动 + DSR 应答，字节比对 CR）；
  黑盒 `tests/node/repl.rs::p2_repl_options_surface`（R4 测试同流 input/output
  踩 4.221 挂死，改分离流复验）。
- 推广铁律：**凡"独占线程持 raw mode 行编辑 + 他线程产输出"的 TUI，输出面
  必须过同步协议或 CRLF 化，禁裸直写终端**；判定用 pty 抓字节（数 CR），
  文本流比对看不出阶梯（LF 在管道渲染里天然对齐）。

### 4.226 全局 console 改原生为 JS 包装后补全文档回落 + trace 空消息冒号（2026-09-28，§7-②）

- 症状：`p2_repl_cli_complete_bridge` 的 `dot-empty` 由 true 翻 false——
  `console.trace` 文档摘要从手写表 `(...data) — stderr + stack` 变成通用
  `trace(...args)`；另 `console.trace()` 无参时自拼 `Trace: ` 与真机 `Trace`
  （无冒号）差一字符。
- 根因：① `__sigDesc` 先抽 `toString` 形参、非空即回落通用形，手写表只在
  形参为空（原生函数）时命中——trace/assert 由原生改 JS 闭包 `(...args)` 后
  命中分支改变，表中 `stderr` 文案永久不可达；② 本引擎
  `Error.captureStackTrace` 不合成 `Name: message` 首行（plain object 上
  stack 为空，见 §7-②探针），trace 首行须自拼，而 V8 空消息时省略 `: `。
- 修法：补全侧——新暴露的 7 个 console 方法进 `__wjsReplSig` 表（原生/空形参
  面仍命中），trace 的桥断言改为通用回落形 `startsWith("trace(")`（桥文档对
  JS 实现面本就按此规则）；trace 侧——`msg === "" ? "Trace" : \`Trace: ${msg}\``。
- 复现：`__wjs_cli_complete("console.")` 的 trace 项；`console.trace()` 双侧对照。
- 推广铁律：**改某内建的实现形态（原生↔JS）时，同步 grep 其 toString 消费者**
  （补全签名/错误文案快照类测试），形态变则文档分支变，旧断言多为过渡态 incidental。

### 4.227 补全手写文档表被 toString 提取永久遮蔽（2026-09-28，§7-②跟进）

- 症状：`console.` 补全浮窗里主方法全显示通用 `log(...args)`，手写表
  `(...data) — stdout` 等通道信息不可见——用户报"补全没有文档"。
- 根因：`__sigDesc` 先抽 `toString` 形参、非空即返回；09-25 起 console 主方法
  是 JS 包装（自带 `(...args)`），表项永久不可达。4.226 只改了测试断言，
  没动优先级，治标。
- 修法：精确 dotted 路径（`console.log`）的表查询提到的 toString 之前；
  Ctor/裸名回落仍在 toString 之后——用户自有同名方法（如 `o.assign(a,b)`）
  不受内建表遮蔽（探针实证）。`dot-empty` 断言回到通道信息形。
- 复现：`__wjs_cli_complete("console.")` 对比改前/后摘要列。
- 推广铁律：**"精确表优先、启发式在后"**——凡手写映射表与运行时反射并存，
  精确键查询永远放反射提取之前；反射只做兜底，否则任何重构（原生→JS、
  改名、包装备案）都会静默降级文档。

### 4.228 嵌进 JS prelude 的文案含撇号：整仓 prelude 解析失败（2026-09-28，`.doc` 轮）

- 症状：`Document's` 进签名表后，`__wjs_prelude.js:5222 SyntaxError`，
  TLA/补全等所有依赖 prelude 的面全挂（黑盒一片红，极易误判为引擎回归）。
- 根因：表值是 JS 单引号字符串字面量，文案里的 `'` 未转义即截断字符串；
  Rust 侧编译期无感（`r#"` 原样收），炸点在运行时首个 prelude 求值。
- 修法：值内 `'` → `\'`（`\\` 先行）；提交前跑冒烟即现形（本轮即冒烟抓包）。
- 复现：任意含撇号文案进 `__wjsReplSig` 后 `--eval '40 + 2'`。
- 推广铁律：**凡进 prelude/loader 内嵌 JS 的外部文案（文档/错误文案/i18n），
  落盘前必须过引号转义检查**；跨语言搬运（MDN→JS 字面量）默认视为脏输入。

### 4.229 具名函数表达式赋给 globalThis 属性不建词法绑定（2026-09-28，本体化轮）

- 症状：补全核心下沉 prelude 后，`__wjs_cli_complete("globalThis.Object.assign")`
  经 `--eval` 对、经 `--run` 文件即空集（`sig[0][0] is undefined`），且
  `import "node:repl"` 与否无关——极易误判为 node 壳冲掉本体。
- 根因：核心以 `globalThis.__wjs_repl_default_complete = function __defaultComplete(...)`
  注册，函数名只在函数体内可见；桥里裸调 `__defaultComplete(...)` 即
  ReferenceError，又被桥内 `try/catch` 吞掉回空集。`--eval` 对是因为当时
  二进制仍是旧构建（未重编），非语义差异——stash/checkout 换代码必重编再探
  （4.62/4.69）同族。
- 修法：桥内经属性取后调用（`const __core = globalThis.__wjs_repl_default_complete`，
  判函数形再调）；定位靠"core 直调对、桥调错"的同文件双探针逐段二分。
- 复现：`--run` 文件内 core/bridge 同参连调（本轮 `/tmp/wjs-probe-o4.mjs` 口径）。
- 推广铁律：**`globalThis.X = function Name` 后一律经 `globalThis.X` 调用**，
  裸 `Name` 只在函数声明（`function Name(){}`）后可用；吞错的 `try/catch`
  桥内，空结果先疑调用点绑定错，不疑被调用方。

### 4.230 prelude 求值期 `process` 尚不存在：快照即死引用（2026-09-29，命名空间轮）

- 症状：`Deno.env.set("K","1")` 后 `process.env.K` 取不到（`undefined`），
  而 `Deno.env.get("K")` 自洽——双 store 分裂。
- 根因：主 PRELUDE 求值先于 NODE_PRELUDE（`process` 挂载在后），顶层
  `const store = process.env || {}` 快照到 `{}` 死对象；之后 `process.env`
  是另一对象。`--eval` 单测若只验自洽（set 后 get）全绿，跨面一读即穿帮。
- 修法：凡读 `process`/`require("node:*")` 的 prelude 逻辑一律懒求值——
  每次调用经 `globalThis` 现场取（`__wjs_ns_envstore`），`require` 只在函数
  体内调，不在顶层。
- 复现：`Deno.env.set("WJS_NS_T","1")` 后读 `process.env.WJS_NS_T`。
- 推广铁律：**prelude 顶层只放纯数据与函数定义，任何宿主对象一律调用期
  现场取**；自洽绿≠跨面绿，断言必须跨面读。

### 4.231 补全要二级展开的值成员禁 getter（2026-09-29，命名空间轮）

- 症状：`Deno.version.` Tab 恒 `NO RECORDS`，而 `Deno.env.`（数据对象）正常；
  冻结与否无关（冻数据对象可补全）。
- 根因：R3 补全核心对成员链逐步求值，getter 一律拒入（防副作用）。
  `version/args/argv` 等写成 getter 即二级死胡同——值对但不可补全。
- 修法：值型成员一律数据属性；`process` 侧活值经 `__wjs_ns_sync()` 在
  NODE_PRELUDE 尾刷新（`node/mod.rs` 调 `__wjs_` 内部面，§7 顺向；用户代码
  之前，无覆盖之忧），`Deno` 刷新后才冻结。标量 getter（`pid` 等）无二级
  可展，不必改。
- 复现：`__wjs_cli_complete("Deno.version.")` 修前 `[]`、修后非空。
- 推广铁律：**凡要 `.x.` 二级补全的值，先问是不是 getter**；补全面只认
  数据属性，活值走启动同步，不走 getter。

### 4.232 工具回"成功"≠落盘：edit 后必 grep 复核再跑构建（2026-09-29，REPL 文档轮）

- 症状：`edit` 連報成功、单测 2 passed，数分钟后 `git status` 不见
  `src/repl_doc.rs` 修改、新建 11 页语料变空目录；`eval` 行为回到旧版。
- 根因：未定位到单一元凶（并行会话同仓改 `Cargo.*`、U+F8FF 卷路径、
  工具/终端两条路径所见不一致皆有可能）；凡"成功了但像没改"，一律按
  未落盘处理——以 `grep`+`git diff --stat` 为准，不以工具回执为准。
- 修法：逐个 `edit` 后当即 `grep` 复核；`git status` 在提交前再看一遍；
  大改分批提交（本轮路由与语料分两提交）；build 前 `git diff --stat`
  确认改动仍在。
- 复现：本轮 `wjs_group_lookup` 三处改动"成功→通过→消失"，重做一次即稳。
- 推广铁律：**构建/提交前先 `git diff --stat` 看改动还在不在**；状态机残留
  先查工作区，不先怪环境（§4.140/4.197 同族）。

### 4.232 编码器色型断言在 nounwind 上下文即 abort（2026-09-29，image 轮）

- 症状：`WinterJS.image.encode(px, "pnm")` 整进程 abort（`Invalid buffer length:
  expected 32 got 16` + `panic in a function that cannot unwind`），非可捕获异常。
- 根因：`image` 各编码器用 `assert_eq!` 校验（通道数×尺寸），色型不匹配
  （ppm/Pixmap 要 RGB 却喂 RGBA、farbfeld 要 16 位、exr 只要 f32）即 panic；
  native 经 `unsafe extern "C"` 进 JS 引擎，panic 不可 unwind，直接 abort。
- 修法：调编码器前**先按目标色型转换**（ppm→RGB、farbfeld→u16、exr→f32），
  把断言变成不可达；凡调第三方 `encode` 系，先 grep 其 `assert` 再定转换。
- 复现：2x2 RGBA 调 pnm/ppm（修前 abort，修后 23 字节 P6）。
- 推广铁律：**进 `unsafe extern "C"` 的第三方调用，`assert`/`panic` 路径
  一律前置校验转干净错误**；黑盒必须含 panic 路径用例（§0.7），且先跑通
  再提交——abort 不留现场。

### 4.233 include_dir 语料加页不触发重编（2026-09-29，文档轮）

- 症状：`winterjs-content/` 加了 4 页后构建"成功"但 `.doc` 仍出父页——新页不在二进制里。
- 根因：`include_dir!` 编译期读目录，但 cargo 只盯源文件 mtime；新增的
  非跟踪文件不触发重编，"成功"的是旧物。
- 修法：加页后 `touch src/repl_doc.rs`（或对应引用页）再编；冒烟里加一条
  新页 `.doc` 即现形。
- 复现：加页 → 直接编 → `__wjs_doc_summary(新主题)` 回父页/None。
- 推广铁律：**凡编译期嵌目录（include_dir!/include_str! 指向目录）加文件，
  必 touch 引用处再编**；`git status` 见新页 + 二进制行为不变先疑此条。

### 4.234 站 luoli 炸了部署不拦（2026-09-29，文档轮）

- 症状：`Failed to load pages/cli-zh.luoli: unmatched }` +
  `pages/api-zh.luoli: reserved word 'native'`——站全白，但 Actions 全绿。
- 根因三合一：① `native` 是 CoffeeScript 保留字（数据数组名撞了）；
  ② cli-zh actions 行尾多 ` }]`（与英文版逐字对即现形）；③ 部署工作流
  只做静态上传，零校验，带病上线。
- 修法：数组改名 `wjs`；括号与英文版逐字对；`pages.yml` 加部署门
  （`scripts/check-luoli.js`，原版 coffeescript 编 coffee: 段，exit 非零即拦）。
  template:/style: 段是 luolita 预处理方言，裸 pug 验不了（缩进基不同），
  只验 coffee + diff 评审。
- 复现：`NODE_PATH=… node scripts/check-luoli.js`（旧文件必 FAIL）。
- 推广铁律：**无校验的部署链等于没有门**；前端 DSL 进仓即配校验脚本，
  今后推站前先等 Actions（用户令）再看站。

### 4.236 async 形态的参数校验必须同步抛（2026-09-29，P2-crypto）

- 症状：`generateKeyPair(type, badOptions, mustNotCall())` 不抛——校验写在
  `queueMicrotask` 回调里，`assert.throws` 抓不到同步异常（MISSING-EXCEPTION 簇）。
- 根因：真机口径是"校验同步抛、生成才排队"（实测 `generateKey('hmac',{length:-1},cb)`
  为同步 `ERR_OUT_OF_RANGE`，callback 根本不背锅）；旧实现把校验和生成一起丢进 microtask。
- 修法：入口先跑全套同步头检（type/options/编码形/算法参数；`__checkKeyPairHead/
  TypeKnown/Encs/RsaKeyOptions`），再 `queueMicrotask` 只做生成。sync/async 双入口
  共用同一套校验函数（`__genPairSync` 内再检一次防直调）。
- 复现：keygen 67/86/303 行（async 校验三连）；改后 sync/async 同形全过。
- 推广铁律：**凡 async 双形态 API，参数校验一律同步抛，microtask 内只留必成功的体力活**；
  写 async 包装时先问"校验在哪"——在回调里即错。

### 4.235 业务层 f32→u8 重解释禁手写 from_raw_parts（2026-09-29，media 轮）

- 症状：`src/builtins/media.rs set_rval_f32` 用
  `unsafe { std::slice::from_raw_parts(out.as_ptr() as *const u8, out.len() * 4) }`
  把 `&[f32]` 重解释成 `&[u8]`——业务逻辑层出现 `unsafe`，违反 AGENTS §6
  （业务层禁 unsafe）与 §0.6（存量只减不增）；注释只有 SAFETY，无 UNSAFE-BOUNDARY 标签。
- 根因：§6 三问第①问即证伪：`bytemuck` 已在 `Cargo.lock` 内（1.25.2，
  `image`/`jxl-oxide`/`resvg` 带入），`bytemuck::cast_slice(out)` 是同语义 safe 写法
  （对齐/长度由类型保证，零拷贝视图）；不愿加依赖时 `to_ne_bytes` 拼 `Vec` 亦可（多一次拷贝）。
- 修法：直引 `bytemuck = "1"`（零新增传递，见 `docs/dependencies.md` §16-6 跟进），
  该行改为 `let bytes: &[u8] = bytemuck::cast_slice(out);`，删业务层 `unsafe`；
  保留的 `TypedArray::<Uint8>::create` 那块 `unsafe` 是 §6 引擎边界（不可去），注释写明。
- 复现：`rg "from_raw_parts" src/builtins/media.rs` 修前命中 1 处，修后 0 处；
  `sample/media/basics.js` 解码能量断言照常绿。
- 推广铁律：**标量切片重解释（f32/u16/i16↔u8）一律走 `bytemuck::cast_slice`，
  禁手写 `from_raw_parts` + 裸指针强转**；凡在锁内已有的纯 Rust 轮子，直引即零成本，
  不要为"省一个直接依赖"造业务层 unsafe。

### 4.237 单例头表勿凭名字猜，须逐行对 matchKnownFields 前缀（2026-09-30，P3-http）

- 症状：`content-encoding`/`x-forwarded-host` 重复头被当单例首个赢，真机却是
  `', '` 合并（`gzip, br` / `a, b`）。
- 根因：`matchKnownFields` 以返回有无 `\u0000` 前缀区分单例/可合并——
  `content-encoding`/`x-forwarded-host` 返回 `\u0000…`（joinable），仅 18 项无前缀
  才是单例；旧表凭"看起来单值"手写，多收两项。
- 修法：单例表删两项并注释列出 18 项来源；`~/wjs-data/probe/dup.mjs` 双侧对拍
  （`authorization:1` 首个赢对照）。
- 复现：raw socket 发双 `Content-Encoding` + 双 `X-Forwarded-Host`，修前丢第二个，
  修后与真机同串。
- 推广铁律：**移植 node 头表一律逐行对 `lib/` 原文前缀，不凭语义猜**（§4.115）。

### 4.238 ServerResponse 缺 OM 品牌位即 finished 不等 close（2026-09-30，P3-http）

- 症状：`test-http-outgoing-finished` 第二个 `finished(res)` 在 `close` 前回调，
  `closed===false`（真机 `finish→close→FIN`，我方 `finish→FIN→close`）。
- 根因：`ServerResponse extends Writable` 未继承 `OutgoingMessage` 品牌四件套
  （`_closed/_defaultKeepAlive/_removedConnection/_removedContLen` + `_sent100`），
  `isOutgoingMessage` 恒 false → `willEmitClose` 恒 false → `finished` 在 `finish`
  即回（真机 `ServerResponse` 无 `_writableState`，走 `!state && isServerResponse` 为 true 等 close）。
- 修法：构造期补五项（初值与 `OutgoingMessage`/`_http_server.js` 同源），
  `writeContinue` 置 `_sent100=true`；`isClosed` 走 `wState` 路径不变，其余消费者仅
  `willEmitClose`（意图内）。
- 复现：`probe/fin.mjs`（`finish→FIN→close` 修前，`finish→close→FIN` 修后，与真机同序）。
- 推广铁律：**凡 `extends Writable` 的 node 消息类，构造期先对 `lib/` 原文品牌字段**；
  `finished` 时序红先查 `willEmitClose`（`_closed` 四件套 + `_sent100`），不先调时序。

### 4.239 argon2 的 async 校验必须同步抛 + 缺参/文案逐字对（2026-09-30，P2-crypto）

- 症状：`test-crypto-argon2` 首败 `Missing expected exception`（9 条坏向量同步全抛，
  异步坏参却进回调不抛）；修后连环三败（缺参错码/文案/算法缺参错码）。
- 根因三合一：① async 把参数校验丢进 microtask（与 4.236 同族：真机校验同步抛，
  生成才排队）；② `Number(undefined)=NaN` 致缺参落区间门（OUT_OF_RANGE），真机为
  INVALID_ARG_TYPE；③ `passes` 下限手写 0（真机 ≥1）+ nonce 短文案自造 +
  算法缺参/错值不分码（真机缺参 INVALID_ARG_TYPE、错值 INVALID_ARG_VALUE）。
- 修法：JS 层 `passes` 改 1 起；`intArg` 首检 `undefined` 即 INVALID_ARG_TYPE；
  `nonce` 文案改 `The value of "parameters.nonce.byteLength" is out of range…`；
  算法分两支（非 string 即 TYPE、错值即 VALUE）；async 先同步跑全套
  `__argon2Args` 再验回调（顺序先参数后回调，真机坏参+无回调即 OUT_OF_RANGE）。
- 复现：`probe/argon2async.mjs`（修前 async 坏参 NO-THROW，修后与真机同码）；
  全坏向量 `argon2bad2.mjs` 双侧 9/9 同码；`run1.sh test-crypto-argon2.js` 0。
- 推广铁律：**KDF/密钥系 async 包装先问"校验在哪"（4.236）；区间门前先拦
  `undefined`（NaN 会偷渡错码）；文案/分码一律真机逐项实测，不凭记忆拼**。

### 4.240 Cipher 系输出编码门缺失：首编码粘住 + 未知即抛（2026-09-30，P2-crypto）

- 症状：`test-crypto-encoding-validation-error` 四断言全 NO-THROW（换编码/final
  异编码/坏编码名全吞）。
- 根因：`__outBuf`（hash 侧复用）未知编码吞回 Buffer，而 cipher 真机口径是独立
  `getDecoder` 状态机：首个非 buffer 输出编码粘住（`utf-8` 归一 `utf8`），再换即
  `ERR_INVALID_ARG_VALUE`（`cannot be changed from 'utf8'`），未知即
  `ERR_UNKNOWN_ENCODING`，`buffer`/缺省不粘。
- 修法：cipher 侧新 `__cipherOut(inst,…)` + 实例 `__decoder`（Cipher/Decipher 四处
  `update/final` 全换；hash 侧 `__outBuf` 不动——digest 真机即吞码口径，见既有记档）。
- 复现：`probe/encval.mjs` 四项双侧同码；`run1.sh` 0。
- 推广铁律：**同名辅助跨域复用先对真机口径**——hash 与 cipher 的"非法编码"语义
  相反（吞 vs 抛），复用即错；新事件/状态门一律实例级存放，不放模块级。

### 4.241 pkcs8 加密导出须走 PBES2 + AKP JWK + DER 嗅探三件套（2026-09-30，P2-crypto）

- 症状：`test-crypto-pqc-encrypted-pkcs8` 三连败——①导出 `PRIVATE KEY` 明文/传统
  PEM（真机 `ENCRYPTED PRIVATE KEY`）；② fixture JWK `Unsupported JWK kty`
  （`AKP` 未实现）；③ 自家 DER 加密体导入 `Invalid PKCS#8`（DER 无标签不嗅探）。
- 根因：pkcs8+口令导出误复传统 PEM 路径（那只属 pkcs1/sec1）+ DER 完全不走
  cipher；AKP（`{kty,alg,priv,pub}`）是 PQC 新面；导入只认 PEM 标签。
- 修法：新 `__pbes2Encrypt`（PBKDF2-SHA256/2048/8B 盐 + AES/DES + PRF 带 NULL，
  与 `__pbes2Decrypt` 对称，fixture/OpenSSL 互解）；pkcs8 私钥 +cipher 的
  der/pem 同走 PBES2（pem 标签 `ENCRYPTED PRIVATE KEY`；pkcs1/sec1 传统路径不动）；
  AKP 分支（alg 表 6 集 + 种子形 PKCS#8 自拼 + 既有展开派生比对 pub，零新 native；
  文案 `Unsupported JWK AKP "alg"`/`Invalid JWK AKP key`/无 priv 三形逐字对真机）；
  DER 显式 pkcs8 + 口令 + PBES2 OID 嗅探先解密（`__sniffPbes2`）。
- 复现：自回环 pem/der 双绿 + fixture 双绿 + 错口令 `ERR_OSSL_BAD_DECRYPT`
  （与真机同码）；`run1.sh` 0。
- 推广铁律：**加解密对必须同批落地验双向**（解密先行、导出后补是半拉子）；
  新 `kty`/`alg` 面先查 fixture 再写码；DER 无标签面一律配嗅探，不只认显式 type。

### 4.242 async 包装调两次生成器即公钥私钥错配（2026-09-30，P2-crypto-R4）

- 症状：crypto3 keygen-async 簇十余件同红——x-JWK 公私钥错配、RSA 解密失败、
  DSA 超时，表象各异。
- 根因：异步 `generateKeyPair` 回调里调了**两次** `__genPairSync`（公钥取自
  第一次结果、私钥取自第二次）——两对毫不相干的键；另附带双倍慢（DSA 2048
  两次生成即超时）。
- 修法：microtask 内单次生成再分发编码（`const out = __applyEncoding(
  __genPairSync(...)); cb(null, out.publicKey, out.privateKey)`）；R4 一并转绿 22 件。
- 复现：`generateKeyPair('ed25519', {jwk/jwk}, cb)` 对比 `publicKey.x ===
  privateKey.x`（修前恒 false，修后 true）。
- 推广铁律：**凡"一次调用产一对"的 async 包装，生成器只调一次**；同簇多件同红
  先疑共享上游（本轮另例：jwk 免 type/paramEncoding 翻正，一改带走 6 件）。

### 4.243 全仓机械改名 checklist（2026-09-30，winterjs→winterjs2）

- 症状：改名后构建/测试多处挂——`include_str!` 路径（assets/svg、content 语料目录
  先搬）、env 前缀裸串（`with_prefix("WINTERJS")` 无下划线被 pattern 漏过）、
  前缀长度硬编码（`slice(11)` 随 `__wjs_prim:`→`__wjs2_prim:` 变长失效）、
  fixture 数据被改名（签名向量绑定的消息串）、`CARGO_BIN_EXE_<旧名>`/insta
  快照名随包名变。
- 根因：① 改名分多遍跑时 pattern 缺"已改名"守卫会叠加（`__wjs2`→`__wjs22`）；
  ② `rg` 不支持 lookahead——"零残留"校验必须换写法（字符类），不可信其空输出；
  ③ 占位符本身含可匹配词会被二次改名。
- 修法：单遍脚本（ alternation + 全守卫：`__wjs(?!2)` 等）+ 占位符用无意义串；
  校验用 `rg -e "X([^2]|$)"` 字符类写法；改名前列禁区（上游 vendor/Bun/Deno/MDN
  语料、历史文档、第三方链接/域名/标准格式名）。
- 复现：本轮 `__wjs222`/`WinterJS22` 三连击 + `rg` lookahead 静默失败。
- 推广铁律：**批量改名前先列"禁区 + 守卫 pattern + 校验命令"三件套**；fixture
  数据与前缀长度常量逐个过目，不进机械替换。

### 4.244 改名后工具链默认二进制路径漏网：sweep 跑的仍是旧名 stale 件（2026-10-03，P2-process）

- 症状：process 首簇 `--eval` 探针全绿，`run1.sh` 五件却全红且报错全是旧行为
  （`can't convert undefined to BigInt`、`process.release is undefined`）——
  探针与套件用了两个二进制。
- 根因：改名只改了仓内码，`scripts/sweep-bg.py` 默认 `target/debug/winterjs`
  （glob 精确路径）+ `~/wjs-data/probe/run1.sh` 默认同路径仍指旧名；
  `target/` 下新旧两份 308M 共存，旧件 09-30 02:59 一直被 sweep/run1 命中；
  proc1（21/82）即旧二进制跑出的无效基线。
- 修法：sweep-bg.py 默认路径/pgrep/pkill 全 `winterjs→winterjs2` +
  run1.sh 默认路径/pkill 同改 + 删 stale `target/debug/winterjs`（含 `.d`）；
  重跑 proc2（`target/debug/winterjs2`）21→25/82 方为有效基线。
- 复现：`ls -lh target/debug/winterjs*` 双二进制共存即中招；`status.json` 的
  `wjs` 字段是唯一真相（proc1 指向旧名）。
- 推广铁律：**改名后先 `ls target/debug` 看双二进制，再跑任何 sweep/探针**；
  工具链默认路径与 pgrep/pkill 模式进改名 checklist（4.243）必查项。

### 4.245 macOS 无 RUSAGE_THREAD + errors 端口无 RangeError 子构造（2026-10-03，P2-process-R2）

- 症状：`threadCpuUsage` 在 macOS 无 `RUSAGE_THREAD`（libc apple 只有
  SELF/CHILDREN），`ERR_INVALID_ARG_VALUE.RangeError` 在本仓 errors 端口为
  `undefined`；另 `THREAD_BASIC_INFO` 是 i32 而 flavor 要 u32。
- 根因：① Darwn 线程 CPU 须走 Mach `thread_info`（libc 有 `thread_basic_info`/
  `mach_thread_self`/`thread_info`，独缺 `mach_port_deallocate`）；② 本仓 errors
  宏只建主构造，node 的 `.RangeError`  flavor 不存在。
- 修法：`thread_info(mach_thread_self(), THREAD_BASIC_INFO)` 取 user/system_time
 （`as u32` + 小 extern 块补 `mach_port_deallocate`，§6 三问注释；Linux 走
  `RUSAGE_THREAD`，其余回零记档）；范围错按 `RangeError` 名 +
  `ERR_INVALID_ARG_VALUE` 码 + 真机文案逐字手拼（`The property 'x' is invalid…`）。
- 复现：`run1.sh test-process-threadCpuUsage-main-thread.js` 0；
  `process.cpuUsage({user:-1,system:2})` 与真机三元组逐字同。
- 推广铁律：**拿轮子先 grep 目标符号在该平台真有**（registry 源码为准，不抄文档）；
  errors 新 flavor 先 `--eval typeof` 探存在性，不存在即手拼码名文案三件。

### 4.246 返回码命名空间撞车：EPERM 本体即 1（2026-10-03，P2-process-R3）

- 症状：`seteuid('nobody')`（本机存在，uid 4294967294）报
  `ERR_UNKNOWN_CREDENTIAL` 而非 EPERM；连 `root` 都"不存在"。
- 根因：native 返回码 0=成功/1=未知身份/正数 errno 三义共用一 int——EPERM
  的 errno 本体就是 1，非 root 下所有 set*id 调用的 EPERM 全被 JS 读成"未知身份"；
  探针逐段无辜（JSON 层、getpwnam 均对，`getpwnam("nobody")` 非空），错在编码层。
- 修法：成功 0 / 未知 1 / 失败 `-errno`（setgroups/initgroups 早就是负 errno，
  统一；JS 侧 `r < 0` 取反成错）。
- 复现：`process.seteuid('nobody')` 修前 UNKNOWN 修后 EPERM，与真机逐字同。
- 推广铁律：**凡"成功/分类失败/errno"三义通道，errno 恒走负值**（EPERM=1、
  EINVAL=22 等正数会撞分类码）；先 `id nobody` 确认环境，再疑代码。

### 4.247 inspect 转义表照抄要逐项对 meta（2026-10-03，P2-process-R6 附带）

- 症状：`process.execve(path, ['123', 'abc\0cde'])` 校验文案差一个转义——我方
  `Received 'abc\0cde'`，真机 `Received 'abc\x00cde'`。
- 根因：inspect 转义表 0/7/11/27 四项与真机 `meta` 表不同（我方 `\0`/`\a`/
  `\v`/`\e`，真机 `\x00`/`\x07`/`\x0B`/`\x1B`）；转义是"看起来对"的重灾区，
  肉眼 review 过不了。
- 修法：逐项对 node `lib/internal/util/inspect.js` 的 `meta` 表改四项；
  黑盒 `phase11_inspect_control_escapes` 四断言钉住。
- 复现：`node -e "console.log(require('util').inspect('\0'))"` 即出 `\x00`。
- 推广铁律：**凡"表驱动"的移植（转义/信号/errno/状态码），表按原文逐项 diff**，
  不凭记忆手写；配黑盒逐项钉表。

### 4.248 带 pending 进 JS 调用非法：分发前须 take（2026-10-03，P2-process-R7）

- 症状：入口分发接上后，无接管的入口抛错渲染成 `undefined`（无文案无栈）——
  有接管路径全绿，裸错路径全崩。
- 根因：`dispatch_entry_throw` 里 `call_one` 跑在 pending 未清的状态下；
  SpiderMonkey 带 pending 进 `JS_CallFunctionValue` 非法（静默吞错），后续
  `error_info` 读空。
- 修法：分发内 take-调-放回三段：先 `take_pending_exception` 再调
  `__wjs2_uncaught`；无人接则 `set_pending_exception` 放回，原 fatal 重读
  （与未分发字节一致，stash 对照实锤）。
- 复现：`--run throw-new-Error.js` 修前渲染 `undefined`，修后 `Error: …` + 栈；
  capture 路径 `cap foo` + rc=0 不变。
- 推广铁律：**任何 JS 调用前先确认 pending 已清**（take 即清场语义）；
  新分发点必配"无人接"裸错渲染对照（stash 二进制 40 秒即得）。

### 4.249 Rust set_var 遇空键 panic：node 口径是静默忽略（2026-10-03，P2-process-R8）

- 症状：env.js 全程崩（rc=139）：`failed to set environment variable '""' to
  '""'`，native 帧直接 abort（panic in nounwind 区，连栈都抓瞎）。
- 根因：套件自带 `process.env[''] = ''`（还有 `TEST=` 一行）；Rust `set_var`
  遇空键/`=`/NUL 即 panic，真机（libuv）静默无操作——也不是 throw。
- 修法：native 先拦（空/`=`/NUL，含值 NUL）→ 回 undefined 不写；裸赋值在严格
  模式下亦不抛（套件即裸写后读 undefined）。
- 复现：修前 rc=139，修后 rc=0；定位靠给 native 加临时 eprintln 看调用序列
  （复现后即删；JS 侧包 `__wjs2_env_set` 抓栈亦可，见 R8 过程）。
- 推广铁律：**凡 Rust 标准库会 panic 的前置（set_var/remove？unwrap），native
  入口一律先拦**；`env['']=x` 这类"合法 JS、非法 OS"键，真机行为是忽略不是抛。

### 4.250 SpiderMonkey 时区缓存清不动（2026-10-03，P2-process-R8，记档）

- 症状：`process.env.TZ='Europe/Amsterdam'` 后 `Date` 仍旧区；启动前置 TZ
  则生效（首用即缓存）。
- 根因：SM 另有引擎侧时区缓存（启动/首 Date 即定）；`tzset` 只刷 C 库，
  对 SM 无效；mozjs/mozjs_sys 均无时区重置口（grep 空），ICU 动刀超出边界。
- 修法：env-tz 记档（引擎边界）；`tzset` 保留（POSIX hygiene，无害）。
- 复现：`TZ=... --eval Date` 生效 vs 运行期置 TZ 不生效，两行即判。
- 推广铁律：**"启动生效、运行期不生效" = 引擎侧缓存**，先 grep 绑定层有无
  重置口，无则记档不动（§6 mozjs 是墙）。

### 4.251 SM 删不可配置属性的文案与 V8 不同：Proxy 拦 deleteProperty 回真机形（2026-10-03，P2-process-R9）

- 症状：exit-code-validation 套件 `delete process.exitCode` 断言正则
  `/Cannot delete property 'exitCode' of #<process>/`，本仓抛
  `property "exitCode" is non-configurable and can't be deleted` 即红。
- 根因：exitCode 按真机 `configurable:false` 后，delete 错文案是引擎实现定义——
  SM 与 V8 逐字不同；断言锁的是 V8 形。
- 修法：`globalThis.process` 包一层 Proxy，只拦 `deleteProperty`（exitCode 即抛
  真机形文案），余下默认透传（get/set 经 target；PROTO_FIXUP 的 setPrototypeOf
  亦透传；回归 prototype/ppid/title 全绿）。
- 复现：`--eval 'try{delete process.exitCode}catch(e){console.log(e.message)}'`。
- 推广铁律：**凡断言锁引擎报错文案的，先对真机逐字抠，SM 形不同即在边界层
 （Proxy/包装）对齐，不动引擎**。

### 4.252 uncaught 三段路由：monitor 先行 + 监听再抛即 exit 7 + _fatalException 置空即 6（2026-10-03，P2-process-R9）

- 症状：monitor 套件 stdout 空（仅 monitor 监听时 count=0 致 drain 跳过）；
  exit-code 套件 exitWithThrowInUncaughtHandler 期 7 得 0（旧 `__wjs2_emit`
  吞监听抛错）、exitWithUndefinedFatalException 期 6 得 1。
- 根因（execution.js 原文）：`process.emit('uncaughtExceptionMonitor', er, type)`
  恒先行（throwing）；capture 次之；`emit('uncaughtException')` 抛错冒泡；
  `_fatalException` 置空即 C++ 回落默认 exit 6；monitor/监听抛的新错替代原错
  exit 7。旧实现三处偏离：count 只数 uncaughtException、__wjs2_emit 吞错、
  fatal_exit 无条件盖 1（连预设 99 一并盖掉后又连 7 一并盖掉）。
- 修法：`__wjs2_uncaught_count` 含 capture/monitor；`__wjs2_uncaught` 先 throwing
  版 emit monitor，再 capture，再 throwing 版 emit uncaughtException；
  `_fatalException in p && === undefined` 即置 exitCode 6 回 false；
  Rust 分发抛错（call None + 新 pending）即置 7 并保留新 pending（不恢复原错）；
  fatal_exit 保留 6/7、余下盖 1（含预设码覆盖，exitWithOneOnUncaught 点名）。
- 复现：monitor1/2 fixture 真机 rc=1/7 对拍；`p._fatalException=undefined` 黑盒断 6。
- 推广铁律：**宿主吞错（__wjs2_emit）与用户可见抛错（emit）是两条路，分发链
  上按 node 原文逐段选用**；fatal 收尾的置码须列出保留集（6/7），不得无条件覆盖。

### 4.253 本仓 builtinModules 双形表 vs 真机混合表 + ESM 无 default 即 import.default 失配（2026-10-03，P2-process-R9）

- 症状：get-builtin 套件连环三红：`getBuiltinModule(1)` 走 `String(id)` 前缀拼出
  `node:1` 抛错（应 ERR_INVALID_ARG_TYPE）；`getBuiltinModule('test')` 回模块
  （真机 builtinModules 无裸 `test`，应 undefined）；`node:timers/promises` 的
  `import().default` 为 undefined（无 default 导出）而 require 有值。
- 根因：本仓 `builtin_modules_json` 对每 canonical 发裸名+`node:`双形，真机是
  裸名（老模块）/`node:`专形（test/sqlite/sea/ffi/quic/vfs 新模块）混合；
  `getBuiltinModule` 照抄 require 别名语义即越界；timers/promises 刻意无 default
  致 ESM/CJS 双面不等。
- 修法：`getBuiltinModule` 非串先 ERR_INVALID_ARG_TYPE，裸 test/sqlite/quic/sea/
  ffi/vfs 与 `internal/*` 直回 undefined（require 别名不动）；双形表对新风格
  六件只发前缀形；timers/promises 补 `export default __api`（timers 的
  `import * as promises` 同步改 default 引用，deepStrictEqual 不散）。
- 复现：自写 probe 扫 51 裸名 `getBuiltin===import.default`（修前 1 坏，修后 0 坏）。
- 推广铁律：**"经 require 可达"≠"真机 builtin"**，凡涉及模块名单（builtinModules/
  getBuiltinModule/isBuiltin）以 `node -e builtinModules` 实表为准，不以自家
  注册表为准。

### 4.254 同源双实例状态分叉：internal 门面须重导出锚定公开实例（2026-10-03，P2-stream-R1）

- 症状：`enabledHooksExist` 已在 `node:async_hooks` 导出，eos 内部分支与三个
  finished 套件仍全假——`require('internal/async_hooks')` 读到空状态。
- 根因：`node:async_hooks` 与 `node:internal/async_hooks` 同 SOURCE 字符串但
  各自求值，模块级状态（ALS Map/enable 集）两份分叉；eos 的 AsyncResource
 （公开实例）快照的 ALS 上下文，internal 实例永远看不见。
- 修法：`node:internal/async_hooks` 改门面源（重导出公开实例的
  `enabledHooksExist`，状态锚定一处）；eos 垫片 `internal/async_hooks` 改引
  真门面（移植体 `require('internal/async_hooks')` 原文不动，只改胶水映射）。
- 复现：ALS 测试修前 `false !== true`，修后绿；门面改回同源即复现。
- 推广铁律：**凡模块级 JS 状态（Map/Set/计数器），`node:X` 与
  `node:internal/X` 同源注册即分叉**——后来者一律门面重导出，不再同源注册。

### 4.255 internal 连字符/下划线双拼写：精确优先、失配回落（2026-10-03，P2-stream-R1）

- 症状：套件直引真机形 `internal/streams/end-of-stream` /
  `internal/streams/add-abort-signal`，本仓表内 `end_of_stream` /
  `add_abort_signal`，两件"未映射"红。
- 根因：本仓 internal 表为 Rust 标识符友好用下划线，真机全连字符；
  `normalize_internal` 只做精确匹配。
- 修法：精确命中优先，失配且含 `-` 再试下划线形（zip 系原生连字符精确命中，
  不受影响）；单测断言两例回落。
- 复现：修前两件 `unmapped internal require`，修后绿。
- 推广铁律：**凡"本仓为实现方便改名"的注册表，对外规范名须双向可达**——
  精确优先 + 回落，而非改名即断。

### 4.256 E() 吞变体类：`ERR_X.TypeError` 须逐类挂载（2026-10-03，P2-stream-R2）

- 症状：iter 面 `new ERR_INVALID_STATE.TypeError(...)` 报
  `ERR_INVALID_STATE.TypeError is not a constructor`。
- 根因：自家 `E(sym, val, def, ...otherClasses)` 只处理了
  HideStackFramesError 标记，其余变体类（TypeError/RangeError/URIError）
  被忽略；真机逐类挂 `ErrClass[Clazz.name]`。
- 修法：循环挂载（标记类走 HideStackFrames 分支，余下按名挂载）。
  纯加法：旧 `codes[sym]` 基类不变，既有 `instanceof` 断言不受影响。
- 复现：`new (require('node:internal/errors').codes.ERR_INVALID_STATE.TypeError)('x')`。
- 推广铁律：**移植"注册器"函数（E/defineProperty 循环）须逐行对原文**，
  丢分支即整族面静默缺失（87 个 E() 共用，无单件可观察）。

### 4.257 ESM 环的新边：移植体顶层 import 即建边，懒 require 也救不了（2026-10-03，P2-stream-R2）

- 症状：classic 引入后 `import('node:stream/iter')` 报
  `can't access lexical declaration '"default"'`，栈顶在 duplex:67
  `require('internal/streams/readable')`。
- 根因：classic 为懒函数配了顶层 ESM import（readable/writable），新建
  classic→readable 边；DFS 到达 compose→duplex 时 readable 尚在环中，
  duplex 顶层解构即 TDZ。旧图无此边故一直绿——新边是唯一变量。
- 修法：classic 改 `__reg` 运行时解析（duplex/duplexify 同款），并给
  readable/writable 补 `__reg.set` 尾（加法）；移植体懒函数体不动。
- 复现：直引 classic 即现；摘掉两 import 即消（对照实锤）。
- 推广铁律：**新移植文件的顶层 import 即新边**——凡原文有 lazy-require
  注释（"defer the require"/"avoid circular"）处，一律走 `__reg`
  运行时解析，不建静态边。

### 4.258 同名双实例之外：`instanceof` 跨 realm 恒 false，ArrayBuffer 系须结构判（2026-10-03，P2-stream-R2）

- 症状：cross-realm 套件 `fromSync(crossRealmAB)` 抛
  `Received an instance of ArrayBuffer`——判定说不是，文案说认识。
- 根因：`isArrayBuffer/isAnyArrayBuffer/isSharedArrayBuffer/isDataView/
  isTypedArray` 用 `instanceof`，跨 compartment 恒 false（4.57 本例）。
- 修法：改 `Object.prototype.toString` tag 比对（DataView/TypedArray 同理；
  伪造 tag 误判记档，真机品牌检查无此问题）。
- 复现：`vm.runInNewContext` 造 AB 调 `isAnyArrayBuffer`。
- 推广铁律：**凡 `internal/util/types` 的形态判定，默认写跨 realm 安全形**
  （tag/isView），`instanceof` 出现即 suspicious。

### 4.259 自家黑盒 stale handler：uncaught 监听不摘即交叉开火（2026-10-03，P2-stream-R2）

- 症状：http upgrade 黑盒 `'sim err' !== 'cb boom'`，干净 HEAD 同败，
  真机跑同文件亦败——与实现无关，测试设计缺陷。
- 根因：u4/u6 两块各 `process.on('uncaughtException')` 永不摘除；
  第二个未捕获到时两监听全跑，先注册的断言先炸（真机 emit 同样不捕获，
  照样败）。
- 修法：两处 `on` 改 `once`（意图不变：各收一次即撤）。
- 复现：双运行时同败即实锤测试问题（4.65 先实测再定责的用例）。
- 推广铁律：**一个文件内多个 uncaughtException 断言必须 `once` 或手动摘**；
  新红先双运行时对照，再动手（本轮另两例 node 侧红亦同理记档）。

### 4.260 真机文案 `No such built-in module`：CJS/ESM 同文，裸名/前缀双形（2026-10-03，P2-stream-R2）

- 症状：iter-disabled 套件要求无旗下 `require("node:stream/iter")` 报
  `No such built-in module`，自家报 `Cannot find module ... is not a builtin`。
- 根因：真机 CJS（ERR_UNKNOWN_BUILTIN_MODULE）/ESM 解析器同文案；
  自家三处（resolve/prepare/require）各写各的旧文案；另裸名形仍
  `Cannot find module`（真机同，双形并存）。
- 修法：`node:` 前缀三处统改新文案（`bun:` 沿旧）；`require()` 内
  `node:` 先拦（外层 "Cannot find module" 包裹仍在，正则子串命中）；
  旧黑盒两处按 4.65 翻转。
- 复现：`node -e 'require("node:path/nope")'` 首行即文案。
- 推广铁律：**"经 require 可达"≠文案一致**——缺失路径的文案须对真机逐字抠，
  前缀形/裸形分开断言。

### 4.261 transform 要的是句柄协议不是算法：缓冲式 shim 只译协议（2026-10-03，P2-stream-R3a）

- 症状：`zlib/iter` 全灭（`internalBinding is not defined`，transform 顶层直调）。
- 根因：transform 经裸 `internalBinding('zlib')` 取 C++ 流式句柄（init/
  write/writeSync/close + writeState 双槽 + processCallback）；轮子只有算法
  无此协议层（crates.io 无 Node 私有 ABI 轮子，预期内）。
- 修法：新 `iter_zlib_binding` 模块实现同协议（攒输入、FINISH 整包同步压、
  writeState 分次吐、`processCallback` 微任务回；`ZSTD_e_flush` 按 PROCESS 攒；
  常量取真机值，缺失档补 `zlib.js`）；transform 体逐字不动，头补局部
  `internalBinding` 映射（无全局污染）；另补 `ERR_ZSTD_INVALID_PARAM` +
  `ZSTD_c/d_*` 族（coverage 按名定界）。
- 复现：gzip 回环 31B↔"hello world"；transform×5 + interop/to-readable 转绿。
- 推广铁律：**"轮子只管算法，协议层手写适配"**（tls wrap 同构）；körper 用量
  先数协议动词（init/write/close/回调），再估行数——本例约 200 行。

### 4.262 end-again 归属错文件：OM 行为安到 Writable 头上（2026-10-03，P2-stream-R3b）

- 症状：writable-destroy（node 套件原文）要恒 DESTROYED，与既有
  `state.errored ?? DESTROYED` 偏离行冲突。
- 根因：9 月 http 轮把 OM（ClientRequest/ServerResponse）的 end-again
  同值语义修进了共享的 `Writable.prototype.end`；node 原文该行无条件
  DESTROYED（套件即证）；OM 的 end-again 走自家 `__omErrored` 路（且 http
  已冻结，writableFinished 系既有红另案）。
- 修法：writable_flow 回滚逐字；OM 侧不动（冻结域）。
- 复现：双套件对打即现（一方要恒值一方要已记错，必有一方是错文件）。
- 推广铁律：**共享基类函数的"特例分支"必须有inctance 判据跟行**（本例
  `this.__omErrored`）；裸改共享行即跨域互斥——先问"谁也在调它"。

### 4.263 console 无回调即无 tick：复用 errorHandler 才是真机形（2026-10-03，P2-stream-R3b）

- 症状：samecb-singletick 要 1 次 TickObject init，实测 0 次。
- 根因：自家 Console 调 `stream.write(text)` 无回调（nop 路 needTick 恒假）；
  真机传复用 errorHandler（同对象百次）→ 首写 1 次 nextTick + 合批计数。
- 修法：实例复用空回调（错误仍走 emit，不拦截，行为不变）。
- 复现：包 process.nextTick 计数器，真机 5 写 1 tick。
- 推广铁律：**"无回调"与"复用空回调"在合批语义下不等价**——凡涉及
  nextTick 合批的调用点，回调同一性是可观测行为。

### 4.264 BOM 默认剥：WHATWG 与 fs/StringDecoder 分家（2026-10-03，P2-stream-R3b）

- 症状：preprocess 套件 `readFileSync(..., 'utf8')` 丢 BOM（要保留）。
- 根因：`__fsDecode` 直调 WHATWG TextDecoder（默认剥）；真机 fs 经 Buffer
  路不剥。流式侧同理（StringDecoder 内建 TextDecoder 默认剥）。
- 修法：两处加 `ignoreBOM: true`（readFileSync 侧 + StringDecoder 构造）。
- 复现：BOM 文件 `read(1)` 真机 `"\uFEFF"`，修前 `"a"`。
- 推广铁律：**凡"读文件/流转串"的解码点，先问 BOM 留不留**（WHATWG 默认
  与 Node fs/stream 默认相反）。

