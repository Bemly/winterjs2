# 踩坑分卷6（4.265–4.274）

> 本卷为 `docs/pitfalls.md`（主索引）分卷之一，只收正文；查阅先看主索引，编号 `§4.N` 全仓唯一。

### 4.265 process.exit 裸传当回调即 receiver 错位（2026-10-04，base16）

- 症状：cluster-net-listen（worker 内 `net.createServer().listen(process.exit)`）
  `TypeError: this.reallyExit is not a function`（base15 绿 → base16 红）。
- 根因：自家 `exit()` 方法体走 `this.reallyExit`；裸传后 this=server。
  真机 process.exit 与 receiver 无关（C++ 绑定）；此前绿因 worker 内 server
  根本起不来（R 前 listen 即死 → 进程正常退出），stream R 修好 server 后现形。
- 修法：`exit()` 内 receiver 守卫（有 reallyExit 用 this，否则回落
  `globalThis.process`；§4.97 二选一；mock reallyExit 照常走同对象）。
- 复现：`http.Server.listen(process.exit)` 最小形；黑盒 `p2_process_exit_detached_receiver`。
- 推广铁律：**"修好 A 即现形 B"是常态**——base 轮后转红件先问"是不是之前死太早"。

### 4.266 throwDeprecation 同步抛是伪语义：真机 nextTick 异步走 uncaught（2026-10-04，base16）

- 症状：process-warning test4 `assert.fail('Unreachable')` 被触发。
- 根因：自家 `throwDeprecation` 分支同步 `throw`；真机 `node -e` 实测无同步抛，
  警告经 nextTick 异步抛 → uncaughtException 交付。
- 修法：改 nextTick 异步抛（`this.nextTick`，§4.97 this 基）。
- 复现：`node -e` 三行探针；黑盒 `p2_process_warning_throw_deprecation_async`。
- 推广铁律：**"抛"有同步/异步两种，真机探针先定是哪种**（uncaught 类面默认疑异步）。

### 4.267 注释写的"真机实测"与套件矛盾时以套件为准（2026-10-04，base16）

- 症状：crypto-keygen-eddsa（`generateKeyPair('ed25519', cb)` 无 options）
  base15 绿 → base16 红；代码注释称"真机二参即抛"。
- 根因：R4 校验凭一次手误实测写死注释；套件本身在真机绿（`node -e` 三秒可证）。
- 修法：options 缺省即 `{}`；注释按 §4.65 翻转（黑盒 `p2_crypto_keygen_no_options`）。
- 复现：`node -e "require('crypto').generateKeyPair('ed25519',cb)"`。
- 推广铁律：**注释不是证据**——与套件/实测矛盾时注释是错的，先翻转注释再改码。

### 4.268 http2.connect 无视 lookup + 缺 promisify.custom（2026-10-04，base16）

- 症状：promisify-connect-error（自定义 lookup 回错）base15 绿 → base16 红，
  自家报 UNKNOWN DNS 文案（Rust 直拨无视 lookup）。
- 根因两件：① `__start` 直调 `__wjs2_h2_connect`，options.lookup 从未读；
  ② 缺 node internal/http2/core.js 末尾的 `connect[promisify.custom]`
 （once error→reject），致 session error 无人接变 unhandled。
- 修法：`__start` 内 lookup 优先（错原样 error、成拨解析地址）+ 逐字补
  promisify.custom（黑盒 `p2_http2_lookup_and_promisify_custom`）。
- 复现：套件本体即最小形。
- 推广铁律：**"直拨 native"即绕过 node 选项层**——凡 native 直拨面，逐项核对
  options 透传表（lookup/signal/custom promisify 三件最易漏）。

### 4.269 TLS 服务端同字节双派发：单 st 双 __feed（2026-10-04，base16·记档未修）

- 症状：https 迭代四件（default-port/request-agent/url.parse-https.request/
  set-default-ca-precedence-empty）`ERR_HTTP_HEADERS_SENT`——同一请求 handler
  进两次，第二次 writeHead 炸。
