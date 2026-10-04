# 踩坑分卷4（4.170–4.205）

> 本卷为 `docs/pitfalls.md`（主索引）分卷之一，只收正文；查阅先看主索引，编号 `§4.N` 全仓唯一。

### 4.170 tower-http 的 `not_found_service` 恒改写 404 + 非 GET/HEAD 缺省 405（2026-09-21，plan4 T1）

- 症状：`--serve --handler` 下 handler 明明跑了（body 对），但 GET 状态恒 404、POST 恒 405 空体（handler 永够不着）。
- 根因（轮子源码实锤，`tower-http 0.7.1 serve_dir/mod.rs`）：① `not_found_service`把 fallback 包进 `SetStatus<_, 404>`——文档原话"always respond with 404"，fallback 的状态被恒改写（body 保留）；② 非 GET/HEAD 缺省不调 fallback直接 405（`call_fallback_on_method_not_allowed` 缺省 false）。
- 修法：`serve_dir.call_fallback_on_method_not_allowed(true).fallback(js_fallback)`（`fallback` 文档原话"status will not be altered"；`src/serve.rs`）。
- 复现：`curl GET /<缺失>`（修前 body 对 + 404）+ `curl -X POST /echo`（修前 405 空体；修后 201 回声）。
- 推广为铁律：凡"名字像兜底"的轮子 API（not_found/fallback），先读源码确认状态改写语义再选；"静态优先、动态兜底"路由上线即验 GET/POST 双方法。

### 4.171 serve 停机 Wake + 响应构造快照边界（2026-09-21，plan4 T1）

- 症状一：空闲 `--serve --handler` 收 SIGTERM 后恒等 10s 才退，日志 `serve JS session did not drain in time open=0`（在飞为零仍 warn）。
- 根因：停机旗只在 quiescent 路径检查，`serve_loop` 空闲时 park 在通道上，无事件到来即 10s 收尾超时（`src/runtime/serve_session.rs`）。
- 修法：`ServeEvent::Wake` 无副作用事件（`serve_bridge.rs` dispatch 直返 Ok），收尾先置旗再投 Wake 打断 park，SIGTERM 亚秒级退出（`src/serve.rs`）。
- 症状二：handler `new Response(readableStream)` 报 `Response: unsupported body type`（500）。
- 根因：prelude `Response` 构造是快照语义（`__wjs_normBody` 只收 string/U8/AB/null，与 fetch 客户端共享；`http.rs:187`），与 undici可收流不同——属共享语义边界，非 serve 桥 bug。
- 修法（T1 范围）：构造期快照不动，`__wjs_serve_send_resp` 推送时 64KB 分片（多 Chunk 通道 + 单 native 拷贝封顶；`serve.rs:54` serve 驱动内，零外溢）。真流式构造（收 ReadableStream）留待另案（需动共享 `bodyUsed`/text 全家）。
- 复现：`POST 2MB 回声逐字节一致` + `GET 5MB 分带校验` + `SIGTERM 亚秒退出`（探针 `/tmp/wjs-serve-t1b-probe` 形；黑盒 `phase11_serve_large_body_streaming`）。

### 4.172 H3 半关闭 FIN + h3-axum 请求体整收（2026-09-21，plan4 T3）

- 症状：H3 建连/ALPN/h3-build 全过，`send_request` 后服务端静默、客户端 30s `ConnectionError(Timeout)`（服务端日志停在 `H3 request accepted`）。
- 根因：h3 client `send_request` 只发 HEADERS 不带 FIN；`h3-axum::serve_h3_with_axum` 先收齐 body（`recv_data → None`）再调 router—— client 不 `finish()` 即半关闭死锁（curl 等真客户端自动 FIN，只坑手写 harness）。
- 修法：harness `send_request` 后即 `stream.finish().await` 再读响应（`tests/serve.rs::phase11_serve_h3_same_router`）。
- 附带轮限：h3-axum 请求体整收后才调 router（H3 大上传内存 = 体大小），与 H1/H2 边收边泵不对等；T3 只验回声，上传流式对等留另案。另：本机 curl（SecureTransport 版）无 `--http3-only`，H3 以 harness 验收。
- 复现：去 `finish()` 即 30s Timeout；诊断法：服务端 debug 埋点看停在 accepted 还是进 axum（本次停 accepted 即 FIN 面）。

### 4.173 T4 WS 五坑：握手归属/GUID 记忆/构建盲区/自动应答/101 表达（2026-09-21，plan4 T4）

- 坑一（hyper 已握手后再 `accept_async` 永挂）：hyper 接管 101 后流上只有 WS 帧，`accept_async` 等一个永不到的 HTTP 握手。修法：`WebSocketStream::from_raw_socket(up, Role::Server, None)` 直接接管（async 仅构造，infallible；`src/serve.rs::run_server_socket`）。
- 坑二（GUID 凭记忆必错）：自拼 `258EAFA5-E648-…` 系虚构，真值 `258EAFA5-E914-47DA-95CA-C5AB0DC85B11`（tungstenite `handshake/mod.rs::WS_GUID`）；python/实现双绿掩盖（自交一致，§4.54 对称性盲区再进宫）。修法：密码学常量逐字节对轮子源码 + RFC 向量单测钉死（`ws_accept_key_rfc_vector`）。附带教训：单测写完即跑——本次若早跑，错向量当场红，不必绕 python 一圈。
- 坑三（`cargo build` 不编 `cfg(test)`）：bridge 加字段后 build 绿、集成测试绿，但 `cargo test --bin` E0063（`state/mod.rs` 测试 helper 旧构造体）。修法：改结构体必跑 `cargo test --bin <name>`（集成测试只链接二进制成品，不编 bin 的 `cfg(test)`）。
- 坑四（tungstenite 自动回 Close）：对端 Close 到达时库内已排队回帧，应用层手动再发被拒（ClosedByPeer），直接 break 即回帧滞留缓冲 → RST。修法：收 Close 后 `sink.flush()` 推出再结算（tests/ws.rs stub 回帧同族不同术）。附带：client 侧（`ws.rs`）同形缺 flush（对端先关即 RST），既有测试全绿未暴露，另案。
- 坑五（101 表达弃用）：初版 `new Response(null, {status: 101})` 被 prelude `RangeError`（status 限 200-599，undici 同口径，不动共享语义）。修法：handler 直接 `return socket`（`__wjs_wskState.server` 品牌位）即接受；Response 即 Decline。无新模块形状，fetch 契约不变。
- T4 语义记档（三件，另案）：① Decline 双 fetch（offer + HTTP 各跑一次，升级请求专属）；② H3 上传整收（§4.172）；③ 通道 unbounded（背压另案）。
- 复现：`phase11_serve_ws_echo`（去 flush 即 RST；错 GUID 即 tungstenite 客户端握手失败）；`phase11_serve_ws_bad_handshake/static_first`（400×3/101+RFC 键）。

### 4.174 全并行全量偶发 mozilla mutex 解锁失败（2026-09-21，观察中·未闭环）

- 症状：`cargo test` 全并行跑到 `--test node` 时 `util::phase9a_util_promisify_callbackify_deep_equal` 报 `mozilla::detail::MutexImpl::unlock: pthread_mutex_unlock failed: Invalid argument`后 abort；同用例单跑 0.59s 过，整 `--test node` 套件 51s 234/234 全绿。
- 现状：与当轮改动（serve 系）零交集，判定并行负载型 flake，非回归；根因未深究（引擎内部锁，另案）。再现两次即升级为必查。
- 推广：全量红先单跑 + 整套件跑两档复核，再定回归/flake（§4.62 姊妹篇）。

### 4.175 fifo 黑盒全并行负载下挂死：写者缺席读端零 CPU 睡眠（2026-09-21，观察中）

- 症状：全量 `cargo test`（默认并行）在 `fs::streams::phase10f_read_stream_fifo_end`卡死（`running for over 60 seconds`；读端 winterjs 进程 `S` 睡眠、7 分钟仅 0.21s CPU；写者 `sh` 不存在）。同域测试（26 并行）两次 7s 过，单跑 0.59s 过。
- 根因（未完全钉死）：重负载下 `child_process.exec` 的写者 shell 缺席，读端 `open(O_RDONLY)` 永阻塞。域拆分纯搬移（JS/Rust 双字节恒等已验），非回归。
- 修法：`cargo test --test node -- --test-threads=4` 全 node 域 248 绿（90s）；其余 19 target 全绿。全并行卡死先查该用例（`ps` 见读端 `S` + 无写者即此坑）。
- 复现：全并行跑到该用例即卡；降并行即过。

### 4.176 URLPattern 专项四坑（2026-09-21，plan3 §5）

