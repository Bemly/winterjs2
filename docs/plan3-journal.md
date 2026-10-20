# plan3 执行日志（2026-09-19 → 09-25，自 plan3.md §5 迁出）

> 2026-09-25 修订时原样迁出（逐字，未改写），plan3.md 只留现状与下一步。
> 本文件只追加不改写；新会话**不必读**，需要追溯某轮细节时再查。

### 欠账轮落账与新会话入口（2026-09-19 session 收官，新会话据此开工）

**本 session 完成（全部已提交 master）：**

| 簇 | 状态 |
|---|---|
| G1 dgram recvbuf | ✅ 上 session（DIFF 53→32） |
| G2 net server 选项面 | ✅ 上 session |
| G9-1 zlib Rust 状态机 | ✅ 上 session |
| G9-2 zlib JS 流类接线 | ✅ 上轮（6 目标套件全绿 + 连带 5 件，zlib 域 73/81） |
| G3 http 校验长尾/chunk 限深 | ✅ 上轮（子 agent 六提交移植，点名 45 件 SAME0 + 连带 15 件） |
| **G9-3 zlib 尾件**（brotli/zstd 字典/pledged/Web CS·DS） | ✅ 本轮（6 目标套件全绿 + 82 件对拍零回归；一次性压缩走引擎收口 + raw 字典构造期设 + 严格字典校验 + pledged errno=72 + constants 28 项 + Web 全局；AGENTS §4.145-147） |
| **G6 net 尾件** | ✅ 本轮 13/16 转绿（large-string/async-iter/write-after-end-nt/abort/ipv6/HE×3 + G2 顺手 2 件；全量 159 件对拍零回归，black-box 223 全绿）；残件 6 件 infra 级记档（throttle 流控/cluster 协议×2/worker fd 移交×3）；AGENTS §4.148 |
| **G4 fs validators 尾件** | ✅ 本轮 20 套件转绿（stat族/constants/bigint/throwIfNoEntry/DEP0180/fd标准流/statfs frsize+bigint/utimes秒口径/lchown·lchmod·lutimes·_toUnixTimestamp新面/null-byte全API/rename·truncate·fchown·mkdir校验面/latin1/writeFile encoding+abort/WriteStream真open）；黑盒 fs 13/13 + 冒烟 5/5；AGENTS §4.149（e94db50 + 3a4704e） |
| **G5 child 尾件** | ✅ 本轮 25 套件转绿（ChildProcess.spawn 方法面/spawn 事件+多监听fan-out+dispose/stdio 数组+spawnargs/空字节横向校验/`-p` 自举/env 归一/ipc 门/paused 读/removeAllListeners/二次 disconnect 门/uid-gid EPERM/send 校验/stdin 继承/ERR_IPC_ONE_PIPE+INVALID_HANDLE_TYPE；黑盒 child 18/18 + 冒烟 5/5；AGENTS §4.150-151） |
| **G8 fs watch 尾件** | ✅ 本轮 30 套件转绿（ignore 全形态+递归相对路径/StatWatcher 单例EE+异步 stop+零 Stats 首轮/FSWatcher ref-unref+异步 close/encoding 转码/promises.watch 迭代+全校验+_getActiveHandles/flush 选项/exit 首码赢/前沿防抖+Create 二判据分发侧+stat 真 unref+首轮 return 收口）；残件：fs.glob ×2（Bun 快照无此行，记档另案）+ flush 三套件（待 node:test runner 深度）；黑盒 fs 18/18 + 冒烟 5/5；AGENTS §4.152-155 |
| 集成修复 ×2 | ✅ net relisten `__closing` 挂死（46 分钟）+ net SEGV（with_str_args GC 悬垂）+ stream 9b 回归（上轮） |
| 验收 | `cargo test` 全量 21 target 0 失败 0 警告 + 冒烟 5/5 |
| AGENTS.md | ✅ §4.140-144 四坑 + §4.145-147 三坑 + §4.148 net 五坑 |
| 本节欠账表 | ✅ G3/G2/G9-2/G9-3/G6 划线转绿 |

**2026-09-21 test Slice A 收官**：API 核心面 11 套件转绿（suite/ctx/tags/
plan/waitFor/subtest/getTestContext/register；对拍 83 件 SAME0 6→17、
DIFF 64→53，零回归；黑盒 `phase10f_test_*` 四件；AGENTS §4.178）。
残 53：run API ~21/mock 全家并入下轮、spawn CLI ~18 与 reporter ~13 维持另案。

**2026-09-21 test Slice B1 收官**：MockTracker 核心落地
（`node:internal/test/mock` 新建 + 钩子归属重构为真机 Test.run 口径；
mocking.js 55/56，唯一红为私有字段 V8 文案引擎偏离；82 件零回归；
黑盒 `phase10f_test_mock_*` 两件；AGENTS §4.180）。
残：mock-timers 2 件（B2：fake 计时器基建）+ run API ~21 另轮。

**2026-09-21 test Slice B2 收官**：mock.timers 落地（对拍 SAME0 17→19、
DIFF 53→51，零回归；黑盒 `phase10f_test_mock_timers_*` 两件；
AGENTS §4.181）。残：run API ~21 另轮（`run()` 事件流）。

**2026-09-21 test Slice C 收官**：run(none) 事件流落地（同进程加载 +
事件六件 + 发现 + only/tag/plan 门 + 钩子时序全对；对拍 SAME0 19→24、
DIFF 51→46，零回归；testmod 按域拆 core/run；黑盒
`phase10f_test_run_none_and_plan_gates`；AGENTS §4.182）。
残：run process 隔离 ~15（子进程/线程传输）+ spawn CLI ~18 + reporter ~13。

**2026-09-21 test Slice E 收官**：run 语义深化（plan 子计数/stopTest 超时/
TestPlan wait/legacy done/tag 过滤子集/entryFile/调用点文件/种子洗牌/run
coverage 校验；对拍 plan/tags-events/entry-file/randomize 四转绿，
SAME0 30→34、DIFF 39→35，零回归；黑盒 `phase10f_test_run_semantics_*` +
`phase10f_test_run_tag_filter_and_randomize`；AGENTS §4.184）。
残：run 并发/上报深度 ~6 + spawn CLI ~18 + reporter ~13 + mocking 单行。

**2026-09-21 test Slice D 收官**：run(process) 经 worker 传输落地
（expect-error ×2/todo-skip/filetest-location 四转绿 + coverage ×2 附带；
对拍 SAME0 24→30、DIFF 46→39，零回归；黑盒
`phase10f_test_run_process_and_expect_failure`；AGENTS §4.183）。
残：run 并发/超时/randomize/tag 过滤 ~8 + spawn CLI ~18 + reporter ~13。

**2026-09-22 G11 http TIMEOUT 首轮收官**：18 提交（请求超时全家/101 摘池/
Trailer 校验/管线/FIN 递延/流出/abort 级联/1xx/头形态/maxHeadersCount/
keep-alive 修正/回池门/池键/ready 解禁/setTimeout 门控订正 + 黑盒
`tests/node/http/timeout.rs` 4 用例 + 构建 0 警告 + 冒烟 5/5 + http 域 17/17；
约 40 件转 SAME，http-only TIMEOUT 82→63、DIFF 105→100；
sweep2 混二进制（05:47–06:32 跨两次构建）仅当趋势，终局需干净重扫；
未闭环 3 件见上表 http 行；AGENTS §4.185）。

**2026-09-22 G11 半开双杀收官**：5 提交（写端 Close 即发 + holding/halfhold/
native 注册/JS 递延/黑盒；单测 `net_halfhold_balance` + 黑盒
`phase11_net_halfopen_releases_loop` + 冒烟 5/5 + http/net/stream/dgram
域 + bin 185 全绿；`server-keep-alive-timeout`/`server-close-idle-wait-
response` 转 SAME0；AGENTS §4.186）。

**2026-09-22 G11 upgrade 轮收官**：4 提交（升级块判定门 + 三形态 + 体路由/
直调/spill + socket 暂存/destroy 异步 + 黑盒 `phase11_http_upgrade_faces`；
node 域 271 全绿（t4）+ 冒烟 5/5；upgrade 6 件全转 SAME0；AGENTS §4.187）。

**2026-09-22 G11 头面 batch5 收官**：校验门三件（数字头名 HTTP_TOKEN/
奇数组 ARG_VALUE/重发头 HEADERS_SENT）+ 拼写覆写 + 220 unknown +
数组双行 + 对形 writeHead + Host 恒拼/IPv6 框 + 拒写旗（新码
BODY_NOT_ALLOWED，检查禁入 `_write`）+ 黑盒
`tests/node/http/surface.rs::phase11_http_header_face_batch5`；
28 件头面对拍 SAME0（`header-overflow` 的 `socket.push` 系既定另轮）+
http/net/https 域 + 冒烟 5/5 + 构建 0 警告；AGENTS §4.188）。

**2026-09-22 G11 TIMEOUT 深水第一铲**：hostname 优先 + auth 补 Basic +
CONNECT（authority-form/Host 取 path/隧道 detach 双端 end:1）+
server timeout 进门 + socket HWM 65536 + 基类 setTimeout + req.protocol +
黑盒 `tests/node/http/surface.rs::phase11_http_timeout_deep_host_auth_connect`；
11 件转 SAME0；http 27/27 + net 18/18 + 冒烟；AGENTS §4.189。
未闭环：`outgoing-properties`（wl 记账专项）+ handler 抛吞 hang（另单元）。

**2026-09-22 G11 TIMEOUT 深水第二铲**：server 选项类（IM/SR 请求期构造）+
建连选项透传（HWM 进 Socket 构造器）+ socket 双 65536/res 跟随 +
黑盒 `tests/node/http/surface.rs::phase11_http_server_options_surface`；
3 件转 SAME0；http 28/28 + net 18/18 + 冒烟；AGENTS §4.190。
附带 splitting 一件（ERR_INVALID_CHAR `["key"]` 后缀）：response-splitting
转 SAME0 + 黑盒 `phase11_http_invalid_char_key`；AGENTS §4.191。
附带 response 双件（write-after-end 拦截 + 状态码门注册）：res-write-after-end/
response-statuscode 转 SAME0 + 黑盒 `phase11_http_response_gates`；
AGENTS §4.192；未竟 response-cork（另单元）。

**2026-09-23 G11 收尾轮收官**：7 提交（cork 面 / uncaught 双向 / 小面四件 /
multi-CL；§4.193）。cork 三件套（response-cork/drain-cork/outgoing-end-cork）+
uncaught-from-request-callback + test-http-1.0 + null-prototype-options +
max-headers-count + response-multi-content-length 转 SAME0；request-timeout-
keepalive 实为绿（15s sweep alarm 误判"真机自挂"，25s 实证双边绿——§4.193 坑五）。
终局 serial 重扫（sweep4，25s alarm，干净二进制，TEST_THREAD_ID 3599）：
409 件 SAME0=294/SAME1=0/DIFF=102/TIMEOUT=13（8 件转绿逐项复核在册）。
基建轮（2026-09-23，AGENTS §4.194）终局重扫（sweep6，同口径）：
409 件 SAME0=307/SAME1=0/DIFF=91/TIMEOUT=11（+13：push 面 2 + HPE 面 5 +
记账/队列 2 + eager 连带 4；零新增红项）。残件：reuse-drained（process.report
缺失，另域）+ execPath spawn ~18（待拍板）+ parser 内省 ~4（记档偏离）。
黑盒 `phase11_http_cork_faces` + `phase11_http_uncaught_throws` + http 域 25/25 +
node 域 278/278 + 冒烟 5/5。**残件全部定性**：drain-writable-length +
outgoing-properties（outputData 记账 + writableLength 合成 getter）与
header-overflow/read-in-error（socket.push）同根——需 **net.Socket 写侧
流式化**（socket 层写队列/HWM/drain）+ **eager-parse outgoing 队列**
（管线请求立即建 res、无 socket 排队），基建轮另案；execPath spawn ~18 件
（套件 spawn process.execPath 裸脚本 vs CLI 全 flag 铁律 §0.8，需拍板）；
parser 内省 ~4（_http_common parser.initialize/onIncoming 面，记档偏离）；
余散件（async_hooks 资源面/domain 集成/Atomics.wait/process.report/
optimize-empty-requests 等）逐套件记 bun-parity。

**2026-09-23 对拍提速 mapper + createConnection 转绿**：§4.202-① 断言 mapper
落地（`tests/node/helpers.rs` run_suite_mapped：实际值截 200 字 + 套件侧
调用点折算物理行 ±2 节选三行定位；`WJS_MAP_SUITE=` + `phase_mapper_locate_suite
-- --ignored` 用；raw-headers 物理 110 / mutable-headers 物理 187 一击定位；
实测钉住：无壳位置=assert SOURCE 行号安套件名、栈帧行号=CJS 前奏 +1、
rejection 拦不到/exit-hook fatal 不触发；AGENTS §4.202）。同轮
test-http-createConnection 转绿（修前 TIMEOUT：请求级 createConnection 的
oncreate 吞 err——async cb 错永悬、sync throw 靠穿透构造器侥幸；修法真机
_http_client.js 591-607 行逐字 err 臂 nextTick emitErrorEvent + try/catch
收口 + settled 防双投；黑盒三形 `phase11_http_create_connection_error_routing`；
AGENTS §4.203）。http/net/https 三域 52 绿 + node 域 285 绿 + 冒烟 5/5。

**2026-09-23 sweep8 红件簇清扫（15 件转绿）**：四簇连修——① 重复头真机
表驱动口径（joinable+未知头恒 ', '、19 头单值表首个赢、查询面过滤自动头、
GET+用户 TE 帧化；multiheaders×5/mutable-headers/raw-headers 转绿）；
② agent 池（res.destroy 后复用 + req close 蕴含 destroyed；abort-keep-alive/
override-global-agent 转绿）；③ parser 面（TE 整词 token+teInvalid 400、
冒号空格拒收、parser 全局 freelist、writeInformation 门序三形、
optimizeEmptyRequests+IM._dumpAndCloseReadable；smuggling/te-repeated/
parser-free/write-information/optimize-empty/chunk-extensions-limit 转绿）；
④ res 侧 timeout 桥（responseOnTimeout 打 res + IM.setTimeout 自武装，
监听数契约守卫；client-response-timeout 转绿）。出局 4（internals/flags×3 +
process.report）+ 偏离 2（domain 异步/Atomics.wait）+ 预存挂 1（client-
timeout-on-connect）。残：DIFF 5（set-timeout-server/request-timeout-upgrade/
url.parse-https.request/headers-timeout-keepalive/server-capture-rejections）+
TIMEOUT 5（no-read-no-dump/capture-rejection/non-utf8-header/reject-chunked/
should-keep-alive，流控与二进制头深水）。提交 f4b8a52/029a244/834bc15/431fc0e；
AGENTS §4.204；node 域 285 绿 + 冒烟 5/5 ×4 轮。

**2026-09-25 sweep 残部二批（5 件转绿）**：server captureRejections 兜底
（nodejs.rejection 逐字）+ TLSSocket _secureEstablished + ServerResponse.
setTimeout + IM/server 超时桥带 socket 实参 + HPE 门序前置（TE+CL 先于
requireHost）+ 头串 latin1 上网 + OutgoingMessage hasInstance 品牌判定
（原型桥改道 super 全链实锤后弃用）。capture-rejections/url.parse-https.
request/reject-chunked/non-utf8-header/set-timeout-server(前四块) 转绿；
提交 2b1a1ab/566117d；AGENTS §4.204 追补；node 域全绿 + 冒烟 5/5 ×3 轮。
残 4：outgoing-message-capture-rejection / should-keep-alive / no-read-no-dump
（流控与判定矩阵深水）+ set-timeout-server 末段 exit-hold（paused client
EOF 急切检测，G6 infra 族）。

**2026-09-25 §4.202-②③ 对拍提速工具落地**：① mapper 已落地（§4.202-①），
本轮补齐 ② `scripts/flake-classify.py`（新红先分类：整文件×3 + 单块 repro×N，
GREEN/FLAKY/RED-DETERMINISTIC/NODE-FLAKY 四分流，flaky 走定级法勿深挖）+
③ `scripts/sweep-bg.py`（全量 serial sweep 双 fork 后台直跑、status/wait/
tail/stop 轮询、工件落 ~/.wjs-sweep/、失败行 stderr 首行 + debug/ 全量落盘）。
两坑实证（AGENTS §4.205）：统一 runner 给 node 带 `--run` 假红全表（node 22+
--run=跑 package.json scripts）；前缀过滤只认 .js 丢 6 件 mjs（基线 409 口径）。
dogfood 首件：**dump-req-when-res-ends 判 FLAKY（wjs 0,142,142 挂死型，
node 2/2 绿）**——挂死型 flaky，归 §4.148 流控 infra 族随 no-read-no-dump
同轮处理。sweep7 全量 409 件基线后台直跑中（终态见 ~/.wjs-sweep/sweep7/）。

**2026-09-25 sweep 残部三批（2 件转绿 + ②③工具轮）**：②③落地后逐件啃
http 尾巴——① **outgoing-message-capture-rejection** 转绿（`fcf416b`）：
ServerResponse.destroy(err) 把 err 丢在 super.destroy() 外、_destroy 永裸杀
（socket 'error' 不发）→ _destroy 从 __resErrored 找回；连带修 client 侧
体未齐断连 error 递送（destroy(__e) 被 IM._destroy 吞错口径递不出去，改同步
守门递送，真机 p4 差分序 aborted → error ECONNRESET → close 对齐）；②
**should-keep-alive** 转绿（`a683ca5`）：__release 回池门只看 Connection 头，
1.0 缺省响应 socket 被错误入池 → 复用死连接挂死 → 门改 req.shouldKeepAlive
（版本×Connection 折算）+ 池态 socket 收 EOF 即销毁摘池（node socketOnEnd
口径）。黑盒两件（capture_rejection_routing / should_keep_alive_matrix）+
家族对拍零回归。**dogfood ② 分类**：dump-req-when-res-ends 判 FLAKY
（0,142,142 挂死型）；child exec_shell_self 负载形 flaky（6/6 单跑绿）。
**sweep7 终局基线**（409 件，sweep-bg 首跑）：SAME0=375 / SAME1=6 / DIFF=19 /
TIMEOUT=9（sweep6 SAME0=307 → +68）。残件定性：http 尾巴剩 no-read-no-dump
（流控 infra）+ 时序敏感两件（request-timeout-upgrade / headers-timeout-
keepalive）+ set-timeout-server exit-hold（G6 infra 族）；sweep7 新现红
（outgoing-finished / matchKnownFields / 1.0-keep-alive [object Object] 文案 /
catch-uncaughtexception / client-parse-error / writable-true-after-close /
chunk-extensions-limit flake）另批分类。node 域 286 黑盒全绿 + 冒烟 5/5。

**2026-09-25 sweep 残部四批收官（http 尾巴 5/7 转绿）**：接三批续啃——
④ **should-keep-alive**（`a683ca5`）：__release 回池门只看 Connection 头，
1.0 缺省响应 socket 被错误入池 → 复用死连接挂死；门改 req.shouldKeepAlive
+ 池态 socket 收 EOF 即销毁摘池。⑤ **no-read-no-dump**（`7c916b6`）：服务端
体背压流控 infra 四件联动（泵 backpressured 旗 / __feed 停读+pause /
Socket pause/resume 事件 / req._read 消费即解暂停）+ res 完成清 framing 根修
（体在途字节被当新请求头 → HPE 断连）。⑥ **时序敏感两件**（`2ce37ef`）：
Host 校验搬位到升级检测后（node parserOnIncoming 头部对 upgrade return 0）+
劫持撤计时 + headersTimeout 计时模型统一（连接建立/新消息首字节开、请求完成
撤、空闲归 keepAliveTimeout）。黑盒新增三件，node 域 288 全绿，冒烟 5/5；
AGENTS §4.206。**残件**：dump-req-when-res-ends（挂死型 flaky——今日根因
定位到流端口 flowing 排空语义，dump 机制需先修 readable_flow push/flow，
独立轮；§4.206 坑二）+ set-timeout-server 末段 exit-hold（G6 infra 族）+
sweep7 新现红件分类（outgoing-finished/matchKnownFields/1.0-keep-alive 文案
等）。**sweep8 终局**（409 件）：SAME0=379 / SAME1=6 / DIFF=16 / TIMEOUT=8——五件
修复零红；但暴露两件**本轮回归**（outgoing-flush-drain TIMEOUT +
upgrade-large-body-unread DIFF，sweep7 均绿）。**回归根修**（`6a2d64e`）：
二分三段实锤 chunked 泵背压 early-return 为元凶（终结段扣 fr.buf 等再喂
而包不会再有 → 泵停摆）；终解=背压改**状态驱动事件**（缓冲 ≥HWM 发
'pause'、落回发 'resume'，泵不停读不中断，缓冲有界=体长）+ 泵恢复无早退形；
两回归转绿 + 五件守卫绿 + node 域 288 全绿（fifo 按 §4.175 剔除）。AGENTS
§4.206 坑四/坑五（分离 HEAD 提交：bisect 后直接 commit 落 detached，父=旧
提交缺后续修复——cherry-pick 回 master 解）。

**sweep9 终局（本轮收官基线）**：409 件 SAME0=381 / SAME1=6 / DIFF=16 /
TIMEOUT=6——**七件修复全零红**（sweep6 基线 307 → +74）。残件：dump-req-when-res-
ends（挂死型——dump 机制需先修流端口 flowing 排空语义，独立轮）+
set-timeout-server 末段 exit-hold（G6 infra 族）+ sweep8 散红分类
（matchKnownFields/outgoing-finished/1.0-keep-alive 文案/catch-uncaughtexception/
client-parse-error/writable-true-after-close + node 侧独红的 loader 形 8 件）。

## 旧版"新会话入口"（2026-09-20，已作废，见 plan3 §0）

**新会话入口（按优先级，2026-09-20 G8 轮后更新）：**

1. **G5 child ~30 件 → G8 fs watch ~23 件 ✅ 双收官**（G5 25 套件 + G8 30 套件；
   AGENTS §4.150-155；残件：fork/IPC handle 传递出局 + fs.glob×2/flush 三套件
   待 node:test + enoent-after-deletion 间歇超时另查）。
2. **fs 残簇**（2026-09-21 七轮收官：cp/write/read/handle 129 件 70/59 →
    **126 SAME/3 DIFF**，e2f0d28/6412095/b2a473e/4703a16/4f9cc70/七轮读流，
    残件与黑盒见 bun-parity fs 七轮注记）：残 3 全另案——read-worker ×1
    （worker fd 移交，归 G6 残件同族）、eagain/flush ×2（node:test mock，
    runner 深度）；expose-internals ×3 跳过类；pull/writer ×3（需
    stream/iter+zlib/iter 新模块，另轮）；stream 余 err（增量流重写轮）+
    write-patch-open（fork 父端 exit，child 域）；黑盒 fs 26/26 + 全量
    cargo test 21 target 0 失败。
3. **G6 残件 6 件**（infra 级，需独立轮）：throttle（native 读门控+写 EAGAIN
   流控）、cluster×2（internalMessage 协议）、worker×3（跨线程 fd 移交）。
4. **大簇另案**：G10 http2 compat ~105 件、G11 http TIMEOUT ~110 件、
   dgram 余 ~32 件、http OutgoingMessage 缓冲模型 5 件（G3 遗留专项）、
   async_hooks 资源面（FSREQCALLBACK 生命周期，fs roundtrip 末段牵引）。
5. **URLPattern 归属**：对 Bun 1.3 实测后拍板（见上"待核对"）。

（编号对表：G4=fs validators 尾件行、G5=child_process 行、G6=net 尾件行、
G8=fs watch 簇行、G10=http2 compat 行、G11=http TIMEOUT 行；
G1/G2/G3/G9 已收官。）

**新会话开场提示**：先读 AGENTS.md §4.140-149（三轮十一坑，尤其 §4.140
"手工过/cargo 挂≠环境问题"、§4.142 "禁连续建 worktree"、§4.145 跑分
"exec or die"+glob 路径）+ 本节欠账表；跑分 `TEST_THREAD_ID` 用 35xx+
（§4.122 互踩防线）；net 域黑盒先单跑验证（`cargo test --test node net`，
全绿后再并发——§4.140/§4.141 两坑都在 cargo harness 时序下才现形）。

## 2026-09-25 P1 http2 首轮（本会话）

- 基线 base12（全域 Bun 清单 3574 件，2 并发 + 看门狗 42 分钟）：绿 2168（60%）。首跑 base10 因 D1
  前缀旗放行致 `test-promise-unhandled-flag` 自 spawn 无限递归、整机 panic（pitfalls 4.209），作废。
- 全局修复：`process 'exit'` 从未触发（this 绑错，mustCall 退出核对从未执行——此前假绿）、
  全局 console 走 util.format、require 透传原异常 + CJS 行号物理对齐、未捕获错误 node 形渲染、
  `new URL` 错误 node 形（ERR_INVALID_URL）、X.509 v1 证书（tls/https/http2 共 37 件同因）。
- 提速：`WINTERJS_HANG_EXIT`（挂死件报出未触发回调的创建点）、nextest（全量 ~100s）、
  sweep `--jobs/--scope/--rerun-red`/node 缓存/资源看门狗/进程组封顶、`scripts/split-js.py`。
- http2：64（旧口径）→ 88/256（真口径），余件见 plan3 §0.4 P1 行。

## 2026-09-26 P2 共性簇 + tls 首批

- base13（2093）较 base12（2168）净 −75：exit 事件修好后 mustCall 核对生效，143 件旧假绿翻红
  （聚类：`HANG` 挂死自报 108 / `MUSTCALL` 计数不符 132），非退化。sweep 标签由此拆分。
- 共性簇按"未触发回调的监听事件名"聚类找根因：beforeExit（9）、expectWarning（9）、fs 流 close/error（~20）。
  - process：beforeExit 派发、致命错先渲染再发 exit（exitCode 可改）、公开 emit 抛错上抛（4.211）。
  - emitWarning 逐字移植（缺省打印是表内监听，off/once 生效）；CJS 以绝对路径编译（node 栈帧口径）。
  - fs 流逐字移植 `internal/fs/streams.js`；暴露底座两处时序偏差：fs 回调微任务 → setImmediate、
    setImmediate 钳 1ms → 不钳；unref 定时器循环不 alive 不触发（4.212）。
  - 顺手：dgram bind 前 unref 落原生、repl .save/.load 逐字、ClientRequest.setTimeout 同步校验。
- base14 2106、base15 2136。tls：SecureContext 层逐字移植 + 错误码补齐，tls/https 单域 +23（base16 计）。
- 工具：`~/wjs-data/node-lib/` 导出 node 26.8.2 全部内建源；sweep 期间开发用 `CARGO_TARGET_DIR=~/wjs-data/target-alt`。
- `test-socket-write-after-fin-error` 回红：旧绿靠 immediate 1ms 时序；忠实修法需建模 shutdown 完成 → autoDestroy
  （试过 onend 下一 tick end()，连带 write-after-close 反红，已回退）。`exec-maxbuf` 两版本同样偶红（既有 flaky）。
- 待办：tls-b 内存 BIO 引擎（`tls.connect({socket})` 簇）；`test-http-keep-alive-max-requests` /
  `test-stream2-httpclient-response-end` 负载下偶红（immediate 不钳后时序敏感，单跑稳定绿）。

## 2026-09-26 P2-tls-b：TLSSocket 包裹引擎（rustls 由 JS 字节驱动）

- 仓库上 GitHub：私有库 `Bemly/winterjs`（master 跟踪 origin；此前无 remote 单副本）。
- `tls.connect({socket})` / `new TLSSocket(duplex)` 簇（base15 时 21 件红）全链落地：
  - 引擎 `src/builtins/node/tls_wrap.rs`：rustls 手动模式（read_tls → process_new_packets →
    write_tls/reader 排空），引擎表 `PlainState.tls_engines`（state/tls_wrap.rs，与 net 共用 id
    计数）；natives `__wjs_tls_wrap_{open,feed,write,eof,shutdown,kill}`——wrapped 密文 JS 喂入，
    回程 JSON 一次带密文/明文/握手旗/校验捕获/信息（明文逐轮排空防 rustls received_plaintext 撑满）。
  - CaptureVerifier：node 口径"校验失败不中止握手"——错误捕获后随 'secure' 回传 JS 处置；
    rejectUnauthorized:false（直拨面同样改捕获式）→ authorized=false 连接存活，真机口径。
  - tls.js：TLSSocket 构造器对齐现行 node（无 options 交换形；非 Duplex socket 参 TypeError；
    allowHalfOpen 有 socket 参即取 socket 自身）；_start/_finishInit/onConnectSecure/onConnectEnd
    逐字（'secure' 恒发，握手期 'end' → ECONNRESET 带 path/host/port，secureConnect 仅包装层发）；
    convertALPNProtocols 全家原文；UDS path 形（client 内建 net.Socket + 包裹引擎，server
    tls_listen UDS 分支：UnixListener+TlsAcceptor，连接走 ConnectionUds）。
  - net 面最小钩子：write/end/destroy 收敛 `__nativeWrite/__nativeEnd/__nativeKill`（net 本形
    直通原 native，包裹面覆写）；destroySoon 原文补齐。直拨面握手 EOF → End 事件（onConnectEnd 面）。
  - io_code 补 ECONNRESET(54/104)、ENOTCONN(57/107)（收尾 RST 曾落 UNKNOWN，4.213）。
  - https.Server：ALPNProtocols 缺省 ['http/1.1'] 原文口径；createServer 经 Server()。
- 结果：wrap 簇 24 件 16 转绿（EADDRNOTAVAIL NaN / HANG / mustCall 不触发三簇清零）；
  黑盒 tls/https 7/7；全量 nextest strict 717/717；冒烟 5/5。base16 待跑全域 sweep 计数。
- 关键坑：tls.Server.listen 字符串参曾是 host，误改判 UDS path（含 "/" 才是 path，node
  isPipeName 口径）——一度全量 tls/https listen(0,"127.0.0.1") 崩、address().port undefined（4.214）。
- 余 8 件（挂死收尾簇为主，下轮按 §0.2 时间盒）：`test-tls-socket-close`/`-destroy`/
  `-default-options`/`-streamwrap-buffersize`（收尾 net 不归零）、`-on-empty-socket`
  （late teardown error 误上抛）、`-client-destroy-soon`（'readable' 流量面）、
  `test-async-wrap-tlssocket-asyncreset`（ca 链另案）、`test-tls-wrap-econnreset-*` 已绿。

## 2026-09-26 P2-tls-b 第二轮：accept 重排 + EOF 适配（wrap 簇 19/24）

- `tls_listen`（TCP/UDS）：connection 在 TCP accept 即发（node net.Server 口径），
  TlsInfo 随握手完成到 conn 侧——'secure'/'secureConnection' 点位后移；握手失败发
  tlsClientError + conn Close。secureConnect 双发清零（直拨 __ev 只发 'secure'，
  secureConnect 归 onConnectSecure）。
- `TlsCleanEof` 读端适配器：rustls 对 FIN 无 close_notify 严格报 UnexpectedEof——
  node/OpenSSL 同场景是干净 EOF。socket-close/on-empty 的 "connect UNKNOWN" 噪声
  （无监听即崩）由此根除（4.215）。
- Server 内部 'connection' 监听（手动升级形 tlsServer.emit('connection', rawSocket)）；
  TLSSocket._destroySSL / bufferSize wrap 镜像 / Duplex 包裹即时起手。
- 结果：wrap 簇 19/24。余 4 件定性：`test-tls-streamwrap-buffersize`（阻塞于
  **stream.Duplex push(null) 不发 'end'**——duplexPair EOF 面断，stream 域另案）、
  `test-tls-socket-default-options`（收尾 net=5 待建模）、`test-tls-client-destroy-soon`
  （'readable' 流量面，net 域共性）、`test-async-wrap-tlssocket-asyncreset`（ca 链）。
- sweep 定量：tls 域（test-tls-，185 件）SAME0 40→**80**；https 域（59 件）18 绿——同构建对照（17≈18）证实 base15 的 31 系口径差异非回归；base16 全域待 P2 收尾统一跑。
## 2026-09-27 Vue/Vite 生态实测（26.9.27 release 二进制，vite 8.3.1 + vue 3.5.43，~/wjs-data/probe/vue-app）

- **✅ Vue 3.5 SSR 开箱即用**：`vue/server-renderer` renderToString 输出正确（ssr.mjs）。
- **✅ vite 8 模块图完整加载**：纯 ESM 深链 import 无一失败。
- **✅ N-API 原生 addon 生态**：rolldown 1.2 binding（.node）与 lightningcss 都能加载，
  `rolldown.build` 原生打包调用跑通——napi M0-M6 的实战首考通过。
- **✅ vite createServer 全链**：config 解析（vite:config/env 全过）、插件管线
  （vite:oxc/builtin:vite-resolve 等 26 插件）、pluginContainer.buildStart、
  **dev server LISTENING**（http://localhost:5199/）。
- **❌ 首个 HTTP 请求挂起**：raw socket TCP 能连，请求后无响应——请求处理管线
  （transformRequest/中间件）某处挂，transformRequest 探针另出现
  「unhandled rejection 拒因为 Promise → 输出 [object Promise] 且提前退出」怪象，
  同文件三连一致、与相邻文件行为不同，未定位（下轮首要）。
- **❌ vite CLI 直跑静默退出**：bin 的 `import('../dist/node/cli.js')` 后无输出 rc=0；
  cac/argv 形状（process.argv=[bin, script]）待查。
- 未测：vite build、HMR、Vue 客户端 hydration。
- 工程面：`--run x.js -- --help` 的 `--` 收尾口径（4.61）再次生效。

## 2026-09-27 P2-crypto 首簇：三件同源簇转绿（getCipherInfo/Sign/Verify）

- base15 crypto 红 83 件按 `wjs_err` 聚类：MISSING-EXCEPTION 9 / VALUE 6 / keygen-UNHANDLED
  散簇 / STREAM-PIPE 4 / INTERNAL-MODULE 3（`--expose-internals` 件，node 侧过、我方
  `Cannot find module 'internal/crypto/*'`）/ NO-BINDING 2 等。首刀选三件同源簇：
  `getcipherinfo`（nid 往返 deepStrictEqual 红）+ `classes`（`crypto[clazz]` undefined）
  + `sign-verify`（`Sign is not a function`）+ 附带 `verify-failure`（class 无 new 直调抛）。
- 根因二：① `getCipherInfo` 只认名字符串（nid 入参回 undefined）且无视 options；
  ② `Sign`/`Verify` 用 class 直出（无 new 即抛）且漏进默认导出表（`__api` 无此二门，
  `typeof crypto.Sign === "undefined"`，`createX instanceof X` 全灭）。
- 修法（`3c205a2`）：`getCipherInfo` 逐字移植 `internal/crypto/cipher.js`（空串/nid
  越界→undefined；非串非数/非法 options→`ERR_INVALID_ARG_TYPE`；key/iv 错配→undefined；
  ccm iv 7–13、ocb iv 1–15 可变窗；ocb 三档仅元数据，create 保持 Unknown）；
  ccm 192/256 nid 勘误 897/898→899/902（真机）；`Sign/Verify` 改 legacy 函数形
  （Cipheriv 同款无 new 包装）+ 进 `__api`。黑盒新增 `p2_crypto_cipherinfo_nid_options`
  + `p2_crypto_sign_verify_nonew`（正常/报错/边界）。
