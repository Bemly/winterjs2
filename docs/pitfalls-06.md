# 踩坑分卷6（4.265–4.274）

> 本卷为 `docs/pitfalls.md`（主索引）分卷之一，只收正文；查阅先看主索引，编号 `§4.N` 全仓唯一。

### 4.265 process.exit 裸传当回调即 receiver 错位（2026-10-04，base16）

- 症状：cluster-net-listen（worker 内 `net.createServer().listen(process.exit)`）报 `TypeError: this.reallyExit is not a function`（base15 绿 → base16 红）。
- 根因：自家 `exit()` 方法体走 `this.reallyExit`；裸传后 this=server。真机 process.exit 与 receiver 无关（C++ 绑定）；此前绿因 worker 内 server 根本起不来（R 前 listen 即死 → 进程正常退出），stream R 修好 server 后现形。
- 修法：`exit()` 内 receiver 守卫（有 reallyExit 用 this，否则回落 `globalThis.process`；§4.97 二选一；mock reallyExit 照常走同对象）。
- 复现：`http.Server.listen(process.exit)` 最小形；黑盒 `p2_process_exit_detached_receiver`。
- 推广铁律：**"修好 A 即现形 B"是常态**——base 轮后转红件先问"是不是之前死太早"。

### 4.266 throwDeprecation 同步抛是伪语义：真机 nextTick 异步走 uncaught（2026-10-04，base16）

- 症状：process-warning test4 `assert.fail('Unreachable')` 被触发。
- 根因：自家 `throwDeprecation` 分支同步 `throw`；真机 `node -e` 实测无同步抛，警告经 nextTick 异步抛 → uncaughtException 交付。
- 修法：改 nextTick 异步抛（`this.nextTick`，§4.97 this 基）。
- 复现：`node -e` 三行探针；黑盒 `p2_process_warning_throw_deprecation_async`。
- 推广铁律：**"抛"有同步/异步两种，真机探针先定是哪种**（uncaught 类面默认疑异步）。

### 4.267 注释写的"真机实测"与套件矛盾时以套件为准（2026-10-04，base16）

- 症状：crypto-keygen-eddsa（`generateKeyPair('ed25519', cb)` 无 options）base15 绿 → base16 红；代码注释称"真机二参即抛"。
- 根因：R4 校验凭一次手误实测写死注释；套件本身在真机绿（`node -e` 三秒可证）。
- 修法：options 缺省即 `{}`；注释按 §4.65 翻转（黑盒 `p2_crypto_keygen_no_options`）。
- 复现：`node -e "require('crypto').generateKeyPair('ed25519',cb)"`。
- 推广铁律：**注释不是证据**——与套件/实测矛盾时注释是错的，先翻转注释再改码。

### 4.268 http2.connect 无视 lookup + 缺 promisify.custom（2026-10-04，base16）

- 症状：promisify-connect-error（自定义 lookup 回错）base15 绿 → base16 红，自家报 UNKNOWN DNS 文案（Rust 直拨无视 lookup）。
- 根因两件：① `__start` 直调 `__wjs2_h2_connect`，options.lookup 从未读；② 缺 node internal/http2/core.js 末尾的 `connect[promisify.custom]`（once error→reject），致 session error 无人接变 unhandled。
- 修法：`__start` 内 lookup 优先（错原样 error、成拨解析地址）+ 逐字补 promisify.custom（黑盒 `p2_http2_lookup_and_promisify_custom`）。
- 复现：套件本体即最小形。
- 推广铁律：**"直拨 native"即绕过 node 选项层**——凡 native 直拨面，逐项核对 options 透传表（lookup/signal/custom promisify 三件最易漏）。

### 4.269 TLS 服务端同字节双派发：单 st 双 __feed（2026-10-04，base16·记档未修）

- 症状：https 迭代四件（default-port/request-agent/url.parse-https.request/set-default-ca-precedence-empty）`ERR_HTTP_HEADERS_SENT`——同一请求 handler 进两次，第二次 writeHead 炸。
- 根因（定界）：connection=1、userland data=1，但 `__srvDataListener→__feed` 跑两次（第二次拼回已消费字节重解析；plain-http 正常，TLS 服务端独有；JS 层皆单投，疑 Rust `tls_listen` 同明文推两次）。
- 复现：`probe/dbg/p9-count.js`（3/3 稳定：nc=1 nr=2）。
- 推广铁律：**"userland 只见一次"≠"只派一次"**——复现探针同时数三层（connection/request/userland-data），差值即分层定界。
- 状态：未修（Rust 派发深水 + https/timeout 件另案），本轮记档。

### 4.270 修好即多活：DIFF 快败翻 TIMEOUT 挂死（2026-10-04，base16）

- 症状：base16 新增 TIMEOUT 107，其中 105 在 base15 是 DIFF（`wjs=1` 快败 → `wjs=142` 挂死），聚集 cluster×26/http2×40/tls×24。
- 根因：P2 把 server/socket 做"更对"（worker 内 http 可 listen、TLS 握手可过），套件多活到"等一个永不到的事件"（cluster 'listening' 中继本就未实现，见 cluster.rs 头注；此前 worker 早死 → exit 断言快败）。
- 修法：不修（形态翻转仍是红，队列口径不变）；真新 hang 仅 2 件（dns-channel-timeout/http-catch-uncaughtexception，其中 dns 系 FLAKY）。
- 复现：base15/base16 results.log `TIMEOUT∩DIFF` 交集脚本。
- 推广铁律：**全域基线对比先算"形态翻转矩阵"再算涨跌**——DIFF→TIMEOUT 不是回归，是修好的副作用；真回归只看绿→红。

### 4.271 测试函数名禁阶段前缀（2026-10-05）

- 症状：`tests/` 209 个 `fn phase10*/phase11*`——函数名编码计划切片（10a–10g/11），不说明行为；`cargo test` 输出满屏轮次号；新测试跟风加前缀。
- 根因：以计划编号当命名空间；轮次是过程元信息，不是行为归属。
- 修法：测试函数名一律域行为命名（`phase10f_buffer_parity_fixes`→`buffer_parity_fixes`，纯剥 `phaseXXy_` 前缀，零碰撞已验）；轮次只留注释与 journal；`src/` 内 `覆盖测试` 指针对同步改名（UNSAFE-BOUNDARY 追溯不断）。
- 复现：`rg -n 'fn phase(10|11)' tests/ src/` 归零。
- 推广铁律：**标识符禁阶段命名**——变量/函数/文件名按域行为命名，P0/P1/phase10/phase11/W1 等计划号禁进标识符（注释与 journal 除外）。

### 4.272 阶段命名第二轮：里程碑与内嵌轮次（2026-10-05）

- 症状：4.271 只罩了 `phase10/11`——全仓扫出漏网 279（`phase1–9/9a–9m`、`phase_napi(_m1–m6)`、`p2_`、`phase_mapper_`、`phase_g92_`）+ 内嵌轮次 22（`stream_r1/r2/r3`、`crypto_round1/2/4`、`http_parity_round1`、`http_header_face_batch5`、`process_r9/r9b`、`child_g5×2`、`wcover_b1–b6`）。
- 根因：首轮正则只写了当下两切片；同类 token（里程碑 mN、轮次 rN/round、批次 bN/batch、组 gN）同属过程元信息。
- 修法：前缀一律纯剥（`phase_napi_m1_values_matrix`→`napi_values_matrix`，里程碑号连带去）；内嵌 token 按体裁域行为名（`crypto_round2_parity`→`crypto_dh_rsa_keyobject_basics`，`wcover_b1/b2_errors_boundary` 按覆盖面拆 `wcover_assert/net_errors_boundary`）；`src/` 覆盖指针对同步（`phase_napi_m1_values` 残缺指针顺手补全 `_matrix`）。
- 复现：`rg -n -e 'fn (phase|p[0-9]_[a-z])' tests/ src/ benches/` 归零；穷举 `round|batch|_r[0-9]|_g[0-9]|_b[0-9]|_m[0-9]` 余者皆域语义（roundtrip 往返/Batch 批写/slice 切片，逐项 eyeball 确认）。
- 推广铁律：**改名先穷举 token 全族再动手**——首轮修的正则即 scope，动手前把同族变体（大小写/单复数/缩写）一次列全，禁"修一处漏一族"。