- 坑一（组序是哈希桶 artifact）：多组 `groups` 键序真机同名集异序可得异序（`(a,b,c)→[b,c,a]`、`c,b,a→[b,a,c]`，跨进程稳定但无声明规则；映射本身全对）。根因：Node 内部名表迭代序非声明序（疑固定种子哈希表桶序，无源码实锤）。修法：`serde_json::Map`（BTree）天然字典序 + 代码注释记档为确定性偏离；套件只钉单组（无多组序断言），黑盒显式断言排序后形状。教训：先验跨进程稳定性（稳定≠可复刻），不稳定才谈对齐、稳定但无规则即记偏离。
- 坑二（base 失效吞掉）：`new P("https://example.com", null, null)` 应抛 INVALID_URL_PATTERN，初版回成功——Rust 把非法 base（`"null"` 解不出）当缺席（`and_then(parse)`）。修法：parse 路 `Some(b)+解不出` 即抛；test/exec 路维持吞错（真机回 false/null，套件钉住）。
- 坑三（`r#"` 撞 `"#frag"`）：测试 JS 含 `"#frag"`，`r#"` 裸串被提前闭合，全文件编译炸（§4.44 家族：`format!`/`r#` 与 JS 同现一律换定界/落盘）。修法：载荷块改 `r##"..."##`（断言串内无 `"##` 即安全）。
- 坑四（concat 同域双导出）：`url_pattern.js` 与 `url_legacy.js` 各写一次 `export URLPattern` 即 `Duplicated export`（concat 是同一模块作用域）。修法：定义与导出分家——pattern 块只留 `const`（置首位供 default 对象求值），具名/default 双导出全收进 legacy 块。
- 附带：轮子两入口宽严不一——`parse_constructor_string("[")` 抛，`parse(init{pathname:"["})` 过（真机后者不抛）。单测走错入口即红，入口按调用形状选（串形/字典形各归各）。
- 复现：`tests/node/url.rs::phase11_urlpattern_*` + 真套件三件双侧 rc=0。

### 4.177 dgram 余簇五件：校验序/端口序/fork 宽容/伪语义翻转/挂死归属（2026-09-21）

- 校验序：已连接态 `send(23)` 应报 ARG_TYPE buffer 而非 IS_CONNECTED—— msg 形态校验先行，再判连接态，最后 offset/length 越界（真机序；`send(buf,1234,addr)` 的地址串在位置 2 即判连接，同序）。
- 端口序：未连接 `(buf,0,6)` 报 BAD_PORT 而非地址错——`validatePort` 先于地址形态校验（`(buf,6,0)` 之类三参形按 msg/port/address 解，地址数错另案）。
- fork 宽容：`cluster.fork("str")` 真机不抛（env 展开语义，fork 从不校验 env 类型，见 lib/internal/cluster/primary.js；42 同理照走）。自家黑盒旧断言 `fork(42)` 抛错系伪语义——§4.65 翻转（黑盒改不断言抛、改跑真 fork + worker 自退）。
- 挂死归属：unref-in-cluster 实现正确（res-check `[]`）但套件抖动时，先抓"停在哪端"（本轮：P-worker-exit 永缺席 = cluster 退出 race，G6 worker 投递/cluster 协议同族），再定回归/flake；stash 旧树对照若缺 API（恒红）即无效对照，不如直接读退出事件。
- 复现：`tests/node/dgram.rs::phase11_dgram_*` + `test-dgram-send-bad-arguments`修前 `Missing expected exception`（端口进队列未同步校验）/修中 `unexpected throw`（连调定位法：CAUGHT 打实际值，见本轮）。

### 4.178 node:test Slice A 四坑（2026-09-21，plan3 test API 轮）

- 坑一（CJS 包装头垫行，栈行号 +1）：`t.assert.ok` 失败补调用点源码行时，ESM 按栈行号读文件精确命中，CJS 恒差一行（require 五连柯里化包装头垫一行，`require.rs:244` 实锤）。修法：`__callerLine` 窗口向上回扫（行号起向下 5 行），首个含 `ok(` 的行即调用点，落空才回精确行（`src/builtins/node/testmod.rs`）。
- 坑二（async 吞同步校验）：`t.waitFor` 校验写在 `async` 函数体内，`t.assert.throws` 同步调用够不着——抛错变 rejection（另附 4 条 unhandled）。修法：校验提同步段先执行，通过后再进异步轮询（`__waitFor`/`__waitForRun`分家）。推广：凡"同步抛 + 异步跑"双形态 API，校验一律同步段，§4.37 症状一的 async 版。
- 坑三（竞速输家 timer 续命）：`waitFor` 的 `Promise.race([attempt, sleep])`输掉的 `sleep(60000)` 不清——测试全过、小结已打，进程续命 60s（`polls`/`limits` 套件，`timeout: 60000` 现形；真机同为 cancel 语义）。修法：race 落定即 `clearTimeout` 睡眠端。推广：凡带超时的 race，落定即清输家，否则"全过但不退"（§4.93 的 hang 反面：输出齐、退出码无）。
- 坑四（CJS 看不见 ESM 具名导出）：`require("node:test")` 取默认导出本体，`assert`/`getTestContext` 等纯具名导出即 undefined（`test/suite` 因挂在 test 函数上才可见，`run is not a function` 同源）。修法：具名同步挂载 `test.getTestContext/test.assert`（`run/mock/snapshot` 随各片）。
- 附带：`describe` 无 fn 即抛是偏差——真机 `createSubtest` 非函数 fn 即 noop（空 suite 合法），改静默 noop；`strictEqual` 缺省文案改真机逐字（`Expected values to be strictly equal` 前缀，custom-assertions 套件钉住，既有黑盒无文案依赖）。
- 复现：11 目标套件（修前 DIFF 修后 SAME0）+ `tests/node/testmod.rs` `phase10f_test_*` 四件。

### 4.179 全并行 2 挂再现（2026-09-21，§4.174 家族）

- 症状：全并行 `cargo test` 在 `--test node` 挂 2 件—— `child::exec::phase10f_child_exec_shell_self_and_timeout`（`envself` 行缺失）+ `fs::sync::phase10f_file_handle_read_empty`（`MutexImpl::~MutexImpl:pthread_mutex_destroy failed: Resource busy`，§4.174 同款）。
- 定性：单跑双绿 + `--test node -- --test-threads=4` 257 全绿——并行负载型 flake，非回归（本轮改动：testmod/assert，与 child-env/fs 零交集）。
- 推广：全量红先单跑 + 降并行整域两档复核（§4.174 纪律）；`| head` 后 `echo $?` 取的是 head 的码，黑盒/探针判活一律文件落盘 + `${PIPESTATUS[0]}`或重定向后取码（§4.45 三进宫：本轮二分 wait-for 时亲手复现一次）。

### 4.180 node:test 钩子归属 + MockTracker（2026-09-21，plan3 test B1 轮）

- 症状：`local mocks are auto restored` 在 mock 恢复后报 `Expected [Function bar] notStrictEqual [Function bar]`（afterEach 把已复原的 bar 又判成"仍 mock"）。
- 根因：想当然"钩子跑在 owner 身上"——真机 Test.run 跑的是 `this.parent.hooks.*`（钩子归属是父，参数传子 ctx）：父 afterEach 只在子身上跑一次（继承执行），owner 自身结束时跑的是**祖父**的钩子（多为空）。探针 `hook.cjs` 钉住：无子测试的 `t.beforeEach/afterEach` 永不跑；`t.before` 在首个子测试时跑一次；子身上 getTestContext 见 owner 名。
- 修法：`__runOne` 钩子段重排——before（套件 runOnce + 测试 owner 首子一次）、beforeEach（套件由外向内 + owner 的，子 ctx）、afterEach（owner 先 + 套件由内向外，注册序）、owner 自身 `after` 照旧；测试级钩内 getTestContext改推 owner ctx（`__runTestHook(fn, ownerCtx, argCtx)`）。旧模型（自身跑自身 Each）在 Slice A 全绿下掩盖，新钩子一用即现形。
- MockTracker 移植要点：Proxy target 即原函数（name/length/descriptor 全透；`bind` 的 this 透传；construct 经 `ReflectConstruct(impl, args, proxy)`故 `instanceof` 归原函数）；method 经原型链找描述符、自有属性安装；`restore` 判 `methodName !== undefined`（真机仅判 string，symbol 复原是其漏口）；`times`/once 下标门逐字（validators 现成）。
- 引擎偏离（记档不修）：`mocks a constructor` 末断言要 V8 私有字段文案，SM 文案不同且 JS 层无拦截点（mocking.js 55/56，文件级仍 DIFF）。
- 复现：`tests/node/testmod.rs::phase10f_test_mock_*` 两件 + `test-runner-mocking.js`（修前 `target undefined` 全灭）。

### 4.181 mock.timers 四件（2026-09-21，plan3 test B2 轮）

- 补丁面先探再写：`node:timers` 命名空间冻结（`setTimeout is read-only`）——其具名补丁整体跳过（文档化偏差）；`scheduler.wait` 经影子赋值可补（原型有实现，赋后自有、删即复原）；`globalThis.Date/setTimeout` 直接赋值可补。探针 `patch{,2,3}.mjs` 三件先行。
- 测试结束必须 `mock.reset()`（含 timers），只 `restoreAll()` 即跨测污染—— node Test 收尾即此口径（`test.js:1526`），本轮 testmod finally 同改。
- `Date.toString()` 套件钉 V8 单行串——SM 原生多行，直接回真机可观测串（与 §4.110 同类文案桥，就地注释）。
- 复现：`tests/node/testmod.rs::phase10f_test_mock_timers_*` 两件 +真套件 date/scheduler 双转绿（SAME0 17→19）。

### 4.182 run(none) 五坑（2026-09-21，plan3 test C 轮）