- 结果：三件转绿 0（`getcipherinfo`/`classes`/`verify-failure`，run1.sh 复验）；
  crypto 黑盒 29/29、bin 单测 216、冒烟 5/5。`sign-verify` 行 57 止步：
  真机探针证实 `this[kHandle].sign` 内 native 写 `.library`（C++ 层，JS 无此概念，
  `grep library internal/crypto/*` 零命中）——结构性不可复刻；且该套件后续要
  `Sign` 真流式（`s.end()`），与 STREAM-PIPE 4 件同源，记档 P2-stream，不追。
- 顺手：`strip_banner_flags` 未用导入致 2 警告，去之回基线 1（linker 环境音）。
- 下一站：crypto MISSING-EXCEPTION 9 件（逐件小校验）或按队列转 repl（64+1）。

## 2026-09-27 P2-repl 首两轮：input 簇 +8，方法面 +6（17→31/82）

- R1（`af13ccc`）：base15 repl 红按 `wjs_err` 聚类，`input.on is not a function` ~10 件
  二形态——legacy 位置形 `start('', stream, eval)`（string 首参被吞成 `{prompt}`，
  input 空）与 `new Interface(options)` 直构（位置形构造器误吃 options 对象）。
  真机核对：`createInterface({})` 无码 `input.on` 与我方逐字同形——抛错不动，
  修上游误喂。readline Interface 构造器 options 归一（node internal 180 行口径）；
  write 入流排空 + 关后 `ERR_USE_AFTER_CLOSE`（新增码）+ 写即 resume；
  终端历史多行倒序存（reverseString 口径，单文件单行格式使然）+
  removeHistoryDuplicates 清全表；repl legacy 位置形 + 双缺 stdio（lib/repl.js
  299 行）+ `write` 直通 + writer 携 options。黑盒 2 新增；readline 4/4、repl 2/2。
- R2（`7623609`）：复验 repl1（65 红→8 绿：另含 multiline/nested-repls/evalcallback
  被 write 带绿）。方法面：defineCommand 函数形 + help 版式（排序/最长+3/裸名/
  Ctrl 尾行；editor 终端独有致列宽 6）+ editor 缓冲/C-d 求值/空行收尾（.save 末
  换行 node 同款）+ complete 空回（completer 实现另案）。再 +6；
  repl2 复验 57→6 绿。黑盒 1 新增；repl 4/4、readline 4/4、bin 215、冒烟 5/5。
- 坑（记 pitfalls）：同流自回显即真机亦无限递归（黑盒改 duplex 映射避开）；
  `{}` 抛错与真机逐字同形时禁"修正"（先对真机）；多行历史倒序存反直觉但原文有注。
- 余 ~51：真 completer（save-load/computed-props/buffer/files 等约 8）/
  setupHistory（3）/useGlobal（reset-event）/internal/repl（11）/ASSERT 散件 18/
  MUSTCALL 4/sigint 2（spawn 记档面）/杂项，另轮。`--interactive` 旗缺口
  （array-prototype-tempering HOST，wjs=-9/TIMEOUT）记 D1-CLI 另案。

## 2026-09-27 P2-repl R3：子集补全 +8（31→39/82）

- 转绿：save-load（成员补全）/computed-props（串数下标+大小写不敏感）/
  buffer（键枚举去下标/非标识符）/files（fs 路径：既存目录列子项裸名）/
  new-expression（new 剥除）/nosideeffects（调用形恒拒）/custom-completer
  （同步返回形包回调）/on-editor-mode（公共前缀收敛）。
- 关键真机口径（逐项实测）：补全过滤大小写不敏感；`allowBlockingCompletions`
  为 fs 面开关（无之回空）；目录如内列子项且 completeOn 置空；分组/三元纯
  表达式可求值（调用形才拒）；bare `Uin` 经 getOwnPropertyNames（SM 全局键
  非枚举，keys 不可用）。
- 坑：path 分支劫持含引号成员行（分支重排：成员→路径→等号段→拒答→bare；
  walk-fail 即拒不穿透）；模板字面量拒答误杀纯串（纯模板放行/tag·插值拒）；
  同流自回显教训重申（4.221）。
- getters 套件 5/7：余 proxy 两块需 `isProxy`（恒 false 引擎缺口，native 活，
  另案问用户）；plain/getter 拒绝面全过。黑盒新增 `p2_repl_subset_complete`。

## 2026-09-27 REPL 渲染修复（阶梯/prompt 竞争）+ 文档 pane 签名化 + R4 收尾

- 用户实测报 UI 三症：① 函数体回显多行阶梯右移；② miette 错误框碎片错位；
  ③ 报错贴下一轮 prompt 后；④ 补全文档 pane 只有 "timer function" 一类短语，
  无传参/输出提示。
- 根因（pty 抓字节实锤）：读行线程 `read_line` 阻塞时终端处 crossterm raw
  mode（OPOST/ONLCR 关），主循环直写终端的多行输出裸 LF 不回车即阶梯；
  readline 线程发行后立即回环渲染下一轮 prompt，与求值输出竞争（详见 4.225）。
- 修法两层（`7f…` 渲染修复提交）：① 哨兵协议——发行后 drain 旧哨兵 +
  `blocking_recv` 等"本轮输出完毕"，期间不进 read_line（主循环端
  `pending_flush` 旗，行处理轮 pump/rejection 收尾后发）；② REPL TTY 会话
  `REPL_TTY_OUTPUT` 旗 + `crlf()` 工具，console emit 与 repl_out/
  print_completion/error render_string 统一 CRLF 化（覆盖哨兵后异步窗口，
  管道/黑盒路径字节恒等不受影响）；附带 SIGINT 置忽略（哨兵窗口防内核默认
  终止）。`error.rs` 拆 `render_string`（render 文本恒等）。
- 文档 pane（用户口径"要传参/输出提示"）：`candidates()` 全表签名化
  （`setTimeout(cb, ms?, ...args) → Timeout` 形，40 项）。
- R4 收尾：`p2_repl_options_surface` 挂死＝同流 input/output + terminal:true
  自回显（4.221 真机同挂，测试写法对齐 bug）——input/output 分离复验过；
  `completer_faces` 断言随签名表更新。
- 验证：pty 字节复验四场景（回显/错误框/console 多行/timer 异步）全行首对齐；
  repl 域 nextest 30/30；冒烟 5/5（target-alt）；全量 strict 见 plan3 §0.4。

## 2026-09-28 P2-repl R5：CLI 补全接真上下文（Tab 与实际环境打通）

- 用户实测指出：REPL 里 `global` 明明是对象，Tab 补全却只出静态表里的
  `globalThis`——CLI 补全与真实环境脱节（R3 只把真上下文补全做进了
  `node:repl` 模块，CLI reedline 仍是 Rust 静态表，成员形更是全拒）。
- 方案：CLI 补全跨线程接 `node:repl` 的补全核心——`__defaultComplete`
  增 `__ctxEval` 的 globalThis 分支（间接 eval，与 CLI 经典脚本求值面同源），
  新导出 `cliComplete(line)`（同步返回 `[list, completeOn]`，同挂默认导出，
  4.218 教训重申）；`JsCompleter` 改双源——Tab 请求经通道投递主循环
  （`JSContext` !Send，真上下文枚举只在 JS 线程），JS 线程桥
  `__wjs_cli_complete`（会话启动时注入）求值回 JSON，id 配对防迟到旧包，
  150ms 超时降级静态表；`completeOn` → reedline span 换算
  （非行尾段拒映射）；bare 面动态候选与静态表合并去重（静态描述补充）。
- 坑两枚：① `cliComplete` 只挂具名导出致 `require` 面 undefined（4.218
  重演，挂默认导出解决）；② 桥内与主循环脚本各 `JSON.stringify` 一次
  ——双重编码使 Rust 侧拿到字符串而非数组，`v.get(0)` 落 fallback 空集
  （pty 插桩定位：REQ 到、回包到、items=[]）。桥改返对象，主循环统一编码。
- 验证：黑盒 `p2_repl_cli_complete_bridge`（bare 含 global/成员链/大小写
  不敏感/调用形拒答）；repl 域 32/32；pty 端到端 `gl`+Tab 出 `global`、
  `console.lo`+Tab 出 `console.log`；冒烟 5/5；strict 见 plan3 §0.4。
- 补遗（同轮）：`console.`/`global.` 点后**空前缀** Tab 仍 NO RECORDS——Rust 侧
  `prefix.is_empty()` 提前返回挡在动态请求前（R3 JS 侧 filter="" 全键枚举本就
  支持）；修为成员形空前缀直发动态，黑盒补 `dot-empty`/`global-dot` 两断言，
  pty 复验 `console.`→log/assert、`global.`→全键。

## 2026-09-28 R5 方向纠正（用户拍板）：winterjs repl 底座显性化，node:repl 反向复用

- 用户指出根本架构问题：**CLI REPL 是 winterjs 本体，node:repl 兼容面应反过来
  骑 winterjs 自身 repl 底座**——而不是把 CLI 专用能力塞进 node:repl。并要求
  审计全仓同类"方向反了"的域。
- 审计结论（plan3 §7 详表）：node:* ~50 域中 **13 域方向正确**（timers/buffer/
  stream_web/console 无流面/process/path/crypto/sqlite/worker/vm/url/test/http），
  ~25 域语义独立无对应物，**实锤反了 3 处**：① repl（CLI 与模块完全平行，
  求值面都不同源——CLI 每行独立 evaluate_script vs 模块 vm context）；
  ② readline 补全语义（R3 核心在模块、CLI 曾是静态表——桥接方向对但放置错）；
  ③ console 格式化（全局 console Rust join_args vs node:console util.format，
  部分反，待议）。
- 本轮落点（位置纠正）：node:repl 公开导出面恢复 node 真机同形（删
  cliComplete/签名表），补全核心经 `globalThis.__wjs_repl_default_complete`
  注册内部面（`__wjs_` 惯例，不进导出）；CLI 桥 `__wjs_cli_complete` +
  `__SIG` 签名表（~120 条，SM native toString 无形参名故手写；用户函数
  toString 真形参优先）+ 描述摘要（描述符沿链安全读不触发 getter）全部
  搬 **prelude/repl_complete.rs**（winterjs repl 底座第一块显性域）。
  Rust 协议扩为 `(全文, 描述)` 对，IdeMenu 右侧 pane 成员方法也有签名。
- 坑：`__wjsReplSig` 普通对象字面量查裸键沿**原型链**命中 Object.prototype
  同名方法（propertyIsEnumerable 查表拿到函数自身，desc 序列化 null）——
  查表对象一律 `Object.create(null)`（4.23 同型教训的查表版）。
- 验证：黑盒九断言（公开面干净/注册/bare/成员/签名/类型摘要/拒答/点后空/
  global.）；repl 域 32/32；pty `console.` 首屏带 `log(...args)`、
  `assert(cond, ...data)`；strict 见 §0.4。
- **方向铁律（入库 AGENTS §6）**：node:* 兼容面是 winterjs 自身能力的
  下游包装，禁把 CLI/产品专用能力放进 node:* 公开导出面；共享逻辑经
  `__wjs_` 内部注册面复用。repl 深度统一（CLI 求值改骑 vm context、
  CLI=REPLServer 默认实例形态）另案 plan3 §7 待拍板。

## 2026-09-28 R6：repl 求值面统一（top-level await 落地；§7 反关系纠正①）

- 用户拍板"把反的全部纠正了"。事实摸底：CLI 的 let/const 跨行**已天然成立**
  （SM 经典脚本 global lexical env 跨 script 持久，真机同形）；求值面实质差异
  = **top-level await**（node lib/repl.js defaultEval 经 internal/repl/await.js
  的 acorn 重写 + awaitPromise 收割；CLI 直接不支持，带 hint 文案）。
- 落法（无 acorn 的线性近似）：原码编译失败且行含 `await` → 试 async-IIFE
  包装重试；包装文本经底座桥 `__wjs_repl_tla_wrap`（prelude/repl_complete）——
  **末条顶层语句改写 `return { value: (expr) }`**（node await.js 同款语义：
  防 async 返回对 Promise 值二次解包；声明/return 结尾不改写）+ `.then` 双臂
  装标记对（rejected 不进 jobqueue 的 unhandled 收割）。挂起 promise 存
  `state.repl_tla`（§4.40 Heap+trace；跨轮/跨 GC），主循环逐轮 pump 后
  `settle_tla` 查结算（4.116 realm 回落 + wrap_cx）——Resolved 打印完成值、
  rejected 当场 realm 内转 engine pending 取信息，realm 外 D4 渲染。EOF 时
  drain（30s 上限）。非 TTY 分支同步兵背压（行按序，挂起期间管道阻塞）。
- 坑三枚（调试实录）：① 跨 `.await` 的栈式 RootedGuard 存 Promise 不可靠且
  违反 §4.40——改 state Heap 槽；② pump 后未回主 realm 直接裸 JSAPI
  （JS_GetProperty→Atomize 空指针，macOS 崩溃报告定位）——settle 全包
  AutoRealm；③ 初版包装无 return 改写，P1 resolve undefined 值被吞——
  补末表达式改写后才通。`set_pending_exception` 的 rejected 处理也须
  realm 内（同 ①②）。
- 缺口（记 §7）：声明提升（`let a = await x` 跨行存活）为 node acorn AST
  重写语义，待拍板引 acorn（vendored JS）后逐字移植；嵌套 Promise 双解防护
  已随 `{ value }` 包装落地。
- 验证：黑盒新增 repl_tla_await_resolves/rejects/in_async_fn_untouched
  （repl 域 35/35）；管道四场景 + pty TTY 两场景全对（41/tick/boom 渲染/
  后续行继续）；strict 772/773（child exec 单跑 1s 过=并发偶发）；
  冒烟 5/5；行数守门 ok（state/mod.rs 压行 1000）。

## 2026-09-28 R6b：声明提升（acorn vendored 落地；§0.5 拍板）

- 用户拍板引包：acorn 8.18.0 / acorn-walk（node 26.8.2 deps 内建同款，MIT，
  `wjs-data/node-lib` 取得）——prelude/vendor/ 切片 7 片（split-js.py 字节恒等）+
  包装 IIFE 强制 UMD CJS 分支挂 `globalThis.acorn`/`acornWalk`。
- `__wjs_repl_tla_wrap` 升级为 node internal/repl/await.js 的
  processTopLevelAwait **逐字移植**（primordials 直映原生方法；Recoverable 删——
  CLI validator 保证行平衡）：末表达式 return 化 + 顶层 let/const/var/class/
  function 声明提升（跨 async 边界存全局词法）。声明完成值 undefined（node 同形）。
- 坑三枚：① split-js.py 把第一片写回原路径——项目内 acorn.js=片 1（设计使然），
  完整原件在 wjs-data/node-lib；② UMD 包装两参 `({exports:{}}, {...}.exports)`
  是**两个不同对象**——factory 写后者、module.exports 是前者空对象 → 同一对象
  双参才挂得上；③ UMD global 分支挂好的对象会被我们的尾挂行**覆盖成空**
  （单参调用时）——链路错位虚虚实实，必须 CJS 分支确定性。
- 坑四：acorn 进 PRELUDE 后 worker/child 小窗口时序测试竞态翻红（worker
  terminate 5/5 失败；全会话启动慢 ~10ms 错开 50ms 窗口）——**acorn 惰性化**：
  移出 PRELUDE，仅 REPL 会话启动注入（vendor 常量 + repl_complete::REPL_TLA_JS
  由 runtime/repl 注入）；摘除后 worker 5/5 恢复、启动开销归零。
- 验证：`let a = await 41` 跨行 42、多声明+function 提升、resolve/reject/timer
  全对（repl 域 37/37）；acorn 22 键 version 8.18.0；strict 见 §0.4。

## 2026-09-28 §7-②：console 格式化统一收尾（全局 console 全量对齐 node）

- 范围（纯 JS，零 Rust 改动、零新依赖、零 unsafe）：`REQUIRE_PRELUDE` 的 console
  包装器扩展——assert（首参字符串前缀/否则 unshift，经 wrap 后的 warn 二次格式化，
  constructor.js 475-484 行原文）+ trace（format 取 message + captureStackTrace 取帧，
  自拼首行，空消息裸 `Trace`，V8 同形；帧行 SM 口径偏离记档）+ 8 缺失方法
  （table 沿模块面 format 落盘偏离；dirxml/groupCollapsed 别名；context/Console
  惰性复用 node:console 同一类；profile/profileEnd/timeStamp/createTask stubs 与
  模块面同形）+ `__wjsReplSig` 补 7 项。
- 实测：assert 4 场景（格式化/多参/裸参/真值静默）逐字对真机；trace 首行对齐；
  8 方法 `typeof` 全 function；`console.Console === node:console Console` 同一类；
  模块面 assert/table 经包装器同行为；冻结内建套件双侧 exit 0（包装器先于冻结安装）。
- 连带红一枚：`p2_repl_cli_complete_bridge/dot-empty`（trace 文档 `stderr`→通用回落，
  4.226）——修测试断言为回落形 + 补签名表，strict 回 776/776。
- 量化：console 域 sweep（tag console3，前缀须 `test-console`）SAME0 2→3/16，
  余 13 DIFF（写错流/颜色/TTY/栈/proxy/toString）深水记档不追（0.2 时间盒）；
  黑盒新增 `phase11_console_global_unified`（正常+报错+边界）；冒烟 5/5；行数守门 ok。
- 遗留记档：trace 帧含 2 行 `__wjs_` 管线帧（D4 过滤仅覆盖未捕获路径，另案）；
  countReset 无标签警告沿模块面偏离；table 无列对齐沿既有偏离。

## 2026-09-28 §7-②跟进：补全文档遮蔽（4.227）

- 用户报 REPL `console.` 补全没有文档——实测手写表有文案但被 `__sigDesc` 的
  toString 优先分支永久遮蔽（JS 包装自带 `...args`）。修法：精确 dotted 路径
  表查询提到 toString 之前，Ctor/裸名回落不动（`o.assign(a,b)` 探针仍显示自身
  形参）。`console.` 全员恢复通道/语义摘要；`dot-empty` 断言回到通道信息形。
- 验证：repl/console/builtins 120 件全绿；行数守门 ok。

## 2026-09-28 irb 方向：JS 面补全文档全覆盖（node 面除外）

- 用户拍板：对标 irb 不对标 node repl；JS 面全包（WinterCG + SpiderMonkey 内建 +
  winterjs 自有），node:* 兼容面不投文档（兼容层非本体，"不是复刻 node"）。
- 语料证据（实测）：mdn/content 全仓 ~496MB，其中 web/javascript 9.1MB（1356 件）、
  web/api/console 168KB（30 件，`~/wjs-data/mdn-content` 稀疏检出可查）；
  nodejs/node LICENSE 覆盖 doc（MIT，同既有 vendoring 纪律）；
  WHATWG Console Standard 为 CC-BY 4.0 且源码内引用部分转 BSD-3——console 释义
  按此改写零摩擦；MDN 正文 CC-BY-SA（NPL-1.1 不在其兼容名单，逐字搬运须独立文件
  保留原协议头），故一律手写改写、只取事实（事实不受著作权保护）。
- 落法：`__wjsReplSig` 从签名表升级为"签名 + 一句话"（console 17 项 WHATWG 改写，
  偏离处如实写记档如 table/countReset/clear；WinterCG 36 bare + 33 成员，存在性逐项
  `--eval` 实证，FormData/navigator/SubtleCrypto 缺席即跳过）；匹配序收敛为
  "表（精确路径/completed 名）优先，具体 Ctor（非 Object）次之，toString 兜底"——
  原生短形参（`get(n)`）让位文档，普通用户对象永远真相。
- 验证：新黑盒 `phase11_repl_sig_js_docs`（正常+遮蔽边界）；strict 777/777；
  冒烟 5/5；行数守门 ok。深水不追：`crypto.subtle` 方法面、table 列对齐。

## 2026-09-28 `.doc` 整篇文档（irb show_doc 方向，用户拍板三连）

- 语料 JS 全量进仓：`mdn-content/`（1783 `.md`，7.6MB；`web/javascript` 全量 +
  `web/api/console` 全量 + WinterCG 接口页；图床不要，原件不动，路径即出处；
  `ATTRIBUTION.md` 载 CC-BY-SA 署名）。只做 `.doc` 命令（CLI 专属，node:repl 不动；
  TTY 走 termimad 样式，管道走纯文本）。termimad 0.35.5 MIT，闭包无 links，
  §0.5 登记进 `docs/dependencies.md` §4。
- 用户裁定（回头看最关键的一句）：pane 禁搬运文档句，文档只读语料——
  `__wjsReplSig` 回签名本位（R3 原样 + 4.227 排序），我贴进去的 94 条 MDN 句、
  23 个【】注记、80 个 WinterCG/存根表项全退（`git diff` 净删 100+ 行）。
  偏离记档住老地方（bun-parity/模块头/journal），不嵌文档串，语料更新零负债。
- 实现：`src/repl_doc.rs`（显式 slug 72 + console/SM 派生规则，存在性校验；
  sanitize 修三族宏显示文字 + jsxref；`Dot::Doc` 进点命令分发，TTY/管道共用）。
- 验证：单测 slug 全枚举 72/72 + 黑盒 `.doc` 三路 + TTY pty 实测样式化整篇；
  strict 781/781；冒烟行数全过。坑 4.228（文案撇号）。

## 2026-09-28 浮窗文档实时读（用户纠正会错意后重做）

- 纠正：pane 一句话此前是粘贴副本（94 条），用户裁定"不要搬运，每次直接读"——
  表回签名本位（R3 原样），浮窗摘要改为 Tab 时实时读语料（与 `.doc` 同源）。
- 实现：`__wjs_doc_summary(topic)` native（`builtins/mod.rs` 注册）+
  桥 `withSig` 内拼装（签名 — 摘要）；实例面（变量名）经 `Ctor.key` 再试，
  普通 Object 跳过（用户方法不受染）；桥直调与 Tab 同一代码路径。
- 坑两枚：① APFS 大小写不敏感致 slug 校验假绿，改 git ls-tree 精确校验
  （72/72）；② MDN 源码段落折行，摘要须按段取（首行截断教训）。
- 验证：strict 782/782；行数守门 ok。

## 2026-09-28 浮窗两行化 + 整段文档（用户：签名文档别挤一行，文档不止一句）

- 格式：`签名\n文档`（reedline 描述盒原生多行，50×10 容下）；文档 = 首页首段 +
  首个代码块（调用形状），机械提取（宏解析显示文字、跳引用块），无编撰。
- 验证：pty Tab 菜单签名行/文档行同屏；strict 782/782；行数守门 ok。

## 2026-09-28 文档 pane 浅底整块（irb 式区分，用户要色块）

- 落法：`doc_description_style()`（黑字亮灰底）经
  `IdeMenu::with_description_text_style` 正门刷漆，零布局风险；
  pty 实证描述行包在 `\x1b[107;30m … \x1b[0m` 里（nu-ansi-term 的
  LightGray 即亮白底 107）。
- 行内多色（签名参/返、散文/code 各异）判不可做：reedline 0.52 描述盒
  `split_string` 按字节算宽 + grapheme 裸切分，内嵌 ANSI 会被拦腰切断
  （源码实证 `menu/ide_menu.rs:1014`）；要做须 fork 菜单渲染或等上游，另案。
  行间已按"签名/散文/代码"分行落在同一浅底块里区分。
- 验证：单测钉 Style 值；pty 验块；strict 783/783；行数守门 ok。

## 2026-09-28 浮窗整块文档（用户：签名文档别挤一行，文档不止一句）

- 两处改：① 摘要加料（前两段 + 首个代码块 + Parameters 节机械提取，
  条目边界截断 600 字；`console.timeEnd` 现含 See-Timers 段 + 调用形状 +
  `label:` 参数）；② 描述只放文档（描述盒按空白重排，签名文档同放恒挤成
  一段——`__wjsReplSig` 只做缺页回落；`_斜体_` 去标记，标识符保留）。
- 验证：pty 浅底块内纯文档（`outputs a message` 在 `107;30m` 块里）；
  strict 784/784；行数守门 ok。

## 2026-09-28 签名文档分两块（用户：别挤一行）

- 落法：桥改三元组 `[全文, 左格签名后缀, 描述]`（`__dispDesc` 去名留参，
  非函数/无参即 null；`__descOf` 描述符抽取与 `__sigDesc` 共用）→ Rust
  `display_override = 全文 + 后缀`（左格签名块，只改显示；选中写入仍走
  `value` 原文——点命令 `.he` 线早有同构先例）→ 描述只放文档（右盒文档块）。
- 可行性依据（源码实证）：左列三处全 ANSI 感知（`parse_ansi` 保留转义、
  `strip_ansi` 算宽、`truncate_with_ansi` 截断），右盒 `split_string` 按空
  白重排——分块是唯一正门，行内分色仍另案。
- 验证：pty 左格 `console.info  (...data) — stdout`、右盒纯文档浅底块；
  会话 Tab/回车后求值正常、无签名串泄漏；strict 784/784；行数守门 ok。

## 2026-09-28 右盒纯文档（用户：签名不进候选框；右盒签名上文档下）

- 按截图字面落：候选框干净名（`display_override` 回 `None`，三元组退回对子），
  右盒只放文档（分隔线方案经 `split_string` 仿真证伪：24 字规则窄屏并入文档
  流、宽屏 trailingige sig，恒无干净断行——描述盒按空白重排是铁律，203 字自创
  chrome 已删；签名表仅缺页回落，零搬运原则不动）。
- 验实：仿真 `console.timeEnd` 在 30/40/46/48 列下的重排确认无断行手段；
  strict 784/784；行数守门 ok。
- 环境注记：本轮 pty Tab 探针系统性 `NO RECORDS`（首 Tab 冷 `require` 撞 150ms
  超时）——bisect 证 triple 版同症，系机器负载（Blender/ffmpeg 并跑，load 11+），
  非本轮回归；交互真机验机待负载回落补。

## 2026-09-28 空行 Tab 列全局（用户：别 NO RECORDS）

- 落法：桥展开（`s.trim()===''` → `getOwnPropertyNames(globalThis)` 过标识符、
  隐 `__wjs_` 内部面、排序；completeOn 置空）+ Rust 空前缀一律走动态 +
  `complete_span` 空串回零宽 span（光标处插入）。R3 核心 `bm === null`
  空集不动（node:repl 模块面保守口径，CLI 本体面展开，§7 方向）。
  词法绑定（let/const）不可枚举，同 R3 记档。
- 验证：新黑盒 `repl_empty_line_lists_globals`（量/有序/无内部面/空 completeOn）；
  strict 785/785（中途两连负载抖动：`read_stream_fifo_end` 超时 +
  `set_immediate_not_clamped` 计时，双双单跑 <1s 过，零交集，见 §4 先分类）。
- 待补：交互 Tab 真机验机（首 Tab 冷 `require` 撞 150ms 超时；机器负载
  load 10+ 未消除，bisect 已证非回归）。

## 2026-09-28 文档第 4 路 slug（用户：encodeURI 没文档？）

- 根因：slug 只有三路（WinterCG 显式/`console.X`/`Head.method`），bare 全局函数
  （`encodeURI`/`eval`/`Proxy`/`parseInt`…）直通 `None`。语料里 51 页全有
  （`global_objects/{lower}/index.md`，git 精确校验；`WebAssembly` 住别处不管）。
- 落法：第 4 路 bare 分支（字符集限字母数字/`_`/`$`，无穿越可能；无需 allowlist，
  `lookup` 存在性是唯一真相）。node 私货（`setImmediate`）有形无页，自然 None。
- 验证：单测 slug 形状 + 存在性 + 穿越拒收；`.doc encodeURI` 黑盒；浮窗经
  `__wjs_doc_summary` 自动带出；strict 785/785；行数守门 ok。

## 2026-09-28 文档全接上（用户：mdn 有的全接上）

- 现状：已进仓但够不着的三块——sm_head 白名单外的头（Atomics/Temporal/
  ArrayBuffer…）、Web 静态方法（`URL.parse` 的 `_static` 惯例）、语句/操作符
  （`for`/`typeof`）。`WebAssembly` 等住 `web/` 别处的，不管（没进仓）。
- 落法：`slug` 主规则不动（精确可测），`lookup` 加存在性试探（首中即返，
  段字符集限定防穿越）：`A.b` → `global_objects/a/b`、`web/api/a/b`、
  `web/api/a/b_static`；bare → `statements/t`、`operators/t`、`web/api/t`。
  白名单从此只走快路径，不再是覆盖边界。
- 验证：单测试探四路 + 双 miss；`.doc Atomics.add` 黑盒；浮窗自动带出；
  strict 785/785；行数守门 ok。

## 2026-09-28 空行 Tab 真修（用户：做好了怎么又掉了）

- 根因（实测）：桥本身通（148 项），但交互链恒超时——空行 148 项 × 逐页
  `summary` 实测 **458ms**（warm），远超 150ms 补全窗。黑盒走管道不经超时窗，
  故此前全绿，属测试盲区。认错：只验了桥，没量热路径。
- 三管：`SUMMARY_CACHE`（语料静态纯函数，会话内复用；4.41 无需复位，内容确定性）
  + 启动预热 `require('node:repl')`（REPL 专属，worker/child 不走）+ 超时
  150→800ms（成功路径 200µs 步进即时返回，cap 只管失败显示；覆盖首轮 + 负载余量）。
- 测试补盲：`completer_empty_line_live_response`（有应答假 JS 线程：候选 + 零宽 span）。
- 验证：130 件目标 + strict 786/786（load 21 下一次过）；行数守门 ok。
- 残留诚实注记：load 20+ 时 pty 首 Tab 仍可能撞窗（冷摘要 450ms+ 畸变），
  二 Tab（缓存热）即稳；终极解法是描述懒加载（reedline 无此 API，另案）。

## 2026-09-29 命名空间文档轮（Bun/Deno `.doc` 进语料）

- 语料：`scripts/gen-ns-docs.py`（stdlib）由上游 `.d.ts` TSDoc 抽取 MDN 形状页
  （散文+`## Syntax`+`### Parameters`，`summary_inner` 零改）：bun 29 页
  （`bun.d.ts`+`serve.d.ts`+`shell.d.ts`）+ Deno 53 页（ns+net+unstable；
  `listenDatagram` 注 unstable）+ WinterJS 手写 5 页；MIT 各记 ATTRIBUTION。
- 真机校准三处：`Bun.cwd`/`Deno.statFs` 真机无（删别名，node 面照常用），
  `Deno.readLink` 大写 L（小写旧别名改名）；坑 4.230（prelude 快照死引用）。
- 接线：`repl_doc.rs` 三语料路由（MDN 之后、fallback 之前）+ 签名表 86 项；
  单测 `ns_lookup_hits_generated_corpus` + 黑盒 `repl_doc_ns_pages`。
- 验证：builtins+repl 45/45 strict；vendor 与代码分两提交；冒烟 5/5。

## 2026-09-29 命名空间补全轮（Deno.version. 空集）

- 根因：R3 getter 拒入（刻意）撞上命名空间全 getter 值——`T2` 对照实证
  （数据 ✓/getter ✗/冻结数据 ✓），冻结无辜。
- 修法：值型成员全改数据属性 + `__wjs_ns_sync()`（NODE_PRELUDE 尾经 `__wjs_`
  内部面调，§7 顺向；Deno 刷新后冻结，Bun/WinterJS 保持可写）+ 文档
  `ns_lookup` 认 `global.` 前缀（`global` 是 Node 口径别名）。
- 附带回答：`Deno.version` 求值 `[object Object]` 是对的（`{deno,v8,typescript}`
  对象）；`Deno.args` 在 `--eval` 下空是 argv 本就短，`--run f -- a b` 即有值。
- 验证：builtins+repl 47/47 strict；坑 4.231。

## 2026-09-29 WinterJS.image 轮（15 格式编解码进本体）

- 范围：`WinterJS.image.{formats,info,decode,encode}`；位图 13 格式经 `image`
  （dds 双 false 进不来，avif 不在树内）；svg/svgz 经 `resvg`（解预乘）；
  jxl 经 `jxl-oxide` 首帧。质量参数全透传（jpeg quality/png 压缩+滤波/gif
  speed+repeat/pnm subtype+encoding；webp 无损无参、svg scale 记档）。
- 坑三连（全 abort 级，修后进用例）：ppm-RGBA（4.232）、farbfeld-16 位、
  exr-f32；另 TGA 无魔数须显式格式、jxl 截断零填不报错（上游流式语义）。
- 缺口诚实记：jxl 成功路径缺真 fixture（错误路径全覆盖，跟进项）。
- 验证：全量 strict 811/812（唯一红 `child_stdin_legacy` 单跑即过，负载 flake，
  与本轮零交集）；冒烟 5/5；行数守门 ok。

## 2026-09-29 站导航五分 + WinterJS 文档补齐

- 站（`sample/docs`，中英 luoli + md 镜像）：导航 `Modules` 单组拆五组
  （WinterJS 原生 / Web 标准 / Node 兼容 / Bun 兼容 / Deno 兼容）；表格同分
  （原生表置顶：storage 移出 Web 表，sqlite 拆 node:sqlite + bun:sqlite，
  bun:ffi 移 Bun 表；新增 WinterJS.image/WinterJS/Bun/Deno 四行）。
  行列锚点双向 65↔65 对过；`api-node-sqlite-bun-sqlite` 旧锚改名（站新建不久，无外链）。
- 样例：`sample/winterjs/image.js` + `namespaces.js`（双语头，全离线，`--run` 双绿）。
- REPL：`winterjs-content/image-{decode,encode,info,formats}` 四页（组机制免改码直通；
  加页须 touch 重编见 4.233）+ 签名表 4 项；`.doc WinterJS.image.decode` 可读。
- 附带修：svg scale 宽高取自未缩放 info（像素已缩放）→ 跟随 scale。

## 2026-09-29 站全白抢修（luolita 双雷 + 部署门）

- 修：`native`→`wjs`（保留字）+ cli-zh 两处多余括号（模板行 472a343 落的 +
  actions 行尾 ` }]`，与英文版逐字对）+ `pages.yml` 部署门。
- 校验法：`scripts/check-luoli.js`（原版 coffee 编全部 8 页 coffee: 段；
  另用 pug 去缩进渲染验过 api 表格 5 组 + 新锚点，属一次性探针未进仓）。
- 流程教训记 4.234：推站后等 Actions 成功再看站（本轮即此口径执行）。

## 2026-09-29 站文档单源化（api.md 并入 luoli）

- 结论：luoli 真站与 api.en/zh.md 双源并存且已裂（qrcode 只在 md+REPL 有，
  站上无；md 另有 assert/util/punycode 行站上无）。md 无站外引用
  （README 只链在线站；Jekyll frontmatter 已无消费方），遂删 md，
  luoli 为唯一真相；别名注记（storage/CompressionStream 的 WinterJS.*）
  已搬进 luoli。
- 教训：`git add` 遇已删路径整体失败——删文件走 `git rm` 后同命令再 add
  其余文件会全丢（本轮 cover 行漏提交，靠线上 raw 发现；SHA 钉死 URL
  可破 CDN 缓存验）。

## 2026-09-29 WinterJS.media 轮（symphonia+rodio+rav1e+mp4-rs）

- 拍板四件全进树：`symphonia` 0.6（直引解码）+ `rodio` 0.22（只开 playback，
  解码走直引 0.6——rodio 自带 0.5 子树已摘）+ `rav1e` 0.8（关 default：
  含 git2/nasm/cli，只开 threading）+ `shiguredo_mp4`（git tag
  2026.6.0-canary.0 钉死，零依赖）。
- 面：`decodeAudio/audioInfo`（f32 交错全量，首轨）+ `play/stop`（后台线程
  即返 id，rodio stderr 提示已关）+ `videoEncode`（RGBA→YUV420 BT.601，
  手写 IVF，不另引轮子）+ `mp4Info/mp4Samples/mp4Sample`（demux；mux 跟进项）。
  jxl 成功路径 fixture 同款缺口：mp4 用上游自带 beep-flac 进仓
  （tests/fixtures/media/，Apache-2.0）。