- 根因（定位到界为止）：connection=1（单 st），userland data 事件=1，
  但 `__srvDataListener→__feed` 跑两次（第二次 buf 已消费完又拼回同字节重解析；
  plain-http 单派发正常，TLS 服务端独有；JS 层 ingest/flush/listener 皆单投，
  疑 Rust `tls_listen` 派发层同明文推两次）。
- 复现：`probe/dbg/p9-count.js`（3/3 稳定：nc=1 nr=2）。
- 推广铁律：**"userland 只见一次"≠"只派一次"**——复现探针要同时数三层
  （connection/request/userland-data），差值即分层定界。
- 状态：未修（Rust 派发深水 + https/timeout 件另案），本轮记档。

### 4.270 修好即多活：DIFF 快败翻 TIMEOUT 挂死（2026-10-04，base16）

- 症状：base16 新增 TIMEOUT 107，其中 105 在 base15 是 DIFF（`wjs=1` 快败 →
  `wjs=142` 挂死），聚集 cluster×26/http2×40/tls×24。
- 根因：P2 把 server/socket 做"更对"（worker 内 http 可 listen、TLS 握手可过），
  套件多活到"等一个永不到的事件"（cluster 'listening' 中继本就未实现，
  见 cluster.rs 头注；此前 worker 早死 → exit 断言快败）。
- 修法：不修（形态翻转，仍是红；队列口径不变）；真新 hang 仅 2 件
  （dns-channel-timeout/http-catch-uncaughtexception，其中 dns 系 FLAKY）。
- 复现：base15/base16 results.log `TIMEOUT∩DIFF` 交集脚本。
- 推广铁律：**全域基线对比先算"形态翻转矩阵"再算涨跌**——DIFF→TIMEOUT 不是回归，
  是修好的副作用；真回归只看绿→红。

### 4.271 测试函数名禁阶段前缀（2026-10-05）

- 症状：`tests/` 209 个 `fn phase10*/phase11*`——函数名编码计划切片
 （10a–10g/11），不说明行为；`cargo test` 输出满屏轮次号；新测试跟风加前缀。
- 根因：以计划编号当命名空间；轮次是过程元信息，不是行为归属。
- 修法：测试函数名一律域行为命名（`phase10f_buffer_parity_fixes`→
 `buffer_parity_fixes`，纯剥 `phaseXXy_` 前缀，零碰撞已验）；轮次只留注释与
  journal；`src/` 内 `覆盖测试` 指针对同步改名（UNSAFE-BOUNDARY 追溯不断）。
- 复现：`rg -n 'fn phase(10|11)' tests/ src/` 归零。
- 推广铁律：**标识符禁阶段命名**——变量/函数/文件名按域行为命名，
  P0/P1/phase10/phase11/W1 等计划号禁进标识符（注释与 journal 除外）。

### 4.272 阶段命名第二轮：里程碑与内嵌轮次（2026-10-05）

- 症状：4.271 只罩了 `phase10/11`——全仓扫出漏网 279（`phase1–9/9a–9m`、
 `phase_napi(_m1–m6)`、`p2_`、`phase_mapper_`、`phase_g92_`）+ 内嵌轮次 22
 （`stream_r1/r2/r3`、`crypto_round1/2/4`、`http_parity_round1`、
 `http_header_face_batch5`、`process_r9/r9b`、`child_g5×2`、`wcover_b1–b6`）。
- 根因：首轮正则只写了当下两切片；同类 token（里程碑 mN、轮次 rN/round、
 批次 bN/batch、组 gN）同属过程元信息。
- 修法：前缀一律纯剥（`phase_napi_m1_values_matrix`→`napi_values_matrix`，
 里程碑号连带去）；内嵌 token 按体裁域行为名（`crypto_round2_parity`→
 `crypto_dh_rsa_keyobject_basics`，`wcover_b1/b2_errors_boundary` 按覆盖面
 拆 `wcover_assert/net_errors_boundary`）；`src/` 覆盖指针对同步
 （`phase_napi_m1_values` 残缺指针顺手补全 `_matrix`）。
- 复现：`rg -n -e 'fn (phase|p[0-9]_[a-z])' tests/ src/ benches/` 归零；
 穷举 `round|batch|_r[0-9]|_g[0-9]|_b[0-9]|_m[0-9]` 余者皆域语义
 （roundtrip 往返/Batch 批写/slice 切片，逐项 eyeball 确认）。