- 坑一（import 期 pump 打架）：内层文件 `test()` 直接调 `__pump`，与 run 的 drain 形成双排空循环共吃队列，顺序全乱。修法：`__innerActive` 旗，泵卫拒内层 drain（`__pump` 直接 return），run 走 `__drainLoop` 直调（§4.24 多 run教训的同进程版：隔离边界 = 状态快照/复原 + 可重入 drain）。
- 坑二（suite 回调早于 before）：真机 Suite 构建先跑父 before 钩、再调 suite回调（order-probe 钉住：直跑/run 同序；异步 before 不阻塞回调）。修法：describe 建套件即 kick 父 before；kick 内联跑同步前缀、遇异步挂链；测试起跑 await 落定（毒化照旧）。
- 坑三（后注册 before 永不到）：套件级 fired 旗太粗——后载入文件的根 before在首 kick 之后注册即漏。修法：逐钩 runOnce（已跑集合 + 每次 kick 补跑新增，全串行；落定清槽以便下轮内联）。
- 坑四（钩子归属再确认）：钩子 `this`/参数 = 运行中测试 ctx，`getTestContext`= owner；before/after/suite 回调的 `this`/参数 = owner；根名 `<root>`（no-isolation 夾具 `this.name` 逐项钉住）。修法：全部调用点改 `fn.call(argCtx, argCtx)` / `fn.call(ownerCtx, ownerCtx)` 两族。
- 坑五（only 批量误伤）：旧批量过滤把整批压成 only，套件内非 only 本该跑。修法：删批量，改 applyFilters 逐项门（祖先标记 + 父门；`only:false` 显式即 noop）。另：无 before 套件的 after 被 beforeFired 门吞——补 `_ran` 位。
- 复现：`tests/node/testmod.rs::phase10f_test_run_none_and_plan_gates` +真套件 no-isolation ×2/enqueue/test-id/tags-validation（修前 DIFF）。

### 4.183 run(process) 五坑（2026-09-21，plan3 test D 轮）

- 坑一（loader 目标回退重复求值）：抛错文件的动态 import 被 ESM→CJS 回退各求值一次，注册翻倍（todo-skip 双跑现形；成功文件单次无事）。修法：双管齐下——① skip 套件不跑回调（真机：跳过即不构建，遂无抛错），② 失败导入回滚本批注册（队列/注册表截断）。
- 坑二（合成错被 expectFailure 回吞）：意外通过合成的 expectedFailure 错在 catch 又被"期望失败→pass"分支吞掉。修法：catch 按 `failureType !== "expectedFailure"` 分流（`§4.51 __callNative` 闭包错码重包的同源教训：包装层必须识别已整形错误）。
- 坑三（skip/todo 是置旗不是抛）：`t.skip()` 后 body 继续、skip 优先、message回显到事件互斥键（真机探针钉住三条）。修法：throw 改置旗 + 终局判定（抛错仍粘滞失败）；静态 todo 改跑 body（失败仍失败）。
- 坑四（cwd 径带 `..` 全等失败）：`process.cwd()` 非规范化，filetest 的 file全等断言挂。修法：`resolve(cwd, given)` 规范化（`node:path` 现成）。
- 坑五（监听抛错被吞即假绿）：`_emit` 内 try/catch 把 mustNotCall 断言吞掉，文件空绿。修法：去吞（真机 EventEmitter 口径）+ `__runFilesAsync` 的 catch改异步重抛走 uncaught（文件可见失败）。
- 复现：`tests/node/testmod.rs::phase10f_test_run_process_and_expect_failure` +真套件 expect-error ×2/todo-skip/filetest（修前 DIFF）。

### 4.184 run 语义深化六坑（2026-09-21，plan3 test E 轮）

- 坑一（文件级 enqueue 在监听前丢失）：`run()` 同步调 `__runFilesAsync`，首事件先于调用方 `.on` 发出（test-id 套件现形：dequeue/start 有、enqueue无）。修法：执行体递延一轮 microtask（监听先挂）；worker 内同理（同函数复用，harness 的 `.on` 同样后挂）。
- 坑二（同文件二次 import 命中缓存）：黑盒两次 run() 同一探针文件，第二次空转零事件。真机同款语义（模块缓存；worker 跨线程则天然隔离），黑盒改双探针文件（§4.168 SAME1 掩盖姊妹篇：缓存使"跑过"恒真）。
- 坑三（helper 改签名丢参数）：`__runOneWorker(given, abs, stream)` 重构丢了 `options` 形参，体内 `options.timeout` 全变 ReferenceError（全部 process用例一夜回红；栈 `__runOneWorker/<` 指认）。修法：改签名必须 grep 全部调用点 + 被调体内全部标识符（§4.182 坑五同源；本轮现形）。
- 坑四（包装与显式发射二选一）：`__emitPass`（内发 pass+complete）上线后，外层残留的显式 `complete` 致每测试双 complete（test-id 计数现形）。修法：包装函数与显式发射二选一，grep 全调用点去重。
- 坑五（skip/todo 置旗三语义）：`t.skip()` 后 body 继续、skip 优先、message回显到事件互斥键（真机探针三条钉住）；静态 todo 跑 body（失败仍失败，通过记 todo）。修法：throw 改置旗 + 终局判定（§4.183 坑三的 E 轮落实）。
- 坑六（loader 回退双求值）：抛错文件的动态 import 被 ESM→CJS 各求值一次，注册翻倍（todo-skip 双跑现形；成功文件单次无事）。修法见 §4.183 坑一（skip 套件不跑回调 + 失败导入回滚注册；本轮探针钉死）。
- 附带：legacy done 回调（streaming 套件现形）；plan 子计数；stopTest 超时竞速 + TestPlan wait；tag 过滤子集；entryFile 转发戳；调用点文件归属；种子洗牌（PRNG 逐字 + 延迟兄弟队列）；run coverage 选项校验（码逐字）。
- 复现：`tests/node/testmod.rs::phase10f_test_run_semantics_*` + `phase10f_test_run_tag_filter_and_randomize` + 真套件 plan/tags/entry/randomize（修前 DIFF 修后 SAME0）。

### 4.185 http TIMEOUT 轮八坑（2026-09-22，plan3 G11）

- 坑一（defer-to-connect）：`req.setTimeout(1000)` 同步覆写把已建连 socket 的超时也改成构造期值。根因：覆写直写 socket，connect 前后未分。修法：connect前只记请求级值（socket 事件仍见构造期值），connect 时落地（`framing_agent.js` `setRequestSocket`）。复现：`client-set-timeout`（修前 socket 事件即 1000）。
- 坑二（`_last` FIN 递延）：GET 管线超 max 的 503 恒丢（POST 靠 pacing 碰巧能到）。根因：FIN 排在 cont（re-feed）之前，已读管线字节的错误响应先被 FIN 截断（write-after-end 丢失）。修法：FIN 递延一轮排在 cont 之后（`framing_outgoing.js`）。复现：`tests/node/http/timeout.rs::p3`（修前 GET 无 503）。
- 坑三（capture 接线）：`captureRejections` 形 error 经 socket 透传丢失。根因：capture destroy 的 err 未进 socket 错误通道。修法：capture destroy 带 err透传（有监听才发，无监听仅 aborted）。复现：`outgoing-message-capture-rejection`（修前 DIFF 修后 SAME）。
- 坑四（ready 解禁）：raw-socket 套件（keep-alive-pipeline-max-requests 等）写饿死。根因：§4.126 暂缓 `ready` 不发射，真机 `onconnection` 后同步触发写。修法：connect后同步发射（`net` 侧；暂缓作废，欠账清零要求语义到位）。复现：上套件（修前 hang）。
- 坑五（池复用 FIN 竞态）：`Connection: close` 响应入池，次请求复用撞 FIN 报 ECONNRESET。根因：回池只门 freeSockErr。修法：`__release(poolable)`——close 响应即销毁（`framing_agent.js`）。复现：`get-pipeline-problem`（修前 DIFF）。
- 坑六（空闲判定漏 res）：`closeIdleConnections`/看门狗误杀在途响应。根因：空闲只看 req 侧。修法：判定补 `st.res` 三处（close/closeIdle/看门狗）。复现：在途响应形（修前被关）。
- 坑七（abort 门控）：服务端无 error 监听时 abort 抛错。根因：error 同步发抢在 aborted 之前。修法：aborted 同步恒发，error 有监听才发且递延（不抢 res 侧 PREMATURE_CLOSE）。复现：`aborted` 块 + `timeout.rs::a2`。
- 坑八（`_ended` 订正）：响应中 `setTimeout` 全被 noop。根因：门控误用请求 finish置位（`__reqFinished`）。真机 `_ended` 置于 responseOnEnd。修法：门控改 `res.readableEnded`，删请求置位。复现：`client-timeout-with-data`（修前 hang）。
- 附带方法学二则（旧坑再现）：① 脏二进制打架两次（6 秒构建误判；§4.62/§4.145姊妹）——后一律 `ls -la` 对时间戳 + 行为验证；② `| head` 后 `$?` 是 head 的码（§4.45 三进宫）——判活一律文件落盘取码。

### 4.186 服务端 destroy 失声 + 半开续命：两处 hang 一次清（2026-09-22，plan3 G11）

- 坑一（写端等读端 EOF 即死锁）：`NetCmd::Close` 只 shutdown 写端，`Close`事件要等读端 EOF——对端半开（allowHalfOpen 客户端）永不 FIN，读端在 `read()` 永驻，`server.__sockets` 残留 1，循环永不 idle（`__sockets.size`探针实锤；
  客户端兜底 destroy 即退是同一根因的反证）。修法：写端收 `Close` 即发 `Close`（`close_once` 防与读端 EOF/错路径双发），不等读端（读端后到 EOF 只发 End）（`net_pumps.rs` 写端 `Close` 臂）。