- 验证：模块单测 + 黑盒正常/报错/边界 + 冒烟；全量 strict 待收尾跑。

## 2026-09-29 media 去 unsafe 跟进（bytemuck 直引；W1 收尾 strict 846/846）

- 用户指认 `src/builtins/media.rs set_rval_f32` 业务层
  `from_raw_parts` unsafe 违规（§6 + §0.6）——§6 三问第①问即证伪：
  `bytemuck 1.25.2` 已在锁内（image/jxl-oxide/resvg 带入），直引零新增传递。
- 修法：`Cargo.toml` §12 加 `bytemuck = "1"` + 该行改
  `bytemuck::cast_slice(out)`；余下 `TypedArray::create` 块是 §6 引擎边界
  （不可去），注释正名。`docs/dependencies.md` §16-6 跟进；坑 4.235。
- 验证：模块单测 6/6 + `sample/media/basics.js` 全 true +
  报错路径干净 TypeError + 冒烟 5/5 + 全量
  `cargo nextest run --profile strict` **846 passed / 5 skipped**
  （repl 死代码 3 警告 + linker 环境音 + proc-macro-error2
  future-incompat，皆预存非本轮）。
- 文档顺带刷新：AGENTS 存量基线 615/1453（09-29 实测）、README pitfalls
  236 条到 4.235（4.232 重号两条如实注）、§0.4 加 W1 本体行。

## 2026-09-29 P2-crypto R1：MISSING-EXCEPTION 9 簇（4 转绿，44/120）

- crypto3 sweep（83 件 rerun 中 80 落本轮域）：SAME0=4（padding/gcm-implicit/
  gcm-explicit/secret-keygen）+ TIMEOUT 1（keygen-async-dsa：校验过后跑真 DSA，
  debug 慢超时）+ DIFF 75。crypto 域 40→**44/120**。
- 根因三：① `CbcEnc` 无 autopad 字段 + 两侧 `setAutoPadding` 空转（构造期
  options 亦丢）；② OSSL 错误缺 `reason`、解密坏填充错码（应 BAD_DECRYPT）；
  ③ generateKey 旧 `(options)` 签名 + keypair 入口无同步校验 + 编码形/rsa
  参数无校验。
- 修法：CbcEnc.autopad + `__wjs_cipher_set_autopad` 新 native（UNSAFE-BOUNDARY
  标签 + 注册；黑盒 double-final 覆盖 panic 路径）+ JS 两侧透传；`__cryptErr`
  补 OSSL reason；CbcDec/Ecb 解密坏填充改 BAD_DECRYPT；generateKey 重做
  `(type, options, cb)`（hmac 按位落盘、校验同步抛）；`__checkKeyPairHead/
  TypeKnown/Encs/RsaKeyOptions` + sign.js 同步预检；GCM `__gcmTagLen`
  （{4,8,12-16}）+ setAuthTag 即时校验 + enc 短 tag 前导切片（套件 379 行口径）。
- 真机实测 10+ 处（tag 有效集/setAuthTag 码形/key lengths/inspect 形/async
  同步抛/二参非法/modulus 无缺省…）；翻转旧断言两处（4.65）：sign.js 二参
  默认 `{}`、rsa modulus 2048 缺省——自家 ed448 黑盒同步改显式 `{}`。
- 验证：新黑盒 `p2_crypto_cipher_setautopadding` 14 断言一次过；crypto 域
  30/30；冒烟 5/5；行数守门 ok。坑 4.236（async 同步校验）。
- 留尾（下轮）：keygen 剩余（dsa/ec-curve/dh/pss 参数 + 4096 慢件策略：
  **单件 4096 keygen debug 下 53s，sweep 天花板**，全绿需 release 探或分片）；
  GCM 短 tag 解密验签（native，對称 CTR+GHASH 手工）；argon2 越界；
  pqc 错口令；enc-validation legacy createCipher。

## 2026-09-30 P3-http 冻结收口 R1：matchKnownFields + outgoing-finished 转绿

- `matchKnownFields`：base15 DIFF（`content-encoding:test` vs `test, value`）—
  单例表多收 `content-encoding`/`x-forwarded-host`（返回 `\u0000…` 系可合并，
  单例仅 18 项无前缀）。删两项 + 注释列来源；`probe/dup.mjs` 双侧同串
  （`content-encoding:gzip, br` / `x-forwarded-host:a, b` / `authorization:1` 首个赢）；
  `run1.sh` 0（node 0），坑 4.237。
- `outgoing-finished`：base15 DIFF（`closed` false）—`ServerResponse` 缺 OM 品牌
  五项致 `willEmitClose=false`，`finished` 跑在 `close` 前。构造期补
  `_closed/_defaultKeepAlive/_removedConnection/_removedContLen/_sent100`
  （+ `writeContinue` 置位）；`probe/fin.mjs` 修后 `finish→close→FIN` 与真机同序；
  `run1.sh` 0，坑 4.238。
- `1.0-keep-alive`：`run1.sh` 0（base15 即绿，复验确认）。
- 验证：http 相关 nextest 52/52 strict + 冒烟 5/5 + 行数守门 ok。
- P3 余件按原定性记档（`reuse-drained`=process.report、`client-response-domain`=
  domain 异步、`keep-alive-timeout-race`=Atomics.wait、`set-timeout-server`/
  `catch-uncaughtexception`/`client-parse-error`/`writable-true-after-close`/
  `client-timeout-on-connect`=挂死型），不再开轮。

## 2026-09-30 P2-crypto R2：argon2 越界簇转绿（44→45/120）

- 根因：async 参数校验丢进 microtask（4.236 同族）+ `passes` 下限 0（真机 1）+
  缺参错码（NaN 偷渡）+ nonce/算法文案分码与真机不合（逐项见坑 4.239）。
- 修法：`crypto_kdf.js __argon2Args`（下限/缺参门/文案/算法分码）+ `argon2()`
  先同步全校验再验回调排队（顺序先参数后回调）。
- 验证：`run1.sh test-crypto-argon2.js` 0；crypto 域 nextest 62/62 strict；
  冒烟 5/5；行数守门 ok。
- 全量 strict 附记：P3 后首跑 846 passed + `phase9e_crypto_asym_errors` 1 超时；
  单跑该件 2.2s 绿，判负载 flake（非本轮回归），下轮收尾重跑确认。

## 2026-09-30 P2-crypto R3：enc-validation + pqc-encrypted-pkcs8 转绿（45→47/120）

- `enc-validation`：`__outBuf` 复用吞非法编码（hash 口径），cipher 真机是独立
  粘住门——新 `__cipherOut` + 实例 `__decoder`（首编码粘、`buffer` 不粘、再换
  `cannot be changed`、未知 `ERR_UNKNOWN_ENCODING`；hash 侧不动）。坑 4.240。
- `pqc-encrypted-pkcs8` 三连：① pkcs8+口令导出误走传统 PEM——新 `__pbes2Encrypt`
  （PBKDF2-SHA256/2048/8B 盐 + AES/DES，PRF 带 NULL 与解密对称；der/pem 同构，
  `ENCRYPTED PRIVATE KEY`）；② JWK `AKP` 未实现——`alg` 表 6 集 + 种子形 PKCS#8
  自拼 + 既有展开派生比对 pub（零新 native；三形文案逐字对真机）；③ DER 加密体
  导入不嗅探——显式 pkcs8 + 口令 + PBES2 OID 即先解密。坑 4.241。
- `gcm-*-short-tag`：crypto3 复验双 0（R1 已修，非本轮欠账）。
- 行数：keys 1025/ec 1016 超限 → `split-js.py` 切片（909+116 / 942+74，字节恒等，
  一文件一提交，域测试绿）。
- 验证：crypto 域 62/62 strict + 冒烟 5/5 + `check-lines` ok。

## 2026-09-30 chore：语料五目录并入 `content/`（用户点名根目录乱）

- 布局（用户拍板）：`content/{bun,deno,mdn,winterjs,ns-dts}/`（mdn 内 `files/en-us`
  不动；`vendor/` 消失；历史 pitfalls/journal 旧路径不动）。
- 两提交：①纯搬移（`git mv`，字节不动）；②接线（`repl_doc.rs` 4 处 `include_dir!`+
  注释、`gen-ns-docs.py` 用法 + `out/ns.lower()`、`ns-dts/README` 再生命令、
  bun/deno `ATTRIBUTION.md` 源路径、`repl_complete.rs` 注释）。
- 验证：脚本新 invocation 重跑 29/29 + 53/53 零 diff（幂等成立）；`touch repl_doc.rs`
  重编（4.233）；`.doc fetch/Bun.serve/Deno.readFile/WinterJS.image.decode` 四路全中；
  repl 45/45 + 全量 strict **847/847** + 冒烟 5/5 + `check-lines` ok。

## 2026-09-30 docs(site)：WinterJS sys 一行拆 12 行独立表示 + 独立 sample

- 用户指认 `api-winterjs-sys` 把 shell/hex/time/retry/graph/git/oauth/transpile/log/
  mime/cookie/httpdate 12 个模块挤在一行——中英 `api-*.luoli` 各拆 12 行
  （`api-winterjs-<name>` 独立锚点/说明）+ `sample/wsys/<name>.js` 12 个独立样例
  （双语头、全离线；`basics.js` 已 `git rm`）。
- 联动：`app.luoli navWjs` 1 锚点补 12 个（侧栏按钮走 `data-anchor`，行列双向对上）；
  中英 id 集合 diff 一致。
- 验证：12 样例 `--run` 行行 true；`check-luoli.js` 8 页全 ok（coffeescript 装
  `~/wjs-data/luoli-mod`，不进仓）；余下同类堆叠行（utils/cover/net/proc/conc/os/ev）
  未动，后续同 pattern 跟进。

## 2026-09-30 docs(site)：全站堆叠行拆独立行 + 独立 sample（用户点名"写到一堆"）

- 范围：web 8 行→24 行、wjs 7 行→35 行（utils/cover/net/proc/conc/os/ev）、core
  querystring/punycode→2 行；共 132 行/132 锚，中英 id 集合一致、导航双向零缺口。
  保留别名/变体行（storage/localStorage、fs/WinterJS.fs、`(+/promises)` 系同一事物，
  非堆叠）。
- 样例 78→128：每行独立文件（双语头、全离线；tcp/serve/tls 走回环，git 需检出目录，
  quic 仅接口面）；orphan  lump 文件全 `git rm`（blob-file/streams/compression/events、
  wstd/basics、wcover/b1-b6、codecs/basics 改名 string-decoder 并瘦身）。
- 实测钉住两处真偏离（写进 row notes）：`Request.clone` / `Response.json` 不存在；
  `WinterJS.ffi` 本构建恒 null（`require('bun:')` 不可用）——样例直引 bun:ffi。
- 样例数 50→128：home/quickstart 中英 + 中英 README 同步（"每个模块一件"）。
- 验证：新样例 `--run` 行行 true；`check-luoli.js` 8 页 ok；分三批提交推送
 （`9ce38e6` web / `bbc11bd` wjs / `872b857` core+计数）。

## 2026-09-30 feat(fetch)：Request.clone + Response.json 对齐真机

- 归属确认（用户问"是谁的"）：皆 WHATWG fetch 标准（四家全有），node 26.8.2
  双 `function`，本仓双 `undefined`——引擎真缺口，非文档误标。
- 真机抠出口径三则：① clone().signal 永 fresh（无信号即新未 abort，有信号即跟随
  abort，`addEventListener once` 转接）；② `json(undefined/函数/BigInt)` 即
  `TypeError: Value is not JSON serializable`（`JSON.stringify` 抛错/回 undefined
  一律吞为该错，用户 toJSON 抛错亦同）；③ `json(null)` 体 `"null"`，init 未给
  content-type 才补 `application/json`，坏 status 走构造器 RangeError。
- 实现（`prelude/http.rs` 纯 JS，零新 native）：`Response.json` 静态 + `Request.clone`
  （快照切片/serve 流 tee 源流；bodyUsed 即 TypeError）。
- 验证：黑盒 `phase11_response_json_faces` + `phase11_request_clone_faces` 双绿；
  serve 流 tee 探针绿；fetch+stream 域 48/48；样例回填 + 站偏离注去掉；冒烟 5/5。

## 2026-09-30 P2-crypto R4：keygen-async 簇 22 转绿（47→69/120）

- 真根因（坑 4.242）：异步 `generateKeyPair` 调两次 `__genPairSync`，公钥私钥错配
  （x-JWK/RSA 解密/DSA 超时同源）。同轮带走：jwk 免 type（6 件）/paramEncoding 翻正
  （der/pem 系误读，无覆盖，按 4.65 翻转）/raw 格式放行 + 兼容门提前（省 RSA/DSA
  慢生成空烧，raw 套件 9s→0.9s 快败）/JWK 曲线错码/传统缺口令改 INTERRUPTED/
  DSA 参数校验（`{}` 缺省翻转，4.65）/P-521 掩码（528 位随机 99.2% 越界）/
  PSS 约束回贴 + 非 PSS padding 禁  + salt 缺省摘要长/promisify 定制/ml
  details `{}` + 私钥 JWK pub 派生/MISSING_PASSPHRASE（DER）。
- 验证：crypto4 sweep（crypto3 红 76 件重跑）SAME0 25（R2/R3 3 件 + 本轮 22）；
  crypto 域 62/62 strict；冒烟 5/5；行数守门 ok。
- 留尾记档（时间盒止损）：bit-length（DSA-2049，dsa 0.7 无任意尺寸）/
  dh-classic（无 dh keygen）/keygen.js（4096 debug 慢天花板）/raw-slh
  （需新轮子，§0.5 待批）/pqc-key-objects×2 + sign-verify（priv-only 形态保持，
  material 模型改）/GCM 短 tag 解密/legacy createCipher。

## 2026-09-30 chore(rename)：全仓 winterjs/WinterJS→winterjs2/WinterJS2 + 仓库改名

- 动因：与 wasmerio/winterjs 重名（README 旧注记），改 winterjs2 避让。
- 范围（用户拍板三条）：logo 双 SVG 改名 + 内文 WinterJS→WinterJS2（svg 格式名不动）；
  历史文档（pitfalls/journal/plan 存档）不动；域名 winterjs.bemly.moe 不动。
- 三提交：`0830586` 机械改名（595 文件：包/二进制/JS 全局/`__wjs2_`/`WINTERJS2_`/
  tracing/语料/样例/站/活文档）+ `1f55941` 四处修复 + 本记账。
- 改名抓出真 bug 三处：env 前缀裸 `WINTERJS`（pattern 漏下划线）、prim 信封
  `slice(11)` 未跟前缀变长、c4x 向量消息串误改（fixture 禁改名）。见坑 4.243。
- 验证：全量 strict 849/849；冒烟 5/5；站 132/132 + 门绿；`gh repo rename`
  winterjs→winterjs2，remote 已切，RAW_BASE 200，Pages 成功，站 200 且新品牌。

## 2026-10-03 P2-process R1：hrtime/nextTick/chdir 校验 + release 面（21→25/82）

- 真机口径（node 原文：`lib/internal/process/per_thread.js` hrtime/
  `task_queues.js` nextTick/`does_own_process_state.js` chdir）：hrtime 非数组→
  `ERR_INVALID_ARG_TYPE`、非 2 元→`ERR_OUT_OF_RANGE`；nextTick 非函数→
  `ERR_INVALID_ARG_TYPE`；chdir 非串→`ERR_INVALID_ARG_TYPE` + 缺失路径→`ENOENT`
  （code/errno/syscall/path/dest 五件）；release `{name:'node',lts:'Jod',…}`
  （versions.node 22.12 口径）。
- hrtime 差值借位：单 BigInt 取余带负号致 `diff[1]<0`（真机秒/纳秒分开减+借位，
  nodejs/node#4751）——改分减后 `d[1]` 恒 ∈ [0,1e9）。
- hrtime 余 `%PrepareFunctionForOptimization` 行：V8 私有语法，SM 永 SyntaxError——
  引擎边界记档（hrtime-bigint.js 同理），时间盒止损。
- 工具链事故：改名漏了 sweep-bg.py/run1.sh 默认二进制路径，proc1（21/82）系旧
  二进制无效基线；修后 proc2（`target/debug/winterjs2`）25/82 方有效（坑 4.244）。
- 验证：run1.sh 4/5（hrtime 记档）；黑盒 `phase11_process_validation_faces`
  （正常+报错+边界 11 断言）；process 域 nextest 13/13 strict；冒烟 5/5；
  `check-lines` ok。

## 2026-10-03 P2-process R2：abort/内存/cpu/umask 面（25→32/82）

- 真机口径：abort 箭头函数（无 prototype/new 即 TypeError，实际 abort 由套件
  行为外保证）；available/constrainedMemory 经 sysinfo 真值（后者无约束回总量，
  套件要 number）；cpuUsage/threadCpuUsage 逐字移植 wrapProcessMethods
  （对象门→user 数门→user 范围门→system 同序）；umask 串形八进制门+数形
  uint32 门（`parseFileMode` 逐字）。
- 底座三事（坑 4.245）：macOS 无 `RUSAGE_THREAD`，线程 CPU 走 Mach `thread_info`
  真值（`mach_port_deallocate` 需手补 extern，§6 三问）；`THREAD_BASIC_INFO`
  i32→u32；errors 端口无 RangeError 子构造，范围错手拼码名文案三件。
- 余 thread-worker：卡 `crypto.randomBytes` 64KiB 配额（proc2 旧红同源），
  crypto 配额域另案，时间盒止损。
- 验证：run1.sh 7/8；黑盒 `phase11_process_resource_faces`（14 断言）；
  process 域 nextest 14/14 strict；proc3 sweep 25→32/82；冒烟 5/5；
  `check-lines` ok。

## 2026-10-03 P2-process R3：POSIX 身份设置簇（32→36/82）

- 真机口径（`does_own_process_state.js` wrapPosixCredentialSetters 逐字）：
  setuid/setgid/seteuid/setegid（validateId 数串双形 + 未知身份错 +
  EPERM 系错）/setgroups（数组门 + 逐元素门 + 未知组位错）/initgroups
  （双门 + 先解组后解用户，双未知报组错）。
- 真凶一枚（坑 4.246）：native 返回码 0/1/errno 三义共 int，EPERM 本体即 1——
  非 root 下全被误读成"未知身份"；改 0/1/-errno，setgroups/initgroups 早同口径。
- 拆分（§0.9 超限）：`process_.rs` 1104→897，身份系整块搬
  `process_cred.rs`（217 行，字节恒等，注册改址；另顺手清 `///`-on-extern
  与 mach deprecate 两告警，dsa.rs 同款 allow）。
- 验证：run1.sh 4/4；黑盒 `phase11_process_credential_faces`（无副作用路径 9 断言）；
  process 域 nextest 15/15 strict；proc4 sweep 32→36/82；冒烟 5/5；
  `check-lines` ok。

## 2026-10-03 P2-process R4：kill/原型/title 面（36→41/82）

- 真机口径：kill 逐字移植（`pid != (pid|0)` 松散门 + 信号数形直通/名形查 os 表/
  `this._kill` 可 mock + errno 成错；native 纯数值 kill）；原型链按真机五断言
  复刻（proto≠EE.prototype 但链上含之 + 自有 constructor 槽；fixup 拼在
  REQUIRE_PRELUDE 之后，prelude 主体求值时 require 尚无）；title 零 Rust 改动
  （--title 末个赢读 execArgv，缺省回 execPath 基名，set 透写）。
- 附带转绿：remove-all-signal-listeners（自发 SIGINT 面；自 spawn 可用，
  先前"CLI 裸参家族不可为"的判断被证伪——execpath/ppid/really-exit 等系
  缺 API（ppid/reallyExit）而非 CLI 挡道，R5 续啃）。
- 验证：run1.sh 4/4；黑盒 `phase11_process_kill_prototype_title_faces`（8 断言，
  kill 只验参+自检不真发信号）；process 域 nextest 16/16 strict；proc5 sweep
  36→41/82；冒烟 5/5；`check-lines` ok。

## 2026-10-03 P2-process R5：ppid/reallyExit/软链自举面（41→44/82）

- 真机口径：ppid（getppid 原生 + 模块双导出）；exit 经 exitCode setter 再派发
  后走可 mock 的 reallyExit（默认哨兵退出；mock 后代码继续跑，真机同）；
  软链自身按 canonical 进 `--run` 自举（execpath 套件）；execPath 原生侧
  canonical（软链起亦与 realpath 同）。
- 验证：run1.sh 3/3；黑盒 `phase11_process_spawn_faces`（ppid/路由/canonical
  4 断言）；process 域 nextest 17/17 strict；proc6 sweep 41→44/82；冒烟 5/5；
  `check-lines` ok。
- 余 35 件分簇（R6 候选）：execve×7（实验面）/ exception-capture×4 /
  env×6 / redirect-warnings×2 / 散件（binding/config/spawn 系/V8 私有语法）。

## 2026-10-03 P2-process R6：execve 镜像替换簇（44→51/82）

- 真机口径：execve 逐字移植（worker/平台双门 + 路径/参数/env 三校验 +
  成功不返回；自身软链/直链补 --run，argv 保持 node 形；失败成系统错，
  ENOENT 口径 `ENOENT, text 'path'`）。
- 附带两修：inspect 转义表 0/7/11/27 按真机 `meta` 对齐（坑 4.247；
  全局影响，全量回归 cover）；errors 端口按需加 `ERR_WORKER_UNSUPPORTED_OPERATION`
  （"按需追加"既定纪律）；kill/execve natives 续搬 `process_cred.rs`
  （kill 块字节恒等；`process_.rs` 纯删）。
- redirect-warnings×2 记档：fork 线程底座吞 execArgv（校验过即弃），属 G6
  infra 族，另案。
- 验证：run1.sh 7/7；黑盒 `phase11_process_execve_faces`（报错面 6 断言，
  真调替换测试进程故不做成功面）+ `phase11_inspect_control_escapes`；
  全量 nextest strict 856/856；proc7 sweep 44→51/82；冒烟 5/5；
  `check-lines` ok。

## 2026-10-03 P2-process R7：uncaught capture 路由簇（51→55/82）

- 真机口径（execution.js）：set/has + null 清除 + 重复设置错 +
  capture 优先于 uncaughtException + 接住后进程续活（exit 0）；v8 桩
  `setFlagsFromString`（收下不兑现；abort 变体 node 侧亦 rc=0，行为一致）。
- 入口改道（CJS/经典双路）：失败先走 `dispatch_entry_throw`（与 Rust 异步侧
  共语义），接住转事件循环，无人接放回 pending 走原 fatal（文案/栈无损）。
- 真凶一枚（坑 4.248）：带 pending 进 JS_CallFunctionValue 非法致裸错渲染
  `undefined`——take-调-放回三段，stash 对照实锤。
- 验证：run1.sh 4/4；黑盒 `phase11_process_capture_faces`（正常+报错+边界）；
  process 域 19/19、全量 nextest strict 857/857；proc8 sweep 51→55/82；
  冒烟 5/5；`check-lines` ok。
- 余 24 件分簇（R8 候选）：env×6 / 散件（binding/config/exit/spawn 系/
  redirect 记档/V8 私有语法记档/thread-worker 配额记档）。

## 2026-10-03 P2-process R8：env Proxy 全家簇（55→60/82）

- 真机口径（同源五件一处改）：原型回落（hasOwnProperty 等走 target）、符号键值
  纪律（读 undefined/写严格抛）、defineProperty 双文案门、DEP0104 警告后照赋、
  空键静默忽略；allowedNodeEnvironmentFlags 按 per_thread.js 结构移植
  （310 表真机 dump + 定制 has 归一化 + 空本体/过滤迭代 + 冻结）。
- 真凶一枚（坑 4.249）：Rust `set_var` 遇空键 panic（rc=139），真机静默忽略——
  native 先拦；定位靠临时 eprintln 看序列（删）+ 包 native 抓 JS 栈。
- 记档两件：env-tz（SM 引擎侧时区缓存清不动，坑 4.250；`tzset` 保留作 hygiene）、
  redirect-warnings×2（沿 R6 记档）。
- 验证：run1.sh 5/6；黑盒 `phase11_process_env_faces`（9 断言，补 `use strict`）；
  process 域 nextest 20/20；proc9 sweep 55→60/82；冒烟 5/5；`check-lines` ok。
- 余 19 件分簇（R9 候选）：binding×2/config/exit×2/spawn 系散件/finalization/
  get-builtin/setsourcemaps/ref-unref/warnings（+既有三记档）。

## 2026-10-03 P2-process R9：杂项收尾 11 转绿（60→71/82）

- R9A（8 件，`30fd3f3`）：exitCode 校验（validateInteger 双错口径 + null/unset +
  delete 不可删经 Proxy 对 V8 文案 4.251）/ `process.binding`（util 16 键恒等 +
  未知即 `No such module`）/ config 深冻 / `_rawDebug`（直写 fd 绕 hijack）/
  `setSourceMapsEnabled` 布尔门 / ref-unref（Symbol.for 优先）/
  `getBuiltinModule`（非串抛、归一化失败回 undefined 4.253；裸 test/internal
  显式拦；builtinModules 新风格六件只发前缀形）/ timers/promises 补 default
  （`import.default===require` 4.253）/ binding 表补 buffer。黑盒 r9（16 断言）。
- R9B（3 件，`bbc5bb0`）：spawn stdio 数字形收 inherit / `--disable-warning`
  按 code/name 过滤（含 NODE_OPTIONS 同源，逗号串天然不支持）/ monitor 路由
  （先行 throwing 版 emit + count 含 capture/monitor；监听再抛即新错 exit 7、
  `_fatalException=undefined` 即 6，fatal 保留 6/7 余下盖 1，4.252）。
  黑盒 r9b（monitor 双触发 + disable 精确过滤 + fatal6 exit 6）。
- 拆分（§0.9）：`process_ids.rs`（身份族 53 行字节恒等）+ prelude 切三片
  `concat!(include_str!)`（48176 字节恒等）；`state/mod.rs` 压回 1000。
- 验证：proc10 sweep（proc9 红 22 重跑）SAME0 11 → **71/82**；process 域
  nextest strict 14/14；冒烟 5/5；`check-lines` ok。
- 余 11 定性：既有记档 6（env-tz 4.250/hrtime×2 V8 私有/redirect×2 fork-infra/
  thread-worker crypto 配额）+ **另案 2**（P2-R10a spawn-ipc 通道 infra；
  P2-R10b 真 GC native + finalization 语义）+ 双红 SAME1 3（dlopen/features/
  load-env-file 环境漂移，不计欠账）。

## 2026-10-03 P2-stream R1：eos/连字符簇 6 转绿（stream 156→162/215）

- 内容（`48ce7fd`）：`enabledHooksExist` 导出 + AsyncResource 构造期同步触发
  init（STREAM_END_OF_STREAM 上下文传播）+ `node:internal/async_hooks` 门面
  （同源双实例分叉 4.254）+ internal 连字符回落（精确优先 4.255）+
  `tty_wrap.TTY` 空类（不可枚举即绿）+ 黑盒 `phase11_stream_r1_eos_hooks_faces`。
- 验证：stream2 sweep（stream1 红 60 重跑）SAME0 6 → **162/215**（75%）；
  stream/async_hooks/process/binding 域 nextest 68/68；冒烟 5/5；`check-lines` ok。
- 转绿 6：finished-als/bindAsyncResource-path/default-path、add-abort-signal、
  base-prototype-accessors、stream2-httpclient-response-end（搭车：res-end 经
  finished 内部，ALS 修复连带）。
- 余件定性：finished.js（http RST 联动，net 底座偏离 4.x 既有记档）/
  **R2-iter 专项**（~40 件：flag 门控 + 12 文件 7.5k 行移植，另轮）/
  consumers 锁码 + TextDecoder 码（Web 面改码涉 whatwg 回归，R2 先对真机）/
  duplex/pipeline-preprocess/readable 系（R2 逐件）/ destroy/pipeline TIMEOUT×3
  （挂死型，R2 末）。

## 2026-10-03 P2-stream R2：stream/iter 门控面落地（162→184/215）

- 内容（`1e5dfdf`）：`stream/iter` + `zlib/iter` 旗门控注册（无旗即未注册，
  双路真机文案 4.260）+ `internal/streams/iter/*` 12 件逐字移植（生成器
  `scripts/gen-iter-ports.py`；pull 超限切两片 include_str 字节恒等）+
  管道件：primordials +26 / validators +2 / internal/util +3 /
  task_queues 新件 / E() 变体类 4.256 / fatal 首行 `[码]` 穿透 /
  internal/types 跨 realm 4.258 / classic 可选链改 `__reg` 4.257。
  黑盒 r2（门控双形 + 回环 + 跨 realm + 变体类）+ cli `[码]` 三件。
- 验证：stream3 sweep（stream2 红 54 重跑）SAME0 22 → **184/215（86%）**；
  全量 nextest strict **863/863**；冒烟 5/5；`check-lines` ok。
- 转绿 22：iter broadcast/from/namespace/pipeto/pull/push/share
  基础簇 20 + disabled×2 + cross-realm + readable-interop-disabled。
- 附带修（全量回归抓出）：http upgrade 黑盒 stale 监听改 once（4.259，
  真机同败）；timers 黑盒按真机改 CJS 双取比法；path/require 黑盒文案翻转。
- 余 32 定性：transform×5 + interop/to-readable 的 zlib 块 + fs-pull 的
  zlib 块（**R3-zlib 句柄 shim**：`internalBinding('zlib')` 原生流协议，
  另轮）/ node 侧红×7（node 自败，我方过，版本漂移记档）/ SAME1×5
  （双红不计欠账）/ 非 iter 旧红 12（consumers 锁码、destroy/pipeline
  TIMEOUT×3、duplex、finished-RST、preprocess、readable 系、writable 系，
  R3 逐件）。

## 2026-10-03 P2-stream R3：zlib shim + 语义收尾（184→198/215，92%）

- R3a（`e0b8cc0`，+8）：缓冲式 zlib 句柄 shim（协议对真机 C++、压缩经同步
  引擎；`ZSTD_e_flush` 按 PROCESS 攒；常量/缺失档补齐；坑 4.261）。
  转绿：cross-realm（顺带）、interop、to-readable、transform×5。
- R3b（`7759429`，+6）：writable-destroy 回滚逐字（OM 归属纠正 4.262，
  http writableFinished 系既有红另案，http 冻结不动）/ duplex DEP0201 /
  to-web type 校验（文件滞留 BYOB 读 hang 另案）/ pipeline-process（stdin
  pipe + stdout finish/close）/ preprocess（BOM 双处 4.264）/ consumers
  （锁码 + TD 码）/ samecb（console 复用回调 4.263）/ pull 切片恢复。
  黑盒 r3（shim 回环 + 锁码 + tick + BOM）；全量 strict 863/863；冒烟 5/5。
- 余 17 定性：TIMEOUT×3（时间盒记档）/ finished-RST（net 底座偏离）/
  byob 读 hang（字节流 plumbing 另案）/ node 侧红×7（node 自败记档）/
  SAME1×5（双红不计）/ fs-pull×3（FileHandle.pull，fs 域另案）。
- 域止损：92% 未达 95% 线，但可转绿件已空（余件皆 infra/另域/双红），
  按 0.2 时间盒收官，余件批量记档。

## 2026-10-03 W1 本体盘点（只读探针 + 门面回归）

- 探针（单发 `--eval`，无构建）：`WinterJS2.image.formats()` 16 项 /
  `WinterJS2.media.formats()` 12 项（`decode/encode`、`audioInfo/decodeAudio/
  play/stop/videoEncode/mp4Info/mp4Samples/mp4Sample` 具在）/
  `WinterJS2` 命名空间 60+（semver/yaml/jsonc/ip/shlex/spdx/qrcode/shell/
  hex/time/retry/graph/git/oauth/transpile/log/mime/cookie/httpdate 等，
  即 wstd/wsys/wcover B1-B6 门面落点）/ `sample/wsys/shell.js` 全 true。
- 回归：`cargo nextest run -E 'test(wstd) or test(wsys) or test(wcover) or test(repl)'`
  **71/71**（wstd 11 + wsys 15 + wcover 21 黑盒 + repl 面；R3 后代码亦绿）。
- 余项（W1 行 🟡 未动）：`.doc`/浮窗文档语料增量、` WinterJS2.image` 15→16
  格式计数行文同步（plan3 W1 行写 15，实测 16）、站单源化部署门。
  repl 求值面（TLA + acorn 声明提升）与 console 统一既判 ✅（§7），不再复验。

## 2026-10-03 另案 tickets（R10a/b + fs-pull + byob）

- **R10a spawn-ipc 通道**：`test-process-external-stdio-close-spawn`
  （`spawn(..., {stdio:['pipe','pipe','pipe','ipc']})` + `child.send('go')`）。
  现状：spawn 四元 ipc 占位忽略，真 IPC 只走 fork（parentPort 桥）。
  入口：`src/builtins/node/child_spawn.js __normSpawnAsyncOpts` +
  `child.rs` spawn 通道。验收：套件绿 + fork 回归。
- **R10b 真 GC + finalization**：`test-process-finalization.mjs` 6 fixtures
  （close/before-exit/cleanup/gc-not-close/unregister/per-thread）+
  `register(undefined)` 校验。现状：`process.finalization` undefined、
  `--expose-gc` 系 async no-op。入口：SM GC 可观测点（`JS_GC` 触发 +
  FinalizationRegistry 对接）+ `bootstrap/node.js` 惰性 getter 移植。
  验收：6 fixtures rc=0 + `register(undefined)` 抛物校验。
- **fs-pull×3**（`file-handle-pull/pullsync/writer`）：`FileHandle.prototype.
  pull/pullSync/writer` 未实现（`fh.pull is not a function`）。入口：
  `src/builtins/node/fs.js` FileHandle 类 + stream/iter pull 桥接。
  验收：三件绿 + fs 域回归（fs 333 件）。
- **byob 读 hang**（`test-stream-readable-to-web-byob`）：`type` 校验已补
  （R3b），残 BYOB `read(view)` 永挂（泵用 enqueue 无 respondWithNewView）。
  入口：`prelude/streams.rs` 字节流 BYOB 分支。验收：单件绿 + webstreams
  域回归（现 100%）。

## 2026-10-03 http2 决策双分支 scope（P1，待 base16 定量后拍板）

- 现状锚点：P1 首轮 88/256（34%）；base15 红 168 聚类：push 系 8 /
  ALTSVC-ORIGIN 2 / settings 6 / tls 系 3 / session-connect-server-client
  大盘 ~100（待 base16 新鲜 `wjs_err` 重聚，9-25 后共享底座动过）。
- **分支 A：h2 裸库直驱**（hyper 高层→h2 0.4 `Connection` 直驱；轮子已在树
  内零新增）。改动面：`http2_client.rs`（`hyper::client::conn::http2`
  → h2 client 握手 + `push_promises` 接 `pushStream` + `send_data/trailer/
  reset` 直调）/`http2_server.rs`（h2 server accept + `push_request` 接
  `pushStream` + SETTINGS 出入 + GOAWAY/窗口/RST 码映射）/ compat JS
  薄层保留。覆盖：push 8 + settings 半数 + GOAWAY-窗口-RST 映射簇；
  不覆盖：ALTSVC/ORIGIN（h2 无此帧，仍偏离）。预算：约 1 周（含 h2c/H3
  回归 + compat 黑盒）。
- **分支 B：书面偏离**：http2 冻结 88/256（+base16 增量），bun-parity
  `## http2` 记一行"hyper 底座不可达：PUSH/ALTSVC-ORIGIN/对端 SETTINGS
  内省/原始帧错映射"，P1 收官。零工时。