### 4.273 计划号藏身非测试标识符（2026-10-05）

- 症状：`mod c4x_tests`（模块名即计划切片，注释自证"见 plan c-4x"）、`const C4X_HEXJS`、`t2_/t4_handler_src`（套件分层编号，7 处调用）、`wsys_misc2_faces`（批次残留，6 处 UNSAFE-BOUNDARY 指针对）、`docs/dependencies3.md` 陈旧指针（`phase10f_` 前缀 + 错误路径双料过期）。
- 根因：前两轮只扫了 `fn <test>`——mod/const/helper/文档指针同属标识符，且计划号会藏进"看起来像域缩写"里（c4x/tN/misc2）。
- 修法：`c4x_tests`→`regression_vector_tests`、`C4X_HEXJS`→`SUBTLE_HEX_JS`、`subtle_c4x_*`→`subtle_*`（aes192 按体裁 `subtle_aes_gcm_192`）、`t2/t4`→`dyn_echo/upgrade_echo_handler_src`、`misc2`→`misc_util`、采购记录指针同步现名现路径；注释内"已收官/c4x 系"字样留作历史。
- 复现：`rg -n -e 'c4x|C4X|t[24]_handler_src|misc2_faces' tests/ src/` 归零（注释史除外）；`mod/const/helper` 同查。
- 推广铁律：**标识符审计按"种类 × token 族"双轴**——种类（fn/mod/const/static/helper/文件名/文档指针）× token 族（phase/m/r/b/g/tier/misc + 数字），单轴扫必漏。

### 4.274 一口气查完：门禁脚本 + 碰撞原则（2026-10-05）

- 症状：命名清理连开四轮（4.271/4.272/4.273 追补 + fixture/标签），每轮都"以为完了"——分轮扫 = 把 scope 拆小，每轮正则即当轮 scope 的天花板。
- 根因：无统一检查清单 + 无回归门禁；另有两类易误判：① 与历史计划 token 碰撞的才算污染（r2/r4/r5/r6、p1-p9、b1-b8、m0、t1/t2、j2-j4、batch5、g5）；② 纯局部序号与域语义永远放过（rq2/s2/cli2/res2/e1、方向缩写 w2w/t2w/f2w、算法名 x448/sha512/chacha20、协议版本 h2/v1、类型名 u8/utf8、argv0、fd 号 ws99）。
- 修法：`scripts/check-naming.py` 一次覆盖五轴（标识符声明位/fixture 名/console 标签/环境变量键/非历史文档指针）+ 白名单注明每条放过理由；`r4/r5/r6→ex-/rs-/ps-`、`m0→ok`、`t1/t2→early/late-ok`、`b1-b8→cl-`、`p1-p13/timeout-p1-p4→裸描述`、`j2/j3/j4→join-auth/nohost/join-wire`、`batch5-done→heads-done`、`g5-validators→validators`、`WINTERJS2_T4→WINTERJS2_ENV_PROBE`、`v0-ok→ok`、`latin1-1→latin1-single`。
- 复现：`python3 scripts/check-naming.py` 归零（exit 0）。
- 推广铁律：**"查完"以门禁脚本绿为准，不以人眼为准**——新命名规则落地必须配守门脚本进提交前检查；误报一律写进脚本白名单并注理由，禁口头豁免。

### 4.275 同仓并发构建的负载抖动：时序测试 TRY1 败 TRY2 过（2026-10-05）

- 症状：`worker_terminate_interrupt_busy_loop` 在 strict 全量里挂，单测连跑 11 次 TRY1+TRY2 双败；同期手工探针同脚本 rc=0 全行齐。曾误判为本次 Utf8Stream 改动引入（stash 干净 HEAD 单跑过、二分三轮锁定"真凶"到 errors.rs 一词之差）。
- 根因：另一会话正在同仓库 `cargo build`（`ps` 见 rustc 39.5% CPU，load 4.71，共享 `target/`）。terminate-interrupt 是刀锋竞态，高负载下 50ms 窗口落地即翻；负载回落后 TRY2 即过（nextest 判 FLAKY）。所谓"二分锁定一词之差"是负载时间相关，不是因果。
- 修法：时序测试红先看 `uptime` + `ps` 查同仓并发构建（sweep 看门狗只杀自家进程组，不管别家 cargo）；flake-classify 走 TRY1/TRY2 分布判，双败多次 + 手工过 = 先查负载再二分。
- 复现：`cargo nextest run -E 'test(worker_terminate_interrupt_busy_loop)'` 高负载下 TRY1 败率高，空载即稳过。
- 推广铁律：**"确定性回归"先证伪环境**——手工过 + TRY2 偶过即停手查负载，禁拿着 stash 二分追到荒谬单因；同仓多会话并行时构建错峰或分 target。

### 4.276 fs 回调 IO 改同步执行：线程池 FIFO 的单线程最近似（2026-10-05）

- 症状：Utf8Stream 落地后 `flush-sync`/`destroy` 套件红——`flushSync()` 后同步 `readFileSync` 读到旧内容；`destroy()` 后异步 `readFile` 读空。
- 根因：本仓 `fs.read/write` 把 IO 推迟到微任务（`Promise.resolve().then`），而 node 是线程池并发提交——"写已在飞行中"，后续同步 IO 读得到。Utf8Stream 的 `flushSync`/`destroy` 语义依赖"写与后续同步 IO 的落盘序"。
- 修法：`read/write/readv/writev` 四族统一"写盘同步执行、回调仍经 `__fsDefer`（setImmediate）派发"。真机实测 `interleave.js` 5/5 确定性 `he`（后发写不越过挂起读）后黑盒旧断言 `fd-read null 2 he` 原样成立，零断言改动（§4.65：断言对了，实现向真机对齐）。
- 复现：`tests/node/fs/streams.rs::fs_utf8stream_surface`（修前 u1/u3 块红）；node 套件 `test-fastutf8stream-flush-sync.js`/`-destroy.js`。
- 推广铁律：**回调 IO 的"执行时"与"通知时"解耦**——node 线程池语义下，同步执行 + 异步通知是单线程运行时的标准近似；混用"推迟执行"会撕裂跨 API 的落盘序，症状总在别家套件（流/关闭时序）爆发。

### 4.277 逐字移植三件小坑：变体类 / 探针互踩（2026-10-05）

- 症状一：`new Utf8Stream({minLength:999, maxWrite:8})` 抛 `TypeError` 无 code，真机是 `RangeError ERR_INVALID_ARG_VALUE`。
- 根因一：本仓 `E('ERR_INVALID_ARG_VALUE', …, TypeError, HideStackFramesError)` 移植时丢了 node 原文的 `RangeError` 变体（node-errors.js:1482 `}, TypeError, RangeError, HideStackFramesError);`）。
- 修法一：补回变体（`makeNodeErrorWithCode(RangeError, sym)` 通道现成，R2-iter 已铺）。教训：**E() 注册行逐字对原文**，变体表是语义不是装饰。
- 症状二：探针显示 Utf8Stream "写翻倍"（54 字节 vs 真机 27）。
- 根因二：node 与 wjs2 探针共用同一 dest 文件 + append 缺省——第二跑追加成第一跑的两倍。自摆乌龙，非实现 bug（换新文件名即一致，连 flush+end 竞争的翻倍 + EBADF 都与真机逐字节同形）。
- 修法二：append 模式探针一律新文件名（`Date.now()`/计数后缀）。
- 复现：症状一 `node -e` 一行；症状二 `/tmp/fstest/u1*.cjs`。
- 推广铁律：**"实现 bug"先证伪探针**——append/覆盖/共享路径三问走完再进源码。