- 坑二（收 FIN 半开仍续命）：坑一修后服务端干净（`sockets=0`）仍 hang——半开客户端（`net_open=1`）续命，而真机照常退出（k7/k9/k12 逐项实测：半开且 `ref()` 也留不住；
  读停转后空闲句柄不 ref 循环是 libuv 层事实，写侧仍可用、`write-cb ok` 照常）。修法：`NetEntry.holding` 位（初值 true）+ `net_halfhold` native——`__ev end`内 allowHalfOpen 未销毁即递延一轮 microtask，稳定半开（监听内无同步 destroy/auto-end 动作）才摘续命；
  `net_open` 只数 `refed && holding`；`set_ref` 摘除后只翻位、`purge` 按位结算，防双减（`net_halfhold_balance`单测钉住全部转移）。
- 证伪记录（勿复踩）：半开判定不能下在 End 派发时——正常全关舞蹈的 End→（microtask auto-end）→Close 链中间会出现"无进展 + 计数零"的轮次，直接摘会抢在 Close 到达前退出（Close 在途由 park 的通道唤醒兜住，但写端 shutdown 中的无消息窗口盖不住）。只摘"JS 已结算无动作"的稳定态。
- 复现：`test-http-server-keep-alive-timeout`（修前 TIMEOUT 修后 SAME0）+ `tests/node/net.rs::phase11_net_halfopen_releases_loop`（8s unref 守卫，回归只红不挂）；`drain-writable-length` 仍 TIMEOUT（outputData 缓冲模型，G3 既定另轮专项，不属本坑）。
- 附带：`server.close-idle-wait-response` 同批转 SAME0；`server-request-timeout-keepalive` 真机自挂（node 142，超跑分 alarm），非我方回归；dd3/kadbg  park 偶发未复现（4/4 确定性触发 kaT，疑为同族残留计数所致）。

### 4.187 http 升级流八坑（2026-09-22，plan3 G11 upgrade 轮）

- 坑一（升级判定缺 connection 门）："带 Upgrade 头即升级"系伪语义——真机需 connection token `upgrade` + Upgrade 头双全（advertise case2/3 钉住；llhttp同款）。修法：双门（token 大小写不敏感逗号切）。
- 坑二（无监听回落 vs 销毁三形态）：无回调 + 无监听 → 回落 request（advertise末段/`upgrade-server` no-listener 形 200）；回调放行 + 无监听 → 销毁（TrueWithoutHandler 形 ECONNRESET）；回调否决 → request。旧"无监听即销毁"系伪语义。修法：三向分流（`framing_outgoing.js` 升级块）。
- 坑三（对形 headers）：客户端 `headers: [[k,v],...]` 在 errors 内部空错炸（扁平形才通）。真机双形同发头。修法：首元数组即对形分支（`framing_outgoing.js` ClientRequest 构造器）。
- 坑四（spill 重入无限递归）：spill 经 `sock.emit("data")` 重入服务端同表监听→ `__feedUpgraded` 自递归（700+ 次才爆栈）。修法：`__spillGuard` 守卫。
- 坑五（直调前双发）：native `__ev` 的 `emit("data")` 与 spill 同表——用户收到原始体 + spill 双份。修法：升级后 native 改 `__srvFeed` 直调喂体，用户只收 spill（`net_socket.js` data 臂）。
- 坑六（服务端监听占数吞 spill）：服务端自有 data 监听使 `listenerCount ≥ 1`恒成立，spill 提前冲刷给空（unread 套件 'upgrade head' 丢）。修法：升级/CONNECT 接管即 `off` 摘除服务端监听（native 已直调，残留无用）。
- 坑七（迟挂监听丢字节）：101 先到、data 监听后挂（unread 套件 10ms）即丢——真机缓冲至读。修法：`__dataBuf` 暂存 + `newListener` 递延冲刷（入表后，§4.47）；直发改先暂存后冲刷，保序（`net_socket.js`）。
- 坑八（destroy(err) 同步抛）：同步 `emit("error")` 把 uncaught 语义压成同步异常（body-error 套件）。真机 `emitErrorNT` 走 nextTick。修法：`process.nextTick` 异步发（tick 回调带 uncaught 路由；microtask 落 rejection 走 fatal，不可用）。
- 复现：6 目标套件（修前 5 TIMEOUT + 1 DIFF，修后 SAME0）+ `tests/node/http/upgrade.rs::phase11_http_upgrade_faces`（15s unref 守卫）。

### 4.188 http 头面 batch5 九坑（2026-09-22，plan3 G11 头面轮）

- 坑一（数字头名过 token 门）："3840" 全数字是合法 token 字符，`TOKEN_RE.test(String(name))` 对数字名恒过。真机数字名即 `ERR_INVALID_HTTP_TOKEN`。修法：`typeof name !== "string"` 先判即抛（set/append 双侧，`framing_head.js` + `framing_agent.js`）。
- 坑二（奇长数组错码）：`writeHead(200, ['a','b','c'])` 真机 `ERR_INVALID_ARG_VALUE 'headers'`，旧实现错抛 ARG_TYPE。修法：改码。
- 坑三（writeHead 无发头门）：已发头再 writeHead 真机即 HEADERS_SENT，旧实现无入口检查直接覆写。修法：入口加门。
- 坑四（writeHead 不覆写拼写）：`setHeader('test')` 后 `writeHead({Test})` 真机 wire 为 'Test'——首写优先仅 setHeader 之间，writeHead 恒覆写。修法：合并分支无条件赋值 `__headerNames`。
- 坑五（220 短语 undefined）：未知码真机短语 'unknown'（属性与 wire 同），旧实现属性 undefined、wire 空串。修法：`STATUS_CODES[sc] ?? "unknown"` 双处。
- 坑六（数组同键塌缩）：`writeHead([a,1,a,2])` 旧实现后值覆写前值丢一行。真机逐行保留。修法：首触覆写、再触累积（`__touched` 集）。
- 坑七（对形 writeHead 不认）：`writeHead(200, [[k,v]])` 真机合法（ClientRequest构造器双形同源），旧实现当扁平判奇长即抛。修法：首元数组即逐对取 [0]/[1]归一扁平（`["b"]` 对即 value undefined 走 INVALID_HEADER_VALUE，超长元忽略，真机逐项实测）。
- 坑八（Host 恒省略缺省端口）：旧实现 `port===80` 即省（flavor 缺省），真机（lib/_http_client.js 源码）比较的是**显式配置** defaultPort（缺席即 undefined，`80 !== undefined` 恒拼）。修法：`__cfgDp`（options.defaultPort ??agent.defaultPort）+ 恒拼 + IPv6 双冒号加框（单冒号 'foo:1234' 不加框）。
- 坑九（拒写检查进 _write 毒化流）：`_write` 内同步抛使 writing 态永驻，后续 `end()` 永挂。修法：检查提 `write()`/`end()` 包装层（Node 本体亦在 OutgoingMessage 层），`_write` 保持纯净；连带 `ERR_HTTP_BODY_NOT_ALLOWED`新码 + 服务端选项透传（`rejectNonStandardBodyWrites` 缺省 false，1xx/204/304/HEAD 无体判据，空串亦抛，真机矩阵实测）。
- 复现：28 件头面对拍（修前 5 红 + 旧 11 件，修后 SAME0；`header-overflow`的 `socket.push` 系既定另轮）+ `tests/node/http/surface.rs::phase11_http_header_face_batch5`。

### 4.189 http TIMEOUT 深水第一铲：host/auth/CONNECT 隧道九坑（2026-09-22，plan3 G11）

- 坑一（hostname/host 取反）：`url.parse` 对象同时带 `host: "h:port"` 与 `hostname: "h"`，旧实现取 host 当主机名连过去即 ECONNRESET。真机（lib/_http_client.js 源码）`hostname` 优先。修法：两处（ClientRequest构造器 + agent 建连 opts）同改。
- 坑二（auth 丢失）：`options.auth` 从未转 `Authorization: Basic`（真机 551 行口径）；URL userinfo 经 `urlToHttpOptions` 进 auth（decode 双侧）+ IPv6 去框。修法：`normalizeRequestArgs` 补 auth + 构造器补 Basic（显式头恒赢）。
- 坑三（CONNECT 补斜杠）：`path` 无条件补 `/` 把 authority-form 改成 `/target:443`。真机 293-295 行 CONNECT/OPTIONS * 豁免。修法：双豁免。
- 坑四（CONNECT Host 取错）：Host 取连接主机，真机 546 行取 path 本体。修法：`method === "CONNECT" && options.path` 即 `String(path)`。
- 坑五（隧道不 detach）：隧道建立后两端挂满请求侧监听（client connect 1/data 1/end 2/close 2/error 1/timeout 1，server close 2/error 1/timeout 1），真机两端皆 end:1 其余 0。
  修法：具名存根（connect/secureConnect/agent 单例/net conns/error/close/timeout）+ 双端 detach（client 留 agent onReadableStreamEnd 恰一 end，server 留 end；
  `_httpMessage=null` + 摘池 + req destroyed/close，socket 不动）+ server FIN 守卫（`__connectHijacked`，升级形不动）。