- 推广铁律：**改名先穷举 token 全族再动手**——首轮修的正则即 scope，
 动手前把同族变体（大小写/单复数/缩写）一次列全，禁"修一处漏一族"。

### 4.273 计划号藏身非测试标识符（2026-10-05）

- 症状：`mod c4x_tests`（模块名即计划切片，注释自证"见 plan c-4x"）、
 `const C4X_HEXJS`、`t2_/t4_handler_src`（套件分层编号，7 处调用）、
 `wsys_misc2_faces`（批次残留，6 处 UNSAFE-BOUNDARY 指针对）、
 `docs/dependencies3.md` 陈旧指针（`phase10f_` 前缀 + 错误路径双料过期）。
- 根因：前两轮只扫了 `fn <test>`——mod/const/helper/文档指针同属标识符，
 且计划号会藏进"看起来像域缩写"里（c4x/tN/misc2）。
- 修法：`c4x_tests`→`regression_vector_tests`、`C4X_HEXJS`→`SUBTLE_HEX_JS`、
 `subtle_c4x_*`→`subtle_*`（aes192 按体裁 `subtle_aes_gcm_192`）、
 `t2/t4`→`dyn_echo/upgrade_echo_handler_src`、`misc2`→`misc_util`、
 采购记录指针同步现名现路径；注释内"已收官/c4x 系"字样留作历史。
- 复现：`rg -n -e 'c4x|C4X|t[24]_handler_src|misc2_faces' tests/ src/` 归零
 （注释史除外）；`mod/const/helper` 同查。
- 推广铁律：**标识符审计按"种类 × token 族"双轴**——种类（fn/mod/const/
 static/helper/文件名/文档指针）× token 族（phase/m/r/b/g/tier/misc +
 数字），单轴扫必漏。

### 4.274 一口气查完：门禁脚本 + 碰撞原则（2026-10-05）

- 症状：命名清理连开四轮（4.271/4.272/4.273 追补 + fixture/标签），每轮都
 "以为完了"——分轮扫 = 把 scope 拆小，每轮正则即当轮 scope 的天花板。
- 根因：无统一检查清单 + 无回归门禁；另有两类易误判：① 与历史计划 token
 碰撞的才算污染（r2/r4/r5/r6、p1-p9、b1-b8、m0、t1/t2、j2-j4、
 batch5、g5）；② 纯局部序号与域语义永远放过（rq2/s2/cli2/res2/e1、
 方向缩写 w2w/t2w/f2w、算法名 x448/sha512/chacha20、协议版本 h2/v1、
 类型名 u8/utf8、argv0、fd 号 ws99）。
- 修法：`scripts/check-naming.py` 一次覆盖五轴（标识符声明位/fixture 名/
 console 标签/环境变量键/非历史文档指针）+ 白名单注明每条放过理由；
 `r4/r5/r6→ex-/rs-/ps-`、`m0→ok`、`t1/t2→early/late-ok`、`b1-b8→cl-`、
 `p1-p13/timeout-p1-p4→裸描述`、`j2/j3/j4→join-auth/nohost/join-wire`、
 `batch5-done→heads-done`、`g5-validators→validators`、
 `WINTERJS2_T4→WINTERJS2_ENV_PROBE`、`v0-ok→ok`、`latin1-1→latin1-single`。
- 复现：`python3 scripts/check-naming.py` 归零（exit 0）。
- 推广铁律：**"查完"以门禁脚本绿为准，不以人眼为准**——新命名规则落地
 必须配守门脚本进提交前检查；误报一律写进脚本白名单并注理由，禁口头豁免。

### 4.275 同仓并发构建的负载抖动：时序测试 TRY1 败 TRY2 过（2026-10-05）

- 症状：`worker_terminate_interrupt_busy_loop` 在 strict 全量里挂，单测连跑
  11 次 TRY1+TRY2 双败；与此同时手工探针同脚本 rc=0 全行齐。曾误判为本次
  Utf8Stream 改动引入（stash 干净 HEAD 单跑过、二分三轮锁定"真凶"到
  errors.rs 一词之差——荒谬信号）。