### 4.278 cluster 断开-起服竞速：同步旗才栏得住（2026-10-19）

- 症状：`test-cluster-disconnect-leak/race/shared-leak` 挂死——primary
  fork 后即 disconnect，worker 用户代码尚未跑（`await import` 让出点）。
- 根因：ctl 在 bootstrap 期到达，`CloseServers` 空跑（server 还没 listen），
  派发旗 `__clusterGone` 置位在 microtask，用户 `listen` 插空起服，无人再关。
- 修法：守门认同步旗 `process.connected`（`DoDisconnect` 内同步置 false，
  插空无门）+ 未起服也一律调 `close`（bind 窗口压住 listening 派发）+
  关完 server 才走后续（`checkWaitingCount` 口径，wait-server-close 套件要求
  disconnect 处理时 server 已关）。复现：`leakrep.cjs`（先 disconnect 后 listen）。
- 推广铁律：**"先到先得"的状态必须同步置位**——microtask 设旗栏不住同 tick
  用户代码；竞速守门一律认同步量（connected/closed 旗），派发可异步。

### 4.279 线程续命丢 exit + 先关 port 丢 ack（2026-10-19）

- 症状一：`worker.disconnect` 后主进程先退，`exit` 事件丢失
 （`test-cluster-worker-disconnect` 的 exit mustCall 挂 0）。
- 根因一：worker 线程 `unref` 即不再续命主循环——node 子进程靠进程句柄
  续命到退出，线程 unref 无对等物。
- 修法一：主进程恒等 worker 退出（server 在 disconnect 时同步关，
  无柄即退；不断即等，真机同）。教训：**跨进程语义往线程底座搬时，
  "谁续命谁"必须逐项核对**，unref 只用于明确抛弃的句柄。
- 症状二：worker 主动 disconnect 的本地派发永不到（race 套件挂死）。
- 根因二：`parentPort.close()` 与 primary 回执竞速——port 早关，ack 收不到。
- 修法二：ack 到达后才关 port + 100ms 超时兜底（幂等）。复现：`race2.cjs`
 （nextTick 先于 disconnect  fired，且 fired 必达）。
- 推广铁律：**双向通道"先关后收"必丢信**——关的一方永远是收完的一方；
  超时兜底 + 幂等派发是标准件。

### 4.280 close 的 Rust 侧 uncount 必须与 'close' 发射同 microtask（2026-10-20，W1）

- 症状：MessagePort `close()` 后进程 139（等长改名对照：26 字符文件名必崩、短名过——初判堆布局问题）。
- 根因：先 Rust uncount（`port_close`）再异步发射 'close'，事件循环在发射前排空进入 teardown，悬空发射触碰已销毁状态即 SEGV（§4.8 家族）。
- 修法：`__wjs2_port_close` 随发射同 microtask（finally 保底）；JS 可见态（`__closed`/清队列）仍同步。
- 复现：全量 message-port 套件（单块二分全过、整文件 139）。
- 推广铁律：**续命账与事件发射必须同节拍**——凡先减数后异步通知的，一律合并到同一微任务。

### 4.281 `Route::Local` 跨会话 ID 碰撞误投（2026-10-20，W1 channel 块 3）

- 症状：跨线程转移端口后回包打转（worker 发回 main 的消息回环到 worker 自身）。
- 根因：各会话 `worker_next_id` 各自计数，main 的 peer id 极易撞上本会话本地端口 id；Local 捷径只验 `q.id == p.peer` 即命中冒名者。
- 修法：加互惠校验 `q.peer == p.id`（真本地对恒互指）。
- 复现：message-channel 块 3（eval worker 跨线程端口）。
- 推广铁律：**跨会话寻址禁裸 id 匹配**——凡进程级 id 落到会话级表，必须互惠/归属双校验。

### 4.282 转移后源端 neutered，再校验即误判 detached（2026-10-20，W1）

- 症状：自杀式投递判定恒 false（peer-check 明明 true）。
- 根因：判定跑在 `__toWire` 之后——转移已 offer/neuter，复检清单即撞 "already detached" 被 catch 吞掉。
- 修法：判定必须在转移前（resolve→判定→组装单次，生成器禁二次消费见 4.283）。
- 推广铁律：**校验与变更之间禁插状态依赖**——判定用转移前的快照，变更后不再复检。

### 4.283 生成器 transfer 清单禁二次消费（2026-10-20，W1）

- 症状：`postMessage({ab}, (function*(){yield ab})())` 后 `ab.byteLength` 未 detach。
- 根因：判定与组装各 resolve 一次，生成器第二次 spread 为空。
- 修法：单次 `__resolveTransfer`，判定与组装共享清单（`__toWireList`）。
- 推广铁律：**一次性可迭代对象单次归一**——归一结果向下游传，不向上传。

### 4.284 视图/SAB 在 transfer 清单中恒非法（2026-10-20，W1）

- 症状：自实现允许视图 transfer；真机一律 `Found invalid value in transferList.`。
- 根因：臆测语义——node transferable 只有 ArrayBuffer/MessagePort（视图传 `[v.buffer]`）。
- 修法：视图/SAB/杂项条目一律该文案；重复 AB/端口各有专用文案。
- 推广铁律：**transfer 名单以真机为准**——先探 `[同值×2]/[视图]/[SAB]/[数字]` 五形再写分类。

### 4.285 真机 `close()` 语义三态（2026-10-20，W1）

- 症状：close suites 组合失败（self-close 双调 cb 响、对端关后 close 静默、混合态亦静默）。
- 根因：自关（cb 随事件）/对端关（`__peerClosed` 压倒一切，此后 close 全静默，连 cb 都不调）/混合（对端态赢）三态各异；`PortClose` 原只记账不透事件。
- 修法：`peer-close` 经 `__ev` 透出（幂等去重 + 异步发射，`__closeEmitted` 防双发）；worker 消亡走 `port_sweep_dead_routes` 传递闭包（转发器收件箱 closed + 本地 peer 链）。
- 推广铁律：**远端状态本地化必须显式建模**——peer-closed 不是 closed 的子集，单独成旗。

### 4.286 转移端口自匹配 `Route::Local` 成环（2026-10-20，worker-messaging lane）

- 症状：跨线程直投无限 pong。
- 根因：承接侧新端口的 peer id 恰等于自身 id（offer/accept 同值），互惠校验（4.281）误判为真本地对。
- 修法：Local 条件加 `q.via.is_none()`（accept 侧恒带 via，真本地对恒无 via）。
- 推广铁律：**互惠校验再加来源校验**——id 互指可能是自指，来源（via）才是身份。

### 4.287 孙线程发主误投父线程（2026-10-20，worker-messaging lane）

- 症状：grandchild→main 的消息落在 parent 收件箱。
- 根因：直投只认目标 inbox，不认派生链（node 系全线程直达主）。
- 修法：dest 标签 + 派生链上转（worker→worker 直投保留，主 0 号走 `worker_main_inbox`）。
- 推广铁律：**树形路由带 dest 标签**——中转节点只认标签不认亲缘。

### 4.288 `defineProperty` 部分描述符保留可枚举性（2026-10-20，whatwg lane）

- 症状：for-in 顺序/可枚举断言红——已删后重建仍不可枚举。
- 根因：`{...oldDesc, enumerable: true}` 写法只在记得传时才对；部分调用漏传即沿用旧值。
- 修法：凡重建必显式 `enumerable: false/true`，禁依赖展开继承。
- 推广铁律：**描述符重建逐项显式**——展开即默认沿用，审计时按"缺项即 bug"查。