- 坑六（server timeout 无条件挂监听）：`sock.on("timeout")` 在 `if` 之外，缺省 timeout=0 仍占数。修法：进 `if` + 存根。
- 坑七（socket 无 HWM）：`net.Socket` 无 `writableHighWaterMark`（真机 65536，与 ServerResponse 默认对齐）——旧背压默认 16KB 一并改 64KB（黑盒无 write-false 依赖，实测零回归）。
- 坑八（基类无 setTimeout/protocol）：`new OutgoingMessage().setTimeout` 即 not a function；`req.protocol` 缺席。修法：基类 `setTimeout`（无 socket 等'socket' 事件，**用事件实参**——手工 emit 形下 this.socket 恒 null）+ `this.protocol = flavor.protocol`。
- 坑九（后块同步抛掩盖前块 hang）：多 server 文件里 B1 的 handler 抛（吞进 400 通道即静默 hang）与后块 protocol 同步抛竞速——后块赢即 rc=1（前块 hang被掩盖），protocol 修好后前块 hang 现形。教训：多 server 文件定级只看 rc 会误判"后块全过"，必须分块二分（本轮拆 5 段钉死 B1）。
-  deferred（另轮专项，不在本铲）：handler 抛进 400 通道即静默 hang（真机 crash；`uncaught-from-request-callback` 为关键套件，改动 blast radius 覆盖全 http 域，另立单元）+ `writableLength` 精确记账（headers/帧头计入，`len+8` 形；G3 outputData 专项同源）。
- 复现：11 件转 SAME0（url.parse×5/auth×2/CONNECT×3/settimeout）+ `tests/node/http/surface.rs::phase11_http_timeout_deep_host_auth_connect`；`outgoing-properties` 仍红（HWM 已对齐，余 wl 记账专项）。

### 4.190 http TIMEOUT 深水第二铲：server 选项面三坑（2026-09-22，plan3 G11）

- 坑一（选项类被无视即 handler 抛吞 hang）：`createServer({IncomingMessage:MyIM})` 下 handler 调 `req.getUserAgent()` 在默认类上不存在 → 抛错吞进 400 通道即静默 hang（§4.189 deferred 同源）。
  真机无选项校验（任意值照收）。修法：server 存 `IncomingMessage/ServerResponse` + 请求期当构造器用（`new (self.IM ?? IM)(hwmOpts)`/`new (self.SR ?? SR)(sock)`；
  子类无显式构造器即透传；裸 `http.Server()` 本就可调，无事）。
- 坑二（createConnection 丢选项）：`net.createConnection` 以 `new Socket()`无参构造再 connect，`readableHighWaterMark` 等流选项永不到构造器。修法：首参对象即透传进 `new Socket(__o)`（构造器只读自家键，其余忽略）。
- 坑三（res HWM 不同步）：`res.readableHighWaterMark` 恒 Readable 缺省，真机跟 socket 走（1024 用例）。修法：客户端 res 构造传 `{highWaterMark: sock.readableHighWaterMark}`（Readable 原生键，server 侧同款）；socket 侧补 `__rhwm`（readableHighWaterMark/highWaterMark 逐级，缺省 65536）+ 双 getter。
- 附带真机口径（同轮实测）：socket 读写 HWM 缺省双 65536（旧背压默认 16KB 一并改 64KB；黑盒无 write-false 依赖）；定制只改对应侧（readable 定制不碰 writable）。
- 复现：3 件转 SAME0（server-options-incoming-message/server-options-server-response/incoming-message-options）+ `tests/node/http/surface.rs::phase11_http_server_options_surface`。

### 4.191 splitting 一件：ERR_INVALID_CHAR 缺 `["key"]` 后缀（2026-09-22，G11）

- 症状：`writeHead(200, {foo: "bar\r\nbaz"})` 码对文案错（缺 `["foo"]`）。
- 根因：`E('ERR_INVALID_CHAR')` 定死裸串；`__checkOutboundHeaderValue` 不收键。真机（lib/_http_outgoing.js 664/692/756 行 + internal/errors.js 1486 行）：`(name='header content', field)` 双参，field 在场即拼后缀——set/append/writeHead 三路全带键。
- 修法：E 改 `(field = undefined)` 函数形（无参回裸文案，旧调用零回归）+ `__checkOutboundHeaderValue(validation, value, name)` 全调用点传键；trailer 私有 `__validateHeaderValue` 保持无键（套件未点名）。
- 复现：`test-http-response-splitting`（修前 DIFF 修后 SAME0；附带 validators/value-relaxed/mutable/multiple/invalidheaderfield×2 零回归）+ `tests/node/http/surface.rs::phase11_http_invalid_char_key`。

### 4.192 response 双件：write-after-end 毒化终结块 + 状态码门未注册（2026-09-22，G11）

- 坑一（同步 extra 写吞终结块）：`write/end/同步write` 三连只差一步——异步 extra 写（100ms 后）终结块正常，同步即丢（`5\r\nDATA.\r\n` 后无 `0\r\n`）。
  根因：基类见 errored 压住在途 `_final`，而终结块只活在 `_final` 里；holdTimer 路径（发头+体）照走，终结无人补。
  修法：`write()` 包装层先行拦截已 end 的写（自发 error + 回 false，不进基类不置 errored），终结块走正常 `_final`；
  优先级 end > 拒写旗（204 先 end 后写仍 WRITE_AFTER_END，真机实测）。附带：类内曾有两个 `write()` 定义（旧拦截引未定义的 `__writeAfterEnd`，被后者遮蔽零生效）——删死代码时把夹在中间的 `__isNoBodyStatus` 一并带走，编译不报错（JS 方法悬空引用只在调用时炸），靠 grep 现形。
  教训：删遮蔽方法必 grep 体内标识符。
- 坑二（`ERR_HTTP_INVALID_STATUS_CODE` 从未注册）：`codes.X` 无 Proxy 兜底，未注册即 undefined，`new` 即构造器 TypeError（码错）→ handler 抛吞 hang。修法：E 注册 `'Invalid status code: %s'` RangeError + 调用传原值；门按 Node原文 `statusCode |= 0` 后判（字符串 '1000' 照收越界才抛；`%s` 遇对象走 inspect——{}→'{}'；writeInformation 同换 E 形，门不动）。
- 复现：`test-http-res-write-after-end`/`test-http-response-statuscode`（修前双 TIMEOUT，修后 SAME0；head-throw 零回归）+ `tests/node/http/surface.rs::phase11_http_response_gates`。
- 未竟：`response-cork`（cork 真缓冲 + socket 镜像计数 + end 排空三件，流控手术另单元）。

### 4.193 G11 收尾轮：cork 面双 CRLF + uncaught 吞错 + 小面四件（2026-09-23，plan3 G11）

- 坑一（chunk 帧双 CRLF，整条流错位）：`__frame` 粒度对齐真机 `_send` 链时把尺寸行 hex 写成 `len + "\r\n"`、又独立发一个 `__CRLF`——每 chunk 尺寸行后双 CRLF，客户端 chunked 解析整体错位（首 chunk 吞字节、后续 size 行全歪→ 400/静默 hang；
  同会话回环全灭而跨进程双向皆绿——真实 node 客户端当裁判才定位到"流错位"而非"泵停摆"）。真机 `_send` 链：hex **不含 CRLF**（`_send(len)` 后 `_send(crlf_buf)` 独立一发）。
  教训：对齐"写调用粒度"时逐 send 核对字节内容，CRLF 属于哪一发要看真机 crlf_buf 的使用点。
- 坑二（catch 一刀切吞用户 throw）：`__sockOnData` 的 catch 把一切异常 `destroy(e)`——用户 response 监听里的 throw 被吞成 req 销毁，uncaught永不触发（uncaught-from-request-callback 套件 hang）；
  服务端 `emit("request")` 同病（handler throw 进 400 通道静默 hang，§4.189 deferred）。
  修法：解析错带旗（`__hpe` 加 `__parseErr`）走原 destroy 通道，用户 throw `process.nextTick(() => { throw e; })` 重抛（tick 回调带 uncaught 路由，§4.188 坑八同源）。
  推广：吞错 catch 必须区分"实现内部错"与"用户代码异常"，后者永远上抛——node 语义解析错走返回值通道、用户 throw 原样冒泡。
- 坑三（options 原型链陷阱）：套件在 `Object.prototype` 装 getter 陷阱，我们的 ClientRequest 直读用户 options 走原型链即触发；真机构造器入口 `ObjectAssign({__proto__: null}, input, options)` 先拷 null-proto 再读。修法照抄（`Object.assign({ __proto__: null }, options)`——own 枚举拷贝不触发原型 getter）。推广：对接外部 options 的 API，读属性前先 null-proto拷贝隔离。
- 坑四（同一解析器两种超限口径）：客户端响应头超限 = **静默截断**（node parserOnHeaders "stop collecting"，maxHeaderPairs 上限后不再收集、响应照常完成）；服务端请求超限 = 抛 HPE_HEADER_OVERFLOW 走 clientError。`__parseHead` 加 `__trunc` 旗按调用方分流。教训：max-headers-count 套件的 expected=20 就是截断口径的铁证，"抛错"与"截断"两套件各钉一面。
- 坑五（sweep alarm 量纲误判"真机自挂"）：`server-request-timeout-keepalive`套件 requestTimeout 5s × 1.5 defer，全程 ~18s——15s sweep alarm 双边掐死被记成"真机自挂（node 142）"（§4.186 的误判）；25s alarm 实证双边绿。推广：TIMEOUT 分类前先算套件自身时长（platformTimeout × 倍数 + 余量），alarm 必须 ≥ 套件最坏时长；"真机自挂"结论必须换 alarm 档复核。
- 本轮转 SAME0：response-cork / response-drain-cork / outgoing-end-cork（cork 面三件：机构 cork 滞留 + socket 镜像计数 + end 强制全开 + 写粒度对齐）/ uncaught-from-request-callback / test-http-1.0（_send 面 + sendDate=false 不补 Date）/ null-prototype-options / max-headers-count（客户端截断）/ response-multi-content-length（客户端拒多 CL，HPE_UNEXPECTED_CONTENT_LENGTH 'Duplicate Content-Length'）。
- 黑盒：`phase11_http_cork_faces`（镜像/背压/粒度/end 全开 10 断言）+ `phase11_http_uncaught_throws`（cli/srv 双向 throw 原文到 uncaught）。