- 拍板条件：base16 http2 红中"h2 可达且非 tls"件 ≥20 → A；否则 B。

## 2026-10-03 终局前置核对表（数字待 base16 回填标 *）

| 验收项 | 状态 |
|---|---|
| §1 矩阵 Bun 🟢 全✅ | 待 base16：http* / http2（决策中）/ stream 92%（止损收官）/ 其余域上次全绿待刷新 |
| §1 矩阵 Bun 🟡 parity 确认 | 待补：逐域一行（现有 bun-parity 明细可直接转正，仅 http2/process/stream 三行待本轮数） |
| 10f 对拍报告入库 | bun-parity.md 现 1795 行可用；缺 base16 一轮 + http2 决策附录（本批已写 scope，结论待补） |
| `cargo test` 全绿 0 警告 | ✅ 当前（strict 863/863；警告剩预存 5：repl 死代码×3 + linker + future-incompat） |
| 冒烟 5/5 | ✅ 当前 |

## 2026-10-04 base16 全域基线（2136→2313，+177）+ R16 真回归分诊

- base16（`~/wjs-data/sweep/base16/`，3574 件，2 并发 + 看门狗，~41min，资源正常）：
  SAME0 **2313** / SAME1 27 / DIFF 1103 / TIMEOUT 131。§0.3 表已按
  `sweep-report.py base16` 全量回填（base15 表 queries 经 git 历史可查）。
- TIMEOUT 25→131 分诊（先算形态翻转矩阵再算涨跌，4.270）：107 新增中 **105 系
  base15-DIFF 翻转**（`wjs=1` 快败 → `wjs=142` 挂死；P2 把 server/socket 做对后
  套件多活到等永不到的事件，cluster 'listening' 中继本就未实现），真新 hang
  仅 2（dns-channel-timeout 系 FLAKY 单跑绿；http-catch-uncaughtexception 真 hang 另案）。
- 真回归（绿→红）15 件：单跑复验 3 FLAKY（http-1.0/byetzswritten/dns-channel-timeout，
  记档不追）+ 12 确定性。
- **R16a 修 4 件 5 处**（黑盒各配，strict 待终检）：exit receiver 守卫
 （cluster-net-listen，4.265）/ throwDeprecation 改 nextTick 异步抛
 （process-warning，4.266）/ eddsa options 缺省 `{}`（注释翻转 4.267）/
  h2 lookup 透传 + promisify.custom 逐字补（promisify-connect-error，4.268）。
- **R16c 记档 10**：TLS 服务端同字节双派发（default-port/request-agent/
  url.parse-https/set-default-ca，单 st 双 __feed，p9-count 复现，4.269 未修）/
  1.0-keep-alive（1.0+TE:chunked 响应缺终结 `0\r\n\r\n`）/ catch-hang（uncaught
  后 req close 不到，p11 复现）/ tlswrap-segfault（ssl.fd 生命周期）/
  worker-handle-close（fd 传递 internalMessage 另案家族）/ 3 FLAKY。
- stream 197（自称 198 差 1 = TIMEOUT 件归属口径）/ process 69（自称 71 差 2，
  warning 已修 + SAME1 件）。
- R16a 验证口径：5 件逐件 `run1.sh` 单跑转绿（新二进制）+ 黑盒 4/4（nextest），
  未跑全量 `--rerun-red`（1261 红件约 20 分钟，单件证据已足；下轮域 sweep 自然复验）。
- http2 169 红名簇：push 6 / ALTSVC-ORIGIN 2 / settings 6 / tls 3 /
  帧流控映射 19 / connect隧道 10 / compat 26 / session大盘 97（含 ECONNREFUSED×6、
  原始方法缺失 performServerHandshake/internal/http2/core×2、头形 strictEqual 簇）。
  123 DIFF 皆有 wjs_err 可复核（见 parity 附录）。
- **http2 拍板输入（待用户）**：原 A（h2 直驱 ~1 周）/B（书面偏离）之外，base16
  证据支持 **C（compat-JS 轮，不动底座）**——compat 26 + session 大盘中头形/
  方法缺失/ECONNREFUSED 约 50+ 件属 JS 壳可修面，与"hyper 底座不可达"两回事；
  真底座 bound（push/ALTSVC-ORIGIN/帧错映射/tls ≈ 35）走 B 偏离。
  建议：B（35 件偏离）+ C（compat 轮按名簇逐批），A 否决（h2 直驱不解决壳 gap）。

## 2026-10-04 撤销止损规则（用户拍板）

- 用户审阅后撤销 09-25 三条止损/范围限制（原 §0.2 域止损线、单件时间盒、
  清单外不修），plan3 §0.2 已改：只保留"超 Bun 的保留"+"清单内红件进 0.4 队列"。
- §0.7 已清出名单整体失效回炉：出镜红件照常逐件开轮，不再享受"清单外"豁免。
- 历史记录（journal/parity 旧条目、0.4 表内"按时间盒暂停/按止损线收官"字样）
  为当时事实，不改写。

## 2026-10-05 fastutf8stream 簇 14 转绿 0/14→14/16（Utf8Stream 落地）+ fs 回调时序统一

- 范围：Bun 清单 fastutf8stream 域 14 件全红，根因单一——`node:fs` 缺
  `Utf8Stream` 导出（`TypeError: Utf8Stream is not a constructor`）。
- 落地：`internal/streams/fast-utf8-stream.js` 918 行逐字移植
  （`src/builtins/node/internal/streams/fast_utf8_stream.rs`，require→垫片，
  primordials 直引；`node:fs` 懒 getter 对齐 node `lazyLoadUtf8Stream`；
  循环依赖经构造器内 `__ensureFs` 懒访问解环，§4.59 家族）+
  INTERNALS 注册（80）+ `ERR_INVALID_ARG_VALUE` 补 `RangeError` 变体
  （node 原文 `TypeError, RangeError, HideStackFramesError`，坑 4.277）。
- 时序：`fs.read/write/readv/writev` 改"同步执行、回调仍派发"
  （node 线程池 FIFO 最近似，坑 4.276）——`flush-sync`/`destroy` 落盘序对齐，
  黑盒 `fd-read null 2 he` 断言零改动（真机 5/5 确定性）。
- 验证：node 套件 fastutf8stream **16/16**（清单 14 + 同目录另 2；
  余 2 件 destroy/flush-sync 经时序修转绿）+ 黑盒 `fs_utf8stream_surface`
  （正常/报错/边界 11 断言，4.271 合规名）+ fs/stream 域 73/73 +
  fs 域 sweep SAME0=286/SAME1=4/DIFF=42/TIMEOUT=1（= base16，零回归）+
  strict **869/869** + 冒烟 5/5 + check-naming/check-lines 双绿。
- 环境注：本轮中段 `worker_terminate_interrupt_busy_loop` 在 strict 里挂，
  查为另一会话同仓并发构建（load 4.71）致 terminate 竞态抖动
  （TRY1 败/TRY2 过判 FLAKY，空载稳过），非本轮回归（坑 4.275）。

## 2026-10-05 readline/webstreams 簇 +7（async-iter 3 + webstreams 4）

- readline async-iter 3 件（+3，4→7/20）：`rli[Symbol.asyncIterator]()`
  自造迭代器缺 `watermarkData`——改经 `EventEmitter.on(this, 'line',
  {close:['close'], highWaterMark:1024, 第一参直传})` 委托（node 原文），
  附带修 EOF 残留行（onend 先发残留 line 再关，node onend 口径）。
- webstreams 4 件（+4，3→7/10）：
  - `kIsClosedPromise`（finished/compose 件）：prelude 原生流补内部符号，
    以 closeWaiters 直供；`.closed` 不补（node 26 已移除流级 `.closed`）。
  - `kControllerErrorFunction`（abort-controller 件）：controller.error
    直达（字节流 no-op，node 同款）。
  - close-waiter 独立成队（不吃 chunk，eos 吞块修）+ pull 早退顺带 pump
    （读消费末块后 closeWaiter 结算）+ tee 源出错/收尾主动结算分支。
  - close 后再 error 转 errored（abort-controller 在关流上 abort 口径）。
  - adapter ArrayBuffer→Uint8Array（writable-buffer-sources 件）。
  - CompressionStream 坏块 TypeError 空文案 + 引擎码（bad-chunks 件；
    通用 writer.write 去无码 undefined 守卫 + sink 抛前先 fail 读端）。
- 回退记：kIsClosedPromise 排空感知不可去（有队关闭无人读 node 永不结算，
  实测）；closeWaiters/drainWaiters 曾拆队又合并（单队+排空条件即足）。
- 验证：node 套件 webstreams 7/10（余 compose 深水路由 + adapters-sync
  需 expose-internals loader 特性，记档）/ readline 7/20；黑盒
  `stream_web_interop_symbols` + `readline_async_iterator_faces`；
  stream/readline/fs/fetch/compress/zlib 黑盒 96/96；冒烟 5/5。

## 2026-10-05 module/v8 簇 +5（module 3 + v8 2）

- module 3 件（+3，4→7/27）：`builtinModules`/`isBuiltin` 缺废弃别名
  `sys`（require.rs 内建表尾补裸形）+ `process.config.variables`
  缺 `node_module_version`（补 147，实测真机）。
- v8 2 件（+2，2→4/13）：`v8.serialize/deserialize` 自洽二进制往返
  （标量/串/Buffer/ArrayBuffer/视图/DataView/数组/对象；函数拒；
  非 V8 线格式记档）+ `process.memoryUsage.rss()` 独立函数；
  serialize-leak 附带转绿（--expose-gc gc 既有 + 断言 <10x 宽松）。
- 验证：黑盒 `v8_serialize_faces` + `process_config_rss_faces` +
  nodemodule 扩展断言；process/v8/module 黑盒 36/36；冒烟 5/5。

## 2026-10-05 console 簇 +2（instance/methods；group 记档待布局引擎）

- instance/methods 转绿（3→5/16）：Console 类改 function 形态（无 new 可调、
  custom instanceof 认全局、构造器内箭头包装+正名——forEach 点名绑定、
  new/Reflect.construct 点名不可构造、子类重写优先）；流校验收紧
  （stderr 缺省跟 stdout；无 write 即 ERR_CONSOLE_WRITABLE_STREAM；
  inspectOptions 非对象 ARG_TYPE，文案对 invalidArgTypeHelper）；
  写错透传原错；全局正名+别名搬 REQUIRE_PRELUDE 尾（shim 后加载才稳）；
  require('console')===全局（default 导出本体 + Console 侧表破自循环）；
  原生余方法包构造守卫。
- group 记档：组缩进已落地，余对象多行——node inspect 缺省 breakLength 80
  的布局引擎（本仓 inspect 单行），动全局换行会摇 util 绿件，深水另案。
- 验证：node 套件 console 5/16；黑盒 console 4/4；冒烟 5/5。

## 2026-10-19 cluster P0/P1/P2 轮（25/80→58/80，TIMEOUT 27→6）+ url internal 轮

- 范围：cluster 域 80 件；base 25 绿/28 DIFF/27 TIMEOUT（base16）→
  cluster-p0 30/80（P0 中继 + P2 扇出）→ p1 45/80（P1 构造器/state/destroy/
  setup/cwd/进程桥/退出语义）→ p3 56/80（ack/关完再派发/early-ctl）。
  驗證：黑盒 cluster 4/4、冒烟 5/5、check-lines/check-naming 双绿；
  `child_exec_shell_self_and_timeout` 系 HEAD 预存红（stash 对照同红，非本轮）。
- P0 listening 中继：worker 内 `net.Server.prototype.listen` 补丁，
  起服即报 primary（`{address,addressType,fd,port}` node 形，basic 套件点名），
  跨线程 fd 共享仍无能力（disconnect 套件 round-2 拒连，记档另案：同端口共享）。
- P2 消息扇出 + error 转发 + 起步缓冲（fork 后即达消息攒后重放）+
  真机 ack（worker 主动断开等回执，race 套件）+ 强制退出路（forced-exit）。
- P1：Worker options 构造器 + 函数形（`Worker.call`）+ state/exitCode/
  signalCode + destroy 双侧 + setupPrimary 真机缺省/合并/emit + cwd 虚拟化
  （线程禁真 chdir，`process.cwd` 复写，相对路径记档偏离）+ 进程桥
  （side proc EE；primary stub disconnect 走 raw 路）+ 退出序（exit 先记账，
  disconnect 后派发）+ disconnect cb 等全部退出（intercom 口径）。
- 坑 4.278（断开-起服竞速同步旗）、4.279（unref 丢 exit、早关 port 丢 ack）。
- 余 22（16 DIFF + 6 TIMEOUT）：共享 bind（disconnect/ipv6only-false/
  ipc-throw/net-server-drop-connection…）/RR 分发 3/handle 传递/net backlog
  主侧路由/dgram 族/kill 信号语义（kill-signal 已绿，infinite-loop 另案）
  /uncaught-exception（child  interplay）/relative-path/net-send/
  listen-pipe（net 面）/worker-no-exit（连接续命）/worker-handle-close，
  另案记档，下轮按 triage 优先级啃。
- 附带：url 域 `internal/url` isURL 最小移植（test-url-is-url-internal 转绿，
  url 14/14；黑盒 `url_internal_is_url_faces`）；构建 0 警告收尾
  （ec 括号/repl 死码删除/元组初始化）。

## 2026-10-19 http2 C 轮壳面（首批 13 件转绿）+ cluster 补记

- C 轮（不动 hyper 底座；bound 面另案）：头名归一 `__normName`
  （trim+小写，get/has/remove/set/append/trailer 全走）/writeHead 元组与
  平铺（append 语义 + 奇数长 ERR_INVALID_ARG_VALUE）/伪头文案
  （PSEUDOHEADER_NOT_ALLOWED 连字符）/HEADERS_SENT 三处补门/
  removeHeader('date') 关 sendDate/kTimeout 符号（`session[kTimeout]._
  idleTimeout`）/alpnProtocol h2c/setNextStreamID 文案（`> 0 and <=
  4294967295` + 非数 ARG_TYPE；close 码分档）/unescaped-path 译码
  （NGHTTP2_PROTOCOL_ERROR）/additionalHeaders 门序（INVALID_INFO_STATUS
  RangeError/STATUS_101/伪头/HEADERS_AFTER_RESPOND 文案对齐）/
  :authority 缺省抑制（host-only 经 x-wjs-no-authority 内部标记，
  两端同剥）/sendDate 下沉（hyper auto_date_header(false)，JS 全权）/
  error-order 同步 address（native 同步 bind+from_std，
  forbidden-blocking-fd 注记）/endafterheaders 意图透传（method 缺省
  endStream，native 不再按体空猜）/write-head-after-close 双杀
  （createPushResponse 双参 + INVALID_STREAM；200 兜底改 RST）。
- 转绿（base17 DIFF→0，逐件单跑复验）：compat-socket/headers/send-date/
  writehead-array/serverrequest-host/serverrequest-end/error-order/
  endafterheaders/write-head-after-close/write-head-destroyed/
  setNextStreamID/invalidargtypes/unescaped-path（13 件；黑盒
  `http2_compat_header_faces` 14 断言；冒烟 5/5）。
- 余：create-client-session 应答体截断（服务端 write+end 流式竞速，深水）/
  info-headers 1xx 下发（native interim-headers bound）/create-client-secure
  （tls 面，随 P2-tls）/createConnection 三件（需 native 双工桥输送任意 JS
  duplex + 服务端 connection 接管，另案设计）/push/ALTSVC-ORIGIN/帧错映射
  （bound B）/其余 TIMEOUT 待复判。
- pit 备忘（未单独立编号，记 journal）：close 单发——Writable autoDestroy
  与 stream 转发各派一次，真机只认一次，emit 层幂等；END_STREAM 意图与体空
  解耦（同 tick 的 end() 不得抢跑进 HEADERS）；伪头文案以真机逐字为准。

## 2026-10-19 tls 轮（ALPN 静态协商 + SAN 整形 + X509 PEM）

- ALPN：服务端 `__tlsOpts` 补 `ALPNProtocols`（构造顺位修正）+
  native `ListenOpts` 接表并写入 rustls server config + 无交集
  `NoApplicationProtocol` 映射 `ERR_SSL_TLSV1_ALERT_NO_APPLICATION_PROTOCOL`
  （alpn-server-client 基础四组 + fatal alert 转绿；ALPNCallback 另案——
  rustls 无 select-callback API，需 ClientHello 内省 + JS 回调 mid-handshake）。
  黑盒 `tls_alpn_negotiation`（a/b/无交集三档）。
- SAN：扩展原序混排（分组改逐条）+ NUL 转义字面 `\u0000` 并引号
  （0-dns-altname 转绿；node-main 检出与 26.8.2 行文差已实证同形）。

## 2026-10-19 base18 全域基线（node-main 检出，3574→3528 件）+ 口径说明

- base18（`~/.wjs-sweep/base18/`，2 并发，~52min）：SAME0 **2284** /
  SAME1 203 / DIFF 925 / TIMEOUT 116。base16（node 26.8.2）为 SAME0 2313 /
  DIFF 1103 / TIMEOUT 131。直接数不可比——node-main 引入 SAME1 膨胀
  （双边同红，多系 node-main 自身行为变更/新件，见下）。
- 可比口径（我方确定红）：DIFF 1103→925（-178）、TIMEOUT 131→116（-15）。
  域级绿（base16→base18）：http2 87→98、tls 81→84、cluster 25→59、
  worker 57→57、vm 46→46、http 345→322（-23 系 SAME1 膨胀 23，双边同红）、
  fs 286→282、stream 197→173（SAME1 33，双边同红为主）、crypto 68→62、
  child 71→72、module 4→7、process 69?→70、readline 4?→7。
- 教训：跨 node 版本对拍先算 SAME1 再谈涨跌（4.270 姊妹篇）；基线检出已钉
  本地 node 版本（v26.8.1，`~/node-test` 主仓稀疏检出；`~/wjs-data`
  软链重建指向家目录，外置盘缺席）。
- 本轮记账外：appendHeader 伪头门（compat assertValidHeader 口径）、
  0-dns-altname SAN 整形、alpn 静态协商、X509 PEM/no-start-line。

## 2026-10-19 webcrypto 键面轮（getRandomValues/methods-not-async/hidden-slots）

- subtle 去 async 化（方法须为普通函数）+ 品牌检查（this 非 subtle 即
  ERR_INVALID_THIS 拒绝）+ 7 缺失方法占位（wrap/unwrap/encapsulate/decapsulate
  系 + getPublicKey）；getRandomValues 同步品牌检查。
- RSA 尺寸放宽至 512（真机实测；hidden-slots 要 1024）。
- algorithm.hash 对象化（{name}）+ getter 深拷贝（defineProperty 防原型污染
  写入抛错）+ JWK key_ops 回填 + usages 公私分流（public 仅 verify/encrypt/
  wrapKey 系，private 缺失即 SyntaxError 空用途；真机逐项对拍）+
  needUsage 文案对齐（Unable to use this key to ...）。
- structuredClone(CryptoKey) JS 层包装（无键直通原生；有键则快照重建；
  二进制走 base64 桥；transfer+key 混用记档偏离）。
- KeyObject.from(CryptoKey) 经 prelude 内部槽桥接（secret；绕过可伪造 getter）。
- 转绿：getRandomValues/methods-not-async/hidden-slots；node crypto 黑盒 26/26、
  Rust crypto 单测 15/15。

## 2026-10-20 worker W1 轮（MessagePort 语义，16 件转绿）

- 范围：base18 worker DIFF 50 件中的 MessagePort 语义簇。转绿（逐件单跑复验）：
  message-port/close/constructor/transfer-target/transfer-closed/
  transfer-duplicate/transfer-self/untransferable/arraybuffer/drain/
  infinite-message-loop/message-port-transferring/receive-message/
  terminate-transfer-list/transfer-terminate/message-channel/event/
  mark-as-uncloneable/workerdata-messageport/terminate-unrefed（19 件）。
- 改动（commit 245352d，已 push；构建 0 警告、冒烟 5/5、黑盒 worker 14/14）：
  - 构造门：MessagePort/MessageChannel 函数化（class 无 new 抛带不上 code），
    内部令牌 `__kPortInternal`；原型只留 node 同款 8 名（内部管线全迁实例闭包/模块函数）。
  - close(cb)：cb 经 once('close') 随事件触发，非函数忽略，事件恒异步；
    teardown 崩溃根因——Rust 侧 uncount 必须与发射同 microtask（先 uncount
    会让循环排空进 teardown，悬空发射 SEGV；139 定位：等长改名对照 + 二分）。
  - transfer 归一：裸 array/可迭代/选项 `{transfer}` 三形 + 两套 exact 文案；
    视图/SAB/杂项条目一律 'Found invalid value in transferList.'（真机逐字，
    原视图转移系误实现）；重复 AB/端口各有专用文案；生成器单次消费（判定与
    组装共享清单）。
  - ports 透传：信封 `p` 清单序 nonce，收端先承接后 walk（同 nonce 多槽共享）；
    同步收信无 ports 上下文，残留槽 withdraw。
  - 自杀式投递（transfer 纠缠对端）：warning + 通道丢失 + 发送端失活；
    判定须在转移前（转移后源端 neutered 误判 detached）。
  - 对端关闭透出本端 'close'（PortClose 派发 + `peer-close` 经 `__ev`）；
    `__peerClosed` 压倒一切（此后 close() 完全静默，mixed 探针）；
    worker 死传导：WExit  swept 可达死链（转发器收件箱 closed + 传递闭包）
    并透 close（channel 块 3 根因）。
  - 跨会话 ID 碰撞：`Route::Local` 加互惠校验（`q.peer == p.id`），否则跨线程
    回包误投本地同 id 端口（message-channel 回环根因）。
  - hasRef 真机化（fresh false，message 监听挂载即 ref，覆盖 unref 后再挂载）；
    inspect `MessagePort { active, refed }`（init-hook 逐字）。
  - markAsUncloneable 影响全局 structuredClone（AB/视图/SAB 豁免）；
    Blob/File 字节 roundtrip（prelude `__wjs2_blobBytes` 内部导出）。
  - process 'worker' 事件（Worker 实例，异步）；terminate() 即 ref；
    worker eval/file 完成值不回显（channel `[object Object]` 根因）；
    TextDecoder SAB 快照拷贝解码。
  - Rust natives 新增 `port_is_peer`/`port_is_active`（`port_query` 共用核）。
- 坑（追入 pitfalls-06，编号续排 4.280 起，略）。
- 记档另案（本轮界外）：WASM Module/SAB 真共享（引擎墙）/
  JSTransferable+internal kTransfer（internal 面）/js_stream binding（infra）/
  async_hooks init（infra）/postMessageToThread（新 API 专项）/
  message-before-close TIMEOUT（事件循环深水）/inspect 完整 EventTarget 形。
- 环境备注：本机无 /opt/homebrew（AGENTS §3 路径失效），构建改用 Xcode
  toolchain libclang（`LIBCLANG_PATH=/Applications/Xcode.app/.../usr/lib`）；
  无 cargo-nextest（改 cargo test 单 target）；自建 `~/wjs-data/probe/runone.sh`
  做 PID 精确限时（`pkill -f` 会误杀自家 shell）。

## 2026-10-20 https H1 轮（createServer 双派发 + 响应 HWM，15 件转绿）

- 根因（hwm 系 15 件同形 `ERR_HTTP_HEADERS_SENT`）：`node:https` 的
  `createServer(options, cb)` 经 `new Server(options, cb)`（构造器已挂
  listener）后再 `server.on("request", cb)`——每请求双派发，第二次
  writeHead 必炸。http 侧 `createServer` 直建 base 无此问题（https 独有）。
  修法：createServer 只 `return new Server(options, cb)`。
- 转绿（逐件单跑复验）：byteswritten/drain/session-eviction/
  expect-continue-reuse-race/insecure-parse-per-stream/
  max-header-size-per-stream/max-headers-count/request-arguments/
  selfsigned-no-keycertsign/server-close-all/server-options-incoming-message/
  server-options-server-response/socket-options/truncate（14 件）+ hwm
  （响应 readableHWM 与请求选项同源，`__resHWM`，http 侧高水位套件零回归）。
- 余：TLS verify/SNI 簇（8 件，随 P2-tls；webpki 严格 vs OpenSSL 宽松系）/
  pfx-loadPKCS12 未实现（bound 候选，随 ALPNCallback/DHE）/
  unix-socket（socketPath 误走 443）/set-timeout-server+resume TIMEOUT/
  abort-controller/servername/session-injection-reuse/sockets-leak/
  checkServerIdentity/autoselectfamily/connect-address-family/
  keep-alive-drop-requests/server-close-idle/connections-checking-leak/
  simple/strict/host-headers（散件，H2 轮）。
- R16b 定级（base18 171 红：TIMEOUT 52/push 17/diagnostics 9/compat-shell 12/
  tls 7/frame 6/other-core 68）：A 否决（hyper 底座+C 壳已 98/256，重写不成）；
  B 拟 bound（push 17/ALTSVC-ORIGIN/帧错映射/diagnostics 9/tls 7，用户可推翻）；
  C 续啃（compat 壳+other-core 壳可达面约 40，C2/C3 轮）。

## 2026-10-20 worker postMessageToThread 专项（lane/worker-messaging，3/4 转绿，已合入 master）

- 按 node `lib/internal/worker/messaging.js` 全文移植：进程级 threadId→收件箱注册表（`worker_handle_add` 登记、`WExit` 摘除，主线程 0 不进表走 `worker_main_inbox` + 派生链上转）、会话桥接 `__ev("thread-msg"/"thread-ack")` 双工（回执码 0/1/2）、transfer 复用 `__toWire` 跨线程 offer 表；主/worker 发送序不对称（先路由/先序列化）实测复刻，错误码文案逐字（含 `Cannot sent` 原 typo）。
- 转绿：test-worker-messaging（11 连过，BroadcastChannel mesh 零改）/errors-handler/errors-invalid；黑盒新增路由/报错/transfer 三件（17/17）。坑 4.286/4.287（Local 自匹配 via 守卫/孙线程 dest 上转）。
- errors-timeout 红：引擎墙——worker 线程 `Atomics.wait` 抛 `waiting is not allowed on this thread` + workerData SAB 跨线程真共享缺失；TIMEOUT 机制本身黑盒已证。
- 过程纪律：重构纯搬移先行（`worker_messaging.rs` 新文件 218 行，`pub use` 原位重导出，一文件一提交）；新 natives 注册 2 行（mod.rs 不可避）；`worker.rs` 948 行（check-lines 绿）。

## 2026-10-20 R17 多 lane 并行启动（6 并行）

- 机器：12 核/16G/盘 76G 余；lane target 实测 ~8G（master 24G 含 test/bench 工件）；首编 6 分钟。lane 上限：磁盘约 5 道 + 主仓。
- 拓扑：lane/https-verify（tls SNI/校验簇，复用 WIP）+ lane/http2-c2（compat 壳）+ lane/https-misc（https 散件）+ lane/vm-triage（vm 49 件 triage）+ lane/whatwg（whatwg 40 + 小域）+ 主仓（合流/协调/B bound 落字）。
- 铁律增补（§4.142 禁令用户特批解除）：worktree per lane 独立 target；file-set 互斥（https.rs 归 L1 Agent 面、L4 只读）；plan3/journal 只主仓写；lane 只 commit 不 push 不合流；首编错峰（CARGO_BUILD_JOBS=3 + 随机 jitter）；禁全量 test/sweep（只域套件 + 自家黑盒）。

## 2026-10-20 wave-1 五道合流（+51 件）

- lane/whatwg（19 件转绿）：USP 品牌/缺参/lone-surrogate FFFD/双问号/迭代器品牌+tag；URL 无参/品牌文案/toStringTag/属性可枚举/host:port 切分；Events null-flatten/被动门/signal 校验/handleEager；TextDecoder fatal 文案/ASCII-trim/NUL 安全/大写规范；MIME parse+全 fixture 952 对拍；TES/TDS 缺失补齐；structuredClone 遇 stream 即 DataCloneError(25)；BYOB coded 错误/start 竞速/pull 续进/close 后可用；fs file-URL 中央校验。另：mime.js 按方法边界拆分（1000 行门，字节恒等）。
- lane/https-verify（checkServerIdentity 转绿 + servername/sni `''` 形）：裁决先于冲刷（预连写在 base `__ev` 前执行 CSI，失败即销毁弃置 srv 字节）；服务端无 SNI 即 `servername=false`；补回 WIP 误删的 wrap 面 verifyErr。sni.N 系 webpki 拒全数字尾标签（rustls 无 raw-SNI，引擎墙）；create-connection 系并发抢跑（framing 域另案）；additional-options 系 node 自挂 + agent 级 free 未转发。
- lane/http2-c2（compat 壳 10/18）：客户端 destroy 语义三修 + 服务端 expect/CONNECT 路由两修 + 校验面（link/socket 背书）+ Rust 头到即 response/增量 data/Close 清任务。余 8 件三类 bound：interim 系×4（hyper 单 HEADERS 无 interim API）、serverresponse-end HEAD 块（`NetCmd::H2Respond.end_stream` 一字段，net.rs）、early-hints×2（码形已 exact，卡 `__ev` 抛错到 uncaughtException 的 runtime 投递）、createpushresponse 成功路（push bound，错误双路径已 exact）。
- lane/https-misc（close-idle/host-headers/keep-alive-drop/client-get-url 转绿 + http close-idle 附带）：setHost Boolean 归一；超额管线 503 走正常响应管线；半截头连接非空闲；已关 server 再 close 回 `ERR_SERVER_NOT_RUNNING`；`NODE_TLS_REJECT_UNAUTHORIZED=0` 缺省 + Host→servername 归一。N1-N4 移交 wave-2（见下）。
- 合流验证：builtins 40/40、events 10/10、stream 12/12、util 9/9、worker 17/17、tls 8/8、http2 12/12、https 6/6、http 36/36；W1/USP/H1 回归抽查绿；worker:: 一次 flaky（4.275 家族，复跑绿，记一笔）。

## 2026-10-20 wave-2 五道合流（R18）

- L-A tls-strict（strict 15/15）：v3 无 SAN 证书 CN 回落（SAN 在场/IP 永不回落）；自签两处判定（verify_v1 先判已受信 + UnknownIssuer 路），哨兵透 `DEPTH_ZERO_SELF_SIGNED`；webpki Display 形补进 ALTNAME 映射。sni-N 记 bound（rustls 无 raw-SNI API）；autoselect/address-family 系 net-dialing 缺口（另道）；4 verify 红经 parent-swap 复现皆 pre-existing。
- L-B framing+https（N1/N2/N2b/close-竞速/free 全绿）：Server 参数门镜像 http；socketPath 双钩透传 + globalAgent 活引用（附带修 `NODE_TLS_REJECT_UNAUTHORIZED` 池键污染）；并发 write+end 空包竞速（net 自动回 FIN 抢跑→allowHalfOpen 收链 + end 收口）；agent 'free' 缺失补发。http 36/36 零回归（顺手修 2 transient）。
- L-C http2-C3（compat-serverresponse-end HEAD 块 + close-idle 束转绿）：`NetCmd::H2Respond.end_stream` 一字段；idle-conn 250ms 递延杀 + session stream 序 + finish→close + clean-close 免 abort + GOAWAY drain；自致 3 回归当场修。余：interim×4（hyper 无 interim API，bound）、early-hints×2（码 exact，卡 uncaughtException runtime 投递）、push 成功路（bound）；TIMEOUT 50 全表定级（7 已绿/1 runtime/3 node 自红/24 hyper-bound/15 actionable-shell 待 wave-3）。
- L-D vm-triage（timeout 四件 + 描述符两件转绿）：中断回调一发制 + 看门续发 + 模块 rejected-completion 转码认领；syncOut 三旗回写 + vm_set 重定义清旗。余：linker/membrane/每-context 队列/宿主 import/SM 文案/TLA 无 API/跨域，一律引擎墙记档；blackbox 17/17。
- L-E loader-json（encoding-singlebyte 29/28，余 import.meta.dirname）：ESM `.json` 走 export-default 合成（transpile.rs +39，loader 18/18）；whatwg 9 红复核 bound（7 internal-tree/infra + canparse V8 内禀 + custom-inspect 需 prelude+url-crate 双动）；console/util 全量 triage 表交付（ignoreErrors 簇 4 件 + inspect-core 缺口 + 5 小校验）。
- 主仓 fix-forward 两件：USP 初始化归一（序列 key-presence 判定/定长二元组/record 含 symbol 键/Map 去重）+ USP inspect 面（含迭代器余项/depth<0 坍缩/`inspect.custom` 导出）。

## 2026-10-20 wave-3 五道合流（R19）

- L-A tls B-3（CSI unfqdn/链锚补齐×2 转绿）：CSI 族纯搬移 tls.js→tls_context.js 后按 node 原文移植（unfqdn/RFC6125/IDNA/IP 规范化）；服务端出示链（ca 作 extra chain）+ getPeerCertificate 锚补齐。SNI 双件记引擎墙（addContext 未下沉 Rust resolver）。
- L-B framing+https（connect-address-family 6/6 + unix-socket + globalAgent + create-connection + free 全绿，附带 autoselectfamily）：https 两 dial 闭包吞 lookup/family → net.connect+tls.wrap 改道；setHost/getName 键分层；并发 write+end 空包竞速（net 自动回 FIN 抢跑）+ agent free 缺失。http 36/36 零回归。
- L-C http2-C3（connect-method-extended/upload/propagate/server-socket-destroy/cancel/many-writes/destroy-order/listeners-warning 转绿 + abort/close/flushheaders/serverresponse-end/close-idle 保持）：h2_client/h2_head 拆分 + http2_faces 黑盒。余：socket-close TIMEOUT 未解；respond-file×3 另起（WIP 半截重构已弃）；early-hints×2 卡 uncaughtException runtime 投递；push 成功路 bound。
- L-D vm linker（link/link-shared/linking-requests×3/modulerequests/instantiate 7 件转绿）：JS 壳 BFS + native 会话 hook 换装 + with-属性空格化 + moduleRequests 真属性。synthetic 命名依赖记引擎墙（缺 CreateNamedSyntheticModule/SetSyntheticModuleExport）；basic/errors 各剩一件（importModuleDynamically/initializeImportMeta，需 modules/loader 配合）。
- L-E console/util/inspect/json（ignoreErrors 簇×3/stack-overflow/group/tty-colors×2/styleText/signal/sleep/deprecate/promisify/backticks/getCallSites/dirname 全绿）：encoding-singlebyte 29→57。余：formatTime（debuglog）、promisify-custom-names（stream eos 别名）、getcallsites（自跳）、dotenv（CLI --env-file）、util-format null-proto（inspect bound）。

## 2026-10-20 B-bound 拟稿（待用户一票否决，证据见各轮 journal）