### 4.289 跨 compartment 取 `this.href` 走原型链误品牌（2026-10-20，whatwg lane）

- 症状：`toString` 内读 `this.href` 未触发品牌门（undefined 接收者漏检）。
- 根因：访问器经原型链命中，`this` 合法即过，绕过自有槽检查。
- 修法：品牌门读内部槽（`__wjs2_urlBrandRecv`），不用派生属性。
- 推广铁律：**品牌检查认槽不认链**——凡 `Object.create` 派生可达的面（§4.23 家族），门必须直读内部槽。

### 4.290 promisify custom 改名 fallback 全局污染（2026-10-20，console lane）

- 症状：`promisify(stream.finished).name === 'eos'`，真机为 `'finished'`（test-util-promisify-custom-names.mjs 第 25 行断死）。
- 根因：util.rs 的 promisify 自创"回填原名"fallback（`fn.name !== original.name` 即改名）——node 原文是 custom 原样返回。fallback 直接改写**全局共享**的 custom 函数对象（`promises.finished` 被改名为 'eos'），且 eos→finished 正是 node 故意不同的别名（`Stream.finished = eos` + custom getter 回 `promises.finished`）。
- 修法：删 fallback；各域 custom 在定义处正名——fs.exists 命名函数表达式（内层经 `__existsOrig` 捕获引用，裸调会自递归）/ child exec 系 `defineProperty(wrapped,'name',orig.name)` / h2 connect `Object.defineProperty(function..., 'name', 'connect')`（仿 node `assignFunctionName`：只改名不引入绑定，命名函数表达式会遮蔽外层 connect 致自递归）/ crypto generateKeyPair 同式（防回退污染 keygen-promisify 套件）。
- 复现：`promisify(stream.finished).name`（修前 'eos'，真机 'finished'）；`promisify(finished) === promises.finished` 恒等。
- 推广铁律：**custom 原样返回，名在定义处正**——凡宿主改写用户可见共享函数对象（改名/挂符号）即全局污染；命名函数表达式先查内层同名裸调（自递归），用"改名不绑名"（defineProperty/assignFunctionName 同构）。

### 4.291 `node:` 内部真路径要显式别名 + 注册表长度断言并发 stale（2026-10-20，console lane）

- 症状：`require('internal/util/debuglog')`（`--expose-internals` 套件直引形）Cannot find module；本仓只有 `node:internal/debuglog`（自造短名）。
- 根因：`normalize_internal` 只认 INTERNALS 表项，node 真路径 `internal/util/debuglog` 无条目。另：INTERNALS.len 断言在 HEAD 已 stale（表实 82，断言写 80——多 lane 加表不 bump 数）。
- 修法：加 `("node:internal/util/debuglog", debuglog::SOURCE)` 同源别名（`node:inspector/promises` 同源先例）；len 按实测 83 落数并注记 stale 史。
- 推广铁律：**内部模块 node 真路径优先**——自造短名必须配真路径别名（套件直引用真路径）；**注册表长度断言是谁加表谁 bump**，并发 lane 下先按实测落数再注记。

### 4.292 sweep 基线构建只认干净树（2026-10-20，sweep lane）

- 症状：base19 sweep fs 37 连红（`mode received function`/`invalid flag`），对
  file-history（base18→db1152a 仅 811936c 动 fs 且只碰 URL 分支）完全无解，
  险些误判为已合入回归让无辜 lane 背锅。
- 根因：`cargo build` 编译的是**工作区**不是 HEAD；多 lane 并行时工作区常脏，
  sweep-owner 未查 `git status` 即构建，把他 lane 未提交的 open 重写
  （事后合入 22f2db2）bake 进基线二进制。取证：sweep-time 堆栈
  `at open (node:fs:3048:22/3049:26)` + `m is not defined @ node:fs:3064:3`，
  bundle 行号 2658（fs_base943+streams591+sync9c436+fdsync688）+406
  （`void m;` 在 fs_async 行）=3064 精确命中新代码；旧包同行是 `read()` 体，
  不可能报 `open` 帧——故旧二进制已含重写代码。
- 修法：sweep-owner 构建前必 `git status --short`，非空即停（多 lane 下
  stash 风险高，首选等干净窗口/让各 lane 先 commit+push）。
- 复现：base19（旧二进制脏树构建；journal base19 节 + §0.3 mixed-binary 注）。
- 推广铁律：**基线构建只认干净树**——status 非空不构建、不起 sweep；
  混合二进制基线必须落盘标注切换点，冻结后重跑纯净版。
### 4.293 setImmediate 紧循环饿死 I/O 相（2026-10-20，http2 FIFO 轮）
- 症状：FIFO open 轮询（ENXIO→setImmediate 重试）挂起后，server.listen 回调、`connection` 事件永不到；改 setTimeout(...,1) 即通。
- 根因：本仓事件循环中 check 相紧递归挤占 poll 相（node 同理；nextTick 系更甚）。
- 修法：一切"轮询等待外部进展"的循环一律 setTimeout(≥1ms) 节流；setImmediate 只用于一次性递延。
- 复现：t_srv3（writer open 轮询 + h2 listen 同进程）vs t_srv5（setTimeout 版即通）。
- 推广铁律：**轮询必须带时钟**——无时钟的自驱循环（immediate/microtask）即潜在饿死，先查相位再查逻辑。

### 4.294 FIFO 阻塞 open 卡死 JS 线程（2026-10-20，同轮）
- 症状：with-pipe/error-pipe-offset 双 TIMEOUT，且死在套件 load 期（writer 先行 open）。
- 根因：本仓 fs.open 回调面同步执行（syncFn 立即执行）；FIFO 写端 open 在无读者时 OS 级阻塞，node 靠线程池消化。
- 修法：statSync 先行嗅探（不阻塞）→FIFO 单独 O_NONBLOCK open + 轮询 rendezvous；ENXIO/EAGAIN 重试、余错正常回调；常规路径零改。
- 复现：mkfifo + fs.open(pipe,'w',cb) 无读者即挂。
- 推广铁律：**阻塞式 open 永不进同步底座**——先 stat 嗅探类型，特殊文件走非阻塞重试。（AGENTS §4 候选）

### 4.295 O_TRUNC 开 FIFO 即 EINVAL（2026-10-20，同轮）
- 症状：nb-open 写端 EINVAL 非 ENXIO，轮询永不命中。
- 根因：__fsFlags("w") 含 truncate；O_TRUNC 在 FIFO 上非法。
- 修法：nb-open 只取纯方向位（read/write/append），剥离 truncate/create。
- 推广铁律：**特殊文件 open flags 最小化**——方向位外逐位论证。

### 4.296 POSIX 无写者读端得 0（2026-10-20，同轮）
- 症状：with-pipe 体空（首读 0 被当 EOF 早关，写者随后永 ENXIO）；error-pipe-offset linger 空转。
- 根因：FIFO 无写者在场时读得 0（非 EAGAIN）；EAGAIN 仅"有写者无数据"。
- 修法：游标流/ linger 首字节前的 0 视为可重试（seen 位），见首字节后方为真 EOF；收尾关按 seen/常规分流（直关 vs linger）。
- 记档偏离：空写者（开即关、无写入）无法区分——与"写者永不到"同构永活（node 系 EOF）。
- 推广铁律：**0 不等于 EOF**——流式读的终止条件必须带状态（seen/位置/类型），裸 `r<=0` 即收尾是错的。

### 4.297 node respondWithFile 自动 content-length（2026-10-20，同轮）
- 症状：range 套件 `null !== 3`。
- 根因：node doSendFileFD 对常规文件恒覆写 content-length（min(size-offset,length)），旧移植靠 statCheck 手填。
- 修法：isFile 即覆写（用户值亦覆，node 同）。
- 推广铁律：**移植核对"对方替用户填了什么"**——不只看体，还要看自动头/默认值。