### 4.194 基建轮：socket.push + 服务端解析错 + 写侧流式化（2026-09-23，plan3 基建）

- 坑一（`expectsError` 无 mustCall 即空转）：header-overflow/destroy-socket系套件的 socket-error 断言用裸 `expectsError`（不查调用次数）——实现缺失时恒假绿，输出对、rc=0。本轮加 socket-error 递送后 validator 才真跑，首跑即钉住三件（code/bytesParsed/rawPacket）。推广：对拍"绿"先问断言是否执行过——无调用计数的错误断言一律视为假绿嫌疑，宿主侧以"validator 实跑"为收敛标准（§4.126 ③的 expectsError 版）。
- 坑二（rawPacket=当片非累计）：'FOO / HTTP/1.1' 整头与 '123…' 首字节 '1'的 rawPacket 矛盾——前者整头、后者 1 字节——真相是 llhttp rawPacket=触发本次解析的数据片（multiple-client-error 的 unshift 把 '1' 独立成片）。
  修法：`__lastPkt` 存根（空 re-feed 不覆盖）+ 缺席补齐；bytesParsed=片内偏移（方法分叉点/全消费=片长；
  TE/CL 重门未被点名，记档近似取头长）。另：补齐须在 clientError emit **之前**（有监听分支直接返回，事后补即漏）。
- 坑三（方法匹配是候选集不是 token 表）：token 门把 'FOO' 当合法（全大写 token），llhttp 却报 HPE@1——方法是已知表增量匹配（首字节 A-Z + 逐字节前缀候选，分叉即偏移；空格终结未知词即词长，CR 终结同；'GE' 悬置等数据）。7 探针钉住（FOO→1/Oopsie→1/GETX→3/老小写→0/'*'→0/GE 悬置超时）。连带修好 socket-error-listeners 的 hang（'*' 旧口径合法致 clientError 永不发）。
- 坑四（数组头在存不在发）：double-CL 套件 wire 单行 '1,2'——`__emitOne`早就会数组分行，真凶是 `__lowerHeaders` 存值 `String(v)` 预洗。修法只改存（数组原样），校验仍按合并串（同结果），writeInformation 模板 join 恒等零回归。推广：发散路径（存→发）断链时先查存，不动发。
- 坑五（流 buffering 吞同步计数）：`res.write('asd')` 后同步读 length 仍是旧值——第二个 _write 还没跑（流一次只派发一个 _write，次块等 microtask）。修法：记账上移到 write/end 包装层（同步），_write 内去重（end 块经内部_write 直调不走 write 包装，由 end 包装层补计）。教训：凡"同步读"口径（writableLength），计数点必须与用户调用同 tick，流派发节奏不可信。
- 坑六（write 覆写的 socket-null 早拒）：管线队列上线后 `write` 覆写的 `__sock===null→false` 把入列写全拒（`while(write)` 零块即停，needDrain 永不立）。修法：null 分两种——入列（__queued）走流机构→_write park，独立构造维持旧 false。§4.192"删遮蔽方法必 grep 体内标识符"姊妹篇：改守卫先数清有几种 null（独立/入列/已销毁三种）。
- 坑七（CL 快捷与同步头渲染互斥）：early-render 头即杀 end-only-data 的 CL 快捷（`!__headSent` 门）。修法：dry-run 计数（快照→渲染→取值→还原，Date 同长恒等）+ 落盘递减 + _final 兜底清零——渲染时机零改动，只加记账。真值覆盖（CL 快捷）天然对齐（终态清零），预测偏差不出终态。
- 本轮转 SAME0（13 件）：read-in-error/header-overflow（push 面）/server-client-error/invalid-te/double-content-length/server-reject-chunked-with-content-length/socket-error-listeners（HPE 面）/outgoing-properties（131/139 记账）/outgoing-drain-writable-length（队列+drain）/附带 1.0-keep-alive/pipeline-flood/pipeline-outgoing-destroy（eager 队列连带）/catch-uncaughtexception（destroy(e) 递送 uncaught 通道连带）。
  黑盒 `phase11_http_socket_push_and_server_parse_errors` + `phase11_http_outgoing_writable_length_faces` + `phase11_http_pipelined_outgoing_queue_faces`。
  残：reuse-drained（process.report 缺失，另域）/execPath spawn ~18（待拍板）/ parser 内省 ~4（记档偏离）。

### 4.195 socket 写错透传四坑：异步确认计数 + 递送顺序 + end 取已记错（2026-09-23，剩余轮 outgoing 面）

- 坑一（mock 确认异步一跳）：socket 写错收集初版同步读 box——mock Duplex的写确认经微任务到（自家 _write 异步一跳），同步读恒空，排空递送 null，随后错才到（writable-finished 套件 `null !== {}` 顽固）。修法：pend 计数+ `__afterSockFlush` 等全部 ack（同步全回即下一拍，异步 mock 等确认；确认永不到即 socket 违约，node 同款挂起）。
- 坑二（显式递送 + 带 err destroy 双发 error）：`_write/_final` 显式 cb(err)后再 `destroy(err)`——destroy 无 errored 去重（destroy.rs 实锤：有 err 即排 emitError），mustCall(1) 形得 2 次。
  修法：显式递送（同一 err 对象，strictEqual 同一性）→ `destroy()` 收尾（不带 err：只做 close + socket清理；
  `__failFlush` 注）。顺序再有一层：递送 → destroy 置位 → 终结回调（destroyed 门禁二次 emit；
  终结回调在 destroyed 流上仍触发用户 endCb，仅压住 emit——onFinish 无 destroyed 门，实证）。
- 坑三（end 短路恒 STREAM_DESTROYED）：`end()` 在已销毁流上恒回 STREAM_DESTROYED，end-again 形（失败后再次 end）与真机（回已记错）不符。修法：`state.errored ?? STREAM_DESTROYED`（writable_flow endWritable；其余两处同形早已如此）。
- 坑四（trailer/基类门三件）：OM 基类缺 `setHeader`（子类各有，基类直调即 not a function——proto 套件现形）；trailer 名/值标签与 header 不同（"Trailer name"/"trailer content"，`ERR_INVALID_CHAR` 加 label 次参，旧单参调用零改）；`write` 覆写的子类分流误伤外来 this（`constructor.name !== OM` 即走 super——fake-this 形须按 `instanceof` 判 standalone 先验块形态）。
- 附带翻转（§4.65）：round1 p11 旧静默缓冲系伪语义（真机 proto 抛 NOT_IMPLEMENTED）——改 stub 后断言 + OM standalone `writableLength`走 outputSize。
- 本轮转 SAME0：outgoing-proto/outgoing-buffer（上轮预建，本轮收尾）/outgoing-writableFinished/outgoing-finished（res close-on-finish + willEmitClose OM 臂，见 §4.196）/outgoing-destroyed（silent-destroy + errored-undefined，flaky hang 见 §4.197）。

### 4.196 TLS 无 connecting 面 + 销毁响应禁回池（2026-09-23，剩余轮）

- 坑一（TLS 误判已连通）：mock 识别用 `connecting !== true`——TLS socket根本无 `connecting` 面（tls.rs 实锤零命中），新建 TLS 全走"已连通"分支提前 flush，握手未成就发明文头，首请求即 hangup（https 全域红）。修法：判据加原生柄（`__id` 缺席才是 mock；真新建含 TLS 皆有 __id，照旧等事件）。
- 坑二（destroy 的 res 回池即 hang）：客户端 `res.destroy()` 后 socket 按正常收齐回池（ESTABLISHED 常驻、unref 不靠）、服务端永不见 FIN——pipe形（outgoing-destroyed block3）服务端零感知挂死。真机 destroy 即销 socket（不可复用）。修法：`__finishSock` 首门——res destroyed 即销毁不回池（+ `agent.__noteClosed` 记账）；正常 end+close 才可池化（keepalive复用零回归）。
- 推广为铁律：凡"已连通"判定，先问"哪些真 socket 缺该面"（TLS/UDS/自定义底座逐个核）；凡"复用/回池"路径，先问"销毁态到这了吗"（destroyed 进池即泄漏 + 对端挂死）。

### 4.197 同一失败的二次投递：socket 迟到错 + flaky 定级（2026-09-23，剩余轮）

- 症状：mock socket 写失败同时走 cap（递送写回调）与 'error' 事件（兜底）——res 已因本次失败销毁且记错后，迟到的 socket 错又进兜底，`throw e` 即 uncaught（writableFinished block3 `forced write failure` 实录）。
- 修法：`__httpSockOnError` 首门——res destroyed 且 errored 有值即吞（已走 res 通道递送）；无错销毁仍 throw（升级/空闲形旧口径）。
- flaky 定级法（本轮沉淀）：单块过 + 整文件挂 ≠ 块间污染——先量化（单块×3/整文件×3），再 instrument（分块标记 + lsof 看残留对端 + FIN 流向），最后 master 基线裁决（stash + 同条件跑）。本轮 destroyed 整文件挂即按此法定为 block3 管道收尾缺口（§4.196 坑二），非调度 flake。
- 附带卫生：开工先 `git status`（本轮工作区有前人未提交的 proto/buffer半成品，交接未提及——`git diff` 认领归属后再动手）；探针脚本放 `/tmp/wjs-*` 用完即清（`__wjs_node_compat` 同族纪律）。