- push 系 17 件：Web 已死特性（hyper 服务端无 push API；createPushResponse 错误双路径已 exact）。
- ALTSVC/ORIGIN：hyper 无帧级 API。
- 帧错映射（goaway/窗口溢出/SETTINGS 内省/max-settings 等约 6+24 中部分）：hyper/h2 内部记账无 JS 钩。
- diagnostics-channel http2 系 9+：无 channel 插桩面。
- tls 7 件：随 P2-tls（ALPNCallback/DHE/renegotiation 等 rustls 无对应物）。
- tls sni-N：rustls-pki-types 无 raw-SNI（DnsName 无非校验构造）。
- SAB 真共享 + WASM Module 跨线程 + worker Atomics.wait：引擎墙（SM 限制）。
- V8 内禀（%PrepareFunctionFor.../--allow-natives-syntax）、internal/* 树（--expose-internals 剥离）、sea/wasi：范围外。

## 2026-10-20 会话收尾（多 lane 并行验证通过，worktree 已回收）

- wave-2 5 道 + wave-3 5 道全部合入 master 并 push（0c44143），每道均经黑盒+冒烟+守门验证。
- L-C 中途取消：5 个已提交按原样合入（socket-close TIMEOUT 未解、respond-file 另起），半截 WIP（respondfile 重构+eprintln 调试）已弃，未入库。
- 现场：工作区干净、`git status` 空、4 个 lane worktree+分支已删（磁盘 100G），node-test 检出干净。下会话开工按 AGENTS 开工顺序即可。

## 2026-10-20 B-bound 拍板清单待批（只读整理，不改 plan3 §0.4 状态表）

> 任务4 产出：供用户一票拍板。口径：base18（3574→3528 件，SAME0 2284/DIFF 925/TIMEOUT 116，
> journal:1406-1421）下 http2 171 红 = TIMEOUT 52 / push 17 / diagnostics 9 / compat-shell 12 /
> tls 7 / frame 6 / other-core 68（journal:1505-1508；plan3 §0.4 R16b 行）。
> A 已否决（hyper 底座 + C 壳已 98/256，重写风险收益不成比，journal:1506）。
> 下列 B 拟 bound 件用户可一票推翻；TIMEOUT 另列逐件 time-box 复判，不在本次 bound 内。
> Bun 口径：范围 `docs/bun-scope.txt`（http2 278 / tls 201 / diagnostics 44 行，含 http2-diagnostics 20 件），
> 语义 `docs/bun-compat.md` + `docs/bun-parity.md`（http2/tls/diagnostics 三节）。
> 现象列 DIFF = wjs=1 快败、TIMEOUT = wjs=142 挂死（base18 `~/wjs-data/sweep/base18/results.log`）。

### B1 push 系 17 件 → 建议 bound

- 套件（8 http2 DIFF + 9 diagnostics DIFF 连带）：`test-http2-client-data-end` /
  `test-http2-compat-serverresponse-createpushresponse` / `test-http2-options-max-reserved-streams` /
  `test-http2-respond-file-push` / `test-http2-server-push-disabled` /
  `test-http2-server-push-stream-errors-args` / `test-http2-server-push-stream-head` /
  `test-http2-server-push-stream`（8，现象全 DIFF：`ERR_HTTP2_PUSH_DISABLED` / Missing expected exception /
  `push is undefined`，base18 results.log `PUSH_DISABLED` 计 14 中 http2 占 5 + 报错形 3）+
  `test-diagnostics-channel-http2-client-stream-{close,created,finish,start}`（4）+
  `test-diagnostics-channel-http2-server-stream-{close,created-start-timing,created,finish,start}`（5）
  （9，现象 DIFF `PUSH_DISABLED` 连带——diagnostics 探针建会话即撞 push 门）。
- 根因：协议已死 + 底座不可达——Web 已无 push 消费端，hyper 服务端无 push API（journal:1196-1210 双分支 scope；
  1582 P1 聚类 push 8；1827 base16 附录 push 6；1527/1535/1544 push 成功路 bound 三轮一致）。
- 已试修法：createPushResponse 错误双路径已 exact（PUSH_DISABLED 文案 + INVALID_STREAM 门，journal:1527、
  1366-1380 C 轮 write-head-after-close 双杀）；服务端 push 成功路未做（hyper 无 API 可接）。
- 偏离理由一句话：Web 已死特性且 hyper 无服务端 push 原语，重做需换 h2 直驱约 1 周而 A 已否决。
- 对标 Bun：`docs/bun-compat.md:70` http2 行注记“无 push（Web 已死）”即本仓口径——Bun http2 纵标 🟢，
  push 面亦非 Bun 高度必需（Bun 自带清单 http2 278 件中 push 8 件占比小；真机复核可补，但不影响 bound 判定）。
- 证据：journal:1196-1210（A/B 分支与拍板线）、journal:1246-1254（B+C 建议）、journal:1527/1535/1544
  （三轮 push 成功路 bound）、journal:1550（拟稿）、base18 results.log（`PUSH_DISABLED` 14 行）。

### B2 ALTSVC / ORIGIN 2 件 → 建议 bound

- 套件：`test-http2-altsvc.js`（TIMEOUT wjs=142）、`test-http2-origin.js`（TIMEOUT wjs=142）。
- 根因：底座不可达——hyper/h2 均无 ALTSVC/ORIGIN 帧级 API（journal:1199-1206 “不覆盖：ALTSVC/ORIGIN（h2 无此帧，仍偏离）”；
  journal:1551 拟稿）。
- 已试修法：未开轮（P1 首轮即定性为底座不可达面，journal:254 P1 行“ALTSVC/ORIGIN 帧”；C 轮明确界外，journal:1386-1390）。
- 偏离理由一句话：连 h2 直驱（A 分支）都无此帧，JS 层无任何可接钩子。
- 对标 Bun：Bun http2 🟢未见 ALTSVC/ORIGIN 专项实现主张；属协议边角，Bun 大概率同缺（待 Bun 真机复核，不阻塞 bound）。
- 证据：journal:1196-1198（现状锚点 ALTSVC-ORIGIN 2）、journal:1246（base16 ALTSVC-ORIGIN 2）、
  journal:1389、journal:1551、base18 results.log（上列 2 行 TIMEOUT）。

### B3 帧错映射 6 件（goaway / 窗口溢出 / SETTINGS 内省）→ 建议 bound

- 套件（DIFF 6，base18）：`test-http2-goaway-opaquedata`（STREAM_CANCEL）/
  `test-http2-session-cleanup-on-nghttp2-goaway`（MUSTCALL noop 0）/
  `test-http2-window-size`（INVALID_SETTING_VALUE initialWindowSize undefined）/
  `test-http2-window-update-overflow`（MUSTCALL noop 0）/
  `test-http2-session-settings`（strictly equal，对端 SETTINGS 内省无钩）/
  `test-http2-too-many-settings`（strictly equal）；关联 `test-http2-max-settings`（TIMEOUT，
  归 TIMEOUT 另列）、`test-http2-invalid-last-stream-id`（internal/http2/core 缺失，属另案 internal 面）、
  `test-http2-max-invalid-frames`（DESTROYED，流控竞速边角）。
- 根因：底座不可达——hyper/h2 内部记账（GOAWAY opaque data、窗口溢出计数、远端 SETTINGS 表）无 JS 钩
  （journal:1200-1204 “GOAWAY/窗口/RST 码映射”仅 A 分支可达且预算 1 周；journal:1552 拟稿“内部记账无 JS 钩”）。
- 已试修法：C 轮仅修壳面（GOAWAY drain 递延杀 + stream 序，journal:1535 L-C）；帧级错码映射未开轮
  （P1 即列“原始帧协议错映射”，plan3 §0.4 P1 行）。
- 偏离理由一句话：错码/计数器藏在 hyper 内部，JS 层只能看到归一化错误，重做等于换底座。
- 对标 Bun：Bun http2 🟢无帧错映射专项主张；属 nghttp2 行为口径，Bun（JSC 栈）数字亦不可比，大概率同缺或同偏离。
- 证据：journal:254（P1 余件“原始帧协议错映射”）、journal:1196-1204、journal:1246-1253（帧流控映射 19）、
  journal:1395 前 C 轮余件行、journal:1552、base18 results.log（上列 6 行 DIFF）。

### B4 diagnostics-channel http2 系 9 件 → 建议 bound（infra 另案号待编）

- 套件（DIFF 9，非 push 污染的真插桩缺口）：client body ×5
  `test-diagnostics-channel-http2-client-stream-body-{multiple-buffers-and-strings,multiple-buffers,no-chunks,single-buffer,single-string}`
 （MUSTCALL 未触发）+ client `…-close-error` / `…-error`（AbortError ABORT_ERR）+
  server `…-server-stream-{close-error,error}`（MUSTCALL 未触发）；另 9 件 push 污染已计入 B1，
  合计即 bun-scope 内 http2-diagnostics 18 件全数（`grep diagnostics-channel-http2 docs/bun-scope.txt` 20 件含命名差 2）。
- 根因：infra——本仓 http2 收发路径无 `diagnostics_channel.publish` 插桩面（journal:1553；
  bun-parity diagnostics 节 337-354 行逐件“需 http2 插桩”🟡偏离）。
- 已试修法：未开轮（P1 即列 diagnostics 另案 infra；C 轮界外；wave-2/3 未排期）。
- 偏离理由一句话：插桩面需在每条收发路径逐点 publish，属独立 infra 专项，不随 http2 壳轮附带做。
- 对标 Bun：`docs/bun-compat.md:48` diagnostics Bun 🟡（缺 bounded/多数组内置通道）——Bun 自身插桩亦薄；
  bun-parity 335-354 行本仓记 🟡偏离即对等口径，bound 后与 Bun 同档。
- 证据：journal:1553（拟稿）、`docs/bun-parity.md:337-354`（18 件逐件“需 http2 插桩”）、
  journal:1505（R16b diagnostics 9）、base18 results.log（上列 9 行 DIFF）。

### B5 tls 7 件（http2-over-tls）→ 建议 bound（随 P2-tls，不单开轮）

- 套件（DIFF 4 + TIMEOUT 3）：DIFF `test-http2-alpn`（NO_APPLICATION_PROTOCOL）/
  `test-http2-create-client-secure-session`（strictly equal）/ `test-http2-https-fallback` /
  `test-http2-https-fallback-http-server-options`（UnsupportedCertVersion 握手失败）+
  TIMEOUT `test-http2-connect-tls-with-delay` / `test-http2-multistream-destroy-on-read-tls` /
  `test-http2-tls-disconnect`（wjs=142）。
- 根因：引擎边界 + 底座——rustls 与 OpenSSL 行为差（v1 证书/ALPN 无交集 alert/握手失败文案），
  SecureContext 面随 P2-tls 深化（journal:254 P1“tls 面随 P2-tls 一并”；journal:1395-1401 ALPN 静态协商；
  journal:1498 H1 余 TLS verify/SNI 簇 8 件随 P2-tls）。
- 已试修法：P2-tls-b 包裹引擎 + accept 重排 + TlsCleanEof（journal:288-331，wrap 簇 19/24）；
  tls 轮 ALPN 静态协商 + SAN 整形 + X509 PEM（journal:1395-1404）；H1 双派发修（journal:1485-1497）；
  ALPNCallback 明确另案（rustls 无 select-callback API，journal:1400-1401）。
- 偏离理由一句话：http2-over-tls 红即 tls 引擎差，非 http2 壳可修，归 P2-tls 统一收口。
- 对标 Bun：`docs/bun-compat.md:53/75` https/tls 双 🟡（https 无 SNI 回调；tls 无 psk/OCSP/resume，
  无 SecureContext/getPeerCertificate 记档）——本 7 件与 Bun 缺口同族，bound 后对等。
- 证据：journal:254、journal:1395-1404、journal:1485-1508（H1 余件 + R16b 定级）、journal:1554、
  base18 results.log（上列 7 行）。

### B5 附 tls 域同族 bound 候选（随 P2-tls，不单计数）→ 建议 bound

- 套件：`test-tls-psk-*`（2，握手 AccessDenied）/ `test-tls-dhe`（internal/crypto/util 缺失，internal 面）/
  `test-tls-client-renegotiation-13`（TIMEOUT）/ `test-tls-disable-renegotiation`（Missing expected exception）/
  `test-tls-ocsp-callback` / `test-tls-{invalid,legacy,multi,pfx-authorizationerror,https-pfx}-pfx*`（5，`loadPKCS12() not implemented`）/
  `test-tls-client-resume*` / `test-tls-ticket`（resume/ticket 无对应物）/
  `test-tls-alpn-server-client`（ALPNCallback 面）。
- 根因：引擎边界——rustls 无 psk/DHE/renegotiation/OCSP/PKCS12/resume 对应物（journal:1499
  “pfx-loadPKCS12 未实现（bound 候选，随 ALPNCallback/DHE）”；journal:1554）。
- 已试修法：SecureContext/configSecureContext 逐字移植已做（journal:281 P2 首批 +23）；上列面未开轮。
- 偏离理由一句话：rustls 能力墙，非 JS 可补。
- 对标 Bun：bun-compat tls 🟡“无 psk/OCSP/resume”原文即同缺证据；DHE/renegotiation/pfx 同族。
- 证据：journal:1498-1499、journal:1554、base18 results.log（上列各行）。

### B6 SNI raw（含 https-verify sni-N）→ 建议 bound（引擎墙）

- 套件：`test-tls-sni-option` / `test-tls-sni-server-client` / `test-tls-sni-servername` /
  `test-tls-snicallback-error`（Missing expected exception）/ `test-tls-empty-sni-context` /
  `test-tls-error-servername`（ARG_TYPE 文案差）/ `test-https-agent-sni`（bad servername `sni.2` invalid dns name）
  + wave-1 sni.N 系 + wave-2 L-A sni-N + wave-3 L-A SNI 双件（addContext 未下沉）。
- 根因：引擎边界——rustls-pki-types 无 raw-SNI（DnsName 无非校验构造，全数字尾标签即拒；
  journal:1526、1533、1555）；SNI 双件另因 addContext 未下沉 Rust resolver（journal:1542）。
- 已试修法：https-verify lane 裁决先于冲刷 + servername `''` 形 + verifyErr 补回（journal:1526 转绿
  checkServerIdentity）；tls-strict lane CN 回落/自签双判（journal:1533 15/15）；B-3 lane CSI 族搬移
  + 链锚补齐（journal:1542 转绿 2）；raw-SNI 三轮一致记 bound 未强攻。
- 偏离理由一句话：非校验 SNI 名在 rustls 类型层即非法，JS 层无构造点。
- 对标 Bun：bun-compat https 🟡“无 SNI 回调等”原文即同缺证据。
- 证据：journal:1526（webpki 拒全数字尾标签，rustls 无 raw-SNI，引擎墙）、journal:1533（sni-N 记 bound）、
  journal:1542（SNI 双件记引擎墙）、journal:1555、base18 results.log（`test-tls-sni*` 4 + empty-sni +
  error-servername + `test-https-agent-sni` 各行）。

### B7 R18/R19 各 lane bound / 另案记档（本次一并拍板确认，不单开轮）

- wave-2 L-C interim ×4（`test-http2-info-headers` / `compat-write-information` /
  `compat-serverresponse-{close,flushheaders}` TIMEOUT 含）→ 建议 bound：一句话 hyper 单 HEADERS 无 interim 1xx API
  （journal:1527、1535）。early-hints ×2（`compat-write-early-hints{,-invalid-argument-value}` 码形已 exact，
  卡 `__ev` 抛错到 uncaughtException 的 runtime 投递）→ 建议 bound（同上两行）。
- wave-2 L-D vm-triage 余件（linker/membrane/每-context 队列/宿主 import/SM 文案/TLA 无 API/跨域）→
  建议 bound（引擎墙，journal:1536）；L-E whatwg 9 红（7 internal-tree/infra + canparse V8 内禀 +
  custom-inspect 需 prelude+url-crate 双动）→ 建议 bound（journal:1537）；console/util triage 表
  （ignoreErrors 簇 4 + inspect-core 缺口 + 5 小校验）→ 续啃/另案按表（journal:1537）。
- wave-3 L-D synthetic 命名依赖（缺 CreateNamedSyntheticModule/SetSyntheticModuleExport）→ 建议 bound
  （引擎墙，journal:1545）；basic/errors 各剩一件（importModuleDynamically/initializeImportMeta，需
  modules/loader 配合）→ 续啃（同行）。L-E formatTime/promisify-custom-names/getcallsites/dotenv/
  util-format null-proto → 续啃或 bound 按行（journal:1546；null-proto 记 inspect bound）。
- worker 专项：SAB 真共享 + WASM Module 跨线程 + `Atomics.wait`（errors-timeout 红，worker 线程抛
  waiting is not allowed + SAB 跨线程缺失）→ 建议 bound（引擎墙，journal:1476-1479、1514、1556）。
- 范围外（不做）：V8 内禀（%PrepareFunctionFor…/--allow-natives-syntax）、internal/* 树
  （--expose-internals 剥离）、sea/wasi（journal:1557；plan3 §4 维持否决）。

### 非 bound 件另列（TIMEOUT + C 续啃，不在本次 bound 拍板内）

- TIMEOUT 52（R16b 口径；base18 实测 http2 TIMEOUT 50，差 2 系 C2/C3 已绿 7 的过程差）→
  不建议 bound，逐件 time-box 复判另轮。L-C 全表定级（journal:1535）：
  7 已绿 / 1 runtime（uncaughtException 投递）/ 3 node 自红 / 24 hyper-bound / 15 actionable-shell
  待 wave-3；wave-3 后 `socket-close` TIMEOUT 未解、`respond-file×3` 另起（journal:1544、1562）。
  明细以 `~/wjs-data/sweep/base18/results.log`（`TIMEOUT wjs=142 test-http2-*` 50 行，另 diagnostics 系全 DIFF
  不在 TIMEOUT 内）为准，另轮按“单件 run1.sh 复判 → hyper-bound 即归 B / shell 可达即归 C”逐件销账。
- compat-shell 12 + other-core 68（含 C 壳可达面约 40）→ 建议续啃（C2/C3 轮已证可达：
  C 轮 13 件 journal:1381-1385、C2 10/18 journal:1527、C3 多件 journal:1535/1544；余 15 actionable-shell
  见上）。`test-http2-compat-serverrequest-trailers`（wjs=0 node=1 翻转）属 node 自红，不计我方欠账。

### 拍板选项（用户一票）

- [ ] A：B1–B6 全 bound + B7 确认，TIMEOUT/C 续啃另轮（推荐；A 重写已否决，C 壳继续）。
- [ ] B：一票推翻其中某件（请点名件名，被点名件回 C 队列按 time-box 另轮）。
- [ ] C：全不 bound，B 件回 C 队列（需另批约 1 周 + h2 直驱预算，A 分支 journal:1199-1206）。

## 2026-10-20 B-bound对齐定义复核（用户定义：行为按 Bun/Node 逻辑对齐，逐件重核）

> 方法：Node 原文一律读 `~/node-test/lib/`（稀疏检出，主仓；行号为该检出实测）。
> Bun 证据只用三处：`docs/bun-compat.md` §1 总表（Bun 列）、`docs/bun-parity.md` 相关节、
> `docs/bun-scope.txt` 件数（`^test-<域>-` 行数；与 plan3 §0.3 域表差 1–2 系域归属口径，照实注记）。
> 每件三列：Node 原文行为一句话 + Bun 行为（有/无/同缺，附行号）+ winterjs 对齐定义
> （bound 偏离的精确语义 vs 续啃修到什么形态算对齐）。vm 因 Bun 🟢 不可随手 bound，单列强证据。
> 口径锚点：bun-scope `^test-http2-` 256 行（§0.3 http2 256 一致）/ `^test-tls-` 186 行
> （§0.3 tls 185，差 1 系归属口径）/ `^test-diagnostics-` 35 行（一致，内 http2-diagnostics 18 行）/
> `^test-vm-` 97 行（§0.3 vm 95，差 2 系归属口径）。

### B1 push 17（8 http2 + 9 diagnostics 连带）

- Node 原文行为一句话：`Http2Stream.pushStream(headers,options,cb)` 先判 `pushAllowed`
  （对端 `remoteSettings.enablePush`，`~/node-test/lib/internal/http2/core.js:3144-3157`），禁推即同步抛
  `ERR_HTTP2_PUSH_DISABLED`；compat `Http2ServerResponse.createPushResponse` 经
  `kStream.pushStream` 同门（`~/node-test/lib/internal/http2/compat.js:920-930`）。
- Bun 行为：有（`docs/bun-compat.md:70` http2 Bun 🟢；移植注记“无 push（Web 已死）”是 winterjs 列注记，
  非 Bun 缺口；`docs/bun-parity.md:1575-1591` http2 节 P1 聚类 push 8 未记 Bun 同缺）。
  即本件 bound = 承认欠 Bun 一截，需用户一票背书，非对等偏离。
- winterjs 对齐定义：bound 偏离形态 = 禁推路全 exact 即算对齐偏离——`pushStream` 无条件抛
  `ERR_HTTP2_PUSH_DISABLED`（含 `pushAllowed=false` 恒值）、`createPushResponse` 错误双路径 exact
  （PUSH_DISABLED + INVALID_STREAM，journal:1527 C2 已落地）；成功路（真建推送流）永不实现。
  续啃验收形态（仅用户推翻时）= hyper 换 h2 直驱后 `pushPromise` 真建流 + `pushStream` 回调三参 exact，
  按 A 分支预算约 1 周（journal:1199-1206），本次不选。

### B2 ALTSVC / ORIGIN 2（altsvc/origin，均 TIMEOUT）

- Node 原文行为一句话：`session.altsvc(alt,originOrStream)` 经 `kHandle.altsvc` 发 ALTSVC 帧、
  `session.origin(...origins)` 经 `kHandle.origin` 发 ORIGIN 帧（`core.js:1802-1877`），对端到帧分别
  走 `onAltSvc`  emit `session 'altsvc'`（`core.js:751-758`）与 `onOrigin` 重建 originSet
  （`core.js:785-799`），另有 ALTSVC/ORIGIN 长度与源数四码（`core.js:72-73,78,87,106`）。
- Bun 行为：推定有（http2 🟢 `bun-compat.md:70` 未单列 ALTSVC 缺口；`bun-scope.txt` 含
  `test-http2-altsvc/origin` 2 行即 Bun 清单在列；待 Bun 真机复核，不阻塞 bound）。
- winterjs 对齐定义：bound 偏离形态 = `altsvc()/origin()` 调即抛底座缺口错（或 no-op 记档）+
  `'altsvc'/'origin'` 事件永不触发，即算对齐偏离（h2 直驱亦无此帧，journal:1199-1206 “仍偏离”）。
  续啃验收形态 = hyper/h2 暴露帧级发收钩子后逐字移植 `onAltSvc/onOrigin` + 四码校验，再谈转绿；
  在此之前任何 JS 壳修法都不算对齐。

### B3 帧错映射 6（goaway / 窗口溢出 / SETTINGS 内省）

- Node 原文行为一句话：收 GOAWAY 走 `onGoawayData` 记 `goawayCode/goawayLastStreamID` 并 emit
  `'goaway'`（`core.js:804-830`），发 GOAWAY 走 `submitGoaway(code,lastStreamID,opaqueData)`
  （`core.js:982-991`）；收 SETTINGS 走 `onSettings` 刷新缓存并 emit `'remoteSettings'`
  （`core.js:698-707`），发 SETTINGS 走 `submitSettings` + `settingsCallback`（`core.js:954-981`），
  另 `setLocalWindowSize` 归一校验（`core.js:1518-1523`）与 `validateSettings` 全项门
  （`core.js:1083-1106`，含 initialWindowSize/maxFrameSize 等）。
- Bun 行为：推定有（http2 🟢 未单列帧错缺口；6 件名全在 `bun-scope.txt` http2 256 内；待 Bun 真机复核）。
- winterjs 对齐定义：bound 偏离形态 = GOAWAY 仅保留 C 轮已落地的 drain 语义
  （idle 递延杀 + stream 序 + finish→close，journal:1535 L-C），`goaway-opaquedata` 的 opaque 回传、
  `window-size/update-overflow` 的精确计数、`session-settings/too-many-settings` 的远端表内省三面记偏离，
  不再逐码对齐。续啃验收形态 = 换 h2 直驱拿到 `goaway opaqueData`/`remoteSettings` 真表/窗口计数器后，
  `wjs_err` 与真机逐字一致（含 `ERR_HTTP2_GOAWAY_SESSION/INVALID_SETTING_VALUE` 文案），否则不算对齐。

### B4 diagnostics-channel http2 系 9（真插桩缺口；另 9 push 污染计 B1）

- Node 原文行为一句话：`core.js:199-211` 定义 12 通道
  （`http2.client/server.stream.{created,start,error,finish,close}` + bodyChunkSent/bodySent），在流创建/
  起发/错/体块/体完/终结各点 `publish({stream[,headers]})`（如 `core.js:404,410,428,434,487,907,2012,
  2180-2183,2511,2556,2722-2728,3234-3308`）；`diagnostics_channel.js` 本体
  （BoundedChannel `~/node-test/lib/diagnostics_channel.js:388-444`、TracingChannel 447-698）只是通道容器。
- Bun 行为：同缺/同薄（`docs/bun-compat.md:48` diagnostics Bun 🟡“缺 bounded/多数组内置通道”；
  `docs/bun-parity.md:306-354` 本仓 18 件逐件“需 http2 插桩”🟡偏离，即 Bun 亦无此插桩可抄）。
- winterjs 对齐定义：bound 偏离形态 = 通道名存在（`dc.channel('http2.…')` 可订阅不炸）但 http2 路径永不
  publish，即算对齐偏离（与 Bun 同档）。续啃验收形态 = 在 C 轮头体分离的每个 publish 点逐点补
  `publish`（client/server 各 6 点 + body 2 点），18 件 MUSTCALL/Abort 全转绿，另立 infra 专项号，
  不随 http2 壳轮附带做。

### B5 tls 7（http2-over-tls）+ 同族（psk/DHE/renegotiation/OCSP/pfx/resume/ALPNCallback）

- Node 原文行为一句话：`tls.js:83-85` 设 reneg 上限、`internal/tls/wrap.js:799-800`
  `disableRenegotiation()` 置旗 + `225` 握手期检查；psk 双回调 `onPskServerCallback/onPskClientCallback`
  （`wrap.js:480-546`，经 `kPskCallback/kPskIdentityHint:111-112`）；OCSP 服务端 `requestOCSP/requestOCSPDone`
  + 客户端 `onocspresponse`（`wrap.js:366-419,558-560`）；pfx 经 `SecureContext.loadPKCS12`
  （`internal/tls/secure-context.js:316-330`）、ticketKeys 48 字节门（同文件 346-354）；
  ALPN 服务端回调 `callALPNCallback`（`wrap.js:322-343`，经 `kALPNCallback:109`）。
- Bun 行为：同缺（`docs/bun-compat.md:75` tls Bun 🟡“无 psk/OCSP/resume”原文即证据；
  `docs/bun-compat.md:53` https 🟡“无 SNI 回调等”覆盖 ALPNCallback 面；
  `docs/bun-parity.md:1795-1817` tls 节 wrap 簇已定性 ca 链/收尾另案，未记 Bun 有）。
- winterjs 对齐定义：bound 偏离形态 = `pskCallback`/`ALPNCallback` 选项透传后走“不支持”错（或静默忽略记档）、
  `loadPKCS12` 抛 `ERR_METHOD_NOT_IMPLEMENTED`（base18 已是此形）、renegotiation 缺省禁（与真机
  `ERR_TLS_RENEGOTIATION_DISABLED` 同形记档）、OCSP 永不发 `OCSPRequest/OCSPResponse`，
  即算对齐偏离（随 P2-tls，不单开轮）。续啃验收形态 = rustls 暴露对应钩子（psk 回调/select-callback/
  OCSP  stapling/PKCS12 解析/resume 票据）后逐字移植 `wrap.js` 上列函数，套件逐件转绿，否则不算对齐。

### B6 SNI raw（含 https-verify sni-N；B5 中 SNI 回调面除外）

- Node 原文行为一句话：`Server.addContext(servername,context)` 把通配符转正则入 `_contexts`
  （`wrap.js:1697-1713`），握手期 `SNICallback` 倒序匹配选 context（同文件续段），SNI 原始名经
  `getSessionServerIdentity` 字节编解（`wrap.js:121-162`，`servername+len+bytes` 拼会话票据）。
- Bun 行为：同缺（`docs/bun-compat.md:53` https Bun 🟡“无 SNI 回调等”原文即证据；
  wave-1 `sni.N` 系 webpki 拒全数字尾标签即 rustls 类型墙，journal:1526）。
- winterjs 对齐定义：bound 偏离形态 = 校验型 SNI（合法 DNS 名）全 exact（含 `servername=false` 无 SNI 路、
  CSI 族 B-3 已转绿 2，journal:1542），非校验 raw-SNI（全数字尾标签/`sni.2` 类）记偏离——
  rustls-pki-types `DnsName` 无非校验构造（journal:1555），`addContext` 未下沉 Rust resolver
  （journal:1542 SNI 双件）两墙未除前不算欠账。续啃验收形态 = Rust 侧换 raw-SNI 类型（或自实现
  SNI 字节直通 + JS 正则匹配 `addContext` 全移植）后 `test-tls-sni-*/empty-sni/error-servername/
  https-agent-sni` 全转绿，否则不算对齐。

### B7 lane 件（R18/R19；vm 单列强证据，其余沿 B1–B6 口径）

- interim ×4 + early-hints ×2：Node 原文 `compat.js:956-1012`
  （`writeInformation` 100–199 门 + 101 拒 + `additionalHeaders` 下发，`writeEarlyHints` link 校验拼
  `Link` 头走 103）；Bun 推定有（http2 🟢）；对齐定义 bound = hyper 单 HEADERS 无 interim 1xx API
  （journal:1527/1535）+ `__ev` 抛错到 uncaughtException 的 runtime 投递墙未除前记偏离，
  码形 exact 已是证据（journal:1535/1544），成功下发 1xx 永不实现。
- worker SAB 真共享 + WASM Module 跨线程 + `Atomics.wait`：Node 原文跨线程真共享内存；
  Bun 🟡（worker 🟡，`bun-compat.md:80`）；对齐定义 bound = 副本语义 + `waiting is not allowed
  on this thread` 透传即偏离（journal:1476-1479/1514/1556 引擎墙 SM 限制）。
- whatwg 9 + console/util triage：Node 原文各域原文；Bun whatwg 多 🟢；对齐定义按
  journal:1537 行（7 internal-tree/infra + canparse V8 内禀 + custom-inspect 双动）逐行 bound/续啃，
  不整域 bound。
- vm（Bun 🟢，`docs/bun-compat.md:78`“全，连 ESM classes”，不可随手 bound——强证据如下）：
  Node 原文 `lib/vm.js:321 compileFunction` / `:383 measureMemory`（实验特性）/
  `:418-419` Module 三类仅 `--experimental-vm-modules` 下 patched 可见；
  `internal/vm/module.js:199-208 link(linker)` + `:335-370 kLink` BFS 按序调 linker 对齐
  `moduleRequests`（`:416-417` 真属性）+ `:461-500 SyntheticModule`（无依赖，`link()` no-op）+
  `:379 linkRequests` / `:450 createCachedData`。
  Bun 有（🟢全文）。winterjs 现状：wave-3 L-D 已转绿 7
  （link/link-shared/linking-requests×3/modulerequests/instantiate，journal:1545），不欠 Bun 主干。
  仅三窄面记 bound：① synthetic 命名依赖（缺 `CreateNamedSyntheticModule/
  SetSyntheticModuleExport` SM 原生钩，journal:1545 engine 墙）；② measure-memory 契约
  （本仓 reject vs 真机 resolve，`bun-parity.md:544-545` 两行，需 SM 堆快照钩，无钩即偏离）；
  ③ copy 模型固有 9 + 微任务模式 2 + 解析器 token 1（`bun-parity.md:487-500` ①–③类，V8/SM 跨引擎不可比，
  另案 jsdom 级工程）。`basic/errors` 各剩一件（importModuleDynamically/initializeImportMeta，
  journal:1545）明确续啃（需 modules/loader 配合，非 vm 单域可闭）。整域 vm 永不 bound。

### 复核结论（三列表用户拍板用）

- bound（偏离即对齐）：B1 成功路 / B2 全件 / B3 帧错三面 / B4 publish 面（通道名在即可）/
  B5 psk-DHE-reneg-OCSP-pfx-resume-ALPNCallback / B6 raw-SNI + addContext 未下沉 /
  B7 interim-early-hints + worker 真共享三件 + whatwg 按行 + vm 三窄面。
- 续啃（修到某形态才算对齐）：compat-shell 12 + other-core 壳可达约 40（C2/C3 已证路径，
  逐批至 `wjs_err` 逐字一致）/ vm basic-errors 各 1（等 loader 配合）/ B1–B3 被一票推翻件
  （回 C 队列，h2 直驱预算另批）。
- 另轮 time-box（逐件复判，不在 bound 内）：TIMEOUT 50 全表（L-C 定级 7 已绿/1 runtime/
  3 node 自红/24 hyper-bound/15 actionable-shell，journal:1535；wave-3 后 socket-close 未解、
  respond-file×3 另起，journal:1544/1562），以 `~/wjs-data/sweep/base18/results.log`
  `TIMEOUT wjs=142 test-http2-*` 50 行为准销账。

## 2026-10-20 whatwg9+worker真共享对齐复核（自主定级，不问拍板）

> 口径（parent 更新）：行为默认对齐 node（`~/node-test/lib/` 原文）+ Bun（范围/形态），以文档证据
> 自行定级记档，不再问用户拍板。本段只追加 journal，不碰 http2/vm/console/sweep 四 lane 文件。
> 真机对照：node v26.8.1 vs 本仓 `./target/debug/winterjs2`（Oct 20 构建，不重编避免并行冲突）；
> 守门：冒烟 5/5（`40+2/await-setTimeout/URL.search/TextEncoder/fetch-data` 全过，本段工作区实测）、
> `check-naming: ok`、`check-lines: ok（最大 1000 src/state/mod.rs）`。

### whatwg 9（L-E 余件，journal:1537 “7 internal-tree/infra + canparse V8 内禀 + custom-inspect 双动”）

> 范围：9 件全在 `docs/bun-scope.txt`（`^test-whatwg-` 55 行内，逐件 `grep -qx` 实测 IN scope）；
> base18 `test-whatwg-*` 40 DIFF 中本 9 为 L-E 复核 bound/续啃件，余 31 归 C 续啃不在本段。

- W1 `test-whatwg-readablebytestream.js` → 记 bound（范围外）。
  Node：`require('internal/webstreams/util')` 取 `kState` 探流控制器内部态（测试头 3 行
  `// Flags: --expose-internals`，`~/node-test/lib/internal/webstreams/util.js:57 kState`）。
  Bun：清单在列但 Bun 亦无 internal 树可逐字搬（`bun-compat.md:15-22`“没有一行是可逐字搬的”；
  `bun-parity.md:1305-1306` webstreams 余件“需 expose-internals loader 特性，记档”同族）。
  对齐定义：`internal/*` 按既有范围外（journal:1557）不做；本件 `Cannot find module
  'internal/webstreams/util'` 即偏离对齐，无续啃形态。
- W2–W6 adapters ×5（to-readablestream/to-readablewritablepair/to-streamduplex/
  to-streamwritable/to-writablestream）→ 记 bound（范围外）。
  Node：`require('internal/webstreams/adapters')` 取 `newReadableStreamFromStreamReadable` 等
  做 node 流↔web 流桥（`~/node-test/lib/internal/webstreams/adapters.js` 整文件即桥）。
  Bun：同 W1（清单在列 + 无 internal 树 + parity 记档同族）。
  对齐定义：同 W1，`Cannot find module 'internal/webstreams/adapters'` 即偏离对齐。
- W7 `test-whatwg-webstreams-coverage.js` → 记 bound（范围外）。
  Node：`require('internal/webstreams/util')` 取 `isPromisePending` 做品牌检查
  （`util.js:398 isPromisePending`，测试 `assert(!isPromisePending({}))` 三行即契约）。
  Bun/对齐：同 W1。