### 4.298 h2 'connection' 手动 emit 无监听即 TIMEOUT（2026-10-20，socket-close）
- 症状：test-http2-socket-close 静默 TIMEOUT；debug 日志 rustls client hs 发出后服务端零响应。
- 根因：Http2Server 无 connectionListener（node 靠它做服务端 TLS 握手+建会话；secure 种 listener 挂 secureConnection，手动 emit('connection') 重进 net 层握手链）。
- 修法：adopt 通道桥（AdoptIo：读走 inbox channel、写走 H2Stream/adoptData 事件；server_id 取 conn_id 独占路由；复用 serve_conn）；'connection' 常驻监听（native listen 不经此事件）；connClose 连带 destroy 注入 socket（双向拆链皆洁）。
- 附带坑：Rust payload 漏 connId 前缀致 NaN 路由丢弃（inbound 通、outbound 断；h2c 隔离二分）。
- 推广铁律：**emit 即契约**——手动 emit 的事件必须有内部监听承接；光发不接即挂死。（AGENTS §4 候选）

### 4.299 response 'finish' 在流关闭时（2026-10-20，serverresponse-finished）
- 症状：`response.socket !== undefined` 在 finish 内（得 {}）。
- 根因：node onStreamCloseResponse 在流关闭时置 closed 并（可写已终则）发 finish；旧移植在流 finish 即转发，早了一拍。
- 修法：流 finish 只记账，response finish 延至流 close（writableFinished 前提），socket 门加 __resClosed；destroy 路径即时 finish 不动。
- 推广铁律：**终结事件看关闭不看完成**——finish/close 的派发点与可见性门以 node 状态机为准，不以宿主流事件时间为准。
### 4.300 超时孤儿排空原地 rearm 全杀兄弟 job（2026-10-20，vm lane）

- 症状：`vm/module.evaluate({timeout})` 与其它未决微任务配对即挂——兄弟 promise 永不落定（test-vm-module-basic test1 配 timeout 块现形；单跑各自绿）。
- 根因：超时命中后的孤儿排空沿用旧 `rearm`（过期表项仍在，中断回调恒 false）+ 同轮 `RunJobs`——排空中每个 job 都被中止，兄弟有限 job 连带陪葬（`run_jobs` glue 逐个出队，中断把整轮清空）。
- 修法：摘原表项 → 以短预算（1000ms）重武装 → 排空 → 清标记 → 恢复原命中毫秒供文案。有限 job 照常结算，无限逃逸 loop 到期中止。
- 复现：`pair2`（test1 同步模块 + `while(true)` 500ms 超时模块并跑；修前 T1 永挂，修后双绿）。
- 推广铁律：**超时排空必须限时武装**——原地 rearm 等于全杀；凡"杀孤儿"逻辑先问"兄弟 job 何去何从"。

### 4.301 跨 compartment 恒等必须统一包装（2026-10-20，vm lane）

- 症状：`await import()` 结果与 `mod.namespace` 同引擎对象、`===` 却 false；`e !== __err`（明明同一 rejection reason）。
- 根因：引擎 promise 通路自动 CCW 包装，被测的另一侧（native 直返命名空间/syncIn 写 sandbox）是裸指针——同一目标两种身份。
- 修法：新增 `jsapi_glue::wrap_value`（`JS_WrapValue` 进调用方 compartment，同域无操作），统一包 `vm_mod_ns`/`vm_set`/`vm_get`返回值；global 免包装（身份基础设施，见下）。
- 复现：`zz-ident3`（同对象直读 vs await，修前 false/false，修后缓存统一 true）。
- 推广铁律：**跨域值出边界一律包装**——裸指针跨域即埋恒等雷；包装走同一缓存才恒等。

### 4.302 global 免包装（身份基础设施不进 CCW）（2026-10-20，vm lane）

- 症状：4.301 落地后 `vm_dont_contextify` 翻红（`runInContext("this", w) === w` false）。
- 根因：`w`（DONT_CONTEXTIFY 的 global 本体）与 `this` 求值结果，一侧 CCW 一侧裸指针——global 身份被包装破坏（bookkeeping/自指比较全认裸指针）。
- 修法：`vm_wrap_rval`/`vm_set`/`vm_get` 凡值是已知 global（`vm_global_all`）即直通不包装。
- 推广铁律：**身份基础设施永不包装**——global 这类被到处 `===` 的值，包装即分裂身份；包装只给普通用户值。

### 4.303 `Rooted<Vec<T>>::as_ptr` 拿到的是 Vec 头（2026-10-20，vm lane）

- 症状：`process.listenerCount('unhandledRejection')` 恒返回 0（监听明明存在），致事件永不投递。
- 根因：`rooted!(&in(cx) let argv_root: Vec<JSVal> = ...)` 后 `argv_root.as_ptr()`——`RootedGuard<Vec>` 解引用到 Vec，`as_ptr` 是 Vec 头地址，不是元素区；`HandleValueArray` 指向 Vec 头三字，读到垃圾参数。
- 修法：改 `mozjs::rooted_vec!` + `extend`（元素区连续 rooted，`as_ptr` 即元素首地址）；调用后不再变长。
- 复现：`unh2.mjs`（修前监听恒 0，修后事件双达）。
- 推广铁律：**动态实参走 RootedVec**——`Vec` 的 `as_ptr` 与"元素数组"只在解引用后成立，`RootedGuard<Vec>` 上直接取是取错层。

### 4.304 引擎 cast 不做类型守卫（`GetModuleObject` 等见非目标即读垃圾）（2026-10-20，vm lane）

- 症状：Script 动态导入直接 SEGV（`GetModuleObject` 对非模块脚本）；`GetModuleForNamespace` 对普通对象返回"命名空间"（恒等误判）。
- 根因：这类 API 内部 `as<ModuleObject>()` 无类型检查（release 无断言），错配即读垃圾/野指针。
- 修法：referrer 匹配改编译期定址脚本指针比对（见 4.305）；命名空间判定改逐记录 `GetModuleNamespace` 取值比对（真记录上调用恒安全）。
- 复现：`dbg4.mjs`（Script+回调，修前 139，修后 `ns? true`）。
- 推广铁律：**引擎 cast 前先验类型**——凡 `as<T>` 无检查的 API，只喂已验证的真对象；referrer 这类外部输入一律走自建索引，不直喂。

### 4.305 引擎清模块 ScriptSlot（求值后 referrer 脚本取不到）（2026-10-20，vm lane）

- 症状：已求值模块的动态导入 referrer 匹配 SEGV（`GetModuleScript` 触发 `MOZ_RELEASE_ASSERT`）。
- 根因：`ModuleObject::onTopLevelEvaluationFinished` 清 ScriptSlot（debugger 用后即焚，源码有注）；求值后 `GetModuleScript` 必崩。
- 修法：`VmMod.script` 编译期定址（`Box<Heap<JSScript>>` 保活，trace 同步；hook 只比地址不解引用，恒安全）。
- 推广铁律：**引擎的"用后即焚"槽位不能做索引**——凡引擎文档注明清理的槽，宿主侧编译期自持根。

### 4.306 动态 import hook 行列恒 0（2026-10-20，vm lane）

- 症状：`with` 属性回调恒收 `{}`（oxc 行列定位永不命中）。
- 根因：`HostLoadImportedModule` 的动态分支行列参数恒 0（引擎未填），与静态路径不同。
- 修法：改按运行时 specifier 匹配源码——同字面量首命中，否则源码仅一处动态 import 即兜底，否则空；`line_col_offset` 删除。
- 推广铁律：**hook 参数先实测再用**——引擎传参在动/静路径可能完全不同（恒 0 即"未提供"），别按名猜语义。

### 4.307 vm 模块求值缺 sandbox 同步（2026-10-20，vm lane）