- 根因：另一会话正在同仓库 `cargo build`（`ps` 见 rustc 39.5% CPU，
  load 4.71，共享 `target/`）。terminate-interrupt 是刀锋竞态，
  高负载下 50ms 窗口落地即翻；负载回落后 TRY2 即过（nextest 判 FLAKY）。
  所谓"二分锁定一词之差"是负载时间相关，不是因果。
- 修法：时序测试红先看 `uptime` + `ps` 查同仓并发构建（sweep 看门狗只杀
  自家进程组，不管别家 cargo）；flake-classify 走 TRY1/TRY2 分布判，
  双败多次 + 手工过 = 先查负载再二分。
- 复现：`cargo nextest run -E 'test(worker_terminate_interrupt_busy_loop)'`
  高负载下 TRY1 败率高，空载即稳过。
- 推广铁律：**"确定性回归"先证伪环境**——手工过 + TRY2 偶过即停手查负载，
  禁拿着 stash 二分追到荒谬单因；同仓多会话并行时构建错峰或分 target。

### 4.276 fs 回调 IO 改同步执行：线程池 FIFO 的单线程最近似（2026-10-05）

- 症状：Utf8Stream 落地后 `flush-sync`/`destroy` 套件红——`flushSync()` 后
  同步 `readFileSync` 读到旧内容；`destroy()` 后异步 `readFile` 读空。
- 根因：本仓 `fs.read/write` 把 IO 推迟到微任务（`Promise.resolve().then`），
  而 node 是线程池并发提交——"写已在飞行中"，后续同步 IO 读得到。
  Utf8Stream 的 `flushSync`/`destroy` 语义依赖"写与后续同步 IO 的落盘序"。
- 修法：`read/write/readv/writev` 四族统一"写盘同步执行、回调仍经
  `__fsDefer`（setImmediate）派发"。真机实测 `interleave.js` 5/5 确定性
  `he`（后发写不越过挂起读）后黑盒旧断言 `fd-read null 2 he` 原样成立，
  零断言改动（§4.65：断言对了，实现向真机对齐）。
- 复现：`tests/node/fs/streams.rs::fs_utf8stream_surface`（修前 u1/u3 块红）；
  node 套件 `test-fastutf8stream-flush-sync.js`/`-destroy.js`。
- 推广铁律：**回调 IO 的"执行时"与"通知时"解耦**——node 线程池语义下，
  同步执行 + 异步通知是单线程运行时的标准近似；混用"推迟执行"会撕裂
  跨 API 的落盘序，症状总在别家套件（流/关闭时序）爆发。

### 4.277 逐字移植三件小坑：变体类 / 探针互踩（2026-10-05）

- 症状一：`new Utf8Stream({minLength:999, maxWrite:8})` 抛 `TypeError` 无 code，
  真机是 `RangeError ERR_INVALID_ARG_VALUE`。
- 根因一：本仓 `E('ERR_INVALID_ARG_VALUE', …, TypeError, HideStackFramesError)`
  移植时丢了 node 原文的 `RangeError` 变体（node-errors.js:1482
  `}, TypeError, RangeError, HideStackFramesError);`）。
- 修法一：补回变体（`makeNodeErrorWithCode(RangeError, sym)` 通道现成，
  R2-iter 已铺）。教训：**E() 注册行逐字对原文**，变体表是语义不是装饰。
- 症状二：探针显示 Utf8Stream "写翻倍"（54 字节 vs 真机 27）。
- 根因二：node 与 wjs2 探针共用同一 dest 文件 + append 缺省——第二跑追加
  成第一跑的两倍。自摆乌龙，非实现 bug（换新文件名即一致，连 flush+end
  竞争的翻倍 + EBADF 都与真机逐字节同形）。
- 修法二：append 模式探针一律新文件名（`Date.now()`/计数后缀）。
- 复现：症状一 `node -e` 一行；症状二 `/tmp/fstest/u1*.cjs`。
- 推广铁律：**"实现 bug"先证伪探针**——append/覆盖/共享路径三问走完再进源码。