- W8 `test-whatwg-url-canparse.js` → 记 bound（引擎墙 + 范围外双重）。
  Node：`URL.canParse` 经 `bindingUrl.canParse` 直调 C++ ada 绑定
  （`~/node-test/lib/internal/url.js:1213-1228`，双重载 + 无参抛 `ERR_MISSING_ARGS`），测试另以
  `%OptimizeFunctionOnNextCall` 围 JS 壳测快路径（测试头 `--allow-natives-syntax` + 注释
  “Only javascript methods can be optimized through %OptimizeFunctionOnNextCall”）。
  Bun：`bun-compat.md:63` url 🟢（常规 parse 形态），但 `%…` V8 内禀 Bun（JSC）亦无，
  属跨引擎不可比（`bun-parity.md:487-500` vm ①类同理，记档口径）。
  对齐定义：`URL.canParse` 常规形（`node -e` 实测 `typeof canParse==='function'` 且本仓
  `--eval 'new URL(…).href'` 同绿）即对齐；`%OptimizeFunctionOnNextCall` 快路径断言永不实现，
  该件整体记 bound。
- W9 `test-whatwg-url-custom-inspect.js` → 续啃（双动可达，不 bound）。
  Node：`URL.prototype[inspect.custom]` 按 12 键展开（href/origin/protocol/username/password/
  host/hostname/port/pathname/search/searchParams/hash，`internal/url.js:935-959`，
  另 `depth<0` 坍缩 + `showHidden` 挂 context，journal:1538 fix-forward 已归一 USP 同族语义）。
  Bun：url 🟢（`bun-compat.md:63`），此形态 Bun 有。
  真机对照：node rc=0 全绿；wjs 输出 `'URL {}'`（空壳，`strictEqual` 首断即红，base18
  `Expected values to be strictly equal`），复现确认。
  对齐定义（续啃验收）：prelude 给 `URL` 补 `[inspect.custom]`（12 键 + depth<0 + showHidden，
  走 Rust url-crate getter 真值，searchParams 复用 USP 归一后形态），`util.inspect(url)` 逐字
  等于 node 多行形（含 `URLSearchParams { 'que' => 'ry' }` 内嵌）即算对齐；在此之前不记 bound。

### worker 真共享三件（journal:1476-1479/1514/1556；Bun 🟡副本语义即偏离）

- R1 SAB 真共享（`test-worker-message-channel-sharedarraybuffer` /
  `test-worker-workerdata-sharedarraybuffer` / `test-worker-message-port-wasm-threads` 连带）→ 记 bound（引擎墙）。
  Node：SAB 跨线程真共享内存（workerData/端口投递零拷贝，`~/node-test/lib/internal/worker/messaging.js`
  传输 + `clone_dom_exception.js` 误形还原）。
  Bun：worker 🟡（`bun-compat.md:80`），本仓与 Bun 同档：wire 存 byteLength 按
  `BYTES_PER_ELEMENT` 折算 + `k:"sab"` 品牌信封装副本语义（`bun-parity.md:1641-1646` 三轮根因，
  “真共享内存需跨线程底座，记档”原文即偏离定义）。
  真机对照：base18 三件 `TextDecoder.decode does not accept SharedArrayBuffer views yet`；
  主线程 `Atomics.wait` 本仓即抛（下 R3），SAB 视图跨端即 detach/拷贝，复现确认。
  对齐定义：副本语义 + 跨端 `detached` 文案 exact（`bun-parity.md:1631-1640` 二轮错误形状批）
  即偏离对齐；真共享（futex 可见性 + SAB 视图零拷贝解码）需跨线程 GC 底座，不开轮。
- R2 WASM Module 跨线程（`test-worker-message-port-wasm-module`）→ 记 bound（引擎墙）。
  Node：`WebAssembly.Module` 可经端口投递（结构化克隆扩展）。
  Bun：worker 🟡同档，无 WASM 跨线程主张（`bun-parity.md:1677-1679`“wasm 系列另案”）。
  真机对照：base18 `first argument must be a WebAssembly.Module`（本仓拒收），复现确认。
  对齐定义：抛类型错（不炸进程）即偏离对齐；真跨线程 Module 句柄需 SM 多 realm 共享编译产物，不开轮。
- R3 `Atomics.wait`（`test-worker-messaging-errors-timeout:37` /
  `test-worker-unref-from-message-during-exit:16` / `test-atomics-wake`）→ 记 bound（引擎墙）。
  Node：主线程可定时 wait（实测 `node -e Atomics.wait(…,10)` 回 `ok`），worker 线程可 futex 等待。
  Bun：worker 🟡；`bun-parity.md:1674`“Atomics.wait 2 件：worker 内 futex 等待（引擎 SM 面）”即同族记档。
  真机对照：本仓主线程即抛 `waiting is not allowed on this thread`
  （`--eval Atomics.wait(…,10)` 实测），worker 线程同错 + SAB 缺失双墙（journal:1514）。
  对齐定义：TIMEOUT 机制黑盒已证（journal:1514“TIMEOUT 机制本身黑盒已证”），`Atomics.wait`
  一律抛指定错即偏离对齐；真 futex 等待需 SM 线程阻塞原语，不开轮。

### 定级记档（三列表）

- bound（偏离即对齐）：W1–W8（7 internal + canparse 快路径）/ R1–R3（SAB 副本语义 + WASM 拒收 +
  Atomics.wait 抛错）。证据：node-lib 行号见上 + `bun-compat.md:15-22,63,80` +
  `bun-parity.md:1305-1306,1631-1646,1674,1677-1679` + journal:1537/1476-1479/1514/1556 +
  base18 results.log 上列各行 + 本段真机对照。
- 续啃（修到某形态才算对齐）：W9 custom-inspect（prelude `[inspect.custom]` 12 键 +
  url-crate 真值，`util.inspect(url)` 多行逐字一致）；余 whatwg 31（encoding/events/BYOB/
  searchparams 簇，C 轮路径）不在本段，仍归 C 队列。
- 另轮/范围外：`--expose-internals` loader 特性、`--allow-natives-syntax` V8 内禀、
  sea/wasi（journal:1557 维持否决），永不进对齐验收。

## 2026-10-20 W9 custom-inspect 续啃落地（prelude+inspect 核对齐）

- 实现：`src/builtins/prelude/url.rs`（+136 行，744 行守门内）——`URL.prototype[inspect.custom]`
  12 键展开（href…search/searchParams/hash 真机顺序，ultra 值走 url-crate getter，
  searchParams 嵌套走 USP 自身 custom depth-1）+ depth<0 返自身 + showHidden 挂
  `Symbol(context)`（`__wjs2_URLContext`，ada urlComponents 语义由 href 反推，
  node 5 形实测全对）+ 品牌门显式先行；`src/builtins/node/internal/inspect.rs`
  （重排 26 行）：custom 先于 depth<0 坍缩（真机实证 `o[custom](-1)` 被调，
  原“负深度不调 custom”注记证伪并删除；USP/iterator 系 custom 自返 `[Object]` 不变，
  URL 系返自身落空容器 prefix 形 `URL {}`）。
- 坑：prelude 系 `r#"..."#` 内禁 `"#`（`indexOf("#")` 截断 raw 串致 E0762 级联，
  改单引号 `'#'`）；`Reflect.apply(custom,[],{})` 须显式品牌门（USP 兜底 catch 会吞错致 NO-THROW）。
- 验证：node 套件 `test-whatwg-url-custom-inspect`（4 断言）转绿 rc=0；
  黑盒 `tests/builtins.rs::url_custom_inspect_faces`（正常 12 键+报错品牌+边界
  depth:0/子类/showHidden）绿；`--test builtins` 41/41；相邻
  searchparams-inspect/properties/parsing/global 全绿；`test-util-inspect.js`
  红系 `--expose-internals`（`No such binding: js_stream`）既有范围外非回归；
  冒烟 5/5（alt 二进制）；`check-naming: ok`；`check-lines` 本仓三文件全绿
  （fs.rs/h2_client.js 超限系并行 lane 未提交改动，非本轮）。
- 构建：sweep base19 正用 `target/debug`，本轮一律 `CARGO_TARGET_DIR=~/wjs-data/target-alt`
 （Xcode libclang），未碰主构建目录。

## 2026-10-20 base19 全域基线（base18 之后 ~99 提交，mixed-binary，SAME0 2397/3528）

- 运行（`scripts/sweep-bg.py start --prefix test- --tag base19 --jobs 2
  --wjs target/debug/winterjs2 --thread-id 3599 --port-base 29999`，缺省
  `docs/bun-scope.txt`，node 缓存开，超时 25s）：19:41 起 → 20:42:26 完，
  elapsed 3655.8s（~61min），3528/3528 无 abort。工件
  `~/wjs-data/sweep/base19/`（经软链实为 `~/.wjs-sweep/base19/`：
  status.json/results.log/worker.log/monitor.log/debug/）。node v26.8.1，
  缓存 3994 条。看门狗全程未越线（procs≤7、pressure=1、free 50–60%；
  load 20:14 spike 到 301 系他 lane 并行构建，未触发 abort 条件；
  系统盘 106→72GB 系他 lane 活动，本任务未清盘——开工时 100GB 余量充足）。
- 总表（`scripts/sweep-report.py base19`）：SAME0 **2397** / SAME1 206 /
  DIFF 841 / TIMEOUT 84（绿 2397/3528=68%）。base18：2284/203/925/116（65%）。
  Δ：绿 **+113**、DIFF **-84**、TIMEOUT **-32**、SAME1 +3；
  真实欠账（DIFF+TIMEOUT）1041→**925（-116）**。
- 域级绿变化（base18→base19，头条）：http2 98→**122**（+24，TIMEOUT 50→25）、
  tls 84→**93**（+9，TIMEOUT 25→14）、https 16→**46**（+30）、whatwg 13→**45**
  （+32）、vm 46→**67**（+21）、worker 57→**76**（+19）、console 5→**15**（+10）、
  util 9→**15**（+6）、webcrypto 1→**4**（+3）、readline 7→**10**（+3）、
  crypto 62→63、http 322→325、event 23→24、mime 0→1；
  下滑：fs 282→246（**-36，真回归，见下**）、stream 173→169（-4）、
  zlib 50→46（-4）、net 96→95、child 72→71、repl 55→54、process 70→69、
  stdio 2→1、pipe 7→6；其余持平。plan3 §0.3 表待用户确认后才改。
- 逐件翻转（两版 results.log 对位）：base18 红 1244 → base19 红 1131；
  转绿 **180**（http2 32、whatwg 32、https 31、vm 21、worker 19、console 10、
  tls 10、util 7、http 6、readline 5、webcrypto 3、crypto/event/fs/mime 各 1；
  归 wave-1/2/3 + R16b/C2/C3 各 lane）；新红 **67**（分类见下）。
  种翻转：TIMEOUT→DIFF 15、DIFF→TIMEOUT 6（双向，load 抖动）、
  TIMEOUT→SAME1 2（http2-close-while-writing、http2-server-rst-stream，
  node 侧同红，口径差）、DIFF→SAME1 1
  （http2-compat-serverrequest-trailers，R16b 已记 node 自红）。

### base19 mixed-binary 说明（先前报告已同步 parent，此处落盘为准）

- 19:39 本任务构建（HEAD db1152a）→ sweep 前 ~1410 件（~40%，monitor.log：
  20:02:58 done=1406 → 20:03:14 done=1420）→ 他 lane 20:03:02 重编
  `target/debug/winterjs2`（HEAD 806b5eb，size 319509832，
  md5 6decde08079abc54ef32d0c9e08eb122）→ 后 ~2118 件（~60%）新二进制。
  在飞 ±2 件误差。base19=mixed（决议：跑完不停，冻结后重跑纯净 base19b）。
- **构建污染（本任务过程失误，pitfall 草稿见本节末）**：19:39 构建前未查
  `git status`，工作区含他 lane **未提交**的 open 重写代码（事后合入为
  22f2db2，19:57）。取证：sweep-time debug 堆栈 `at open (node:fs:3048:22)` /
  `(node:fs:3049:26)` + `ReferenceError: m is not defined @ node:fs:3064:3`，
  bundle 行号验算 2658（fs_base943+streams591+sync9c436+fdsync688）+406
  （`void m;` 在 fs_async 行）=3064 精确命中；旧包同行是 `read()` 函数体，
  不可能报 `open` 帧——故旧二进制**已含**重写代码。旧=脏树构建，
  不能当 db1152a 纯净构建解读（前报“db1152a 构建”作废，以此为准）。

### 新红 67 分类（回归 vs 口径差/抖动）

- **A. 真回归·单根因 54 件 → 归 http2 respond-file lane（22f2db2，
  自述“未编译验证”）**：`export const open = __cb1(openSync…)` 被重写成
  固定位 `args[1]=flags/args[2]=mode`，三宗罪全经实测复现——
  ① 2 参形 `(path,cb)`→`invalid flag 'function…'`（test-fs-open.js:41 等）；
  ② 3 参形 `(path,flags,cb)`→`mode received function`
  （writeFile/chmod 经 open 下游；chmod 本体仍是 `__cb1(chmodSync)` 无辜，
  其测试内调 `fs.open` 被连带）；
  ③ 遗留裸行 `void m;`→4 参形及一切常规 open 报
  `ReferenceError: m is not defined`。名单：fs **37**
  （append-file/chmod-mask/chmod/empty-readStream/null-bytes/open-no-close/
  open/read-offset-null/read-stream/readfile-fd/ready-event-stream/
  sir-writes-alot/stat/stream-construct-compat×4/stream-destroy-emit-error/
  stream-double-close/truncate/write-buffer/write-file/write-negativeoffset/
  write-optional-params/write-stream×11/writefile-with-fd/
  writestream-open-write）+ m 下游 **17**
  （http-get-pipeline-problem/http-pipe-fs/pipe-file-to-http/
  readline-async-iterators×2/stdio-pipe-stderr/stream-flatMap/
  stream-preprocess/stream-promises/stream-readable-unpipe-resume/
  zlib-brotli/zlib-concat-gzip/zlib-gzip/zlib-zstd/
  http2-compat-client-upload-reject/http2-compat-serverrequest-pipe/
  http2-pipe）。修一处回 ~54 件。
- **B. 同 commit sendfile 面 4 件**（同 lane）：http2-respond-file-fd-leak /
  respond-file-fd / respond-file-filehandle（NGHTTP2_PROTOCOL_ERROR×2 +
  assert×1）/ compat-serverresponse-finished（assert）。
- **C. TIMEOUT 抖动 6 件**（load 301 期，双向翻转佐证，复判另轮）：
  http-agent/https-agent/net-bytes-written-large/
  process-exit-code-validation/util-inspect-long-running/
  http2-compat-socket-destroy-delayed。
- **D. 单件 3**：tls-close-error（复跑仍红，非 m，归 tls lane 复判）；
  child-process-exec-maxbuf（复跑已绿 rc=0，flake）；repl-tab-complete-
  on-editor-mode（wjs=0/node=1，node 侧 MUSTCALL flake，口径差）。
- 校验：54+4+6+3=67 ✓。

### Top10 欠账域（按清单件差额 DIFF+TIMEOUT 降序，base19 原始口径）

1. http2：n256 绿122 差额122（DIFF97+T25）双红12；2. tls：n185 绿93
   差额92（78+14）；3. fs：n333 绿246 差额76（75+1）双红11
   （**A 类修复后差额≈39，退居第 6**）；4. net：n138 绿95 差额42；
   5. http：n389 绿325 差额41 双红23；6. crypto：n120 绿63 差额40
   双红17；7. worker：n110 绿76 差额34；8. child：n100 绿71 差额29；
   9. vm：n95 绿67 差额28；10. webcrypto：n39 绿4 差额22 双红13。
- 后续：工作区分拆+代码冻结 → 修 A/B（http2 lane）→ 干净树重编 →
  全域 base19b（届时 plan3 §0.3 换表请用户确认）。

### pitfall 草稿（4.N 待编号，sweep-owner 构建纯度——本次失误，请入卷人定编号）

- 症状：sweep 基线混入未提交 lane 代码的回归（fs 37 连红），构建时
  HEAD（db1152a）干净但二进制脏，事后对 file-history 完全无解，险些误判
  为“已合入回归”让无辜 lane 背锅。
- 根因：`cargo build` 编译的是**工作区**不是 HEAD；多 lane 并行时工作区
  常脏（本轮 20+ 未提交修改），sweep-owner 直接 `cargo build` 即把他人
  半成品 bake 进基线二进制。
- 修法：sweep-owner 构建前必 `git status --short`，非空即停（多 lane 下
  stash 风险高，首选等干净窗口 / 让各 lane 先 commit+push）。本次若守此条，
  19:39 即会停下。
- 复现：base19（旧二进制含未提交 open 重写，堆栈 bundle 行号为证）。
- 铁律：**sweep 基线构建只认干净树**（status 非空不构建、不起 sweep）。
## 2026-10-20 W9 新区落地验证（lane/whatwg，worktree-local 构建）

- 迁移：基线 707a929 已含 806b5eb（W9 四文件在树），工作区干净零移植。
- 错峰：首编与 4 lane 全量重编撞车（load 384），当即 kill 全并行任务待回落；
  load 19 后 `cargo build -j2`（Xcode libclang）5m56s 编完，已报协调。
- 验证（worktree-local 二进制）：W9 套件 + 相邻 searchparams-inspect/properties/
  parsing/global 全绿 rc=0；`--test builtins` 41/41；冒烟 5/5；`check-lines: ok`
 （最大 1000 src/state/mod.rs）+ `check-naming: ok`（本区干净，无并行脏文件）。

## 2026-10-20 whatwg31逐件定级（自主定级，不问拍板）

> 方法：`docs/bun-scope.txt` `^test-whatwg-` 55 行全量双跑（新区二进制复用零新构建；
> 53 件在 `~/node-test` 检出，2 件检出无件见末）。base18 40 红 − 前段已定级 9
> （W1–W8 bound + W9 续啃落地）= 31 件逐件 node 真机对照，默认对齐 node+bun 自行定级。
> 结论先行：31 件当前双绿（抽查 9 件 node/wjs 双 0/0 + 55 件全量 wjs 复跑仅 8 红），
> 定级=已对齐（SAME0），下按簇记 Node 一句话 + Bun 行号 + 修法出处。

- E1 encoding ×5（fatal-streaming/api-invalid-label/fatal/utf16-surrogates/singlebyte）→ 已对齐。
  Node：`TextDecoder(label,{fatal,ignoreBOM})` 按 encoding 标非法标签抛错/fatal 抛
  `ERR_ENCODING_INVALID_ENCODED_DATA`（`lib/internal/encoding.js` + WPT 语料）。
  Bun：Web 通用面推定有（`bun-compat.md:63` url 🟢同族；`bun-parity.md` 无 encoding 缺口行）。
  修法出处：wave-1（fatal 文案/ASCII-trim/NUL 安全/大写规范，journal:1525）+
  L-E encoding-singlebyte 29→57（journal:1537/1546）。双跑 0/0。
- E2 events ×3（passive/signal/this-of-listener）→ 已对齐。
  Node：`addEventListener` 非函数监听 warn 而非抛（passive 门）/signal 校验/
  监听器 this 恒收 EventTarget（`lib/internal/event_target.js`）。
  Bun：events 🟢（`bun-compat.md:50`）。修法出处：wave-1
  （null-flatten/被动门/signal 校验/handleEager，journal:1525）。双跑 0/0。
- S1 streams ×5（bad-buffers-and-views/byob/compression/webstreams-encoding/
  transform-cancel-write-race）→ 已对齐。
  Node：BYOB 视图空/坏参抛 `ERR_INVALID_*` + `ReadableByteStream` pull 续进/close 后可用 +
  `CompressionStream` 枚举校验（`lib/internal/webstreams/*.js`）。
  Bun：stream 🟢（`bun-compat.md:59`）。修法出处：wave-1
  （BYOB coded 错误/start 竞速/pull 续进/close 后可用，journal:1525）+ webstreams 轮
  （`bun-parity.md:1292-1308`）。双跑 0/0（byob/compression 抽查）。
- U1 url ×6（custom-global/parsing/properties/setters/tostringtag/invalidthis）→ 已对齐。
  Node：URL 全局属性形态/解析抛 `ERR_INVALID_URL`/`ERR_MISSING_ARGS`/属性可枚举顺序/
  setter 转换先行/非法接收者品牌错（`lib/internal/url.js`）。
  Bun：url 🟢（`bun-compat.md:63`）。修法出处：wave-1（USP 品牌/缺参/lone-surrogate
  FFFD/双问号/迭代器品牌+tag，journal:1525）+ fix-forward USP 归一（journal:1538）+
  W9 inspect（本 lane）。双跑 0/0（parsing/setters/invalidthis 抽查）。
- U2 searchparams ×12（append/delete/entries/foreach/get/getall/has/keys/set/
  stringifier/values/constructor/custom-searchparams）→ 已对齐。
  Node：`URLSearchParams` 增删改查 + 迭代器三形 + 构造器三形（`lib/internal/url.js`
  searchparams 族）。Bun：url 🟢同上。修法出处：wave-1（MIME 全 fixture 952 对拍 +
  TES/TDS 缺失补齐，journal:1525）+ fix-forward（序列 key-presence/定长二元组/record
  含 symbol 键/Map 去重，journal:1538）。双跑 0/0（全量 wjs rc）。
- 前段 8 bound（readablebytestream/adapters×5/coverage/canparse）维持，不在 31 内，
  见上段 W1–W8。
- 检出无件 ×2（`test-whatwg-url-custom-domainto` / `test-whatwg-url-toascii`）→ 记档
  非欠账：Bun 快照有、node-main 检出无（`ls test/parallel` 零命中），系上游更名/删除，
  双边无法跑，不计红。

## 2026-10-20 tls P2-tls 下一簇探针（只探针+定级，不写码）

> 范围：`docs/bun-scope.txt` tls 201 行（`^test-tls-` 186 + 关联行；另 `^test-https-` 76 不在内）。
> 基线：base18 tls 红 101（DIFF 76 + TIMEOUT 25，`~/wjs-data/sweep/base18/results.log`）。
> 探针：`~/wjs-data/probe/tls-next/p{1,2,3,4,5}-*.js`（node v26.8.1 vs 本区二进制；
> 回环短连 + 8s GUARD，无新构建）。Bun 口径：`docs/bun-compat.md:53` https 🟡
> （无 SNI 回调等）/ `:75` tls 🟡（无 psk/OCSP/resume）。

- P1 SecureContext：node 空构造/键证构造真，`ticketKeys` 门在选项形
  （47→`ERR_INVALID_ARG_VALUE` 48 字节，48 经；`setTicketKeys` 非 SecureContext 方法，
  双边同 `not a function`）；pfx 糊串 node 报 `not enough data`，wjs 报
  `ERR_METHOD_NOT_IMPLEMENTED`（loadPKCS12 未实现，bound 维持）。
  wjs 与 node 逐行同形（`empty/keycert/ticket48-47/pfx/SecureContext` 六行全对）。
- P2 getPeerCertificate：node 精简 19 键（含 asn1Curve/bits/exponent/ext_key_usage/
  modulus/nistCurve/pubkey + raw），wjs 12 键（缺上列 7 公钥材料键），余
  subject/issuer/raw/auth 逐字同形（含 `UNABLE_TO_VERIFY_LEAF_SIGNATURE`）。
  → 下簇 N1（见下）。
- P3 ALPN：交集双边同 `a`（服务端偏好序；wjs 连服务端侧 `server-alpn=a` 亦对），
  无交集双边同 `ERR_SSL_TLSV1_ALERT_NO_APPLICATION_PROTOCOL`。静态面已对齐；
  `ALPNCallback` 选项面 bound 维持（rustls 无 select-callback，journal:1400-1401）。
- P4 verify（CSI 6 格）：双边逐格同形（unfqdn/IP/通配 OK，deep 通配/有 SAN 忽略 CN
  报 `ERR_TLS_CERT_ALTNAME_INVALID`，无 SAN 回落 CN OK）——B-3 移植确认。
- P5 SNI：`addContext` 双边行为分歧——node `_contexts` 2（通配正则对），wjs 0
  （未下沉，引擎墙 bound 维持，journal:1542）；无 SNI 时 node `servername=false`，
  wjs 回落 `"127.0.0.1"` → 下簇 N2（见下）。

### 定级表（101 红去向，自主定级）

- 下簇续啃 N1 getPeerCertificate 补 7 键（asn1Curve/bits/exponent/ext_key_usage/
  modulus/nistCurve/pubkey）：需 DER-SPKI 解析（`x509-parser` 候选，新依赖按规
  审批后再开码；验收=19 键逐字 + `peer-certificate/multi-keys` 转绿）。
- 下簇续啃 N2 无 SNI 时 `servername=false`（JS 面小修：SNI 缺席不再回落 host；
  验收=`empty-sni-context` + `https-agent-sni` 相关断言；wave-1 `''` 形已铺路）。
- bound 维持：pfx-loadPKCS12 ×5 / psk ×2 / DHE / renegotiation ×2 / OCSP /
  resume ×3 + ticket / ALPNCallback / SNI-raw + addContext 未下沉
  （rustls 能力墙，`bun-compat.md:53/75` 同缺为证）。
- 另案（net/stream 面，随建模）：socket-close/destroy/default-options/
  streamwrap-buffersize（Duplex EOF 面）/client-destroy-soon（readable 流量）/
  async-wrap ca 链。
- 另轮 time-box（本次未探针，TIMEOUT 25）：cli min/max-version 系 ×6 /
  cert-chains-concat 系 ×2 / handshake/secure-session/interleave 等散件，
  按“单件 run1.sh 复判 → 版本墙即归 B”逐件销账，不在下簇内。

## 2026-10-20 N2 无SNI→false 落地（JS 小修，零新轮子）

- 根因（node `lib/internal/tls/wrap.js:649/1064-65/1201-1203`）：`servername` 初值 null，
  仅显式选项经 `setServername` 置名；finishInit 时 null 即取 `handle.getServername()`，
  未发 SNI 即 false（C++ 无名回 false），永不回灌 host。旧码直拨面 Rust 回落
  host 致客户端 IP 直连自见 `"127.0.0.1"`（P5 探针）。
- 修法（`src/builtins/node/tls.js`，纯 JS）：`__sniSent` 实发旗（直拨/包裹双路 wire
  置名点）+ `__applyTlsInfo` 仅实发才收 info 名、无收客户端亦置 false（服务端行为不动）+
  `__sniHost` 回落 helper（7 处 CSI/报错 host 由 `??` 改串形判定，false 不再透入 CSI）+
  `setServername` 补 Received 后缀（invalidArgTypeHelper 口径）+ 服务端禁签发
  `ERR_TLS_SNI_FROM_SERVER`（wrap.js:1239-1246 口径）。
- 切片先行：`tls.js` 恰 1000 行，`split-js.py` 纯搬移尾 64 行 → 新片
  `tls_defaultecdhcurve.js`（`cmp` 字节恒等；脚本 rs 改写错位已手工扶正多行 concat），
  搬移提交与 N2 提交分离（`git diff -w` 仅路径）。
- 验证：P5 `nosni-servername=false` ✓；node 套件 `test-tls-error-servername` 转绿；
  黑盒 `tls_client_absent_sni_is_false`（显式/IP-缺席/''/setServername×2）绿；
  `--test node tls_` 13/13；冒烟 5/5；`check-lines/naming` 双绿。
  余红（sni-option/servername/server-client/empty-sni）皆服务端选链
  （`no server certificate chain resolved`）即 addContext 未下沉 bound，不在 N2 内。

## 2026-10-20 N1 公钥七键落地（x509-cert 0.3 复用，零新依赖）

- 轮子结论：树内 `x509-cert = "0.3"`（直引）+ `der = "0.8"`（直引）足够——
  SPKI 算法 OID/参数/BIT STRING 全公开可读，RSA 内层 PKCS#1 由 `der_tlv` 手工取，
  曲线名/端口径静态表，无需新 crate，不走 dependencies3 追加。
- node 形状（探针实证）：RSA（bits 2048/exp `0x10001`/modulus 大写 512 无前缀/
  pubkey 取 SPKI 整段 294B）/ EC（asn1Curve `prime256v1`/nistCurve `P-256` 字符串形/
  bits 256/pubkey 取点字节 65B 头 `0x04`）/ 他族（Ed25519 七键全 undefined）/
  EKU 缺席 undefined、有值点分 OID 数组 / 19 键恒在定序（N/A 显式 undefined 不缺席）。
- 修法：`crypto_x509.rs` `x509_parse` 增 pkBits/pkExp/pkMod/pkCurveAsn1/pkCurveNist/
  pkPointB64（RSA der_tlv 取模幂 + BN_bn2hex 去前导零字符口径；EC parameters 解
  曲线 OID + 点字节 b64；AIA 1.3.6.1.5.5.7.1.1→infoAccess OCSP/CA-Issuers 表；
  subject/issuerObj 多值 RDN 数组化）+ `crypto_pqx509.js` `toLegacyObject` 19 键定序
  透传（含 subjectaltname 空值 undefined 化）+ 构造器垃圾串改 PEM 行错
  （旧 `ERR_INVALID_ARG_VALUE` 系偏差，x509 黑盒旧断言同步翻转）+
  附带服务端自持首证 `__ownCert`（wrap/派发双路，multi-keys 套件）。
- 验证：node 套件 `peer-certificate`（pubkey 哈希钉死 + OCSP URI + 链）/
  `peer-certificate-multi-keys`（OU 数组 + 服务端自持）双转绿；
  黑盒 `tls_peer_certificate_pubkey_material`（RSA/EC/坏块/定序）绿；
  `--test node tls_` 14/14、`x509` 6/6、`builtins` 41/41；冒烟 5/5；双守门绿。
  出界记档（非 N1）：`getcertificate-x509` 需 X509 链解析（issuerCertificate 挂链）、
  `client-getephemeralkeyinfo` 需 DH 临时密钥面（rustls 无 DHE/无临时密钥 API），
  另轮定级。




## 2026-10-20 lane合回master merge预览（只读merge-tree，4 lane）

- 方法：`git merge-tree $(git merge-base master lane/X) master lane/X`（只读，
  未动工作区）。四 lane 基线同为 707a929；master 自基线仅进 journal
  （c1a5cf7 base19）+ plan3.md（R16b bound），代码面零推进——故代码合回全绿。
- lane/http2：tip=707a929，master..lane 独占 0 件——**无待合入（空）**。
  注：http2 lane 活全在 master 直推（含 open 回归 22f2db2，已在 master）。
- lane/vm：同上，独占 0 件——**无待合入（空）**。
- lane/console：独占 1 件（336e7b0，纯文档：journal 任务3 + pitfalls
  4.290/4.291 + 索引行）。pitfalls-06.md/pitfalls.md 自动合入无标记；
  **冲突 1：docs/plan3-journal.md 尾部双追加**（我方 base19 段 vs 对方任务3段，
  同文件改重叠、零重命名/零删除改并存；解法：两段全留按时间排序）。
- lane/whatwg：独占 5 件（f1127e3/ee0b4c6/ee67142 journal + 7afbd18 tls.js
  纯搬移切片 + 1246228 N2 fix）。tls.js/tls.rs/tests/node/tls.rs 自动合入
  （全系对方单边改，master 未碰三文件）；tls_defaultecdhcurve.js 单边新增；
  **冲突 1：同上 journal 尾部双追加**（1 组标记块，两段全留）。
  零重命名/零删除改并存。
- 合回顺序（console/whatwg 文件集除 journal 外不交，任一序皆可；推荐）：
  1. console 先（纯文档，零代码风险，journal 冲突先解掉一半）；
  2. whatwg 后（代码：tls 切片+N2 fix+tests，合后重编跑 tls 域+冒烟）。
  http2/vm 跳过。base19b 等 fs 修复+冻结后另排（本任务不启动）。
## 2026-10-20 任务3：console formatTime + promisify-custom-names（console lane，转绿）

- 范围（Bun 清单内 2 件）：test-console-formatTime.js（console 域）+ test-util-promisify-custom-names.mjs（util 域）。真机 node 26.8.1 逐项实测先行（时间格式/颜色开关/inspect 交互全量可观察项以真机为准）。
- R1 formatTime（debuglog 时间机器）：`require('internal/util/debuglog')` 报 Cannot find module（本仓只有自造 `node:internal/debuglog`）。修法：`internal/debuglog.rs` 补 `formatTime` 逐字移植（`pad/kSecond/kMinute/kHour` + 三档 `ms/s/m:ss/h:mm:ss`，primordials 还原直接调用）+ `internal/mod.rs` 加 `node:internal/util/debuglog` 同源别名（inspector/promises 先例）+ 单测：别名解析/同源断言（INTERNALS.len 81→83：HEAD 表实 82 而断言写 80，已 stale 两轮，按实测落数注记）。16 值全对（含边角 0/-5/0.0004→0ms/0.0005→0.001ms/999.9999→1000ms/整分时界/NaN→NaNms；Symbol 抛 TypeError 真机同形）。黑盒 `console_debuglog_format_time_faces`（正常 8 断言逐字 + 报错 + 边界）。
- R2 promisify-custom-names（stream eos 别名）：`promisify(stream.finished).name` 本仓 'eos'，真机 'finished'。根因：util.rs promisify 自创改名 fallback，错改全局共享 custom（坑 4.290）。修法：删 fallback 恢复 node 原文（custom 原样返回）；各域定义处正名：fs.exists 命名函数表达式（`__existsOrig` 捕获避自递归）/ child exec 系 `defineProperty(name, orig.name)` / h2 connect `Object.defineProperty`（仿 assignFunctionName 只改名不绑名；h2_client.js 993→1002 超门，压回 994）/ crypto generateKeyPair 正名（防回退污染）。黑盒 `util_promisify_custom_names_faces`（9 名逐字 + custom 恒等 + 原名不被改写回归钉）+ 既有黑盒 `prom-name` 断言 origFn→""（真机即 ""，旧断言写的是偏离，随修法更正； discipline：旧断言与真机冲突先实测再翻转 4.65/4.71/4.82）。
- 验证：两套件 wjs rc=0；黑盒 console/util 6/6；单测 internal_spec_table；冒烟 5/5；`check-lines` ok（h2_client 994/util 996）；`check-naming` ok。回归抽查：keygen-promisify rc=0；stream-finished DIFF / stream-pipeline SAME1 皆 base18 既有红（干净树重跑另见下）。
- 定级（自主定级，按口径更新：默认对齐 node+Bun，书面偏离）：timers.setImmediate/setTimeout、readline.question 的 custom 缺失——名断言经通用拷贝路径已绿，行为 custom（timersPromises/AbortSignal）属 timers/readline 域，不在本轮改动（改即跨域行为变更，记档不碰）。console 全局 time/timeLog/timeEnd 未改（Rust Instantms 形；test-console.js 系另一簇，_times 暴露/缺标 warning 等未动）。
- 交付 commits：8050f60（formatTime 簇）+ 707a929（promisify 簇），已合入 lane/console 基线。
## 2026-10-20 TIMEOUT C续啃复核（base19 http2 TIMEOUT 25件逐件定级+优先级）

- 口径：L-C 定级（journal:1535）是 base18 50 件 TIMEOUT 的账（7已绿/1 runtime/
  3 node自红/24 hyper-bound/15 actionable-shell）。base19 http2 TIMEOUT 25 件
  （base18 50→19 绿+7 DIFF+2 SAME1+22 留守+3 新增），本节按 L-C 五类逐件复核。
  证据：results.log 全行 wjs=142（我方挂死；HANG_EXIT 自报无输出）+ node 侧
  （24 件 node=0，settings-unsolicited-ack node=1）+ 逐件套件源码（机制归类）。
  monitor.log 无 abort（load 10 前后，TIMEOUT 非资源型）。
- base18 50 件去向：19 转绿（C2/C3/wave：close-idle束/serverresponse-end/
  connect-method/upload/propagate/server-socket-destroy等）+ 7 转 DIFF
  （挂死已解，快败断言，见 C-continuation）+ 2 转 SAME1（close-while-writing/
  server-rst-stream，node 侧同红）+ 22 留守 + 3 新增。