- 症状：`test-vm-module-basic` test1 `deepStrictEqual` 红（沙箱没回写）；基线同红（误记为 importModuleDynamically 单一阻塞）。
- 根因：`runIn*` 有 syncIn/syncOut 快照语义，`SourceTextModule.evaluate` 全无——模块写 global 后沙箱恒旧。快照模型下无活绑定，必须显式回写。
- 修法：`__doEvaluate` 内有 context 即 `__syncIn`（前）/`__syncOut`（落定 done/failed 双路；TLA 异步落定同序）。
- 复现：`zz-t1`（修前 `{"foo":"bar"}` 缺键，修后全等）。
- 推广铁律：**快照语义的域，执行入口与落定点必须配对同步**——只 sync 一端即漏；模块系与 run 系同构，别各写各的。

### 4.308 import `with` 非 `type` 键 SM 直接拒收（2026-10-20，vm lane）

- 症状：`import("foo", { with: { key: "value" } })` 在 vm Script 内直接 reject（`Unsupported import attribute`），回调永不触发。
- 根因：SM153 只认 `type` 键（`We only support "type"`），V8 视属性为不透明透传 linker。引擎在调用点求值期校验，hook 前即拒。
- 修法：编译前空格化剥离含非 `type` 键的 options 子句（换行保留，行列恒等；attrs 仍从原文静态提取）；纯 `type` 子句保留（JSON 引擎面）。
- 推广铁律：**引擎墙属性先剥离再编译**——宿主语义（V8 不透明）与引擎校验冲突时，编译文本按引擎口径裁，语义信息走 side-channel（此处置请求表/源码）保留。

### 4.309 prelude 块级 function 经 Annex B 漏全局（2026-10-20，vm lane）

- 症状：`test-vm-module-errors` 的 `delete globalThis.error` 抛 TypeError（全局多了个不可配置的 `error/map/mapDomain`）。
- 根因：punycode vendored 片段包在裸 `{}` 块里，sloppy 块级函数声明经 Annex B 建全局 var 绑定，且本仓预置全局不可配置。
- 修法：外层 `{}` 改 IIFE 包裹（算法不动，两行）。
- 推广铁律：**vendored 块级作用域一律 IIFE 化**——裸块挡不住 Annex B，全局命名空间只许 `__wjs2_` 与标准内建。

### 4.310 unhandledRejection 投递形状（2026-10-20，vm lane）

- 症状：vm/主域裸 reject 直 fatal，`process.on('unhandledRejection')` 永不触发（旧 report 只在尾收割字符串）。
- 根因：跟踪器只记 promise，reason 在收尾时才挂捕获反应取——监听事件无值可递，索性不递。
- 修法：`runtime/unhandled.rs`：落定时 `promise_settled_value` 直读 reason 原值；有监听即 `emit`（reason, promise）后清表续跑；无监听按 `--unhandled-rejections` 模式（throw/strict fatal；warn 警告+bounded 排空继续；none 静默；warn-with-error-code 警告+exit 1）。旧捕获 natives（on_fulfilled/on_rejected/rejection_reasons）删除。
- 缺口（promise 域另案）：warn 双警告（reason + 出处注记）/lic `rejectionHandled` 事件/node 式 `UnhandledPromiseRejection` fatal 形状未对齐。
- 推广铁律：**运行时先投递后定去留**——检查点只看"有没有"会误杀已监听；事件投递是检查点的前置动作，不是收尾动作。

### 4.311 Close 先于 End 进队即丢尾包（2026-10-20，http2 lane）

- 症状：socket-destroy-delayed（res.end 紧接 setImmediate 杀 socket）TIMEOUT；应答字节零落线。
- 根因：本仓 Duplex 将 `_final` 递延到 setImmediate 之后，Close 命令先于 H2RespondEnd 进队；旧 serve_conn 到 Close 即 break，尾包烂在通道里，优雅关超时后 FIN，客户端有应无收。
- 修法：Close 转优雅模式（GOAWAY 后主循环继续收应答命令，至 conn 结束或 5s deadline；None 通道关沿旧语义）。
- 复现：s1c（去 resume/去 end 版必挂）、s10（close 版亦挂）、s14/s15（延迟 teardown 即绿）。
- 推广铁律：**收尾命令与终结命令同道时，终结只转模式不退出**——优雅期必须继续排空，deadline 兜底。

### 4.312 conn 收尾与流任务唤醒竞态（2026-10-20，http2 lane）

- 症状：同上：send_request 已决议（GOTRESP）而驱动先退，response/data/end 因 H2SessionClose 的 purge 被丢弃，客户端有 end 无 response。
- 根因：驱动任务与流任务分属不同 tokio 任务，退出与唤醒无序；purge 一到，迟到事件全丢。
- 修法：conn 结束处先等流任务落定（AbortHandle→JoinHandle，2s 上限防真挂死），再发 aborted/close，保证"应答先于终结"。
- 复现：GOTRESP 在 CLI-DONE 之后（时序铁证）。
- 推广铁律：**退出前等消费者落定**——生产者退出与消费者唤醒之间必须有 join（带超时），否则 purge 即丢数。

### 4.313 hyper CONNECT-upgrade 接管双向隧道（2026-10-20，http2 lane）

- 症状：proxy-over-h2 / connect-method 隧道字节双向全丢（线字节取证：握手外无 DATA 帧）；外层 200 后自发空 DATA-EOS + RST(0)。
- 根因：hyper 1.x 对 CONNECT-2xx 自动升级为隧道：后续 DATA 只进 upgrade 句柄（常规 Incoming 恒空、body 通道收发皆死），应用须经 `hyper::upgrade::on` 泵接；旧移植全走常规通道，两端互等挂死。node（nghttp2）无此接管。
- 修法（另案设计，未落地）：服务端读 upgrade 句柄→体事件泵 + 抑制常规空泵 + 写侧改走 upgrade 句柄；客户端镜像；隧道生命周期与 h2 流事件解耦。约 200 行 + 新竞态，超 C 轮 time-box。
- 复现：px11（隧道 DATA 服务端无感）；px7/px12（200 后自发 EOS+RST）；node 全绿对照。
- 推广铁律：**底座升级语义先读源码**——轮子对特殊方法（CONNECT/Upgrade）常有接管行为，不读源码即误判为己方 bug。

### 4.314 kill -9 丢 println 缓冲（2026-10-20，http2 lane）

- 症状： contiguous 日志时序自相矛盾（End 已处理又未处理），误导根因定位 1 小时。
- 根因：`println!` 到文件是块缓冲，`kill -9` 丢弃未刷新的尾巴；`eprintln!` 无缓冲才保序。多线程下 eprintln 交错亦不可信，本轮单线程（LocalSet）才可用。
- 修法：调试日志一律 `eprintln!`；JS 侧 console.log/error 跨流不比序。
- 推广铁律：**取证先对齐时钟**——混合流日志先统一到无缓冲单流，否则时序全是幻觉。
### 4.315 hook 入口 realm 即被主 AutoRealm 覆盖（2026-10-20，vm lane）

- 症状：动态 import 上下文兜底恒落主 loader（incumbent 查得主 global，明明是 vm 上下文代码）。
- 根因：`load_hook` 入口先 `AutoRealm(主)` 再调判定——hook 进来的 cx 原在 referrer realm，一进主域 incumbent 即丢；后续 `CurrentGlobalOrNull` 读到的是主 global。
- 修法：hook 第一行先抓 `CurrentGlobalOrNull` 存参，判定用传入值（见 4.1 铁律的反例：守卫本身会改 realm）。
- 复现：`zz-ref.mjs`（修前 incumbent=main → 主 loader 拒收；修后 CTX-CB 触发恒等）。
- 推广铁律：**realm 敏感信息入口即取**——凡进函数先 `AutoRealm` 的协议代码，入参 realm/当前 global 等上下文快照必须在守卫前抓；守卫后读到的都是守卫的 realm。