### 4.198 readable closed 随 close 发射翻位（2026-09-23，剩余轮）

- 症状：`test-http-client-incomingmessage-destroy` 首跑即挂——`res.destroy(err)`后同步读 `res.closed` 得 true（套件 26 行要 false，close 监听 29 行要 true），且随后 `server.close()` 挂死（开着的 res 致 in-flight unref 续命，另案）。
- 根因：`destroy.rs onDestroy` 给读写双侧同步置 `kClosed`——node 口径读写有别（writable 同步翻，readable 随 close 发射翻；套件三段即铁证）。
- 修法：`onDestroy` 只置 w 侧，r 侧移到 `emitCloseNT`（bit 先置后 emit，监听内恒 true；emitClose=false 形同样翻位，只是无事件）。
- 推广为铁律：凡"销毁后同步读"口径（closed/destroyed/errored），读写双侧分开对真机——读写的销毁时序在 node 从来不是对称的。

### 4.199 incomingmessage-destroy 双件 + error/close 分排（2026-09-23，剩余轮）

- 症状：`test-http-client/server-incomingmessage-destroy.js` 双 TIMEOUT——服务端 `req.destroy(err)` 外发 req 'error'（uncaught mustNotCall 形本应静默），且无监听 error 抛 uncaught 会吞掉同 tick 的 close（客户端形挂死）。
- 根因二连：① IncomingMessage 无自有 `_destroy`，基类 destroy(err) 必排 error 发射；② `onDestroy` 把 error/close 嵌套排（`emitErrorCloseNT`）——无监听 error 的 throw 直接吞掉 close。
- 修法：① `IncomingMessage._destroy`：错误经 socket 递（服务端级联杀连接→客户端 hangup ECONNRESET；客户端经请求 error 照常 uncaught）、本体 `cb()`吞错；无错仅未收齐（`readableEnded !== true`）才销 socket——正常收齐后的自动 destroy 不碰（keep-alive 复用/响应在途；loopback 实锤杀了即 hangup）。② `onDestroy` 改 error/close 分开排（各下一 tick；uncaught 抛错不再吞 close）。
- 附带：`writeContinue(cb)` 旧实现吞回调（write-callbacks 套件挂死）——补落盘后 microtask 触发；standalone `write` 已销毁形回 ERR_STREAM_DESTROYED 进回调（outgoing-destroy 套件，不同步抛）。
- 推广为铁律：凡"错误 + 终结"双事件设计，排期必须独立（嵌套排即谋杀)—— uncaught 的 throw 是控制流，会吞掉同回调内的一切后继。

### 4.200 abort 与 destroy 的错误分流 + 孤儿连接守卫（2026-09-24，剩余轮）

- 症状：`test-http-abort-before-end.js` 报 error（mustNotCall）——`req.abort()`后 `req.end()` 走出 ECONNRESET；堆栈 `destroy ← abort`，错在 destroy 内合成。
- 根因：`ClientRequest.destroy` 无响应即合成 ECONNRESET（abort-destroy 套件要的），abort() 经同一 destroy 即误合成。abort-destroy 套件三段即铁证：abort 中途无错 / destroy 中途（有响应）无错 / destroy 事前才 ECONNRESET。
- 修法：合成门加 `__aborted !== true`（abort 恒先置旗），回池门同加（abort 不回池）；connect/secureConnect 到达发现已销毁即杀孤儿 socket不刷盘（abort 后连接才到形；否则半截请求 RST 回 ECONNRESET 且 server零收到被破）。
- 附带同批绿：client-abort3（同源 throw）。
- 推广为铁律：凡 destroy 内合成错误的面，必须区分调用源（abort/signal/用户 destroy/内部错误销毁）——合成是 destroy 的语义，不是 abort 的。

### 4.201 req.signal 早夭 + res-close 排序重构（2026-09-24，剩余轮）

- 症状三连：`request-signal` 要 server req.signal（AbortSignal，早夭 abort、正常永不）；`req-res-close` 要 res-close 在 req-close 前 + 双 destroyed；`content-length` 的 end-with-data 走 chunked（应 CL:11）。
- 根因：① signal 面从零开始（惰性 AbortController + 早夭标记滞后补）；socket-close 时 res 未完（或 req 未完）即 abort（正常收齐看 res end，真机探针钉住）；
  ② res-close 排序：node 是 finish→destroy→close 且 req end 被 res 收尾唤醒（暂停流无 end；
  真机 finish→close→end→close 序实锤）——本仓 res 从不 destroy + req 永暂停；③ `_write` holdback 暂存使 `_final`滞后，timer 先刷不认 CL 捷径（end 侧已落 `__contentLength`）。
- 修法：① signal getter + `__abortReq`/socket-close 早夭 abort（含 `__signalAborted` 滞后）；② res finish-hook 手动 destroy（流机构 auto在 finish 链中重入即 b5 hang；microtask 排，finish 时 destroyed 仍 false）+ `_destroy` 干净 detach/显式杀分流 + `_destroy` 内 resume req；③ `__tryFlush` 认已落定 `__contentLength`。
- 推广为铁律：流机构 autoDestroy 的 destroy 时机不可控（finish 链中重入）——要时序即手动排；"暂停流无 end"是天然门控，唤醒点与收尾点同放。

### 4.202 对拍提速三件套（2026-09-24，效率专项·①已落地②③待做）

- 背景：http 剩余 32 件（9 hang + 23 文案），一周实测约三成时间花在机械活上。本条是施工计划，不是复盘——三件全落地前，对拍效率未达最优，不许再称"已最优"。
- ① 断言 mapper 报实际值：✅ 2026-09-23 落地（`tests/node/helpers.rs` `run_suite_mapped` + 包装壳模板）。
  用法：`WJS_MAP_SUITE=<套件名> cargo test --test node phase_mapper_locate_suite-- --ignored --nocapture`（套件名按 /tmp/wjs-node-test/test/parallel 解析；
  定位器非闸门，红绿不进门；门禁两件 `phase_mapper_locates_async_callsite`/`phase_mapper_passthrough_on_success`）。
  实现：包装壳 require 套件 + `uncaughtException` 监听，AssertionError 自带 `actual/expected/operator/code`（JSON 可序列化，无需解析消息文本），栈里首个套件文件帧即真实调用点，一次输出三行：`[mapper-actual]`（JSON.stringify 截 200 字）/`[mapper-callsite]`（帧 + 折算物理行 + `>>` 源行 ±2 节选）/`[mapper-expected]`（期望值）。
  验收：raw-headers → 物理 110 rawHeaders 断言、mutable-headers → 物理 187数组头 join，均一击定位零二分。
  实测钉住三件引擎事实：（a）无壳输出的位置是 assert SOURCE 内部行号（`node:assert:9:5`/prelude `424:53`）安着套件文件名——包装位置是骗的，真实调用点只在 `.stack` 里；
  （b）栈帧行号 = 物理行 + CJS 包装前奏行数（`.js` 恒 +1，6 处实验一致；`.mjs` 无包装记 0），mapper 按此折算；
  （c）`unhandledRejection` 监听拦不到（引擎自有收割先走，§4.137 路径），`process.on('exit')` fatal 路径不触发——rejection 形失败回落引擎默认输出，mapper 只覆盖 uncaught 形；
  TIMEOUT 件由 helper 20s 看门兜住不挂 cargo。
- ② flake 先分类再动手：✅ 2026-09-25 落地（`scripts/flake-classify.py`）——新红先自动跑 3 遍（整文件×3 + 手抽单块 repro×N），GREEN / FLAKY(k/N) / RED-DETERMINISTIC / NODE-FLAKY 四分流——flaky 走定级法（§4.197），必现才 instrument。
  禁把 flake 当回归深挖（destroyed 整文件挂误判块间污染，实为 block3 管道缺口——§4.196 坑二教训）。
  dogfood 首件：dump-req-when-res-ends判 FLAKY(0,142,142，挂死型而非红绿互跳)。
  跑分纪律沿用 §4.145（exec or die + glob 绝对路径）；与常驻 sweep 并跑时 `--thread-id/--port-base` 错开（sweep 默认 3599/29999，classify 默认 3601/29999）。
- ③ sweep 常驻后台：✅ 2026-09-25 落地（`scripts/sweep-bg.py`）——双 fork脱离会话后台直跑、`status/wait/tail/stop` 轮询、工件落 `~/.wjs-sweep/<tag>/`（status.json/results.log/debug/，家目录 §4.144）；
  失败行带 stderr 首行 + 整份落 debug/（本次排障一击即中）；killpg 连 suite 子进程一起收；全量禁套 alarm（§4.143），单套件 subprocess timeout。
  与 sweep4 家族 409 件基线可比：`.js`/`.mjs` 都进——前缀过滤只认 `.js`会静默丢 6 件 mjs（口径先对齐再比较）。
- 推广为铁律：机械开销（定位/分类/等数）用工具换，推理开销（hang 根因）用人换——前者不投半天，后者永远被前者拖慢；工具先行，啃数随后。

### 4.203 请求级 createConnection 错误被吞：oncreate 只认 socket 不认 err（2026-09-23，G11 TIMEOUT 轮）