- 复核分类（25 件）：hyper-bound **13** / actionable-shell **11**
  （C壳9 + sendfile面2）/ node自红 **1**。L-C 的 7已绿已兑现超额（19绿）；
  1 runtime（early-hints 主件）现为 DIFF（MUSTCALL 未达，投递缺口仍在，
  见下）；3 node自红复核为 settings-unsolicited-ack（node=1，
  首包 initial-SETTINGS 断言漂移）+ close-while-writing/server-rst-stream
  （T→S1，R16b  trailers 系同族）。

### hyper-bound 13（hyper底座/原始帧/引擎面，C 不开轮，time-box 另议）

- altsvc / origin：ALTSVC/ORIGIN 帧面（R16b B2 bound）。
- misbehaving-flow-control / -paused：手搓 WINDOW_UPDATE 滥用 → 窗口溢出映射
  （R16b 帧错映射 bound）。multiplex：手搓 stream-ID 复用 + NghttpError
  （--expose-internals）→ 会话拆除（原始帧+内禀 bound）。
  ping-unsolicited-ack：unsolicited PING ack → Protocol error 映射（bound）。
  reset-flood：手搓非法流 + worker + maxSessionInvalidFrames（bound）。
- max-settings：maxSettings:1 + 双 settings → 会话即毁无流（hyper SETTINGS
   enforcement 面；C2 只做了 remoteSettings 嗅探， enforcement 另议）。
- exceeds-server-trailer-size：wantTrailers + 超大 trailer → frameError
  NGHTTP2_FRAME_SIZE_ERROR 映射（帧错映射 bound；C 轮 RST 注入机器
  若覆盖另提单件复判，默认 bound）。
- allow-http1：hyper ALPN 降级 HTTP/1（底座面）。client-http1-server：
  HPE_PAUSED_H2_UPGRADE + NghttpError Protocol-error 映射（错映射 bound）。
- max-session-memory-leak / forget-closed-streams：maxSessionMemory 会计/
  nghttp2  closed-stream 修剪（hyper 内部 + 1000/10000 次慢型，perf 边界）。

### actionable-shell 11（C续啃，优先级逐件）

- P0（新增 TIMEOUT，先单件复判是回归还是抖动，各跑 3 遍）：
  compat-socket-destroy-delayed（res.end 后 setImmediate destroy，
  base18 绿→19 挂；C2 destroy-cascade 疑连带或抖动）；
  client-proxy-over-http2（CONNECT + emit connection 隧道，C2 connect 面近亲）；
  goaway-opaquedata（session.goaway opaque 回传；journal:1779 已开工，
  GOAWAY-drain 机器现成）。
- P1（机器现成：L-C GOAWAY-drain/destroy-cascade + sendfile lane）：
  socket-close（socket-timeout-first 关停，L-C/1527/1544/1562 三记未解；
  定点复判）；graceful-close（server.close+1MB 放空，drain 面）；
  respond-file-error-pipe-offset（FIFO+offset NOSEEK onError，
  sendfile lane 范围，先等 open-fix 后复探）；respond-with-file-connection-abort
  （sendfile 中途 destroy → ECONNRESET，同上）。
- P2（tls-face/CONNECT 壳，tls lane 协作）：connect-tls-with-delay
  （手 tls.connect + createConnection 注入 + remoteSettings）；
  server-unknown-protocol（unknownProtocolTimeout + 非 h2 ALPN fatal alert，
  ALPN 面 L-A 余温）；client-connection-tunnelling（TLS 上 CONNECT +
  双 emit connection）。
- P3（分发壳）：autoselect-protocol（raw net 嗅探 PRI 分发 h1/h2，
  emit connection 面）。
- 单件复判要点（run1.sh 缺席，用替式，node-test/test/parallel 下，
  TMPDIR=/Users/bemly/wjs-data/tmp，槽位自选如 TEST_THREAD_ID=36xx +
  NODE_COMMON_PORT=31xxx，WINTERJS_HANG_EXIT=20 开自报定位未触发回调）：
  `TEST_THREAD_ID=36xx NODE_COMMON_PORT=31xxx perl -e 'alarm 40; exec @ARGV or die'   /Users/bemly/Projects/winterjs2/target/debug/winterjs2 --run <file>`；
  看点：P0 三遍是否稳定挂（抖动即记 flake 回归队尾）；P1 看 close/goaway/
  drain 回调哪一环未达（自报行即创建点）；sendfile 两件先等 open-fix 合入
  后复探（现挂含 m 连带可能）；P2 看 remoteSettings/ALPN 回调。
- C-continuation（base18 TIMEOUT→现 DIFF 7 件，挂死已解，另队不占 C 轮）：
  client-upload / respond-file-with-pipe（m 连带，open-fix 后自愈，先复探）；
  expect-continue-check / max-concurrent-streams / too-many-streams
  （strict-equal 快败，小轮）；early-hints-invalid-{type,value}（hints ARG
  校验，小轮）；early-hints 主件（MUSTCALL 未达=1 runtime 投递缺口未合，
  归 runtime 面）；ping-settings-heapdump（v8.getHeapSnapshot 内禀缺口，
  转范围外/bound，不开轮）。
## 2026-10-20 P0三件各3遍复判（9 runs，全确定性挂死，零抖动）

- 二进制（零新构建，借现成）：`~/wjs-data/wt/wt-whatwg/target/debug/winterjs2`
  （mtime 21:10，md5 4d540db191d9c19ae6c9d650134bbf51，worktree HEAD 1246228
  = lane/whatwg tip；含 22f2db2 open 缺陷但三件不走 fs.open，无干扰）。
  主仓 target/debug（20:03，6decde08）未动，未使用。
- 命令（node-test/test/parallel 下，TMPDIR 外置，槽位递增
  TEST_THREAD_ID=363x + NODE_COMMON_PORT=3133x）：
  `TEST_THREAD_ID=.. NODE_COMMON_PORT=.. WINTERJS2_HANG_EXIT=20   perl -e 'alarm 45; exec @ARGV or die' $WJS --run <file>.js`
  （注意变量名是 `WINTERJS2_HANG_EXIT` 带 2，见下）。
- 结果（3/3 全同，无一转绿/快败）：
  1. compat-socket-destroy-delayed（3× rc=1，自报 net=1）：未触发
     test:31 `request.once('response')`——连接已建，服务端回包 HEADERS
     从未到达客户端。base18 绿 → **真回归**（C2 destroy-cascade 或
     C3 serverrequest 改动嫌疑，归 C 轮复查）。
  2. client-proxy-over-http2（3× rc=1，自报 net=3）：未触发 test:47
     `server.once('connect')` + test:27 隧道 response——首个 CONNECT
     请求服务端根本未分发。base18 绿 → **真回归**
     （C2 connect-method 面嫌疑）。
  3. goaway-opaquedata（3× rc=1，自报 net=1）：未触发 test:20
     `server.on('stream')` + test:24 session-close——连接已建 stream
     未分发。base18 即 TIMEOUT（留守，GOAWAY opaque 面，journal:1779
     进行中）。
- 共性：三件皆“连接已建、服务端请求/流分发永不到”，compat-server 分发
  路径嫌疑最大（C2/C3 动过 listen/serverrequest/destroy 级联）。
  P0 不再复判，直接进 C 轮修（socket-destroy-delayed 与 proxy 为回归，
  优先于 goaway 留守件）。
- 附带发现（sweep 机制 bug，未修，归 runtime lane）：sweep-bg.py 写
  `WINTERJS_HANG_EXIT`（无 2），runtime 读 `WINTERJS2_HANG_EXIT`（有 2，
  src/runtime/mod.rs:582）——sweep 全量 TIMEOUT 件的提前自报**从未触发**
  （base19 results.log 零 HANG/MUSTCALL 行即证）。修法：一行改名对齐 +
  重编验证（未知哪边是笔误，以 runtime 为准改脚本侧）。
## R19-C3 收尾：respond-file x3 重做 + socket-close adopt（lane/http2）

### respond-file x3（range/with-pipe/error-pipe-offset，转绿待验证）
- 根因三件：
  1. range：node doSendFileFD 对常规文件恒覆写 content-length（min(size-offset,length)），旧移植靠 statCheck 手填 → `null !== 3`。
  2. with-pipe/error-pipe-offset：本仓 fs.open 回调面同步执行，FIFO 阻塞 open 卡死 JS 线程（writer 在 load 期先行调用即死锁，port 侧永远到不了）→ 双 TIMEOUT。
- 修法（纯 JS + 1 Rust 字段，无新 native）：
  - 新 h2_sendfile.js：node core.js doSendFileFD/doSendFD/processRespondWithFD 逐字（类检→statCheck 2参/3参→HEADERS_SENT 复检→自动 content-length→泵体；应答错直 destroy 无 onError；目录 SEND_FILE/显式 offset 在非常规上 NOSEEK）。
  - fs_async.js：async open/writeFile FIFO 分支（statSync 嗅探→O_NONBLOCK + setImmediate 轮询；ENXIO/EAGAIN 重试；常规路径零改）。
  - fs_fd.rs OpenFlags.nonblock 经 custom_flags 直通；fs.rs io_code 增 ENXIO(6)/EAGAIN(35/11)+单测。
  - 坑：O_TRUNC 开 FIFO 即 EINVAL（nb-open 只取纯方向位）；中央 ERR_INVALID_ARG_TYPE 传 ['number','FileHandle'] 数组才渲染 node 形。
- 证据（待 worktree 构建后补）：套件三件 node 双绿基线已取；黑盒 5 件（range/pipe/noseek/dir/fd-validation）。

### socket-close（TIMEOUT 根因 + adopt 桥，待验证）
- 根因：h2Server.emit('connection', socket) 我方零监听；node 链为 net connectionListener→TLS 握手→secureConnection→http2 建会话。debug 日志 rustls client hs 发出后服务端零响应。
- 修法：adopt 通道桥（AdoptIo：读走 inbox channel、写走 H2Stream/adoptData 事件；__wjs2_h2_adopt/_data/_close + state::net_ev_tx；复用 serve_conn；server_id 取 conn_id 独占路由）。JS：Http2Server 常驻 connection 监听（native listen 不经此事件）；connClose 连带 destroy 注入 socket（双向拆链皆洁）。
- 证据（待补）：套件 node EXIT=0 / 我方旧版 TIMEOUT(25s)；黑盒 http2_socket_close_unordered_teardown。

### 过程记
- 主仓多人混写：提交前 hunk 级验纯度（fs_async.js 的 exists-hunk 用过滤 patch 剔除）；commit 22f2db2 push origin/master（后迁 lane/http2 续）。
- 本机无 /opt/homebrew/opt/llvm：LIBCLANG_PATH 用 CLT（/Library/Developer/CommandLineTools/usr/lib）。
- 新区 4 lane 齐编 load>300：错峰（load<50 + cargo -j2），构建停后 -j2 续编。

### 验证轮次（worktree wt-http2，target 本地）
- x3 + socket-close 首验：range绿；with-pipe EXIT=1(mode误传)→修；pipe-offset TIMEOUT→会合分析→linger→绿；
  socket-close TIMEOUT→adopt桥→h2c通/outbound断→查得Rust payload漏connId前缀→修→h2c/TLS全通。
- 回归：fd/filehandle因fd-length=-1被当0长红→泵按length重构→绿；fd-leak因closeSync绕mock红→
  __h2TryClose异步关→绿；serverresponse-finished因finish早发+socket可见红→finish延至close+closed门→绿
  （destroy-suite SAME1双红非回归）。fd-invalid留红（fd-reuse竞态，我方合成fd无此竞态，另案）。
- open三宗罪（sweep c1a5cf7定责）：2参rest切片修、void m删、node三态对齐；三参早已修。
- setImmediate紧循环饿死I/O（listen永不到）→全部轮询改setTimeout(1)（fs __fsPoll/h2 __h2Poll）。
- POSIX无写者读端得0（非EAGAIN）→首0续候+linger-seen位（空写者记档偏离）。
- 黑盒：http2域30/30（含新6件），fs域29/29×3（首跑1败复跑绿，判flakes），io_code单测绿，冒烟5/5，守门双绿。
- fs域sweep c3-fs-verify进行中（worktree二进制，thread-id 7701/ports 33999）。
- fs sweep 收官（c3-fs-verify，333/333）：SAME0=282/SAME1=11/DIFF=39/TIMEOUT=1；
  对 base19（DIFF 75）净修 38（open/writeFile/stream 全家即三宗罪 fallout），
  1 新红 test-fs-write-stream-patch-open：单跑/复现/单文件sweep/分类器(3x wjs绿)皆绿，
  判 sweep 并行交互 flakes（自 fork 套件；与本轮改动无涉：child 侧 open 已 mock 掉）。
  rerun-red 复核 51 件同分布。
- open 黑盒 fs_open_arg_forms 落 tests/node/fs/basic.rs（2/3/4 参形+ENOENT+无cb）。
- commit：9c0daf3（open三态+sendfile+finish，4文件）已推 origin lane/http2；
  adopt 桥（socket-close）随后单 commit。

## 2026-10-20 HANG_EXIT变量错位一行修（sweep-bg写侧→runtime读侧对齐）

- 定名（读两侧源码）：runtime 消费侧 `WINTERJS2_HANG_EXIT`
  （src/runtime/mod.rs:582 + 单测 tests/node/process_.rs:235 用同名 +
  `WINTERJS2_LOG` 前缀惯例）；sweep-bg.py 写侧 `WINTERJS_HANG_EXIT`
  （设置行 + 结果 grep 行）为typo。以消费侧为准改脚本侧。
- 修（scripts/sweep-bg.py，1 文件 3 行）：设置行
  `WINTERJS_HANG_EXIT=` → `WINTERJS2_HANG_EXIT=`；结果 grep
  `"WINTERJS_HANG_EXIT"` → `"WINTERJS2_HANG_EXIT"`；注释同名。
- 构建（主仓 target/debug，load ~9，cargo -j2，50s，树干净，HEAD~1c1fd4d 时代、
  先于 http2 lane 合入 4a93a88；验证件不走 fs.open，结论不受 open-fix 影响）+
  冒烟 5/5 全过。
  注：构建时 pid 43394 常驻 fixture 进程（child-process-persistent.js，
  1h39m）在跑——relink 不影响已 exec 镜像；构建后其 lane 若新 spawn
  才取新二进制（本次仅脚本改动，二进制行为等价）。
- 验证（单件，test-http2-socket-close.js，WINTERJS2_HANG_EXIT=20）：
  rc=1 + `winterjs2: event loop made no progress for 20s
  (WINTERJS2_HANG_EXIT) — open: …net=3…` + `Mismatched … socket-close.js:46`
  定位行——HANG 自报链（环境变量→自报→MUSTCALL 定位→sweep grep 标 HANG）
  全通。对照：不设变量同件 32s 无输出（默认语义不变）。

## 2026-10-20 任务3收尾（lane/console 干净树重验 + 定级）

- 构建：新区首编 `cargo build -j2`（`-j` 须跟子命令后；load<50 错峰，9m33s）。
  本机无 nextest（`cargo nextest` 无此命令），按旧例 `cargo test` 单 target 兜底。
- stream 6 红干净树重跑：首跑 17/7——根因为基线内 sibling 已合入提交
  22f2db2（http2 respond-file/FIFO，注"待验证"）遗留 `void m;`（`fs_async.js:406`，
  `void` 求值未声明 `m` 即抛）→ 凡走 `fs.open` 的流全炸（`open (node:fs:3064)`）。
  修法：删该行（单行，无行为；归属 http2 lane，本 lane 为验证让路，commit 注明）。
  重编后 23/1 → 24/24（`stream_` 全集），三跑记录：fail/pass/fail 唯一波动件
  `fs::streams::read_stream_fifo_end`（FIFO 会合竞速，sibling 新测试，FLAKY，
  http2 lane 跟进；4.275 家族时序味）。
- base19 `m is not defined` 瘟疫复核：45 件中 5 件系 base18 既有
  （`ShadowRealm` 面），余 40 件（fs 17/stream 6/http2 6/zlib 4/readline 3/…）
  皆混合二进制（20:03 构建撞 sibling 半截编辑）污染，非语义回归；base19 扫时
  load 仅 10~33，排除抖动。
- 对应域回归：console::+util:: 17/17；stream:: 12/12；crypto 域含 keygen-promisify
  绿（`generateKeyPair` 正名无回退）；promisified exec 功能探针
  （`promise.child` + `{stdout,stderr}`）正常。两件 `child_*` 红定级：
  `child_exec_shell_self_and_timeout` 系 journal 既有记档的 HEAD 预存红（stash 对照）；
  `child_spawn_abort_and_surface` 三跑中时隐时现，spawn/abort 时序敏感，
  本 lane 改动仅 custom 改名（回调路径零触及）+ 模块加载正常（63 件同跑过），
  判 flaky 嫌疑，child lane 跟进。http2_faces 全量子集本环境 >20min 不收敛
  （挂死件超时；connect 改名面已有 node 套件 rc=0 覆盖），记 infra 超时未竟。
- console 剩余簇（test-console.js，Bun 清单外）逐件探针+定级（草稿见
  `~/wjs-data/probe/console-remaining-draft.md`）：警告 8 件（Warning 无 code，
  文本逐字已钉）/ `_times` SafeMap / dup 不重置 / Symbol TypeError /
  时长 formatTime / 余参续接 / 标签归一 / `__proto__` 可存。
  定级：模块面 5 小修可转（归一+has 守卫+emitWarning+formatTime 引入）；
  全局面需 warning 桥 + `_times` 可见面（中修，或按 §7 骑模块面，另立项）；
  长时 `h:mm:ss` 全局面可记偏离（套件正则短时恒过）。
- 交付：两套件 rc=0（干净树复验）+ 黑盒/单测/冒烟/双守门全绿（见上轮）；
  commits：8050f60 + 707a929 + 336e7b0（记账）+ 本轮 void-m 单行（待推）。

## 2026-10-20 test-console.js 模块面5小修（lane/console，转绿）

- 范围（Bun 清单外，以 node 原文为锚）：`new Console()` 实例的 time/timeLog/
  timeEnd/count/countReset。修法（`src/builtins/node/console.rs` JS 面，
  零 Rust 改动）：入参 `${label}` 归一（Symbol 抛 TypeError，真机同）+
  `time` has 守卫（dup 警告并保留原计时）+ 缺标/缺数走 `process.emitWarning`
 （8 文本逐字，Warning 无 code）+ 时长 `process.hrtime()` 差值喂本轮落地的
  `formatTime`（`node:internal/debuglog` 具名导入，无循环）+ `countReset`
  命中删 `${label}`（不预归一，原文同）。`timeLog` 余参经 `_out` 续接既有。
- 黑盒 `console_timer_count_warning_faces`（输出形×7 正则 + 归一标签 +
  dup 不重置 + `__proto__` + 死链回落 + Symbol×3 TypeError + 8 警告逐字；
  SL| 行前缀避 JSON 转义整行匹配，自研 `regex_lite_match` 只认三构式）。
- 验证：console:: 5/5；util::+stream:: 25/25；两目标套件 rc=0 复绿；
  冒烟 5/5；check-lines/naming ok。全局面（Rust Instant/`Timer…` 文案/余参丢弃）
  未动，另案（见定级稿 R-a~R-h 全局列）。commit 36b1310 已推 lane/console。

## 2026-10-20 console 全局面 warning 桥 + `_times` 可见面（lane/console，转绿）

- 修法（`src/builtins/node/require.rs` REQUIRE_PRELUDE，零 Rust 改动）：
  全局 `time/timeLog/timeEnd/count/countReset` 由 JS 本体接管（Rust 原生留作
  回落）：`__times/__counts` 双 Map（`console._times` 即本体，Map 代 SafeMap，
  套件只用 get/size）+ 入参 `${}` 归一（Symbol 抛 TypeError）+ has 守卫
  （dup 保留原计时）+ 8 警告逐字 `process.emitWarning`（Warning 无 code）+
  `process.hrtime()` 差值喂 `formatTime`（`node:internal/debuglog` 惰性 memo，
  无循环依赖）+ `timeLog` 余参走包装后 `console.log('%s: %s', …)` 续接。
  构造守卫内含；命名函数表达式正名（methods 套件点名 .name 不受影响）。
- 黑盒 `console_global_timer_count_warning_faces`（全局 stdout 断言输出形 +
  余参 + 计数 + dup 恒等 + 死链回落 + `_times` 可见 + Symbol×4 + 4 警告逐字；
  模块面 8 警告已由实例面全覆盖）。另补 `countReset(Symbol)` 抛 TypeError
  断言（node 警告模板内抛，真机同）。
- 验证：console:: 6/6；16 件域套件 12 绿——余 4 红皆既有面缺口，
  与计时语义无关：count（输出路由旁路 hijack，文本 `default: 1` 本身正确）、
  clear/diagnostics-channels（`isTTY` 只读）、stdio-setters（stdio setter
  mustCall）。冒烟 5/5；check-lines/naming ok。
- 未动/偏离：Rust 原生 time 面留作回落（输出通道/缩进仍原生）；长时
  `h:mm:ss` 全局面已随 formatTime 对齐；`_times` 为 Map 非 SafeMap
  （套件面等价，不 bound）。
## 2026-10-20 tls-close-error 硬失败中止（链错发 alert）

- 现象（sweep 新红）：`test-tls-close-error`（客户端无 ca 自签）服务端误派
  secureConnection（mustNotCall 炸）；客户端侧本就对（error UNABLE_TO_VERIFY +
  close 带错）。
- 根因：P2-tls-b 捕获式校验（CaptureVerifier）链错亦捕获放行，两端握手走完才由
  JS 裁决销毁——服务端已派 secureConnection；node（OpenSSL）链错握手期发 fatal
  alert，对端永不 secure（node 侧实测：server 仅 tlsClientError ECONNRESET）。
- 修法：`CaptureVerifier` 增 `abort` 旗（`capture_client_config(ca, reject)`；
  reject 真 + 非名失配哨兵即返 Err 中止，rustls 依例发 alert；哨兵恒捕获走 CSI；
  reject 假走 NoVerifier 不动）+ 回程 err 点亦带校验原文（授权位映射）+
  JS `__pump` 先冲 out（alert 上网）再销毁 + 中止错同步授权位（旧捕获路径等价）。
  客户端可见错经同一 `__handshakeErr` 映射逐字不变。
- 验证：本件转绿；探针双边对（CLI error/close 带错 authorized=false；
  SRV 无 secureConnection + tlsClientError；码面 ECONNRESET 系传输实现差，不断言）；
  黑盒 `tls_close_error_reports`（3/3 稳）绿；`tls_` 15/15、`x509` 6/6；
  附带回归：裸 socket 缺省改假后 `retain-handle-no-abort` 转绿（node 真机：
  构造期 `!!options.rejectUnauthorized` 恒假，见下）。
- 构造路径默认（真回归修补）：node `new TLSSocket()` 缺省不校验
  （`wrap.js:954`），仅 `exports.connect` 归一缺省真；旧码两路恒真致裸连误校验。
  修法：export 归一显式化 + 包裹面缺省看 `__viaConnectApi`（显式值恒优先）。
- 全量子集回归（最终二进制，tls+https 244 件 25s 盒逐件 + node 双边分类）：
  真红 97 / 双红 2（既有 internal 系）/ 对 base18 新红 0 / 转绿 45；
  黑盒 `tls_` 15/15（含新件）、Rust 单测 tls 10 + x509 9、builtins 41/41
  （中途 1 发 load 并发 flake，复跑双绿，4.140 口径记一笔）；冒烟 5/5；双守门绿。


## 2026-10-20 console 余4红修法（lane/console，16/16 满绿）

- 根因收敛：全局输出未走 JS 流面（Rust 直写 fd，无 `_stdout`；`isTTY` 只读；
  无 diagnostics 发布）——count 劫持/clear/stdio-setters/diagnostics 四件同源。
- 修法（prelude JS，零 Rust）：`console._stdout/_stderr` 可写（缺省跟安装期
  进程流引用）+ 未改道走原生 raw（零行为变化，保 REPL CRLF）/改道走流写
  （同步错吞，唯真栈溢出穿透，探针与模块面同式）+ log/info/debug/warn/error
  分体（各发自家 channel；methods 套件容忍别名，diagnostics 要求分体；
  dirxml=groupCollapsed 保持同体）+ clear（TTY+非dumb 写转义）+
  stdio `isTTY` getter+setter（读 live、写覆盖；套件赋值不再抛）。
- 黑盒 `console_global_routing_faces`（hijack 捕获/_stdout 定制/channel 原参/
  clear 双态/写错吞/恢复原生；clear 期望随 TERM 分支，dumb 即静默真机同，
  已真机实证）。
- 验证：16 件域套件 16/16（base18 5/16；L-E 后已转绿一批，本轮收余 4）；
  console:: 7/7；util::+stream:: 25/25；events::/diagnostics_channel::/repl::/
  builtins::/tty::/readline:: 全绿；两目标套件 rc=0 复绿；
  冒烟 5/5；check-lines/naming ok。
- 守门触发拆分：`require.rs` 被顶到 1061 行 → `REQUIRE_PRELUDE` 纯字符串整体
  搬入 `require_prelude.rs`（原位 `pub use` 重导出；require.rs 784/新文件 282，
  纯搬移先行一提交）。
## 2026-10-20 base19b 纯净重跑（干净树，SAME0 2470/3528，open-fix 验证）

- 运行：pull 已最新（f1f1f26）+ status 空 + load ~12 → `cargo build -j2`
  2m39s（含等他 lane 包锁）二进制 21:31（md5 788b9ff6db4a519f884823d4a7bab91c；
  跑后 md5 复核一致）→ 冒烟 5/5 → sweep 同 base19 参数
  （--tag base19b，jobs 2，thread 3599，port 29999，scope 缺省，node 缓存），
  21:31:56 → 22:16:33（~45min，缓存热故快），3528/3528 无 abort。
  工件 `~/wjs-data/sweep/base19b/`。系统盘 49→38GB（他 lane 活动，看门狗安好）。
- 总表：SAME0 **2470** / SAME1 207 / DIFF 820 / TIMEOUT 31（绿 70.0%）。
  vs base19（2397/206/841/84）：绿 **+73**、DIFF -21、TIMEOUT **-53**；
  真实欠账 925→**851（-74）**。逐件：fixed 75（fs 37/http2 13/stream 5/
  zlib 4/net 3/tls 3/http 2/https 2/readline 2/console/pipe/stdio/timers 各1），
  newred 仅 2（keygen-async-dsa TIMEOUT=既有慢 DSA flake，journal:811；
  http2-multiplex DIFF=复探 rc=0，flake），TIMEOUT→DIFF 46 翻转。
- 重点① fs：DIFF 75→**38**（-37 = open 三宗罪全愈，数与 base19 新红 fs 37
  严丝合缝；绿 246→**283**，87%，回到 base18 上方）。余 38 皆真欠账
  （promises-file-handle 族/realpath/readdir/glob/cp/mkdtemp…），待 C 后续。
- 重点② HANG 自报：results.log **34 HANG + 107 MUSTCALL 定位行**
  （base19 为 0）——f1f1f26 变量对齐生产验证通过；TIMEOUT 件转为带定位
  DIFF（46 件），http2 TIMEOUT 25→**2**（仅 client-http1-server +
  reset-flood 留守，皆帧错映射 bound 面）。
- 重点③ P0 两回归仍挂（定位与 9 遍复判同址）：socket-destroy-delayed
  DIFF/HANG@test:31、goaway-opaquedata DIFF/HANG@test:20——C 轮继续；
  proxy-over-h2 / tunnelling / socket-close / pipe-offset **转绿**。
  （后两者 base19 挂疑 load-301 抖动或级联，base19b load ~10 即绿。）
- 域级 Δ（绿/TIMEOUT）：http2 122→134（T25→2）、tls 93→96（T14→1）、
  net 95→98（T4→0）、http 325→327、fs 246→283、https 46→48、
  stream 169→174、zlib 46→50、readline 10→12、timers 42→43、
  console 15→16、stdio 1→2、pipe 6→7；crypto 63→62（-1，dsa flake）。
- Top10（base19b 差额）：http2-109、tls-89、crypto-41、net-39、http-39、
  fs-39、worker-34、child-29、vm-28、webcrypto-22。
- base19b 为干净基线候选（§0.3 换表待用户确认；本任务只记账不改表）。
## 2026-10-20 child_process 29件欠账定级（零构建探针，node真机全对照）

- 范围：base19b child 红 29 件（DIFF 26 + TIMEOUT 3 中的 spawn-typeerror…
  实为 node 侧抖，见下）。二进制沿用 base19b 构建（md5 788b9ff6 未变，
  零新构建）；node v26.8.1 逐件对照（TEST_THREAD_ID 分槽 + perl alarm，
  9 批）；Bun：child_process ✅/🟡IPC 缺口（bun-compat:66）+
  parity handle 传递出局 + fork/IPC 另案（bun-parity:1052/1061）。
- node 侧：27 件一次过；2 件抖动——send-keep-open（本次 node 亦 mustCall
  未达 :46）/ spawn-typeerror（sweep 时 node=142，本次 node 绿）→ 口径差，
  非我方欠账。exec-maxbuf 本次 wjs 复探 rc=0 → flake。
- 定级（默认对齐 node+Bun，自主定级）：**修 17 / 另案 9 / 口径差 3**。

### 修-small（校验/事件语义，P0 体量，8 件）

- advanced-serialization（首半）：spawn `options.serialization` 非法值未抛
  ERR_INVALID_ARG_VALUE（Missing expected exception；node 首段即断死）。
- fork-timeout-kill-signal（:28）：`timeout:'badValue'/{}` 未抛 ARG_TYPE。
- fork-stdio-string-variant（:16）：`stdio:'33'` 未抛 ARG_VALUE。
- kill（:75）：stderr 'end' 发两次（actual 2；去重）。
- spawnsync-non-string-args（:5）：args `toString` 调两次（validation 与
  落盘各一次；只 stringify 一次）。
- ipc（:47）：stdout 两 chunk 并成一次达（分块面；node 逐 write 分发）。
- fork-close（:35）：close 早于 message/exit（`!gotClose` 翻转；收尾排序）。
- fork-ref（:57）：disconnect 后 stdout '3' 丢失（silent 子 disconnect
  处理）。

### 修-medium（机械面，9 件）

- internal 真路径 6 件（4.291 同构：`node:internal/child_process` 别名暴露
  **同一份** live 对象）：bad-stdio / exec-kill-throws（mock
  `ChildProcess.prototype.kill`）/ spawnsync-kill-signal（重赋值
  `spawnSync`）/ validate-stdio（`getValidStdio`）/ windows-hide
  （mock `prototype.spawn`）/ spawnsync-shell（同上 + shell 断言/DEP0190）。
- parent-disconnect 2 件（同根）：ref2（:43）/ fork-url.mjs（:9）——子
  `process.disconnect()` 后父 `disconnect` 事件不到。
- detached 1 件：孙进程 detached 常驻语义（hang，child=1；unref/ detached 面）。

### 另案 9（深水/Bun 同缺，书面记档不碰）

- spawn-ipc 通道 2：stdin-ipc / stdout-ipc（spawn `stdio:'ipc'` 报
  NotSupportedError；真进程 socketpair 通道另案，Bun 🟡同缺 bun-compat:66）。
- 高级序列化往返：largebuffer / splitted-length-field（IPC 通道 stub
  `ERR_NOT_SUPPORTED`；随 spawn-ipc 另案）+ advanced-serialization 后半
  （validation 修后，roundtrip 归此案）。
- handle 传递 2：fork-net-server / fork-net-socket
  （`ERR_INVALID_HANDLE_TYPE`；parity:1052 出局面，Bun 同缺）。
- ipc-next-tick：message 处理抛错→uncaughtException×10→disconnect
  （hang 且零句柄；IPC 分发深水，parity:1061 fork/IPC 另案同族）。
- closed-channel-segfault：`cluster.Worker.process` 不存在（cluster 底座，
  Bun cluster 🟡同缺）。
- fork-exec-path：`execPath` 换二进制 fork（线程底座做不到真换 binary；
  记档，或 fork 遇 execPath 显式偏离）。

### 口径差 3（非欠账）

- send-keep-open（node 本次亦红）/ spawn-typeerror（sweep node=142，
  本次 node 绿）/ exec-maxbuf（本次 wjs 绿）→ flake 队尾，复判另轮。
- 校验：17+9+3=29 ✓。P0 开码另行确认分支（本任务只定级，未动代码）。

## 2026-10-20 net42 零构建探针+定级（P0/P1 分道）

> 基线 base19b net 43 = DIFF 42 + SAME1 1（internal 系双红，非欠账）。
> Bun：net 🟢（`bun-compat.md:72` 回环+allowHalfOpen/destroy 全对；`bun-parity.md:781` net 节），
> 42 件无 Bun 同缺，皆真欠账。node 对照：base19b DIFF 即 node 绿 + 本段逐件 wjs 实跑取断言差。
> 范围 `bun-scope.txt` net 141 + socket 5 行。

### P0 报文/校验形（13 件，小步直修）

- address-null ×5（immediate-destroy/reset-before-connected/destroy-twice/
  reset-twice/timeout-unref）：`server.address()` 关后即 null，真机关完前仍有效
  （`connect-immediate-destroy.js:7` 取 `.port` 炸）。修到：close 事件前 address 有效。
- better-error ×2（listen-path/listen）：listen 失败错缺 `address`（`undefined !==
  '/blah/fadfa'/'1.1.1.1'`）。修到：错体带 address/port。
- validation ×4（socket-address/socket-options-invalid 缺抛 + custom-lookup
  错码 `FAMILY≠IP_ADDRESS` + autoselect-attempt-timeout-cli-option `2500≠615`
  缺省值）：修到文案/码/缺省逐字。
- legacy ×2（socket-connecting `_connecting` undefined≠true + no-halfopen-enforcer
  listenerCount 0≠1）：修到：`_connecting` 品牌透传 + 非半开 end 门装配。

### P1 时序/竞速/深水（23 件，按簇开轮）

- reset/destroy/nodelay 事件序 ×12（connect-destroy/nodelay/paused-connection/
  reset-after-destroy/reset-until-connected/reset/dns-error/dns-lookup/
  server-nodelay/server-reset/end-callback/setnodelay）：MUSTCALL 未达，多为
  destroy 竞速 + nodelay 回调 + DNS 错路由。逐件 run1 复判再修。
- shutdown 建模 ×3（write-after-fin `er undefined` + connect-destroy MUSTCALL +
  write-cb-on-destroy-before-connect）：P2 结转 net=5 收尾建模（FIN 后写错面）。
- timeout/水位 ×3（socket-timeout `100` + buffersize `9≠0` + stream 混接）：
  超时门 + bufferSize 记账 + stream 互操作。
- dns-custom-lookup HANG + perf_hooks + listen-handle-cluster HANG：lookup 透传/
  perf 钩 + cluster 句柄（跨线程面，随 cluster 另案）。

### 另案/范围外（6 件，不进 P0/P1）

- normalize-args（`internal/net` 缺失）→ 范围外（internal 树既定）。
- child-process-connect-reset（ipc spawn）/ server-close-before-ipc（HANG）→ infra 另案。
- server-async-dispose（`Symbol.asyncDispose` 新 API）→ 另轮（显式 using 面）。
- server-call-listen-multiple-times（本段空跑待编码期复判）/ server-capture-rejection
  （kaboom 透传面，`__ev` 抛错到 uncaughtException 投递，http2 early-hints 同族）→
  编码期首件复判定去留。
- allow-half-open（本段空跑，MUSTCALL:22 未现）→ 编码期复判。