### 4.316 记录操作必须进记录所属 compartment（2026-10-20，vm lane）

- 症状：上下文回调返回主域模块即 SEGV（`JS::ModuleLink (.cold.1)` 断言崩）。
- 根因：跨域回调返回异域记录，link/evaluate 仍在 referrer realm 执行——SM 内部按 compartment 断言，直接崩。
- 修法：`dyn_commit_record/ns` 内按 `record_ctx_of` 重进记录 realm（同域即无操作）；`payload` 跨域 `Resolve/RejectPromise` 由引擎包装，无碍。
- 推广铁律：**引擎对象操作认归属 realm**——凡跨 compartment 拿到的对象（回调返回/别域记录），操作前重进其所属 global 的 realm；只认"当前 realm 方便"即埋断言崩。

### 4.317 帮忙函数住主 global（2026-10-20，vm lane）

- 症状：跨域 async 动态导入帮忙 lookup 报缺失（helper 在票据 realm 查不到）。
- 根因：帮忙函数由 `vm_modules.js` 接线在主 global 上；票据 realm 为 vm 上下文时按票据域查找永 miss。
- 修法：`dyn_then_helper`/`dyn_commit_async` 一律进主 realm 查调（helper 归属域固定）。
- 推广铁律：**内部帮忙函数按归属域查找**——JS 接线的内部 helper 只存在于接线域；跨域调用先回接线域，别按"当前上下文"想当然。

### 4.318 oxc 箭头函数是独立 AST 节点（2026-10-20，vm lane）

- 症状：TLA 探测把 `const f = async () => await 1` 判为顶层 await。
- 根因：`visit_function` 不覆盖 `ArrowFunctionExpression`（oxc 里箭头是独立节点类型，有自己的 visit/walk）。
- 修法：加 `visit_arrow_function_expression` 记 span（函数体排除集）。
- 推广铁律：**oxc 函数族按节点类型全覆盖**——`Function`/`ArrowFunctionExpression`/`MethodDefinition` 各有 visit；只写 function 即漏箭头，单测必须含箭头形。

### 4.319 after-evaluate 跨上下文队列是引擎墙（2026-10-20，vm lane）

- 症状：`test-vm-module-after-evaluate` 要求 `await` 异域 promise 永挂（mustNotCall 永不触发），我方单共享队列恒决议 → 必红。
- 根因：V8 按上下文分微任务队列（决议挂 inner 队列等 checkpoint）；我方单 `JobQueue` 全局排空，无"某上下文 checkpoint"概念；真机 `microtaskMode` 即队列归属语义。
- 修法：书面偏离（引擎架构级；`createContext` 仍校验 `microtaskMode` 口径，单上下文流等价）。
- 推广铁律：**队列归属语义先认架构**——凡"某域 checkpoint 才推进"的测试，先看自家队列是共享还是分区；共享队列下挂起语义不可模拟，直接偏离别硬拗。

### 4.320 link 连带落定的依赖也要结算闭包（2026-10-20，vm lane）

- 症状：`hasAsyncGraph` 传递用例 `bar` 对、`foo` 错（foo 被 bar 的 instantiate 连带 link 但闭包未算）。
- 根因：闭包只给 entry 算了一次；seen 里被连带的依赖状态变了但派生值没跟。
- 修法：seen 落定循环里逐个结算（已 true 跳过；`__vmTlaOf` 递归 + seen 截断环）。
- 推广铁律：**落定循环里状态与派生同行**——凡 BFS 连带改状态的收尾，派生值（缓存/闭包/计数）必须同循环内逐个结算；只照顾 entry 即漏连带。

### 4.321 upgrade 句柄丢弃即隧道拆除（2026-10-20，http2 lane）

- 症状：CONNECT-200 后外层自发空 DATA-EOS + RST(0)，隧道字节双向全丢（proxy-over-h2 TIMEOUT）。
- 根因：hyper 在 extensions 里给应用留 upgrade 句柄（OnUpgrade/H2Upgraded）；service 析构 req 时连带丢弃 → `UpgradedSendStreamBridge::drop` 触发 `close_notify` → up_task 发空 EOS (+RST) 拆隧道。我方旧移植从不取句柄，每隧道必死。
- 修法：service 预取 `hyper::upgrade::on(&mut req)` 并持有；2xx 起双向中继泵（读→体事件，写←outbox）；句柄在 H2Respond-2xx 门控建链，非 2xx 空收尾。
- 复现：px12（无 emit 亦有 EOS+RST）；px11（泵起后隧道字节通）。
- 推广铁律：**轮子给的升级/接管句柄必须持有泵接**——带有效负载的 Future/IO 随容器析构即触发对端动作，丢弃≠无视。

### 4.322 EOS-before-200 CONNECT hyper 不升级（2026-10-20，http2 lane）

- 症状：connect-method 仍红（`'' !== 'hello'`），与 base19 同错；proxy 过而它不过。
- 根因：客户端 EOS 先于 200 到达时 hyper 不创建 upgrade 对（Pending 永决议），隧道字节困在 hyper 内部 RecvStream，无任何 API 可取（常规 Incoming 对 CONNECT 恒空）。属 hyper 模型局限，非我方状态机可补。
- 修法：无（本轮不动；候选是客户端 CONNECT end 抑 EOS，改线语义，另案评估）。
- 复现：px14（end('hello') 即挂）vs px11（只写不 end 即通）。
- 推广铁律：**半关与升级互斥时先认半关**——隧道类流的 EOS 时序决定底座走哪条管线，探针必须覆盖"先关后应答"序。

### 4.323 net socket 读嗅探三件（2026-10-21，http2 lane）

- 症状：autoselect-protocol TIMEOUT；`socket.read(3)` 恒 null、`readable` 永不发。
- 根因：net socket 读侧只有 data 事件投递，`read` 硬编码 null，无 readable 发射、无 unshift。
- 修法：`read(n)` 按需消费缓冲（不足 n 即有多少取多少，无即 null，与真机对过）；`unshift` 回填队首；到包且无 data 监听即微任务发 `readable`。`read()===null` 旧断言（空缓冲）不受影响。
- 复现：auto3（readable 不火）vs 真机（火）；multiple-client-error 同模式红。
- 推广铁律：**嗅探面三件不可缺一**——read 取、unshift 还、readable 告，任缺其一 sniff 即挂。

### 4.324 会话级协议错事件（2026-10-21，http2 lane）

- 症状：goaway-cleanup（超大 HEADERS）会话 error 永不发（旧 DIFF 两处挂）。
- 根因：serve_conn conn 收尾直接丢弃 hyper 错误（`let _ = r`），FRAME_SIZE 等协议错无会话事件。
- 修法：conn-done Err 且 h2 原因码属协议类（1/3/6/9）即发 `sessionError`（ERR_HTTP2_ERROR/Protocol error），干净收尾与传输错保持静默；JS 侧转会话 `error`，收尾照旧。套件 3/4 转绿，仅剩 raw-close。
- 复现：gw（真机 error+close，旧我方静默关）。
- 推广铁律：**协议错与传输错分流上报**——前者会话 error（应用可断言），后者静默收尾；一刀切丢错即丢可观测性。

### 4.325 CANCEL-RST 收尾剃刀竞态（2026-10-21，http2 lane）

- 症状：close-aborted-destroy 仅 rstCode 不符（`undefined !== 8`），其余全绿；真机在此件 hang（无真值）。
- 根因：服务端 RST 处理与客户端 FIN 在 loopback 上 µs 级竞速（Path A：RST 先到→error+close；Path B：FIN 先到→静默收尾）。RST/close 路径上任何新增工作（snoop 发事件、同步 native、defineProperty 懒取）都确定性翻转路径——已实证三版全翻。
- 修法：无（本轮止损；候选是先给 Path B 补齐 error+close 使双路径皆满足套件，再设码，另案）。
- 复现：A/B/A 对照（加码即翻、去码即回）；ab 系列复现（改 emit 监听即扰动）。
- 推广铁律：**剃刀竞态先加边距再加料**——事件/调用开销会翻转 µs 级确定性竞速；先让双路径各自正确，再补观测。
### 4.326 Cli 加字段必须配 en+zh 翻译（2026-10-21，child lane）