- 症状：`test-http-createConnection.js` TIMEOUT 且零输出。套件六块插桩定位——四个成功块全过（SRV-HIT 各一），async 错误块（`createConnectionAsyncError`：`process.nextTick(cb, new Error('async'))`）永不决议；sync throw 块（E1）反而正常 reject。
- 根因：请求级 createConnection 的 `oncreate = (err, s) => { settled = true; if (s) this.__attach(s, false); }`只认 socket、完全无视 err——async cb 错被吞，请求既不挂 socket 也不发 error，promise 永悬（挂死）；sync throw 形靠"异常穿透构造器"侥幸走到 assert.rejects（真机是 try/catch 收进 emitErrorEvent，永不同步抛——机制不同结果碰巧同）。
- 修法（真机 `_http_client.js` 591-607 行逐字）：oncreate err 臂 `process.nextTick(() => this.emit("error", err))`——无监听经 EE rethrow 原错落 uncaughtException（真机 emit('error') 无监听语义，本仓 EE 已逐字）；sync throw `try/catch → oncreate(err)` 收进同路；`settled` 门前置防双投（`createConnectionBoth1/2` 的 cb+return 双形，node 用 `once()` 同义）。
- 复现：套件修前 rc=142 修后 0；黑盒 `tests/node/http/parity.rs::phase11_http_create_connection_error_routing`（async cb 错 / sync throw 不同步穿出 / 无监听 uncaught 原文 三形）。
- 教训：① cb(err, s) 双参回调的 err 臂"暂时用不上"也必须路由——`if (s)`单臂回调是 hang 制造机；套件四个成功块全绿掩盖了错误块，块标记插桩十分钟定位（§4.202-① mapper 覆盖 uncaught 形，TIMEOUT 件仍走插桩）。② "结果碰巧对"（sync throw 穿透）不等于"机制对"——换一个调用形状（async cb）即现形；对真机要对机制，不只对结果。

### 4.204 http 欠账清扫轮七坑（2026-09-23，plan3 G11 sweep8 红件簇）

- 坑一（重复头 join 缺省反转）：真机 `_http_incoming _addHeaderLine` 是**表驱动**——joinable 表 + 未知头缺省恒 `', '` 合并，19 头单值表才首个赢（matchKnownFields 无前缀名单：age/host/from/etag/referer/expires/server/location/user-agent/content-type/max-forwards/authorization/last-modified/content-length/if-modified-since/proxy-authorization/if-unmodified-since/content-encoding/x-forwarded-host），cookie `'; '`、set-cookie 数组不受旗控；
  joinDuplicateHeaders 旗**只压单值表**。旧"未知头首个赢"系 authorization单值头行为被错误推广——对真机要对机制不只对结果（§4.203 教训实例）。
- 坑二（查询面只见用户头）：node 查询面（getHeader/hasHeader/getHeaderNames/getHeaders/getRawHeaderNames）读 `[kOutHeaders]` 用户头——自动头（Date/Connection/Keep-Alive/自动 CL/TE）对查询不可见。修法 `__isAutoKey` 谓词（date/connection/keep-alive 旗 + CL/TE 无用户拼写名），内部状态机读 `__headers` 不受影响。
- 坑三（GET+用户 TE 裸体）：请求侧 `__sendHead` 缺真机 _storeHeader 的 TE值扫描——用户 TE 含 chunked（整词）须置 `__chunked`，否则体裸发 + 终结块照发（双机构打架），服务端 'bad chunked body' → 挂死。
- 坑四（TE 整词 + 冒号空格，走私向量）：① chunked 判定必须逗号切分整词全等——`chunkedchunked` 不得命中；TE 在场非 chunked = teInvalid 分型：请求派发（handler ×1）但 data/end 永不发，体字节到达即 HPE → 400 + close。② strict 模式拒收冒号前空格（RFC7230 §3.2.4；lenient 放行）。
- 坑五（parser 全局池）：真机 parser 来自 `_http_common` 全局 freelist（回收复用），**不是每连接**更不是每请求——parser-free 套件 maxSockets=1串行 100 请求恒同一对象；free（res end）字段置空回池，attach 回填 onIncoming/joinDuplicateHeaders。
- 坑六（res 侧 timeout 桥 × 监听数契约）：socket 超时双路——req 侧走 req.setTimeout 的 timeoutCb 独立通路；
  res 侧 responseOnTimeout 打 **res**（真机 1055 行）。**恒挂会多占 EE 监听数**（set-timeout-after-end 套件 `listenerCount('timeout')===1` 钉住——真机 net 单例不是 EE 监听，我们的是）；
  res.setTimeout 后置形由 IM.setTimeout 桥自武装（complete 哑/close 摘）。stash 基线定级（§4.197）实锤回归后回修——改超时路由必跑 timeout 家族全量。
- 坑七（出局归类纪律）：红件先定性再动手——`--expose-gc/--expose-internals` flag 门控 + internals 模块（reused-gc/leaky/keepalive-req-gc）、process.report 另域（reuse-drained）出局；domain 异步路由（§1 记档）、Atomics.wait 引擎面（ka-race）书面偏离；sweep 红绿互跳件先重扫再定级（chunk-extensions-limit 两次扫描间自绿 = flake 族）。
- 复现/回归：本轮 15 件转绿（multiheaders×5/mutable-headers/abort-keep-alive-destroy-res/override-global-agent/parser-free/smuggling/te-repeated/write-information/optimize-empty/chunk-extensions-limit/response-timeout/dump-req-when-res-ends），timeout 家族 6 件守卫零回归；node 域 285 绿 +冒烟 5/5 ×4 轮。
- 追补（2026-09-25 二批五件）：① server 兜底 `Server[nodejs.rejection]`（_http_server 716 行逐字：未发头清头+500 / 已发头 destroy）+ TLSSocket双路 `_secureEstablished` + ServerResponse.setTimeout + IM 桥 timeout 带 socket 实参 + server 连接级**无条件**三路转发（req 未完结/res/server）—— capture-rejections/url.parse-https.request/set-timeout-server 前四块转绿；
  ② HPE 门序：TE+CL/重复 CL 门必须**先于** requireHost（llhttp 解析期校验先行——TE+CL 缺 Host 形 clientError 先到且无 400 直写）；
  ③ 头串 latin1 编码（逐 charCode 低 8 位）——`'binary'` 形非 ASCII 头值按 latin1 字节上网（__storeHeader/writeInformation/__sendHead 三处）；
  ④ **原型链桥接禁用**：`setPrototypeOf(ServerResponse.prototype,OutgoingMessage.prototype)` 会改道 super.write/end/cork/destroy 全链（cork 面实锤 writableCorked 错 0）——身份语义改走 `OutgoingMessage[Symbol.hasInstance]` 品牌判定（`Object.defineProperty`——`Symbol.hasInstance` 只读直赋即 throw）+ `ServerResponse.__omBrand`。
- 残件定性（第三批）：outgoing-message-capture-rejection（res 写错 capture通路）、should-keep-alive（1.0/1.1 × Connection 判定矩阵）、no-read-no-dump（socket 'pause' 事件 + POST 背压 + 管线，§4.148 throttle infra 族）。
  set-timeout-server 末段 exit-hold（paused client FIN 的 EOF 急切检测/readStop——真机 readStop 后循环放行，本仓按需读永不见 EOF）归 G6 infra残件族。
  本批 5 件转绿（capture-rejections/url.parse-https.request/reject-chunked/non-utf8-header + set-timeout-server 前四块），cork 家族 4 件 + timeout 家族 6 件守卫，node 域全绿 + 冒烟 5/5 ×3 轮。

### 4.205 统一 runner 给 node 也带 `--run`：假红全表 + 过滤丢件（2026-09-25，②③工具轮）

- 症状一（node 假红全表）：smoke sweep 9/9 全 DIFF 且 `wjs=0 node=1`，2.7 秒跑完 18 个进程——真套件不可能这么快。
  node stderr 一行：`Can't find package.json for directory /private/tmp/.../parallel`。
  根因：统一 `run_one(binary, path)` 给两个二进制都硬编码 `--run`——node 22+的 `--run` 是"跑 package.json scripts"命令（node --run <name>），套件目录无 package.json 即报此错 rc=1。
  wjs 侧 `--run` 是本仓动作 flag，同名不同义。
- 症状二（409 基线缩水）：前缀过滤只认 `.endswith(".js")`，静默丢 6 件 `test-http-*.mjs`——与 sweep4 家族 409 件基线不可比。
- 排障路径（值得记：三类对照全做完才定位）：① 环境二分（env -i 最小环境 +逐变量加回）排除环境；② python 前台复刻（同 env 同 cwd 同 capture）绿；③ 内联双 fork 复刻绿——最后靠"失败行落 stderr 首行"一击命中。教训：**失败行的 stderr 是最短路径，走复制粘贴式复刻对照是弯路**——工具先行落 stderr 捕获，比人肉二分快一个量级。
- 附带实测：上一轮 sweep 挂死留下的孤儿 winterjs（`--expose-gc`/`--expose-internals`套件滞留数小时）会占端口/状态污染基线——两个工具 start 前均 pgrep 预警。
- 修法：`run_one(..., prefix)`——wjs 传 `("--run",)`，node 传 `()`；文件过滤 `.js`/`.mjs` 双认。修后 smoke6 upgrade 9/9 SAME0 与在册记录一致。
- 推广为铁律：① 包装两个相似 CLI 的统一 runner，实参差异必须参数化而非取交集默认——同名 flag 语义不同（本仓 `--run` vs node `--run`）是重灾区；② 新跑分工具首跑必抽查 2-3 件已知绿套件对表，elapsed 异常短（<200ms/件）即"根本没跑起来"的信号（§4.145"全绿得可疑"的姊妹篇：全红得可疑同理）。