## 2026-10-20 net P0 13件落地（报文/校验形）

- address-null ×5：真机 listen 无 host 即同步 bind（显式 host 异步，同步 address
  即 null）→ native 第 5 参同步路（std bind + rval `id:port`，失败仍异步
  ServerError/Close 不抛；UDS 面既有同款），JS 仅 host===null 置旗并同步落
  `__listening`（地址沿用 0.0.0.0，不跟真机 `::`，保既有绿）。
- better-error ×2：`__bindHost` 跟踪 + 错体补 `address` + 失败清 stale。
- validation ×4：getsockname 咨询（node `address EPERM` 形）/ 构造器 objectMode
  三键（lib/net.js:625 口径，先于 fd）/ lookup 校验序（先 IP 整数组、次 family；
  all:true 对象数组走 HE 旧路）/ 缺省读旗（attemptTimeout 500/`=v` 覆盖 +
  autoSelect 三态直读剥存，存根分不出缺席）。
- legacy ×2：`_connecting` 透传 + 自门常驻（构造期首挂，用户监听后触发）。
- 监听数连锁（node 真机计数：fresh 1/active 2/池态 1/detach 1）：agent 门存根化
  （回池/响应收齐/隧道摘除同步摘，出池重挂）+ req 面 end 递延 socket 事件后一拍
  （I/O 不穿插 microtask）+ 服务端 abort 门劫持摘除。附带修好 keepalive/cli-sock/
  http_timeout_deep（srv 1）三计数。
- localPort 透传（client-bind-twice）：native 第 6 参 + resolved 签名扩展（tls
  调用点补 0），占用即 EADDRINUSE。
- 验证：P0 13/13（含双旗形）+ 双黑盒新件绿；`net_` 19/19、`http_` 35/35、
  `tls_` 15/15、`https_` 12/12；custom-lookup 单发抖动复跑 5/5（load 并发 flake
  记一笔）；冒烟 5/5；双守门绿；`split-js` 未触发（tls.js 960/net 全员 <1000）。

## 2026-10-20 vm merge-note #2/#3 落地（lane/vm：importModuleDynamically + unhandledRejection 投递）

- 范围：R19 L-D linker 轮遗留两 merge-note。base18 vm 46/95 → 本轮 sweep（vm-dyn3 tag）SAME0 75/95 DIFF 20/TIMEOUT 0（+29 件；基本/错误/动态-import×2/命名空间/import-meta/no-callback 7 件全转绿）。
- #2 importModuleDynamically（`src/builtins/node/vm_dyn.rs` 新文件 + `modules.rs` 双 hook 分支 + `vm.js`/`vm_modules.js` 接线 + `state/vm.rs` 注册表）：
  三路 referrer（模块记录/compileFunction 结果/Script 帧栈）→ `Reflect.apply` 调用户回调（this undefined；spec/ref/空原型 attrs/phase 串真机序）→ 同步分类（模块记录→link+evaluate；命名空间/synthetic→`ResolvePromise` 直解；非法→`ERR_VM_MODULE_NOT_MODULE`）/async 回调→开票+JS 帮忙落定。纯 Resolve 方案（不调 `FinishLoadingImportedModule`，免 referrer 脚本跨轮定址）。
  引擎墙记档：`with` 非 `type` 键编译期剥离（SM153 只认 `type`，V8 不透明；attrs 仍从原文取）；async 回调返 thenable（非原生 promise）按非法处理；file 主 loader 静态依赖走原路失败。
- #2 initializeImportMeta（`metadata_hook` vm 记录分支）：无回调即空 meta（真机口径）；有回调 `Reflect.apply(undefined, [meta, module])`，返回值忽略，抛错即失败。
- #3 unhandledRejection（`src/runtime/unhandled.rs` 新文件；旧捕获 natives 删除）：`report_unhandled_rejections` 改直读结算值；有监听即 emit（reason, promise）后清表续跑；无监听按 `--unhandled-rejections`（throw/strict fatal 旧文案；warn 警告+bounded 排空继续；none 静默；warn-with-error-code 警告+exit 1）。事件循环检查点改先投递后定去留（监听/告警/静默不清表即续跑，致命才跳出）。
  缺口（promise 域另案）：warn 双警告/rejectionHandled 事件/node 式 fatal 形状。
- 过程根修（三处阻塞件的真根因，非 merge-note 字面）：
  1. punycode vendored 块级 function 经 Annex B 漏全局（`error` 非配置，致 errors 件 `delete` 崩）→ IIFE 包裹（先行单提交 9c2db89，已合 master）。
  2. 超时孤儿排空原地 rearm 全杀兄弟 job（test1 配 timeout 块永挂；基线 stash 同现，潜伏）：摘表项+1000ms 限时武装排空（4.300）。
  3. 模块求值缺 sandbox 同步（test1 deepStrictEqual 红；基线同红，journal 原"各剩一件"误记）：`__doEvaluate` 配 syncIn/syncOut。
  4. 跨域恒等分裂（`ns === namespace`/`e === __err` false）：`wrap_value` 统一包装（mod_ns/vm_set/vm_get/完成值非 promise；global 免包装）（4.301/4.302）。
- 过程坑（4.293-4.299 引擎墙系列 + 4.303-4.310）：`GetModuleObject`/`GetModuleForNamespace` 无检查 cast（Script 路 SEGV/误判）→编译期脚本定址比对；`ScriptSlot` 求值后清零（4.305）；hook 行恒 0（4.306，改 spec 匹配）；`Rooted<Vec>` as_ptr 误用（4.303，listenerCount 恒 0）；`RootedGuard<Vec>` 同类（call_method 已改 RootedVec）。
- 验证：7 目标套件全绿（exit 0）；timeout 逃逸三件绿；新黑盒 `tests/node/vm_dyn.rs` 4 件绿（动态导入面/import-meta 面/投递面/守卫面）；既有 vm 黑盒 18/18；`builtins::unhandled_rejection_is_fatal` 绿；冒烟 5/5；`check-lines`/`check-naming` 双绿；sweep vm 75/95（余 20=预存深水 +3 回归核销见下）。
- 回归核销：`function-declaration/getters/global-define-property` 三件 base18 绿→本轮初红（rval/同步包装漏 global 豁免），已修绿；`basic` sweep DIFF 系并行负载抖动（单跑/单件 sweep 绿，见 vm-basic1 tag）；`set_immediate_not_clamped` 计时抖动系已知负载 flake（journal 2026-09-28 有案，单跑 2/3 过）。
- 余 20 DIFF 定性：synthetic 作依赖（引擎墙，记档）；referrer-realm（主 loader 静态回落，linker 另案）；after-evaluate×2/hasasyncgraph/hastoplevelawait（linker 求值语义另案）；其余 12（context-async/global 系/proxies/symbols/sigint×2/cached-data/low-stack 等）皆 base18 预存红（非本轮引入，见 comm 差集：仅上三件新增且已修）。
- 构建：lane 独立 worktree（`lane/vm`，基线 707a929）+ worktree-local target；`LIBCLANG_PATH=/Library/Developer/CommandLineTools/usr/lib`（brew llvm 缺失，主仓 AGENTS 路径此机不可用）；错峰 -j2（load 386 时停编，降后复工，已报 parent）。
## P0 真回归2件（lane/http2）：destroy-delayed 修绿 + proxy-over-h2 另案

- 基线：node 双绿；base18 双绿（proxy 系 DIFF）；base19 双 TIMEOUT；复判挂。
- destroy-delayed ✅ 转绿（commit 862eaca，已合入 master）：根因两件——
  ① Close 常先于 H2RespondEnd 进队（本仓 Duplex `_final` 递延到 setImmediate 之后），
  旧到 Close 即 break 丢尾包致零字节 → 改优雅模式（GOAWAY 后继续收应答命令至
  conn 结束或 5s deadline）；② 客户端 conn 收尾与流任务唤醒竞态（GOTRESP 在
  CLI-DONE 之后，purge 丢事件）→ conn 结束处等 JoinHandle 落定（2s 上限）。
  黑盒 `http2_socket_destroy_delayed_flush` + `http2_session_close_flush`。
  回归面：close-idle/flushheaders/session-destroy/propagate/finished/socket 系绿；
  goaway-cleanup 系 base18/19 双红（同错，非回归）。
- proxy-over-h2 ⏭️ 另案（hyper CONNECT-upgrade 接管，见 4.313）：
  线字节铁证——握手外隧道方向零 DATA 帧；外层 200 后 hyper 自发空 DATA-EOS +
  RST(0)；常规 Incoming 恒空、body 通道收发皆死，应用须经 `hyper::upgrade::on`
  泵接。node（nghttp2）无此接管故绿。修法设计：服务端读泵 + 写改道 + 客户端镜像
  + 生命周期解耦（约 200 行），超 C 轮 time-box，下轮专项。连带：connect-method
  同根因 DIFF（base19 同错，非回归）；proxy 套件行为 TIMEOUT→EXIT=1 系 dial
  example.com 环境所致（隧道未通前即 protocol 错）。
- 过程：单流定序（eprintln）+ 方向归属（evid）+ 帧解码三板斧；`println!` 块缓冲
  坑（4.314）；`git checkout` 回退 proxy 半截活保 lane 可合。

## 2026-10-20 vm linker 余件（lane/vm：referrer-realm/hasasyncgraph/hastoplevelawait 修 + after-evaluate 偏离）

- 范围：R19 L-D linker 遗留，parent 定件 4 件（referrer-realm/after-evaluate×2/hasasyncgraph/hastoplevelawait；synthetic 作依赖维持引擎墙偏离）。
- referrer-realm（真机：间接 eval 无 JS 栈 → referrent null → 走上下文回调）：
  createContext 接 `importModuleDynamically`（函数/undefined/符号三态校验，node 口径 early-return/validate 顺序对齐）+ `__wjs2_vm_dyn_reg_ctx` 注册上下文条目；
  hook 兜底：referrer 未命中三路即按 hook 入口 realm 归属 vm 上下文用其回调（referrer 即 sandbox 本体，`wrap === ctx` 恒等）。
  过程坑：①hook 入口先 AutoRealm(主) 会覆盖 incumbent——入口第一行先抓（4.315）；
  ②main 模式免推帧致 incumbent 劫持 Script 自身符号指令→三路一律推帧（携直通旗，Script 优先于上下文）；
  ③跨域记录用 referrer realm 做 link/evaluate 即 SM 断言崩→记录操作进记录所属 compartment（4.316）；
  ④帮忙函数按票据 realm 查找 miss→一律进主 global 查（4.317）。
  套件全绿（含 testMissing CALLBACK_MISSING 双路 + createContext 非法值校验）。
- hasTopLevelAwait/hasAsyncGraph：oxc 静态探测（`visit_function` 不覆盖箭头→补 `visit_arrow_function_expression`，4.318）+ 构造期 `__tla` + link/instantiate 落定 `__tlaClosure`（连带依赖同循环结算，4.320；linkRequests 未实例化抛 STATUS 文案逐字）。
- after-evaluate×2：书面偏离（4.319）——V8 按上下文分微任务队列，我方单共享队列恒排空，无"某上下文 checkpoint"概念；`createContext` 仍校验 `microtaskMode` 口径。
- 验证：4 件中 3 转绿（referrer/hasasyncgraph/hastoplevelawait）；黑盒新 2 件（`vm_referrer_ctx_faces`/`vm_tla_faces`）+ 既有 24/24 绿；7 件旧套件 + 超时三件 + identity 三件全绿；冒烟 5/5；check-lines/naming 双绿；sweep vm-linker2：78/95（+1 dont-contextify；余 17=after-evaluate×2 偏离+synthetic 引擎墙+basic 并行抖动+13 预存红）。
## proxy-over-h2 专项（lane/http2）：隧道架构落地转绿

- 承接 P0 另案设计：服务端读 upgrade 泵 + 写改道（outbox）+ 客户端镜像 + 收尾解耦。
- 根因终定（4.321）：service 析构 req 连带丢 upgrade 句柄 → close_notify → up_task
  发空 EOS+RST 拆隧道；隧道字节双向困 hyper 内部，无 API 可取，只能泵接句柄。
- 落地：serve_conn 取句柄（CONNECT）+ H2Respond-2xx 建链（泵+outbox+升级旗）+
  Data/End 写改道 + 抑制常规空泵 + CONNECT 恒报 streaming；客户端镜像
  （resp 取句柄+泵+写改道+跳常规体循环）+ AdoptIo 共用桥（client_adopt 三件）+
  JS 双向桥（createConnection/stream-emit）+ CONNECT 收尾横扫。
- 证据：proxy 套件 TIMEOUT→EXIT=0（收尾序与真机一致）；px11 隧道字节通；
  connect-method 仍红（EOS-before-200 hyper 不升级，4.322，base19 同错）；
  session-destroy-stream-handler TIMEOUT 系 5s/轮旧码同形慢（node 在此件 abort，
  从未进 sweep）；server-stream-session-destroy 系 SAME1。
- 黑盒三件：tunnel_basic/tunnel_factory_error/tunnel_outer_destroy_cascade 全绿。
## 2026-10-21 net P1 落地（事件序/reset/RST/lookup/EPIPE/finish 全家）

- 事件序×12 全绿：reset 空闲 ERR_SOCKET_CLOSED（lib/net.js:1112 口径）/
  destroy 未连接自发 close（单发守卫）/ nodelay 去重透传（kSetNoDelay 初 false，
  变值透传；connect 期异步调柄供 mock；server 侧 accept 透传+预置）/
  plain-handle 接管（kHandle 口径）/ lookup 事件（系统成功 peer 透传 + 失败
  DNS 码先发 + 自定义恒发，发射异步防同步 mock 漏接；IP 字面不发，真机实证）/
  end-cb（unref listening 经 microtask 送达）/ paused 预连摘续命（resume/data
  装回，G11 半开让路）。
- RST 真语义（reset-after/until/server/stream 五件）：`__wjs2_net_reset` 经
  SO_LINGER(0)+SHUT_RD 唤醒读端，常规 teardown 关 fd 即 RST（raw close 有 fd
  复用竞速致 segfault，已删）；读端静默只走 Close；服务端读错形 read ECONNRESET；
  未读销毁（paused 积压）亦 RST；fd/静默位走 OnceLock 静态（ports 同款，
  purge 即清）+ 单测 `net_rst_ledger`。
- finish/EPIPE/校验：auto-end 走完整语义（无显式 end 亦 finish，单发守卫；
  标志递延保 'end' 内 writable 真）；EPIPE 限 peerFin+已 end+非半开；
  write 序 null→chunk→destroyed→EPIPE→writable→句柄（echo 旧偏离翻转）；
  setTimeout 全校验（无参亦抛，真机实证，黑盒同步翻转）+ write-cb
  （pre-connect 销毁未刷回调 SOCKET_CLOSED_BEFORE_CONNECTION，文案逐字）/
  captureRejections（Server 符号钩，connection 拒即 destroy 递错）/
  perf net 条目桥（公开面零新增，TCP 成功记，pipe 不记）/
  写失败 surfacing（写 task 错即 Error+Close 收尾）。
- 运行时序观测（记档，未动引擎）：I/O 回调内 queueMicrotask 晚于
  setImmediate(0)（ord 探针，node 相反）→ 自门改 setImmediate 保序；
  嵌套调度下同拍 nextTick 不先于 microtask → 未连接自发 close 改双拍
  nextTick（https bad-lookup 保 error 先行）。
- 定级 P2（另案）：listen-handle-cluster ×2（worker 传柄 + raw TCP 句柄
  listen，需 cluster IPC 柄/句柄 fd 底座）；dns-custom/lookup 余 HANG 面已清。
- 验证：P1 23 件全绿（事件序 12 + shutdown/timeout/nodelay/dns/perf/cluster
  已修面；双旗复验；until-connected/server-reset 串行抖动各一笔，隔离 3/3、
  2/2）；黑盒 +3（reset/lookup/epipe）与旧件两处按真机翻转（st-noarg 抛、
  reset 对端 ECONNRESET）；`net_` 22/22、`http_` 35/35、`https_` 12/12、
  `tls_` 15/15；附带修 https_connect_lookup_family（回归）；冒烟 5/5；
  双守门绿（含 `split-js` 切 net_socket_ev.js 280 行 + OnceLock 退 struct 字段）。

## 2026-10-21 net P2 设计（listen-handle-cluster ×2，IPC 柄底座）

- 套件语义（真机实测）：① cluster-1：primary `worker.send(null, server)`
  传已监听 Server，worker 重建 `instanceof net.Server` 即 `process.exit(0)`；
  primary 见 exit 关 server 即退。node 内幕（child_process.js）：按型注册
  `send/got`——'net.Server' 发原始 `_handle`（fd 过 IPC），收端
  `new Server()` + `listen(handle)` 后交付。② cluster-2：
  `internalBinding('tcp_wrap')` 裸 `TCP.bind('0.0.0.0', 0)` 回 0，
  `listen(handle)` 即听后退出（`--expose-internals` 本仓已放行）。
- 关键判定（同线程 worker_threads 共享 fd 表，无需 SCM_RIGHTS）：
  复用 holdToken（`net_hold_put/take` 预留底座）+ cluster internalMessage
  信封（有柄才包、无柄线形不变），只新增最小桥，不另起传输。
- 方案：
  1. cluster-1 发端：`worker.send` 认 Server（`__wjs2_netServer` 旗），新
     native `__wjs2_net_dup_listen(serverId)` dup 监听 fd（同 socket 双
     accept，内核分发；原 fd 不动），信封 `{__wjs2_msg, __wjs2_handle:
     {type:'net.Server', fd}}` 经既有 postMessage。
  2. 收端：`globalThis.__wjs2_handleRevivers` 注册表（net 侧注册，无硬依赖），
     重建 `new Server()` + `listen({fd})` 有效形，listening 后交付
     `process "message"` 第二参（node `got` 同序）。
  3. fd 认领：`listen({fd})` 有效 fd 走新 `net_listen` `"FD:<n>"` 标记
    （"UDS:" 同款约定；SO_TYPE==SOCK_STREAM 校验，无效沿旧 EINVAL 路——
     fd0/ebadf 套件不动）；`from_raw_fd` 单一归属（发端 dup 出、收端一次
     消费，UNSAFE-BOUNDARY 注记）。
  4. server fd 记账：TCP listen-bind 成功点（同步/异步双址）记 id→fd
    （OnceLock 静态，purge/close 即清，RST 面同款）。
  5. cluster-2 裸柄：tcp_wrap `TCP.bind` 真绑 + `net_hold_put` 预留，柄整
     BoundSocket 形（address 函 + `__boundPort/Host/Token`），`listen(handle)`
     走既有 adopt，唯 token 命中改 take-direct（新 `"HELD:<token>"` 标记，
     免关绑竞速；BoundSocket 旧路一字不动）。
  6. primary 收尾：既有 fork/exit/drain 机制，复验。
- 不做：`net.Socket` 句柄传递（`_setupWorker`/`_connections` 记账大件，无套件
  点名）；BoundSocket transfer 协议；spawn stdio 传柄（fd-server 另案，见下）。
- 另案记一笔：test-listen-fd-server.js 现红（spawn stdio>3 项拒），P0/P1 未碰
  spawn，base19b 未收录（疑基线口径/环境），P2 不碰，终验复判。

## 2026-10-21 net P2 落地（listen-handle-cluster ×2）

- 按设计实现：`__wjs2_net_dup_listen`（dup 传柄）+ `__wjs2_net_tcp_wrap_bind`
  （真绑预留）+ `net_listen` `"FD:"/"HELD:"` 标记（"UDS:" 同款；SO_TYPE 门，
  fd0/ebadf 沿旧错）+ server fd 记账（同步预绑即记，purge/close 即清）+
  `listen({fd})` 有效认领 + BoundSocket 形裸柄 take-direct（`__takeHeld` 门，
  旧路不动）+ `worker.send` 信封（有柄才包）+ 收端 reviver 注册表
  （listening 后交付第二参）+ tcp_wrap 通配真绑（环回存根面不动）。
- 排障两则：① 同步 listening 微任务先于任务记账 → 预绑点同步记 fd；
  ② 插入 HELD 分支时重复了 `once/listening+__setupHandle`（bound 双响）→ 去重。
- 验证：cluster-1/2 双绿；黑盒 +2（handle 传送/tcp_wrap 真绑）；`net_` 22/22、
  `http_` 35/35、`https_` 12/12、`tls_` 15/15、`cluster_` 6/6；fd0/ebadf/
  boundsocket/call-listen/localaddress 皆绿；冒烟 5/5；双守门绿。
- 未覆盖面（设计内不做，实证从未绿）：boundsocket-transfer-worker
  （worker transfer 校验拒，无基线收录，注释本就标另案）；fd-server
  （spawn stdio，见设计另案）。




## 2026-10-21 crypto 41 件探针定级（base19b 口径，lane/console；只探针不定码）

> 范围：base19b 非 SAME1 的 test-crypto* 41 件（40 DIFF/TIMEOUT + webcrypto-aes 一件）。
> 真机 node 26.8.1 逐件实测：37 绿、3 自红（aes-wrap/getcipherinfo/hmac）、
> x509 真机 SIGSEGV（-11，node 自崩，语义取 lib 原文）。
> 我方（二进制=bcce48e）：5 绿（上 3 + keygen-async-dsa×2 本地快过，sweep 侧 TIMEOUT 系环境抖动，标观察），36 红。
> 铁律先行：4.43（lock：digest 0.10.7+0.11.3 双轨、sha2 0.10.9+0.11.0、rsa 0.9.10、hmac 0.13；argon2 0.6/ml-kem 0.3/ml-dsa 0.1.1 均已入库，用户已点头——PQC/argon2 走接线不算新依赖；仅 slh-dsa 与 p224/p192 缺件）、4.54（试解看 OID，交叉必双向）、4.135（手组密码学双向真机交叉，自交绿不算数）。
>
> 定级（修/偏离/维持）：
> - 维持（node 自红，我方按套件绿，不跟）：aes-wrap、getcipherinfo、hmac。
> - 观察（本地绿、sweep 抖）：keygen-async-dsa×2（慢 keygen，base19c 复验）。
> - 修小（校验码/文案/小面，预计一批）：oneshot-hash（outputEncoding 码）、pbkdf2/prime（长度码）、keygen/random（invalidArgTypeHelper 文案）、dh.js（encoding buffer）、dh-curves（verifyError 形）、webcrypto-aes-tag（OperationError 名）、ecdh-setpublickey-deprecation（DEP0031）、sign-verify（library getter 读取序）、sec-level（internal/crypto/util 常量面）。
> - 修中（需动手 + 双向交叉钉向量）：dh-generate-keys（重生成语义）、dh-padding（值）、dh-classic/keygen-dh（classic DH）、ecdh-convert-key（缺 API）、eddsa-variants（向量+导入；先对 RFC8032 逐字节）、psychic-signatures（长度门，4.55 姊妹）、keygen-bit-length（DSA-1024，node 实测支持）、certificate（verifySpkac）、x509（internal/crypto/x509 类面；x509 底座已有）、keyobject-clone-transfer（clone 语义待查）、publicDecrypt（PBES2 密码名映射）、crypto-stream 等 4 件 pipe（Hash/Cipher 接 Stream）、domain（BAM 透传）、subtle-cross-realm（跨域 brand 按 isView + assert.partialDeepStrictEqual 补件）、argon2（binding 接线，crate 已有）、pqc 三件（ml-dsa/ml-kem crate 已有；仅 slh 除外）、keygen-raw（除 slh 部分）、worker-thread c-4（先查 c-4 缺口大小再定批）。
> - 偏离（不动）：slh-dsa 两件（需新 crate，§0.5 先问）、key-objects:1154（非 NIST 曲线能力，4.135 坑七已记，需 p224/p192 轮子）。
> - 备注：41 件全在 Bun 范围内；Bun 直跑仍被 common global-leak 守卫挡（沿用 readline 轮结论）。

## C P1 收尾（lane/http2）：复探 + tls-face×3 + autoselect + goaway 留守

- 复探（open-fix 合入后）：respond-file range/with-pipe/error-pipe-offset 三绿；
  socket-close 绿；compat-aborted 绿。close-aborted-destroy 剩 rstCode 一处
  （真机 hang，4.325 剃刀竞态定级，error+close 确定性绿）。
- tls-face×3 全绿：secure_post_echo/secure_reject（错 CA 即 error+close，码面顺延
  tls lane）/secure_big_body（256KB）；既有 secure 面同绿。
- autoselect ✅：autoselect-protocol TIMEOUT→EXIT=0（net socket 读嗅探三件，
  4.323）。multiple-client-error 推进到 http 层 clientError 缺口（http lane 事，
  传输已通）。buffersize 系写背压问题（本轮外）。
- goaway 留守：session-cleanup 3/4 转绿（sessionError 事件，4.324），仅剩 raw-close
 （双 FIN 后无 close；gw8d 同构通而套件不通，时序敏感，另案）。
- 黑盒：tunnel 外新添 goaway_frame_size_error + read_unshift_readable，全绿。
## 2026-10-21 Event 体系缺口分析（repl brandcheck 转单）

- 范围：`internal/event_target` 引用 12 件 + 全局 Event 面。真机逐项对照后定级。
- 已修转绿（小，顺手落地）：global-customevent（全局===内部同一性透传）、
  events-static-geteventlisteners（直接注册侧读桥）、events-customevent
  （构造校验/tag/inspect）；messageevent-brandcheck 保持绿。黑盒 +1
  （events_web_event_faces：正常/报错/边界）。
- 修面（prelude/events.rs + internal/event_target.rs + events.rs 桥）：
  只读面（composedPath 派发期 [target]、returnValue、eventPhase 派发期 2、
  cancelBubble 读写、initEvent、阶段静态、toStringTag 全家）、构造校验
  （options 对象形逐字、type 缺失/Symbol 形、子类无参）、派发语义（异常
  不中断后续→uncaughtException 无 error/unhandledRejection、重派发
  ERR_EVENT_RECURSION、非 target 调 ERR_INVALID_THIS）、元数校验、监听校验、
  Symbol type、defineEventHandler/makeEventHandler、null 监听 Warning、
  ET max 警告 + etSetMax 桥、listenerCount/getEventListeners 直读。
- 定级大（记档不做）：eventtarget-brandcheck 余（全员 ERR_INVALID_THIS
  品牌检查约 50 点 + NodeEventTarget 混入类）、eventtarget.js 余（弱监听
  存储/GC 语义 + 63954 保持）、nodeeventtarget/on-async-iterator
  （NodeEventTarget）、memoryleakwarning（MessagePort/AbortSignal 警告面；
  另该套件现 node 自红，MessagePort 形已漂移）、abortcontroller-internal
  （弱监听 + gc，大）、timers-promisified ×3（promisify 同一性，非 Event）、
  bootstrap-modules（process.moduleLoadList，非 Event）。
- 验证：`events_` 11/11、`worker_` 17/17、`http_` 35/35；message 系列绿；
  冒烟 5/5；双守门绿。abortcontroller HANG/messageport-hasref  extra 字段/
  cloneable 缺 transferable API 皆为既有缺口（未碰其代码面，另案）。

## 2026-10-21 child P0 落地（small 已推 3d8b07e + medium 本轮，29→17绿）

- small（3d8b07e，已推 origin lane/child）：校验 7（serialization/timeout/
  stdio-stringvalidate + fork-timeout-kill 功能）+ `-e`/`--eval` 拆分
  （Mode::EvalQuiet，kill 套件字节精确；smoke 保 `--eval` 打印）+
  stringify 单次 + openStdin（ipc 套件）+ fork-close 排序递延 +
  signal-duck + abort-dispose；黑盒 validation 2 + cli quiet 4 全绿，冒烟 5/5。
- medium（本轮）：internal 真路径 6 件全绿（`node:internal/child_process`
  注册表 + getValidStdio 逐字 + ChildProcess 同对象 + spawnSync
  call-time 分发；bad-stdio/exec-kill/validate/windows-hide/kill-signal/
  shell 逐个啃下——killSignal 原值保留、shell 预包装 argv0 位 + 无参形
  直行、DEP0190 同步面、windowsHide 缺省 false、shell 路径/comspec/
  android 平台、-pe 翻译）；disconnect 双向信封后 ref2/url 绿 +
  DISCONNECTED/CHANNEL_CLOSED 双码；detached（unref/ref natives +
  state 摘除计数）；silent 管道（fork 子输出 base64 信封进父端管）；
  fork-close 经 CJS 分流加载（扩展名判定，require 单次）。
- 29 件终表：17 绿 / 12 红 = 另案 10（spawn-ipc 2 + 序列化往返 3 +
  handle 2 + next-tick + cluster-process + execPath）+ fork-ref
  （worker-flush 另案，见下）+ send-keep-open（node 抖）。
  spawn-typeerror/stdio-string 本轮绿（抖动收敛）。
- 转 worker lane（根治另案）：worker 线程退出丢弃未分发 parentPort
  消息（unlisten 端口；c18/c19 探针：dummy 监听 3/3 通、无监听 0/3 通、
  同线程双求值排除、plain worker 正常）。需求：退出前排空 inbound +
  flush outbound（node IPC 语义）。本 lane 用常 listened 试过——idle 子
  永不退（续命判定与监听时机耦合），已回退。
- 踩坑：clap 新字段必须配 en+zh 翻译（pitfalls 4.326，本分支已记；
  mut_arg remove+push 打乱相对顺序致全表错位，血泪 3 小时）；构建期改
  源码必等编完（base64 信封曾漏编半轮）；sweep 产物禁落 /tmp（磁盘满
  作废一次，改道 ~/wjs-data/tmp）。
- 三件套：模块级 internal_spec_table（alias+len 84）+ 黑盒 validation 2 /
  medium 4 / cli quiet 4 + 冒烟 5/5；UNSAFE-BOUNDARY 2 natives
  （child_unref/ref，detached-faces 覆盖）。

## 2026-10-21 lane/http2 续 C（px11 收尾：抑发+泵半关+拆级联；raw-close 转绿）

- px11 所指（"需钉 px11" 解码）：`~/wjs-data/probe/h2c-connect.mjs`（CONNECT 隧道
  EOS-before-200 形；node 真机双向通）+ pitfalls 4.322。非 flake 件——机制性红，
  不走 flake-classify。
- 前任半截 hunk 验纯：h2_client 抑 EOS 意图成立但两处未落地——① `!__opened` 路
  `__openNow(null)` 把攒体带进 open，Rust 非流式 open 即排 `End`（EOS 照发 pre-200）；
  ② streaming 路直发 pre-200 DATA 经 hyper CONNECT 体黑洞抛弃（下）。net 两 hunk
  （end 侧 rehold + ev 侧 `!__ended` 守卫）成对成立，直推 session-cleanup 3/3 转绿。
- 根因链（hyper 源码 + 线抓帧双实证）：① hyper h2 客户端永不驱动 CONNECT 请求体
  （`poll_pipe`: `is_connect` 即无 PipeToSendStream，`send_request(req,false)` 后体
  通道即孤儿——tap 证 open 体不上线）；② 200 前 EOS 即无 upgrade 对（4.322）；
  ③ 应答后桥写有效（tap 证 DATA 上线）；④ immediate-FIN 形结构性输竞速（200 后
  flush 天生晚一 RTT，目标接通即 FIN 即 proxy socket 先死）→ connect-method 书面偏离。
- 落码：JS 全量抑发（HEADERS-only open + 写侧缓冲 + END 递延，应答后桥 flush+补 END；
  扩展 CONNECT/已应答/trailers 原路径；Rust 隧道永不 eager-End 兜底）+ 隧道先决议再发
  response 事件（消 response 处理器内写竞速）+ 双泵半关续读（写关后仍服务读侧，双侧皆
  关才退）+ 泵 JoinHandle 入表（H2RespondReset/会话收尾 abort，保 4.321 拆除级联；
  无此则 proxy 套件功能过但进程永生——已实证回退）。
- 验证：冒烟 5/5；patient 双向 TARGET-OK+CLIENT-OK（rc=0）；proxy-over-h2 回绿；
  session-cleanup 3/3（base19b DIFF→绿，raw-close 收官）；connect-method 仍红
  （target-end ""，fast-red 非 hang，即偏离口）；extended 绿；cant-turn-off 黑白盒
  双 flaky（基线 3/12 vs 本轮 1/14，p≈0.3，无系统机制——毒化 waiter 竞速，记档观察）；
  net_ 24/24（含新件）、http_ 35/35、https_ 12/12、tls_ 15/15、cluster_ 6/6；
  goaway-opaquedata 红如基（B3 bound）；mce-http 定级 http lane（传输已对齐，
  缺第二 clientError）；buffersize 另案（与 base19b 同签名，POST close-after-end
  写回调序）；net.rs 1036 超千行系 HEAD 遗留（本轮新黑盒独立成文件，未加重）。
- 黑盒 +3：http2_tunnel.{deferred_end_patient,teardown_clean}、
  net_rawclose.end_after_fin_survives；守门 check-lines（除 net.rs 遗留）+ naming 双过。
## 2026-10-21 base19c 全域基线（lane全合入后，SAME0 2523/3528）

- 运行：pull 到 cacb403（childP0 等全合入）+ status 空 + load ~5 →
  构建卡 E0004（cacb403 合丢 Mode::EvalQuiet argv 分支，还原一行，
  commit 8a0b217；构建树干净，二进制 md5 跑后复核一致）→ 冒烟 5/5 →
  sweep 同参（--tag base19c，jobs 2，thread 3599，port 29999），
  02:36:36 → 03:19:08（~43min），3528/3528 无 abort。
  工件 `~/wjs-data/sweep/base19c/`。残留 26 进程告警（他 lane 常驻
  fixture + 探针，未清理、无干扰）；系统盘 46→36GB。
- 总表：SAME0 **2523** / SAME1 208 / DIFF 769 / TIMEOUT 28（绿 71.5%）。
  vs base19b（2470/207/820/31）：绿 **+53**、DIFF -51、TIMEOUT -3；
  真实欠账 851→**797（-54）**。HANG 34 + MUSTCALL 83 行（自报链正常）。
- 域级 Δ（绿）：net 98→128（+30，net lane 波）、module 7→26（+19）、
  child 71→82（+11，P0 落地）、vm 67→78（+11）、readline 10→20（+8）、
  promise/crypto/worker/socket 等小幅 +；tls 96→84（-12）、fs 283→276
  （-7）、process 69→63（-6）、http2 134→131、http/stream/url 小幅 -。
- 逐件：fixed 122（net 34/module 19/child 16/vm 11/readline 8/http2 7/…）、
  newred 69（tls 16/http2 10/fs 7/process 6/child 5/http 4/net 4/url 3/…）。
- 新红主因（~25 件，已修）：child P0 的 spawnSync mock 分发未守 `opts`
  缺席（`opts.shell` 直读炸；tls/h2/fs/http/process 各域凡经 opensslCli
  等无参 spawnSync helper 的全连红），commit ab3a76d（一行守卫），
  ecdh/spawn-argv0/max-header-size/read-stream/execpath 复探全绿 +
  mock 三套件复绿。base19c 二进制不含此修（跑后合入），下轮自愈。
- 其余新红（待各 lane 复判）：-11 崩溃 3（readfile-pipe-large×2/
  process-warnings，复探已绿，load 抖）；fs-mkdir-rmdir（回调次参
  object vs undefined，fs 面）；fs-stat/statfs/http-chunk/
  client-close/net×4/process-config 等断言差；TIMEOUT  churn
  （capture-rejections 等）。
- Top10（base19c 差额）：http2-112、tls-101、fs-46、http-41、crypto-39、
  worker-32、cluster-22、webcrypto-21、diagnostics-21、child-18。
  （child 29→18，P0 兑现；tls/fs 含上述已修 crash 簇，实欠更低。）