- 症状：`Cli` 加一个字段（`-e` 静默 eval）后全旗解析错乱（`--run X` 报
  lang 取值错、`--eval X` 落 run、`-e X` 进错位），无 panic、无报错，
  行为级静默错位；回退字段即好。
- 根因：`with_localized_args` 对每个有译文的 arg 做 `mut_arg`（remove +
  末尾重插）；有译文者相对顺序不变故无事，新字段无译文被跳过、
  留在原地——相对顺序一变，clap build 后内部索引错位，全旗串位。
  取证：bypass i18n 即全对；dummy 无关字段亦坏；补 en+zh key 即好。
- 修法：新增 Cli 字段必须同步 `locales/en.yml` + `locales/zh.yml`
  加 `arg.<id>.help/value`（与字段同 commit）。
- 复现：加字段不加翻译，`--run <file>` 即报 `--lang` 取值错。
- 推广铁律：**Cli 字段与翻译同提交**——i18n mut 环后相对顺序是解析正确
  的前置条件；未翻译字段即便显示正常也会毒化整表。

### 4.327 hyper CONNECT 体黑洞与隧道半关泵续命（2026-10-21，http2 lane）

- 症状：CONNECT 隧道上行字节三种死法：①随头体不上线（tap 零 DATA）；② 200 前
  EOS 即无升级（4.322）；③递延 END 后下行尾包丢失（patient 下游 hang）/ 泵长生
  致 proxy 套件进程永生。
- 根因：① hyper h2 客户端 `poll_pipe` 对 `is_connect` 不建 body 管道——请求体通道
  先天孤儿（读 hyper 源码 `send_request(req, !is_connect && eos)` 处确认）；
  ② 4.322 机制成立（EOS 先行即 Pending 永不决议）；③泵 `Out(None) 即 break` 在
  半关常态下早退（己方 END 消费后对端数据翌至）；④直改③则拆除级联断（4.321 依赖
  句柄丢弃→close_notify→对端 END——泵永生即级联永挂）。
- 修法：JS 全量抑发（HEADERS-only + 缓冲 + 应答后桥 flush/补 END）+ 隧道先决议再发
  响应事件 + 双泵半关续读（双侧皆关才退）+ 泵 JoinHandle 入表（reset/收尾 abort，
  正常半关泵自退不走此表）。退路：immediate-FIN 形结构性不可胜（DATA 只能 200 后
  发）→ 书面偏离，不硬拗（另见 4.319 判据）。
- 复现：px11（immediate-FIN `""` 快红）vs patient（双 OK）；proxylog 全过但进程
  永生（泵 linger 取证：cli outClosed 后读永 pending、srv 零变迁）。
- 推广铁律：**半关泵的生死两面**——写关续读保尾包，拆除靠显式 abort 保级联；
  只改一面即拆东墙补西墙，泵生命周期必须双表（自退 + 外杀）。
### 4.328 内嵌 JS 改后 bundle 解析挂先验二进制新鲜度（2026-10-21，console lane）

- 症状：改完多个内嵌 JS 后 `winterjs2 --eval '1+1'` 报 `node:crypto:5796: Expected '}' but found 'EOF'`；V8（`node --check`）逐文件+全 bundle 皆过，SpiderMonkey 拒。
- 根因：构建竞态 stale 二进制（cargo 读到正在落盘/指纹未更新的源拼出坏包），非语法问题；同内容重编即好，无需二分。
- 修法：JS 改动构建后先三验——①二进制 mtime 新于源；② `strings` 含本次标记串；③冒烟 1 条过；三者齐才可二分归因。
- 复现：本轮 crypto_sign.js 误判（逐文件回退"定位"到 helpers 段，实为多次重编中某次命中 stale，还原即自愈）。
- 推广铁律：**先证二进制新鲜，再二分源码**——凡"解析/加载类"报错且多文件参与拼接，第一步永远是 mtime + 内容标记 + 最小冒烟；跳过此步的二分结论不可信。

### 4.329 pump 整批排空 + 端口同步连发丢 once 后继（2026-10-21，vm lane）

- 症状：message-before-close（transfer 端口连发 two + close，双 once 收）在小轮次偶过、
  200 轮必挂（第二条 5s 超时）；同会话 transfer 200 轮全过、parentPort 30 轮全过。
- 根因：pump 逐轮 `while try_recv` 整批排空 worker inbox、无 microtask 间隙——两条
  PortMsg 同批到达后单次 queueMicrotask flush 同步连发两条；第一条触发 once 摘除，
  第二条射进零监听虚空。node 逐 task 派发、microtask 间隙重挂监听，故 10000 轮全过。
- 修法：`__portFlushQueue` 循环内复查 `listenerCount("message")`，零即停刷留队、
  新监听经 newListener 续刷（node paused-queue 语义）；`on` 常驻/close-while-receiving
  不受影响（close 不摘监听）。200 轮 0.7s 全过。
- 复现：once 竞速探针（5s 超时版首轮即丢）vs 修复后 30/30；`git show` 前必跑同构黑盒。
- 推广铁律：**整批派发配同步发射即埋 once 陷阱**——凡"排队+延迟刷"管线，发射循环内
  必须复查监听存活，无人即停，禁连发进虚空；修泵粒度不如修队列语义（影响面小）。

### 4.330 info 读取消费 pending，先读后分发即永假（2026-10-21，vm lane）

- 症状：worker 入口抛错 + uncaughtException 监听永不触发（WError + exit 1）；
  主线程经典入口正常（走 CJS 分支）。
- 根因：经典分支先 `error_info_from_exception_stack`（注释自称"消费异常值"）
  后 `dispatch_entry_throw`——info 读取取走 pending，分发 take 永空。
  主线程正常因真文件多走 cjs_interop→CJS 分支（先分发后读）；worker-eval
  伪文件名落经典分支。取证：分支入口 pending=true，分发点 false。
- 修法：info 成功且 pending 已空即 `set_pending_exception` 放回再分发
  （未取走重设等值无害；info 失败/throw undefined 不碰）。
- 复现：worker 入口 `process.on('uncaughtException', noop); throw`（修前
  WError+1，修后 exit 0）。
- 推广铁律：**先分发后取证**——任何"读错 + 路由"两段式，先路由（take），
  后取证；取证函数一律视为消费 pending，复用前先放回。

### 4.331 worker fatal 必须先派发会话内 exit 再定码（2026-10-21，vm lane）

- 症状：exit-code 矩阵 6/7 格（exit 监听改码）恒 1；且 error 事件照发。
- 根因：真机口径 = uncaught（接住与否）→ 会话内 'exit'（码为当时 exitCode，
  监听可改）→ 终码；旧实现 fatal 直落 WError + 1，会话内 exit 从不派发。
- 修法：`fatal_exit` worker 臂：落 1（6/7 保留）→ 会话内 `emit_exit` →
  原错原样返回（WError 照发）；`run_worker_thread` Err 臂终码听
  `state::exit_code().unwrap_or(1)`；`_fatalException` 置空另压
  `worker_crash_silent`（无 error 事件、stderr 直出、exit 6）。
- 复现：11 格矩阵探针（单跑/矩阵双过；孤立过矩阵挂即查跨会话状态）。
- 推广铁律：**fatal 先办后事再定码**——exit 监听是改码的最后机会，任何提前
  定码（unwrap_or(1) 写死）都是与真机为敌；终码只读会话 exitCode 状态。
