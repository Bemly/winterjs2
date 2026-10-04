# 踩坑分卷3（4.125–4.169）

> 本卷为 `docs/pitfalls.md`（主索引）分卷之一，只收正文；查阅先看主索引，编号 `§4.N` 全仓唯一。

### 4.125 net 可观测表面五连坑（2026-09-17，10f net 五轮）

- 坑一（`JSON.stringify` 吞 undefined）：`JSON.stringify([true,5,undefined])` 呈
  `[true,5,null]`——探针误读为"真机转发 null"，险些把实现写错。修法：形态断言一律
  `typeof` 逐项，禁 `JSON.stringify` 看 undefined（`wjs-10f-par.py` 的 W 行同理，
  首行截断只看形状不看空位）。
- 坑二（`EBADF` 含 `BAD`）：黑盒哨兵 `assert!(!out.contains("BAD"))` 被
  `w1 write EBADF` 误触发（§4.42 自摆乌龙）。修法：哨兵一律带分隔符（`BAD `）。
- 坑三（柄关 vs 柄空是两种状态）：`_handle.close()` 后写 → `write EBADF`，
  `_handle = null` 后写 → `ERR_SOCKET_CLOSED`——同为"柄没了"，错误各异
  （真机 26 实测）。修法：`close()` 只标 `__handleClosed` + 延迟 destroy
  （同步 destroy 会提前置空，两态坍缩为一）；write 内先判空柄（CLOSED）再判
  关柄（EBADF），destroyed 面保持 `__writeErr` 不动。
- 坑四（销后噪声必吞）：destroy 后 Rust 侧 teardown 竞速错（RST/EOF，
  `UNKNOWN` 整形）会以 error 事件迟到——双块并发下 1/3 flaky
  （单块 8/8 绿，§4.122"单独过并行挂"姊妹篇）。修法：`__ev error` 见
  `destroyed` 即吞派发（`__hadError` 照记，close(true) 口径不变）；
  真机同为销后不派发（stream `errorOrDestroy` 语义）。
- 坑五（真机 fresh 柄即 null）：`new net.Socket()._handle === null`
  （预连接打补丁即 TypeError）——本仓构造期建桩是偏差。修法：_handle 只在
  连接存活期非空（`__realConnect/__attach*` 建、`destroy/__ev-close` 置空），
  无柄期的 setNoDelay/setKeepAlive 只缓存不转发。
- 复现：`tests/node/net.rs::phase10f_net_socket_surface`（坑三/四修前 flaky 红）；
  探针 `/tmp/wjs-nv-probe/p2.mjs`（双块并发，修前半数多一条 UNKNOWN error）。
- 附带真机口径（同轮实测）：setKeepAlive ms→s 下取整、缺省位转发 undefined、
  四元组全同跳过；TOS 三文案逐字；autoSelectFamily 默认 true/timeout 500；
  server keepAlive 默认 false/0（`src/builtins/node/net.rs`）。

### 4.126 重负载验证禁令：禁并行压力循环与重复全量子集（2026-09-17，机器约束）

- 背景：为抓 pingpong 并行偶发静默死亡（exit -10），用 10-worker 对拍子集循环
  15+ 遍（每遍 ~200 进程启动）+ 6~7 次全量构建，单轮烧掉 200G+ 内存 IO——
  纯验证方法学开销，非代码泄漏（本轮改动全为纯 JS prelude 小字段），但机器扛不住。
- 铁律：验证只用轻量三档——单文件直跑、小域子集
  （`cargo test --test node net::`）、全量 `cargo test` 一次；对拍全量子集
  （157 文件 ×2）每轮至多跑一遍；**禁并行压力循环**（同文件 ×N 并发、
  后台负载围攻、repro 脚本反复跑、为攒统计样本重复子集）。
- 资源上限（2026-09-17 补）：单轮验证常驻内存不超过 **8G**、
  新增测试产物不超过 **16G**（含 `target/` 增量与 `/tmp` 探针文件）。
  超限先清场再跑：`rm -rf /tmp/wjs-*`、清各测试 `.tmp.*` 残留、
  `cargo clean -p winterjs`（慎用，全量重编更烧）；峰值以
  `/usr/bin/time -l` 的 maximum resident 为准记录。
- flaky 并行崩溃改走证据路线：抓到一份 `.ips` 栈即停手记档，不用"统计显著性"
  去换根因。本次即例：`ready` 发射与 -10 强相关（有 1/8、无 22/22）但机制未定，
  到此为止，不再追。
- 附带技术记档（本轮未闭环三件）：
  ① `emit("ready")`（connect → ready 真机序，已接受端不发）暂不发射——与并行
  静默死亡强相关，待引擎侧闭环；`ready-without-cb` 对拍计数不受影响（见③）。
  ② `.ips` 实证：`net dispatch → with_str_args → JS_CallFunctionValue →
  js::Call` 内 SIGBUS（KERN_PROTECTION_FAILURE），疑存活期/GC 时序旧患
  （§4.40 家族），与本轮纯 JS 改动无直接代码关联，另案。
  ③ 本仓不执行 common mustCall 的 exit 钩——只靠退出码时，"mustCall 未触发"
  类失败恒为假绿（ready-without-cb：不发射也 exit=0）；以后对拍报告注明此局限，
  关键语义必须另写内容断言黑盒（本轮 `phase10f_net_remote_surface` 即此路）。

### 4.127 zlib 轮子 framing 与收尾四坑（2026-09-17，10f zlib 首轮）

- 坑一（brotli 显式 `flush()` 多包 2 字节）：`CompressorWriter` 的 `flush()`
  只吐非终结同步块，终结靠 drop 的 FINISH——`write+flush+drop` 比 Node
  one-shot（单 FINISH）恒多头尾 framing（空输入 3B vs 1B、非空头尾各异、
  中段载荷逐字节一致）。修法：删显式 flush（`src/builtins/node/zlib.rs`
  `zlib_brotli_compress`），非空与真机逐字节一致才算对。
- 坑二（ruzstd 默认 `hash` 特性带 checksum）：`default = ["hash","std"]` 使每帧
  补 4 字节 content_checksum（空帧 13B），Node/libzstd 默认无校验（9B）。
  修法：`ruzstd = { default-features = false, features = ["std"] }`
  （`Cargo.toml`；解码侧有无校验自适应，既有往返不受影响）。
- 坑三（`Z_BLOCK` 是 5 不是 2）：flush kind 集按引擎各异——zlib {0,4,5} /
  brotli {0,1,2,3} / zstd {0,1,2}（真机逐项实测；2 是 `Z_SYNC_FLUSH`，
  别当 `Z_BLOCK`）。另补缺失的 `ZSTD_e_continue/flush/end` 常量（0/1/2）。
- 坑四（zlib `close()` 是撕毁不是 end）：真机 `write+close` 无 data/finish/
  end、只有 close+cb（待刷数据直接丢）。`end` 等效是伪语义。修法：
  close = 置旗 + 无错 destroy + cb 落 close（已销毁只等）；`reset()` 已关闭
  即 `ERR_INTERNAL_ASSERTION`（真机实测，非臆测）。
- 复现：`tests/node/zlib.rs::phase10f_zlib_stream_teardown`（7 套件附带修好；
  `test-zlib-flush` 需真增量状态机，整收架构下另案）。
- 推广为铁律：压缩轮子的"完成"语义（flush vs finish/drop）先对空输入逐字节，
  再对非空——空帧是 framing 的最小探针；改 framing 类输出前先 grep 自家黑盒
  有无精确字节断言（本轮全是相对/往返断言，故零回归）。
- 追补（flush-flags/close-after-write 同轮）：① `flush()` 方法 kind 集与构造
  `flush/finishFlush/fullFlush` 选项集是两套口径（方法 zlib {0,4,5}、选项恒
  0..5），抄串了即把合法的 `Z_BLOCK=5` 判死——选项/方法分开核对；
  ② `ERR_INVALID_ARG_TYPE` 的 property 形（`The "options.X" property …`）直接
  复用 errors 移植 helper，禁手写文案（`Received type string ('x')` 含引号，
  手写必错）。

### 4.128 child 同步族六坑（2026-09-17，10f child 首轮）

- 坑一（自举翻译）：`spawnSync(execPath, …)` 的子进程是 winterjs 自身，
  Node 形 argv（`-e` 脚本/裸文件/shell 串首 `"<execPath>" -e`/`$NODE` 形）
  进全 flag CLI 即死（input/timeout/maxbuf 全灭于此，与实现质量无关）。
  修法：`__selfArgv`（数组形）+ `__selfCmd`（shell 串首形）两道映射到
  `--eval`/`--run`；他家二进制（`cat` 等）原样透传。禁给 CLI 加 `-e` 别名
  （全 flag 铁律 §0.8）。
- 坑二（三处缺省各不同）：spawnSync/execSync/execFileSync 缺省 **Buffer**，
  exec **异步**缺省 utf8——实测与文档字面（utf8）相反。修法：只改同步族；
  动 async 即把 `phase10f_exec_live_handle` 打红（本轮亲测）。旧测试两处
  `.trim()` 伪语义同步翻转（§4.65）。
- 坑三（同步等待管道死锁）：等退出（try_wait 轮询）后才读输出 = 1MB+ 输出
  永挂（子撑满 64K 管阻塞、父永不见退出）。修法：take 出 pipe 即起读出线程，
  等与读并发（`run_command`）。
- 坑四（killSignal 两层）：错型先行 ARG_TYPE，查表落空才 UNKNOWN_SIGNAL
  （初版合一即挂 validation-errors，插桩二分半天才定位到 killsig 段——
  "311:53" 系固定伪位置，禁按行号找断言，改四分文件）。
- 坑五（视图输入）：`new Uint8Array(u16view)` 按元素拷（长度减半+内容错位），
  须 `new Uint8Array(buf, byteOffset, byteLength)`（§4.124 姊妹篇；
  DataView/多字节视图全中）。
- 坑六（`args=null` 吞 opts）：`spawnSync(file, null, opts)` 的重载分流把
  null 当 options 覆盖——四处（sync/async/execFile）同修；另 `error.spawnargs`
  为参数数组（非 undefined）、`syscall` 带命令、起 spaw 失败 `pid` 为 0、
  缺省 SIGTERM 阻塞 wait（旧 SIGKILL/直杀两处伪语义已翻转）。
- 复现：`tests/node/child.rs::phase10f_child_sync_surface`；
  另 `process.argv0` 缺省（runtime 取真 argv[0]）+ `ChildProcess.emit`
  （四位分发）附带补齐（spawn-argv0/execfile 套件）。
- 推广为铁律：子进程族"子是谁"先分类——自身/真程序/shell 串三条路，
  翻译层只动自身路；"311:53" 式固定伪位置出现即改文件二分，不读行号。

### 4.129 crypto 首轮六坑（2026-09-17，10f crypto 首轮）

- 坑一（无 new 调用）：`crypto.Hash/Hmac/Cipheriv/DH` 系真机可无 new 调用，
  class 直出即 TypeError。修法：内类改名 `XImpl` + 同名 `function X(...args)`
  包装 + `prototype` 接线（`instanceof`/方法面不变）；Hash/Hmac 附 DEP0179/
  DEP0181 一次性警告（`deprecate()` once 语义，真机 `length/name` 伪装不抄）。
- 坑二（流式 end 丢 head）：Cipher 系 update 即增量吐块，end 只调 final 即少块
  （`wrong final block length`）。修法：end 拼 update-head + final-tail；
  Hash/Hmac 系 update 无输出，照旧只 final。
- 坑三（copy 无参回默认长）：XOF `copy()` 不带 options 即回默认输出长
  （非保留源长），真机实证。修法：copy 内 XOF 恒经新 native setter
  （`__wjs_crypto_hash_set_len`，覆盖/默认两路）。
- 坑四（超长 update）：输入 ≥2^31-1 真机抛无码 `Trying to add data in
  unsupported state`（nodejs/node#45757，精确边界 2147483646 ok/2147483647 挂）。
- 坑五（Received 裸形）：null/undefined 的 Received 无 type 前缀
  （`Received null/undefined`），`__needStr` 全族统一；Hmac 参数名是 "hmac"
  非 "algorithm"；`__outBuf` encoding 先 `String()` 显式转（用户 toString 抛错透传）。
- 坑六（旧断言翻转三件）：copy 保留源长/DH 数值形抛 NOT_SUPPORTED/shake 负长
  ARG_VALUE——三处旧黑盒全系伪语义，真机实证后翻转（§4.65/§4.82 三进宫）。
- 复现：`tests/node/crypto.rs::phase10f_crypto_round1_parity`（40 断言）；
  对拍 133 件 17→24 绿（`docs/bun-parity.md` crypto 节）。
- 推广为铁律：对拍修文案先探真机逐字（含大小写/标点/Received 形态），
  别凭记忆拼；"全绿旧测试"在语义升级面前先对真机再信。

### 4.130 crypto 二轮八坑（2026-09-17，10f crypto 二轮）

- 坑一（品牌≠原型）：`instanceof` 是纯原型检查，品牌另算——定制
  `hasInstance` 把两者绑死即坏 `spoofed instanceof === true` 断言。
  修法：`instanceof` 保持默认，内部 15 处改显式 `__isKeyObject`
  （WeakMap 品牌 + 手动走链，override 期免疫）；实例零自有属性经
  WeakMap + 原型访问器（131 处 `x.__foo` 文本零改动）。
- 坑二（Group 与实例同形不同命）：`getDiffieHellman` 回 Group Flavor
  （constructor 归 Group、setters 置 undefined），`createDiffieHellman`
  回 DH Flavor——同源不同命，探真机才分得清；另手抄素数丢字节
  即环错，改脚本精确替换。
- 坑三（`Signer::sign` 内 unwrap）：钥短于摘要即 panic→139（非 catchable）。
  修法：先验长度转可读错（`rsa_sign`，`PrivateKeyParts::size` 既有先例；
  `TrySigner` 在所钉版本不存在，别硬引）。
- 坑四（验签形态错须回 false）：空/短签名抛错即挂"mustSucceed 永不到"——
  ed/RSA/EC/DSA 四处预检回 false（真机逐项）；顺带 `sign/verify` 补 callback
  异步形（旧同步独占，回调永不到）。
- 坑五（混合 OAEP 几何）：种子长取 oaep 哈希长（非 mgf）——自交全绿掩盖，
  真机预言机（双候选 EM 投真机解密）一锤定音；双向交叉必做（§4.43 再进宫）。
- 坑六（默认 padding 按方向）：加解密 OAEP/签式 v1.5——统一默认 4 即错半边。
- 坑七（key 对象 encoding 双解码）：`{key: hex串, encoding}` 的 data 串同解码
  （真机实证，非文档臆测）；PEM 装甲裹 Buffer 同嗅探（fixtures 无编码读回）。
- 坑八（旧断言翻转四件）：copy 保留源长/asym throw 面/KeyObject 互传规则/
  export 无参——翻转前逐项真机实测（§4.65 四进宫）。
- 复现：`tests/node/crypto.rs::phase10f_crypto_round2_parity`（34 断言）；
  对拍 133 件 24→38 绿（`docs/bun-parity.md` crypto 节）；x448 无轮子卡
  `key-objects.js`（§0.5 待拍板）。
- 推广为铁律：到了"看起来都对但文件还红"时，停手写新探针——把**原文段**
  整体抽出来跑，交互污染（二分头段全绿、全文件红）只认整体复刻。

### 4.131 http 对拍首轮八坑（2026-09-18，10f http 首轮）

- 坑一（Agent 不是 class）：`http.Agent({...})` 无 new 裸调用是合法面（keepalive
  系四套件）——node 的 Agent 是老式函数（`if (!(this instanceof Agent)) return
  new Agent(options)`）。class 直出即 TypeError。修法：函数 + `__init` 方法 +
  `Object.setPrototypeOf` 双挂（Agent.prototype→EE.prototype、Agent→EE）；
  子类（http/https flavor）同形，函数体内调 `BaseAgent.prototype.__init.call`。
- 坑二（键位就是 API）：agent 池键不是 `host:port` 而是 getName 形
  `host:port:localAddress(:family)`——缺省位仍带分隔冒号（`localhost:80:`），
  测试以 `agent.getName({port})` 命中 `agent.sockets`。修法：acquire/release/
  addRequest 全走 getName；ClientRequest 缺省 host 同步改 `localhost`（否则键
  对不上）。https 的 getName 是 23 字段 TLS 会话键（lib/https.js 全字段追加），
  不是 http 形——照抄勿自创。
- 坑三（createConnection 双形态）：agent 级/request 级 createConnection 的实现
  可能同步回 socket、可能走 cb、可能都做——settled 旗 + `maybe` 回值双收口，
  防双 attach。
- 坑四（ServerResponse 构造首参是 req 形对象）：node 的
  `new ServerResponse(req)` 首参是请求信息（method/httpVersion…），不是 socket
  （standalone 套件）；把它当 socket 存即 assignSocket 首挂误拒
  ERR_HTTP_SOCKET_ASSIGNED。修法：构造器只认 `typeof sock.write === "function"`
  为 socket，其余存 req 形；双拒判定走独立 `__sockAssigned` 旗。
- 坑五（头+首块合并写）：node 的 `_send` 在头未发时把 header 与首块**合并为一次
  socket write**（standalone 套件断言单 chunk 以 body 结尾）——我们头/体分两次
  write 即红。修法：`_final` CL 快捷路径合并（`__headBytes()` 拆出，`__sendHead`
  保留独立写路径）；HEAD/204 等无体形态不合并。
- 坑六（408 计时器不因数据重置）：requestTimeout/headersTimeout 是**逐消息
  deadline**（消息起点到消息完结），部分数据到达**不重置**（interrupted/delayed
  系套件依赖）；ka 计时器只在响应完成后臂——体齐响应未完时挂 ka 会误杀在途
  响应（armIdleTimers 拆 withKa 两相）。
- 坑七（408/400 字节逐字对拍）：`HTTP/1.1 408 Request Timeout\r\nConnection:
  close\r\n\r\n` 与 400 同形精确断言；管线残渣（合法请求后跟 `hello world\r\n`）
  在**行终结时**即 400，不等 `\r\n\r\n`（blank-header 套件）——头未齐也要
  增量校验请求行。
- 坑八（真机 paused 语义先行）：无 data 监听也不 resume 的 res，'end' 不发——
  真机同款（node 26 实测同挂），黑盒想当然 `await end` 即挂死；测试侧
  `res.resume()` 后再等 end。rc 陷阱再进宫（§4.45）：`cmd | tail; echo $?`
  是 tail 的 rc——退出码断言必须 `>file; echo $?` 或 `${PIPESTATUS[0]}`。
- 复现：`tests/node/http.rs::phase10f_http_parity_round1`（13 组）；
  对拍 404 件 95→125 绿（`docs/bun-parity.md` http 节）；https 随行 8→13。
- 推广为铁律：对拍修"缺方法"前先跑真机探针把**属性面**（自有属性有无/键形/
  构造形态 no-new）逐项定型——node 老式函数面（Agent/Server 的 no-new）与
  class 面（IncomingMessage 不可 no-new）混存，凭"都是构造器"猜必翻车。

### 4.132 worker 对拍五轮七坑（2026-09-18，10f worker 二轮起）
- 坑一（wire 字段语义跨界即炸）：端口信封 view 分支存 `byteLength`，解码却把它当
  typed array 第三参（**元素数**）——BPE>1（Int32/Float64 系）全 OOB。且异常在
  worker 内 `workerData` 常量求值期炸出，而该常量位于模块 **class 声明区之前**，
  求值中断即整模块 class 绑定 TDZ，require 命中报 `can't access lexical
  declaration "Worker"`——TDZ 是下游症状，不是根因。修法：wire 存 byteLength、
  解码按 `BYTES_PER_ELEMENT` 折算。教训：信封字段是跨端 ABI，编解码两侧的
  语义（字节/元素）必须成对核对；模块顶部的"数据落地常量"是炸点，能懒则懒。
- 坑二（报错归属三重错位）：worker 内异常经 `Error::Script` 上报，文件名是主脚本、
  行号是内嵌模块行号（`313:53`/`665:59` 跨文件同值即此症）。定位时先认出"行号属于
  内嵌模块源"，再用 `awk`/python 按内嵌源行号切片，别在测试文件里找 665 行。
- 坑三（端口同步收信的三次试错）：node `receiveMessageOnPort` 是同步语义，而事件
  派发是 task 级。JS 直推对端队列+微任务 flush → 微任务链式 ping-pong **饿死
  定时器**（infinite-message-loop 10001 轮）；改 `setTimeout` flush → 与 close
  定时器在轮内不保 FIFO；kick 事件经 pump 收割 → 收割在 RunJobs 前后各一轮仍同轮
  链式。正解：**本地 pair 直入对端 Rust pending 表（纯串无 GC 值），pump 逐轮
  统一派发**——事件节奏回归通道模型，同步收信走表。推广：跨 JS/Rust 的消息面
  改投递节奏前，先画 pump 的收割/RunJobs/timer-check 顺序图，微任务链是
  定时器杀手（§4.46 姊妹篇）。
- 坑四（per-session id 空间相撞）：本地 pair 与 cross 口（parentPort）共用
  各自会话的 `worker_next_id` 计数器——cross 口表项的 `peer` 是 worker id，
  可与本地 pair id 相撞，按 peer 反查对端对象会拿错（甚至自指自旋）。本地路由
  必须 `!peer_is_worker` 双向过滤。
- 坑五（recount 只回差值不落账）：`port_recount` 返回计数净变化，落账靠调用方
  `port_bump`——`port_peer_closed` 置 `peer_closed` 后忘了 bump，对端关后端口
  仍续命循环（hang）。且在 `with_rooted` 闭包内直接调 `port_recount`（同样走
  with_rooted）→ RefCell panic rc=101（§4.14 三进宫）。修法：闭包内只改字段，
  差值闭包外 `port_bump(port_recount(id))`。
- 坑六（undici webidl 文案）：node 26 的 MessageEvent 校验走 undici webidl 层，
  值回显是 `inspect(v, {quotes:'double'})` 再外包一对引号（`1 → ("1")`、
  `"str" → (""str"")`），not-iterable 用裸 inspect——照 V8 惯例猜必错，逐值
  实测；instanceof 门经隐藏槽 `__wjs_MessagePort`（模块求值期登记）判定。
- 坑七（原始值错误跨线程）：worker 顶层 `throw 42` 的 error 事件要收到**原始值
  本身**（`err === 42`、注册 Symbol 同一性）——`Error::Script` 只有文本。修法：
  捕获点对非对象异常打包 `__wjs_prim:{json}` 信封（kind=None 时 message 即信封，
  `worker_error_text` 直通勿加前缀），JS 侧按类还原；Symbol 描述经 prelude
  helper（`JS::ToString` 对 symbol 抛 TypeError，Rust 侧原生路不通）。
  复现：`tests/node/worker.rs::phase10f_worker_*`（修前 Int32Array 跨端静默丢、
  TDZ 级联、receive-message 收尾 hang）。

### 4.133 crypto 四轮 + http2 流式化六坑（2026-09-18，10f crypto/http2 收尾）

- 坑一（`Uint8Array.equals` 不存在）：`__b64urlDec` 回 `Uint8Array`（无
  `.equals`），OKP/EC 的 JWK `x` 比对直调即 `xBytes.equals is not a function`。
  修法：一律 `Buffer.from(x).equals(...)`（`crypto.rs` OKP/EC 两处）。
- 坑二（`ChanBody.done` 初值吞体）：客户端上传 `Data+End` 已入队但 `done` 仍
  `true`，首轮 `poll_frame` 即 `None`，POST 体恒空（服务端 `srv-end ""`）。
  修法：有体即 `done=false`（空体无 trailer 才 `true`）。
- 坑三（`try_recv` 丢 waker 饿死流式）：服务端应答 `ChanBody::poll_frame` 用
  `try_recv` 空转 `Pending` 且忽略 `cx`，后到的体块永不唤醒（`res.write` 后
  客户端零 `data`）。修法：改 `rx.poll_recv(cx)` 注册 waker（首版注释记坑）。
- 坑四（Duplex 基类只读 `closed/destroyed`）：`ClientHttp2Stream extends Duplex`
  后 `this.closed=false` 即 `setting getter-only property "closed"`（state 位图
  只读；旧 EventEmitter 壳无此约束）。修法：自有 aboard 旗用 `__` 前缀，
  `close()` 以 `destroyed` 只读判幂等、`destroy()` 置位（服务端
  `Http2ServerStream` 同口径早已 `__closed`）。
- 坑五（`Object.create(Socket.prototype)` 撞只读 `connecting`）：socket 代理
  `base.connecting=false` 同样 getter-only 抛错。修法：
  `Object.defineProperty(base,"connecting",{writable:true})` 自有遮蔽。
- 坑六（`writeHead` 不发头）：本仓 `writeHead` 仅缓冲、`__sendHead` 在
  `write/flush/end` 才发——双 `writeHead` 不抛（真机发头即抛
  `ERR_HTTP2_HEADERS_SENT`）。测试用 `flushHeaders()` 先发头再断言抛错；
  `write+end` 体会拼接收尾（断言须按全形 `"xgate:..."` 或改 `flush` 形）。
- 复现：`tests/node/http2.rs::phase10f_http2_streaming`（修前 POST 空体/
  流式零 data/`gate false`）；`tests/node/crypto.rs::phase10f_crypto_round4_parity`
  （修前 `xBytes.equals`/`Invalid JWK EC key`）。

### 4.134 crypto 五轮 raw 门六坑（2026-09-18，10f crypto 五轮）

- 坑一（恒定位 `313:53` 系 assert 内部抛点）：套件失败行号恒为
  `assert.js:313:53`（`__checkThrow` 的 `unexpected throw` / `throws` 的
  `Missing expected exception` 抛点），不是套件位置——读行号找断言必错。
  修法：截断二分（按顶层 `}` 块逐段 `head -n` 落临时文件跑，§4.115 四分法
  的机械版）；`hasOpenSSL(3,5)` 在本仓为 true，ml/slh 块全执行（别当跳过）。
- 坑二（预解码洗白字符串门）：`createPublicKey` 的 encoding 预解码把 raw
  字符串 key 先洗成 Buffer，`__parseKeyMaterial` 内的字符串门永不触发。
  修法：预解码限 pem/der 系，raw 系（`raw-private/-public/-seed`）跳过。
- 坑三（门序即语义，三层各异）：`want` 门（`key.format` 无效）→
  字符串门（`key.key` 实例断言）→ akt 链（缺/坏 akt）→ seed/尺寸语义门，
  逐层实测定序，不猜（如 `createPrivateKey({format:'raw-public'})` 报
  format 门而非 akt 错；字符串门却排 akt 链之前）。
- 坑四（ml 展开形 PKCS#8）：`ml_dsa_44_private.pem` 非种子形而是
  OCTET{ SEQ{ OCTET(32 seed), OCTET(2560) } }——种子解析器只认 `[0]` 形即
  `Invalid PKCS#8 key`。修法：`mlkem_pkcs8_seed` 加 SEQ 首子 OCTET 分支
  （Rust 单测钉种子形/展开形/错长三件）。
- 坑五（压缩点别手写 BigInt）：P-384/P-521 的 `b` 凭记忆必错——树内
  p256/p384/p521/k256 全直引（零新增），新 native
  `__wjs_ec_import_compressed` 经 `PublicKey::from_sec1_bytes`（固有方法，
  含解压+上曲线校验）一行落地；JS 侧只做形态分流（02/03→native、
  06/07→04 同道、坏前缀落 `bad()`）。
- 坑六（zsh `===` 展开，§4.3 二进宫）：`echo ===` 裸写即 hemis——本轮
  `grep ... | head` 后误跟裸 `===` 又炸一次；分隔符一律加引号。
- 复现：`tests/node/crypto.rs::phase10f_crypto_raw_seed_parity`（`r5-*` 行；
  另 `test-crypto-key-objects-raw.js` 修前 DIFF 修后 SAME0）。

### 4.135 crypto 六轮 PSS/PBES2 八坑（2026-09-18，10f crypto 六轮）

- 坑一（a1 剥层连翻两次）：MaskGen `[1]` 内容是**完整** `SEQ{OID-mgf1, SEQ{hash}}`——
  首版 `children(it.body)` 得 `[SEQ]` 判长 2 即 null；改"剥外层 SEQ"又写成判长 1，
  还是 null（hexdump 明明对得上）。两翻同源：不动手算，拿 `__derRead` 逐层打印
  （本次 `a1 body: 3018…` 钉死 `[OID, SEQ]` 两元才落定）。教训：DER 嵌套层数
  只认逐层打印，不认"看起来"。
- 坑二（SHA-1 OID 键多拼长度字节）：map 键 `052b0e03021a` 混入 `05` 长度——
  map 键一律 OID **body** hex（`2b0e03021a`），tag/len 剥干净再当键。
- 坑三（门序即真机序）：PSS 约束 salt 先 digest 后（sha1+小 salt 落 salt 错，
  套件 1007 行钉住）；验签缺省 salt 同取键约束值（sign/verify 双缺省互通，
  真机实测钉住：混合键缺省签出 salt 20 非摘要长 64）。
- 坑四（自验绿≠互通）：混合 MGF 自签自验全绿，但真机拒收——缺省 salt 取了
  摘要长。收敛标准：手组密码学一律双向真机交叉（本仓⇄真机互验），自交绿不算数
  （§4.54 对称性盲区再进宫）。
- 坑五（`?? key` 洗白 null）：`key.key ?? key` 把 null/undefined 洗成 options
  对象，JWK 对象门永不触发（"Unsupported JWK kty" 现形）。修法：直透 `key.key`。
- 坑六（`__cryptErr` 全大写门）：`DataError: ...` 小写码被 `^([A-Z]...)` 门漏掉
  落裸文——装载宽容的 catch 须按消息窄匹配，不能按 `e.code`。
- 坑七（getCurves 顺序即契约）：JWK-unsupported-curve 块靠 `find(!supported)`
  首个非支持曲线——本仓仅 NIST 四曲线时 `assert(namedCurve)` 空值，
  属能力偏离（非语义缺口），不造假曲线凑数。
- 坑八（Node 自己都不逐字节）：PSS 导出 round-trip，真机对 sha256/sha512 对
  非逐字节（重编码 params），本仓 stash 原文反而逐字节——"与真机逐字节一致"
  在重编码面不成立，以"解析等价 + 细节稳定"为准。
- 复现：`tests/node/crypto.rs::phase10f_crypto_pss_gates`（`r6-*` 行；
  另 `test-crypto-key-objects.js` 余唯一红块见 bun-parity）。

### 4.136 Zip移植双坑：HideStackFrames漏挂 + 解压背stop缺失（2026-09-18，10f zlib Zip）

- 症状一：`contentSync` 篡改包报 `ERR_ZIP_ENTRY_CORRUPT is not a constructor`。
  根因：新增 `E('ERR_ZIP_*')` 未挂 `HideStackFramesError`，而移植稿按 Node 原文
  解构 `codes.X.HideStackFramesError`——undefined 当构造器即炸。修法：7 ZIP 码 +
  `ERR_INVALID_STATE` 全挂 HideStackFramesError（真机 E 定义逐字对拍）。
- 症状二：伪造小头（declared 50/实际 5000）报 `produced 5000 bytes, expected 50`
  而非 `inflates beyond its declared size of 50`。
  根因：`inflateRaw`/`zstdDecompress`（Sync + 回调）丢 `maxOutputLength`——Zip 的
  declared+1 背stop（compression.js `outputCap`）永不触发，全量解出后才在
  `checkDecoded` 落错；既有仅 brotli 有该面。修法：两路补 maxOut（校验 +
  超限抛 `ERR_BUFFER_TOO_LARGE`，Sync/回调双侧），使失败发生在解压内、
  经 `rethrowDecodeFailure` 转成 corrupt-inflates 口径。
- 复现：`test-zlib-zip-hardening.js` 伪造小头件（修前 rejects: unexpected throw；
  修后 30/30）+ `phase10f_zlib_zip_archive`（corrupt 行）。
- 推广为铁律：新增错误码即 grep 移植稿解构形态（`HideStackFramesError` 有无）；
  新增"限输出"调用方前先查被调解压面是否真 honor 该选项——"传了" ≠ "用了"。

### 4.137 入口失败 + 开着句柄 = 永不收割（eval 路径版）：then 吞 rejection + unhandled 只在循环尾（2026-09-19，10f child 牵引）

- 症状：`--eval 'spawn("sleep",["30"]); throw new Error("x")'`（或 `Promise.reject`）进程
  hang（模块路径 `--run` 同样 hang 在未处理 rejection 形）。§4.70 修了模块入口的
  `entry_failed`，但 eval 路径无捕获、且 unhandled 表只在循环尾收割——句柄开着
  循环永不 idle，两个收割点都永不到。
- 根因（三连）：① eval 的 async IIFE 包装以 `.then(v=>…, e=>{__wjs_error=e})` 收尾
  ——rejection 被 then 吞掉，链式 promise 正常 fulfill，失败信息只落全局变量；
  ② `__wjs_error` 在 `extract_eval_result`（事件循环**之后**）才读；③ 未处理
  rejection 表只在 `event_loop` 尾 `report_unhandled_rejections` 收割（REPL 才有
  逐轮收割）。
- 修法（三件配齐）：① wrapper 的 rejection 处理器重抛（`return Promise.reject(e)`，
  链式 promise 保持 rejected）；② eval 包装求值成功后对 rval（promise）挂
  `entry_native_values()` reactions（镜像 run_module；`entry_rejected_native` 消费
  重抛，无二次 unhandled 噪声）；③ `event_loop` pump 后加检查点：
  `unhandled_pending() > 0` 即 break（tracker Handled 已在表内摘除，checkpoint
  时非空 = 整轮排空后确无处理 = node 的 checkpoint 末 fatal 语义）。检查点放
  event_loop 不放 pump_once——pump 与 REPL 共用，REPL 逐轮收割不退出（node
  REPL 同款），pump 内早退会跳过当轮结算。
- 复现：`tests/node/child.rs::phase10f_entry_failure_open_handle_exit`
  （eval throw/eval reject/module reject 三形；修前 alarm 打不到头，修后 exit 1
  带原文）。
- 推广为铁律：§4.70 完整形态二——凡"失败信息落在循环尾才读的地方"（全局变量/
  表），必须同时有一条**提前跳出的检查点**；eval 包装的 then 处理器不是失败
  通道（吞 rejection），失败必须以 promise 形态回到宿主侧。

### 4.138 net listen 面三坑：字符串端口当 UDS + 无重听守卫 + 柄不随出口清（2026-09-19，10f net）

- 坑一（`listen("0")` 建出套接字文件）：`__doListen` 把一切字符串当 IPC 路径
  ——`listen("0")` 绑出名为 "0" 的 UDS；真机 normalizeArgs 判据是
  `Number.isFinite(+s)`（可解析为有限数的非空字符串 = TCP 端口，"abc" 才是
  path）。listen-options 套件两次 `listen("0")` 第二次 EADDRINUSE 现形。
- 坑二（重听无守卫）：listening 期间再 `listen()` 须同步抛
  `ERR_SERVER_ALREADY_LISTEN`（真机文案 `"Listen method has been called more
  than once without closing."`，逐项实测）。守卫谓词用 `this._handle`（node
  同款），不用 `__listening`——后者只在 listening 事件后才有值，盖不住
  绑定窗口。
- 坑三（柄不随出口清）：`close()` 只 destroy 不清 `_handle`/`__id`；listen 失败
  的 error 派发也不清——"close 后可再听"（套件第三段）与"EADDRINUSE error 后
  可立即重听"（第一段）全卡死。三出口（close/error/成功转移）柄同步即清。
  注意：Server/Socket 各有 `__ev`（同名方法两个类），server 侧清柄别插进
  Socket 的 error case（`__port`/`__udsPath` 仅 server 在 `__doListen` 置位，
  可作判别但首选插对类）。
- 复现：`tests/node/net.rs::phase10f_net_listen_surface` +
  `test-net-server-call-listen-multiple-times.js`/`test-net-server-listen-options.js`
  （修前 DIFF，修后 SAME0）。
- 推广为铁律：凡"listen/打开"族 API，字符串首参先过数字归一化再分流
  （真机 normalizeArgs 为准）；"重听/重开"语义 = 守卫谓词 + 全部失败出口的
  柄清理，两件缺一即卡死或漏抛。

### 4.139 fork 非 silent 的 stdio null 三面：流式初始化别覆盖真机 null 缺省（2026-09-19，10f child）

- 症状：`phase9m_child_fork_ipc` 的 `c.stdout === null` 反绿为红——fork 非 silent
  的 stdout/stderr 变 undefined。
- 根因：10b/10f 给 ChildProcess 补流式面时，`__initStreams` 按 stdio 数组分派
  （pipe→流，否则 null），但 fork 走独立构造路径，旧代码里 `proc.stdout = null`
  显式缺省被删除——非 silent（stdio 继承）分支没人置 null，字段落 undefined。
  真机 26 逐项：非 silent 三面（stdout/stderr/stdin）全 `=== null`（继承无管道
  句柄），silent 才挂管形流。
- 修法：fork 内 `if (!o.silent) { proc.stdout = proc.stderr = null }` 恢复
  （stdin 本就有 `proc.stdin = null`）。
- 教训：§4.86 姊妹篇——"流的面"与"无流时的缺省"是两条路径，改流式初始化时
  grep 该构造器全部创建点（spawn/promisified/fork/死句柄）逐一对缺省值；
  `=== null` 类断言对 undefined 也红（`undefined === null` 为 false），
  反绿为红先查缺省丢失而非语义翻转。
- 复现：`tests/node/child.rs::phase10f_fork_nonsilent_stdio_null`（修前
  `nonsilent false false true`）。

### 4.140 G2 `__closing` relisten 残留：手工过/ cargo 挂 ≠ 环境问题（2026-09-19，欠账轮）

- 症状：全量 `cargo test` 在 `net::phase10f_net_listen_surface` 挂死 **46 分钟**
  （子进程 tokio `park_internal` 睡死，lsof 见 TCP LISTEN 已建立、listening
  回调永不触发）；**同一份 p.mjs 手工 shell 跑 6 秒全过**——结论打架，一度
  误判为"cargo 环境问题"，clean 全量重建后照样复现，排除构建脏状态。
- 根因：G2 的 close-during-listen 窗口旗 `__closing` 在 `close()` 置位后
  **从不重置**——close 后 relisten，新一轮 listening 派发被残留旗吞掉
  （`__ev("listening")` 开头 `if (this.__closing) break;`）。手工 shell 与
  cargo harness 的 bind 回调派发时序不同，恰好掩盖/暴露它——**不是环境问题，
  是状态机残留旗 bug**。
- 修法：`__doListen` 开头 `this.__closing = false`（net.rs）。修后 0.54s 绿。
- 推广为铁律：① 凡"窗口旗"（临时置位拦截某类派发/回调）必须与柄一样随
  close/error/listen 全部出口复位，新增窗口旗时逐出口清单化核对；②
  **"手工过、cargo 挂"≠环境问题**——先查状态机残留/时序型状态污染，两种
  启动方式只是时序不同；先 bisect 到引入提交再下结论（本轮 bisect 实锤
  G2 引入，非 G3/G9-2）。

### 4.141 with_str_args 参数跨分配悬垂：rooted 必须在第一个分配之前（2026-09-19，欠账轮）

- 症状：`net_remote_surface`/`net_unix_socket_roundtrip` 在 cargo harness 下
  SIGBUS（exit=None 信号死亡、stderr 空），手工 3/3 稳定过；崩溃报告
  （`~/Library/Logs/DiagnosticReports/winterjs-*.ips`，**SEGV/SIGBUS 定位
  第一手段**）栈实锤：`net.rs dispatch → with_str_args → call_two →
  JS_CallFunctionValue → js::Call memset_pattern16`。
- 根因：`with_str_args(cx, global, fun, kind, payload)` 的 `global`（裸指针）
  与 `fun`（裸 JSVal）**跨 `to_jsval` 分配**——分配可触发 GC 搬移，
  悬垂后进 `JS_CallFunctionValue` 即 SIGBUS。首补丁只 root dispatch
  调用点、漏函数体内跨分配参数，照样崩——**rooted 必须在任何分配前
  覆盖全部跨 GC 存活值**（§4.80 第 N 例；dispatch 三处 `net_target()` 返回值
  与 `get_prop_value` 读出 `__ev` 同批入槽）。
- 修法：with_str_args 内 `g`/`f`/`a`/`b` 全先入 rooted 槽再 to_jsval；
  dispatch 三处 net_target() 返回值立即 `target_r` 入槽。
- 推广为铁律：**新增 Rust→JS 调用 helper，函数体第一行先把全部 JS 值
  参数入 rooted 槽，之后才允许任何分配型调用**；reviewer 按
  "参数表 → 第一行 rooted"逐项对。

### 4.142 bisect 禁止连续建 worktree：每个 worktree 的 target 都是全量重编（2026-09-19，欠账轮·灾难记录）

- 症状：为 bisect G3 六提交，**连续建 4 个 worktree 且各自 `cargo build`**，
  每个 worktree 独立 target/ 全量重编依赖树（每个 10-20GB），磁盘 36G→0
  急速耗尽，连 ZCode 工具自身的日志都写不下（ENOSPC）——**连"删文件"的
  命令都失效**，只能用户外部手动 rm 恢复；G9-2 检查点构建中途断供作废重来。
- 根因：git worktree 共享 .git 不共享 target/；`cargo build` 在新 worktree
  = mozjs/全部依赖重编。连续建 N 个 = N 份全量。
- 修法/铁律（**拒绝连续建 worktree 压榨空间**）：
  ① **bisect 一律用主仓 `git checkout` + `git stash`**（单一 target，增量
  切换 16 秒），不用 worktree；worktree 只留给"必须并行持有多份完整构建"的
  场景（如双 agent 并行开发），且用完即删（`git worktree remove --force` +
  `rm -rf` 残留 + `git worktree prune`）；
  ② **每建一个 worktree 前先 `df -h` 看余量**（一个 debug worktree 按 20GB
  计）；余量 < 40GB 不建；
  ③ 磁盘 ENOSPC 的第一症状是"工具静默失效/日志写不下"，看到即停手清盘，
  不要继续任何编译类操作。

### 4.143 全量 cargo test 禁套 alarm/timeout（2026-09-19，欠账轮）

- 症状：`perl -e 'alarm 570; exec @ARGV' cargo test` 跑全量——全量（编译全部
  target + 数千测试）20-40 分钟，alarm 到点中途击杀，后台任务永远等不到结果，
  反复轮询超时。
- 根因：alarm 口径（20 秒级）只适用于**单个真机套件冒烟**；全量跑无界。
- 修法：全量跑 `cargo test` 后台直跑（`> file 2>&1`），轮询 `grep -c
  "test result: ok"` 看进度；**单测/冒烟才用 alarm**。同族：shell 里
  `ps aux | grep winterjs` 的 etime 是判断黑盒子进程挂死的硬指标
  （>60s 的单 phase 即挂，正常 0.5-2s）。

### 4.144 手工复现脚本放 /tmp 会被清（2026-09-19，欠账轮）

- 症状：二分/真机对照用的手工脚本放 `/tmp/wjs-*.mjs`，用户清 tmp 后消失，
  "手工过"的结论无法立即复核，差点把已修好的当未修。
- 修法：手工探针脚本每次从测试文件现提取（python re 从 `tests/node/*.rs`
  的 `r#"..."#` 抽 JS 源），或放 `/Users/bemly/probe`（家目录，不随 tmp 清理
  消失）；结论引用脚本时注明来源与生成方式。

### 4.145 跑分包装 exec 失败静默假绿：`exec or die` + glob 路径（2026-09-19，G9-3 轮）

- 症状：6 个 zlib 目标套件"双侧全绿"，实为 winterjs **从未执行**——
  `perl -e 'alarm 20; exec @ARGV' <BIN> --run f.js` 的 exec 对不存在路径
  失败后 perl 继续走完脚本，正常 exit 0（永假绿）；本仓路径含 U+F8FF
  （`/Volumes/ Projects`），工具调用里手敲极易被转义打断成
  `/Volumes//Projects`（no such file），两坑叠加差点把欠账判成已清。
- 修法：跑分包装一律 `exec @ARGV or die "exec failed: $!"`；二进制路径
  一律 `WJS=$(echo /Volumes/*/Projects/winterjs/target/debug/winterjs)`
  glob 解析，禁手敲含特殊字符路径；"全绿得可疑"时先验证二进制真跑过
  （输出非空/无 "no such file" 尾巴）。
- 复现：`perl -e 'exec @ARGV' /nonexistent; echo $?` → 0（die 之前）。
- 推广为铁律：harness 的每条 exec 都必须 or die；结论与常识打架时
  （node 套件不该全绿）先查执行痕迹再信退出码（§4.93 姊妹篇）。

### 4.146 一次性 native 丢 opts 二进宫 + raw 字典必须构造期设（2026-09-19，G9-3 轮）

- 症状三连：① `zstdCompressSync`/`brotliCompressSync` 走专用一次性 native，
  opts（dictionary/pledgedSrcSize）**整体丢弃**——pledged 不抛
  ZSTD_error_srcSize_wrong、brotli dict 静默失效（§4.136"传了≠用了"二进宫）；
  ② brotli 字典 'string' 漏过校验（`__zBytes` 收 string——数据输入合法但
  字典非法），漏到 brotli crate 报 "Decompression failed"；③ raw 族字典流
  inflate 报 "repeated call with bad state"——被动 NEED_DICT 恢复对 raw
  留 Mode::Bad（raw 无 FDICT 头，zlib 语义字典必须在首次 inflate 前设）。
- 修法：① 一次性压缩改走 `__zEngineOnce`/引擎（dict/pledged/错误口径
  单点接线；切前先字节对比——同 quality 下与裸 native 逐字节一致，零回归）；
  ② `__zDictBytes` 严格校验（Buffer/TypedArray/DataView/ArrayBuffer），
  一次性面 + `__zStreamBase` 两收口点共用；③ `RawInflate` 建引擎即
  `set_dictionary`（zlib 族被动 NEED_DICT 恢复保留，dictionary-fail 套件
  依赖其 "Missing/Bad dictionary" 文案）；④ 四个死 native + 注册项 + 孤儿
  `__zCall` 一并删除（`grep` 验空）。
- 教训：选项"传到 JS 函数"≠"传进引擎"；一次性面与流面共用收口点后，
  新选项只接一处，杜绝两套皮漂移。

### 4.147 Web CS/DS 错误落 readable：pipeTo 的 cancel 语义（2026-09-19，G9-3 轮）

- 症状（设计坑，type-error 套件牵引）：DecompressionStream 尾垃圾错误若从
  `write()` 抛，WritableStream 的错误经 pipeTo 走 **cancel 链**——readable
  正常结束，`Array.fromAsync(readable)` 读不到 reject，套件必红。
- 修法：CS/DS 的解码错误（junk/截断）一律 `ctrl.error()` 落 readable 侧；
  done 后再写的 junk 也落 readable（套件 `[valid, empty]` 形），不从 write 抛。
- 真机 26.8.2 逐项对拍：不继承 TransformStream（`instanceof` false，proto
  链独立）、format 枚举 TypeError（"1st argument 'x' is not a valid enum
  value of type CompressionFormat."）、`[object DecompressionStream]` tag、
  junk = TypeError `ERR_TRAILING_JUNK_AFTER_STREAM_END`（node:zlib 引擎
  junk 码同文复用）、无静态 supportedFormats。
- 复现：`test-zlib-type-error.js`（修前 DS 缺失 3 not ok；修后全绿）。
- 推广为铁律：Web 流管道的错误出口看消费侧——readable 的迭代器要 reject，
  错误就必须 `controller.error()`，走 write 拒绝等于把错误送进 cancel 黑洞。

### 4.148 net 尾件五坑：分包解码/end 回调挂点/abort 发射时序/EPIPE 三条件/HE 回落钩（2026-09-19，G6 轮）

- **坑一（large-string）**：Socket `setEncoding` 用 `new TextDecoder(enc)` 逐
  chunk 解码——TCP 分包把多字节序列切断，每碎片各吐一个 U+FFFD（40962 vs
  40960 之谜）。修法：持久 `StringDecoder`（`__dec` 与 `__enc` 同生命周期），
  `__ev end` 时 `end()` 补齐残余。教训：凡"逐 chunk + 字符编码"，解码器必须
  跨 chunk 保态（stream 系同查）。
- **坑二（async-iter）**：`end(cb)` 的回调挂 'close'——node 流语义挂
  **'finish'**（FIN 刷完即发）；半开对端不回 FIN 时 close 永不来，
  `end(resolve)` 卡死。改挂 finish 前先 grep 依赖 close 时点的旧套件。
- **坑三（abort-controller）**：`ac.abort()` 后套件才挂 `once('close')`——
  我们 destroy(err) **同步** emit error/close，抢在 once 挂载前 → 未处理
  error + once 永挂。node 的 destroy 发射是 nextTick。修法：abort 触发的
  destroy 一律 `queueMicrotask`（connect 侧/构造器侧两处）。另：Socket
  构造器此前根本无 signal 分支（查到的是 Server 的——先确认函数归属再改）；
  直调 `addEventListener` 须入 `__etAdd` 侧表，`events.listenerCount` 才可见。
- **坑四（write-after-end-nt/writable）**：对端 FIN 后写 → EPIPE
  'This socket has been ended by the other party'。条件三缺一不可：
  `__peerFin && __ended && !allowHalfOpen`——仅对端 FIN（writable 套件
  'end' 后写合法且无错）、仅本地 end（STREAM_WRITE_AFTER_END 旧形）、
  半开（async-iter 套件 FIN 后写要成功）都不走此路；cb 与 error 事件
  都下一 tick（同步返回 false 时 hasError 仍 false）。回归三板斧：
  writable/write-after-close/blocklist 三件旧绿套件先受累后修复——
  **改 write 路径必跑这三件**。
- **坑五（autoselectfamily-default/blocklist）**：HE 串行回落 = lookup
  `all: <autoSelectFamily 生效值>`（mocked lookup 只在 all:true 给数组）+
  `__heOnErr` 钩吞中间失败（error case 不落用户监听）+ close 后 `__heReset`
  重试（**保留 `__pendW`**——回落期间的用户写带到最终连接）+ 尝试统一走
  `__doConnect` 闭包（blockList 校验每地址生效，直接 `__realConnect` 会
  绕过拦截且无 close 事件→链停摆）。记档：attemptTimeout 竞速未实现。
- 回归：全量 `test-net-*` 159 件对拍——116 绿（+13）/ SAME1 14 / 仅我们红
  29（与 stash 旧二进制红集**逐一相同**，零回归）；black-box 223 全绿。
- 推广为铁律：①改 net write/end 路径，writable/write-after-close/
  write-after-end-nt/async-iter/blocklist 五件是固定回归组；②"事件 X 后
  才挂监听"的套件形状 = 发射必须异步（node destroy 语义）。

### 4.149 G4 fs validators 轮七坑（2026-09-19，欠账 G4 轮）

- **坑一（utimes 数字实参 = 秒，不是 ms）**：`utimesSync(path, 2**31, 2**31)`
  真机把数字当**秒**（y2K38 套件 2^31 s 断言）；本仓旧实现按 ms + 黑盒测试
  也编码了 ms——测试随实现偏差翻转（§4.65 姊妹篇），真机口径一锤定音。
  Date → ms 直传（精度全保）；_toUnixTimestamp 负数回当前秒（真机实测怪形，
  逐字照抄）。lutimes/futimes 同批对齐。
- **坑二（__cb1 回调先于值校验）**：`fchown(1, '')` 无回调——__cb1 先取末参
  当 cb → /callback/ 错误；node 值校验在前 → /uid/。__fdCb 重写：cb 非函数时
  先跑 syncFn（值校验错误原样抛、操作错误让位）再校验 cb；**且不得造孤儿
  rejected promise**（lchown ×7 unhandled rejection 根因：p 已 reject、cb 校验
  又同步抛，无人接）。uid/gid 域 `[-1, 4294967295]`（-1 = 不变更哨兵）。
- **坑三（writeFile opts 在 rest[2]）**：writeFile=(path,data,opts,cb)、
  readFile=(path,opts,cb)——参数表序号照搬 readFile 的 rest[1] 拿到的是
  data 字符串，signal 面静默失效（c1/c2 abort 后仍 success）。**复用包装
  helper 时先画参数表**。
- **坑四（fs_err::read 单阶段丢 syscall 语义）**：目录 readFile——node 是
  open 成功、read 失败（syscall 'read'）；fs_err::read 一把梭报不出阶段。
  open/read 两阶段手写，各报各的 syscall；fs_err 换 std 直用保 raw errno
  （§4.121 三进宫：mkdir/rmdir/read_file 全切）。
- **坑五（TextDecoder latin1 = windows-1252）**：node 'latin1' = 字节直映
  码点；TextDecoder 的 latin1 标签是 win-1252（0x80-0x9F 段不同）——✓/😀
  文本 roundtrip 必挂。latin1/binary 走 Buffer。
- **坑六（TDZ：const 箭头 helper 与导出顺序）**：`export const lchown =
  __fdCb(...)` 写在 `const __fdCb` 定义**之前** → 模块求值 ReferenceError →
  **整模块绑定全未初始化**（表象是"can't access lexical declaration"）。
  const 箭头 helper 必须先于全部使用点；function 声明无此问题。
- **坑七（模块级报错行号不可信）**：套件报 `xxx.js:346:53` 而文件仅 58 行
  ——错误位置映射失真时，靠 `console.log` 插桩（CK/TI/AV 标记）定位到
  用例级，不猜。
- 回归：全量 `test-fs-*` 355 件 166 绿（净 +20：constants/stat-bigint/stat/
  statfs/readfile/rename-type-check/null-bytes/options-immutable/mkdir-mode-
  mask/rmdir-throws/truncate/timestamp-parsing/lchmod/lchown×2/fchown/utimes/
  y2K38/append-file-sync/write-file-sync/write-file/roundtrip）；残件：
  roundtrip 末段 async_hooks FSREQCALLBACK 资源面（另案）、write-stream/cp/
  watch 簇（G8/大簇）；black-box fs 13/13、冒烟 5/5。
- 推广为铁律：①改时间戳 API 先对真机量纲（秒/ms/µs）+ 全 grep 旧测试的
  量纲假设；②包装 helper（__cb1/__fdCb 族）新增变体时列出 node 的完整
  校验顺序（值 → callback → 操作）+ 孤儿 promise 检查。

### 4.150 edit 工具可能静默吞行：oldString 跨行即 diff 复核（2026-09-20，G5 轮·工具约束）

- 症状：两次 edit 后相邻整行消失——`__normExecOpts` 的 maxBuffer 行、
  `__normForkOpts` 的 killSignal 行；oldString 里根本没含那两行，编辑却"成功"，
  maxBuffer 套件全灭（err null + 全量输出）才现形。
- 根因：未深究（疑似多行 oldString 的模糊匹配吞了中间行；§4.28 姊妹篇）。
- 修法：补回整行（与原文逐字节同）；此后凡多行 edit，提交前必
  `git diff | grep "^-"` 逐条核对删除行是否全部有意（本轮即靠此抓到第二处）。
- 推广为铁律：edit 的删除行默认全部可疑——无 `-` 行是意外之喜，有即逐条认领；
  大 edit 拆小步 + 步步 `git diff`（§4.28 验落盘的 diff 版）。

### 4.151 外层模板字符串内禁写内层模板字面量（2026-09-20，G5 轮）

- 症状：`node:child_process` 模块整体加载失败，
  `child_process:1486:77: Expected a semicolon...`（全套件全灭，非单点红）。
- 根因：`__FORK_CHILD_SRC` 本身是外层模板字符串（反引号界定）；在其内部新写的
  `process.send` 校验用了内层模板字面量（反引号 + `${}` 插值）——内层反引号把
  外层提前闭合，`${v}` 在父作用域求值/残文变垃圾，oxc 解析期即炸。注释行里的
  反引号/`${}` 同样顶破外层（1486 行即注释行）。
- 修法：fork 子源块内一律字符串拼接（`"'" + v + "'"`），注释亦禁反引号与插值
  写法；提交前 grep 块内反引号/`\${` 计数归零（本轮 `inner backticks: 0`）。
- 推广为铁律：凡"JS 生成 JS"的模板块（fork 子源/worker eval 串），内层禁一切
  模板字面量；注释是代码，同样禁。

### 4.152 静默窗防抖在持续写下饿死：fs.watch 改前沿触发（2026-09-20，G8 轮）

- 症状：1ms/10ms 写循环套件（test-fs-watch.js/encoding/promises-watch）全 hang；
  单写/偶写套件全过。同一机制下只有目录自身元事件能出来，文件事件全丢。
- 根因：`debounce_loop` 是 300ms **静默窗**（到期才刷、同键刷新 deadline）——
  持续写使窗口永不到，事件饿死。旧设计为 `test --watch` 的"首事件赢"抄来的，
  但语义错了：node/libuv 无静默窗，事件即时流。
- 修法：前沿触发 + 同键 300ms 抑制窗——首事件立即刷（Create+Modify 的 rename
  先到先赢，§4.27 诉求保留），窗内同键丢弃，窗后首事件再刷（`src/builtins/node/fs.rs`
  `debounce_loop`；testrun 自有 debouncer-mini，不受影响）。
- 复现：10ms 写循环 watch（修前 foo.txt 永不到，修后即达；
  `tests/node/fs.rs::phase10f_fs_watch_rapid_and_rewrite`）。
- 推广为铁律：凡"等安静再动"的设计，必须回答"一直不安静时怎么办"—— perpetual
  busy 下静默窗 = 饿死；要么前沿触发，要么加最大等待上限。

### 4.153 notify 回调线程的 TLS state 是错表：分类移分发侧（2026-09-20，G8 轮）

- 症状：Create 二判据（seen 表 + birthtime）上线后，预存文件重写仍首报 rename——
  调试打印 `seen=false`，而种子明明已标记同一路径。
- 根因：notify 回调跑在 **notify 线程**，`state::with_plain/with_rooted` 读的是该线程
  的 TLS state——与 JS 线程的表完全是两张皮，mark/has 跨线程hello对不上（静默错，
  不 panic）。此前回调内无 state 访问，故从未暴露。
- 修法：回调只做纯数据搬运（生 kind + 展示名 + 全路径键）；seen/birthtime 终分类
  移到 `dispatch`（事件循环 = JS 线程，TLS 正确）（`src/builtins/node/fs.rs`）。
- 推广为铁律：§6 线程模型的反面——Rust 侧多线程经 channel 回 JS 线程**之后**才能碰
  TLS state；回调线程内如需查表，一律把生数据送过界、在分发侧判定。review 时按
  "回调跑在哪个线程"逐项对。

### 4.154 `process.exit` 哨兵被用户 catch 即覆盖：首个码赢（2026-09-20，G8 轮）

- 症状：`test-fs-realpath-pipe.js` 挂——自举子进程 `try{exit(2)}catch{exit(1)}`
  的 rc=1（应为 2）。
- 根因：exit 经 JS throw 实现，用户 `catch` 能吞掉哨兵；`process_exited` 旗无条件
  覆盖，第二次 exit(1) 把第一次的 2 洗掉。真机 exit 即终结，catch 永不触发。
- 修法：`process_exit` native 只在旗为空时落账（first-wins；`src/builtins/node/process_.rs`），
  `run()` 的旗检查本就优先，一处改全链对（ESM/CJS 双入口黑盒钉住）。
- 复现：`tests/node/process_.rs::phase4_process_exit_codes` 的 first.mjs/first.cjs 行。

### 4.155 首轮特例的无条件 return 会吞真变迁（2026-09-20，G8 轮）

- 症状：`test-fs-watch-file-enoent-after-deletion.js`（watch 后秒删）必挂；
  心跳探针证明事件循环活着、50ms 轮询 timer 正常、手动复刻同逻辑却能触发。
- 根因：`__statPoll` 的"缺席首轮发 (zero,zero)"分支 `return` 无条件——首轮恰为
  (null,real) 真变迁（unlink-then-poll 形）也被吞，随后 (null,null) 恒跳过，
  永静默。手动复刻"碰巧"走了另一条时序故能过，极具误导性。
- 修法：仅"首轮且双 null"走零值分支并返回，其余一律落正常路径
  （`src/builtins/node/fs.rs` `__statPoll`）。
- 推广为铁律：凡"首轮/首包特例"分支，默认写成"特例命中才返回"，特例不命中
  必须落回正常路径；特例分支的返回条件与触发条件逐字同写，禁大包围 return。

### 4.156 内部别名 normalize 只认裸形：`node:` 前缀形落空（2026-09-20，G11 联调缺口）

- 症状：http 全域 15 个黑盒齐挂 `Error: 'node:_http_common' is not a builtin`
  （fs/cluster 等全绿，具有误导性——以为 http 栈坏了）。
- 根因：G11 只给裸形（`_http_agent`）写了 normalize 臂；自家 http.rs 用
  `node:_http_common` 前缀形导入（套件直引与内部互引两形并存），前缀形走
  `strip_prefix("node:")` 后无臂命中即 None。INTERNALS 表项一直在，只是够不着。
- 修法：四别名臂并入 `strip_prefix` 后的 match（`src/builtins/node/mod.rs`
  `normalize_spec`），裸/前缀两形同归一。
- 推广为铁律：新增内部别名必须双形验证（裸 `require('_x')` + `node:_x`
  导入各跑一次）；"表里有" ≠ "够得着"，注册链（映射→表→source）逐段断言。

### 4.157 错误包装函数必须幂等：`__fsErr` 剥前缀致嵌套重包（2026-09-20，G8 轮）

- 症状：`tests/permissions.rs::phase8_permissions_fs` 挂——allow-list 读
  `/etc/hosts` 得 `Error/UNKNOWN`（应 `PermissionError`），而裸 `--allow-read`
  写拒绝分支正常。
- 根因：`__fsErr` 的 PermissionError 分支用 `m.slice(prefix)` 剥掉前缀后重建
  Error；读路径经 `__fsCall("open")` 包 `__fsReadWhole` 包 `statSync` 内层
  `__fsErr`——同一错误过包装函数**两次**，第二次前缀已失认，落通用分支重包成
  `UNKNOWN …, open '…'`。写路径只过一次，故正常（§4.37/§4.119 同源第三例：
  包装函数须区分"已整形"与"待整形"）。
- 修法：前缀保留原文重建（`new Error(m)` + 改名），二次进入同分支直通，
  天然幂等（`src/builtins/node/fs.rs` `__fsErr`）。
- 推广为铁律：错误包装函数默认会被嵌套调用（`__fsCall` 层层包）——构造的错误
  必须能无损地再过一次本函数（幂等）；凡 `slice/replace` 去特征头的写法，
  先问"第二次进来还认得吗"。

### 4.158 稀疏检出缺 fixtures 即污染对拍基线（2026-09-21，fs 残簇轮）

- 症状：`test-fs-cp-sync-copy-file-to-directory-error` 等在真机 `node` 下 rc=1
  （`ENOENT`，`fixtures.path('copy/kitchen-sink/README.md')` 不存在），与本仓
  SAME 对齐成"双红"，另有数件从 DIFF 变 SAME，全是假信号。
- 根因：`git sparse-checkout` 只取了 `test/parallel/test-fs-*` + `test/common/*`，
  漏了 `test/fixtures/copy/`——`kitchen-sink/` 以空目录存在，不报错只缺内容。
- 修法：`sparse-checkout add 'test/fixtures/copy'` 后重取真基线（本轮 129 件
  70/59 → 96/33，一夜变天全是 fixtures 的功劳）。
- 推广为铁律：对拍前先断言 fixtures 完备（`ls kitchen-sink/README.md`）；
  "真机自家套件挂"第一反应是环境缺件，不是 Node 有 bug。

### 4.159 mustNotMutateObjectDeep 的 Proxy 断 WeakMap 身份键（2026-09-21，fs 残簇轮）

- 症状：`mustNotMutateObjectDeep({ signal })` 包过的信号一读 `.aborted` 即
  `can't access property "aborted", __wjs_abortState.get(...) is undefined`；
  `addEventListener` 则在 `st.get` 直接炸（st 为 undefined）。
- 根因：该 helper 递归 Proxy 包裹（get 转发、set/define 直接 fail）——WeakMap
  的精确身份键遇 Proxy 即断裂；且经 Proxy 加监听记的是代理身份，真触发 miss。
- 修法：AbortSignal 状态双轨——WeakMap + symbol 自有属性（构造/触发期写真实
  对象，读经 `??` 回落；读穿透 Proxy 只因 get 转发，永不触发 set 陷阱）；
  `addEventListener` 无表决建表不抛（代理监听 miss 即 benign，abort 竞速由
  aborted 轮询门覆盖）（`src/builtins/mod.rs`）。
- 推广为铁律：凡 WeakMap 键存宿主内部态，先问"用户传个 Proxy 进来还认得吗"——
  读路径一律 symbol 回落；只读不写是穿透 Proxy 的唯一安全形。

### 4.160 sync 底座流的双重早退：同步终结派发 + 无句柄续命（2026-09-21，fs 残簇轮）

- 症状：`createWriteStream(f); s.end(); s.on('close', …)` 的 close 永不到，
  且无 timer 时进程 rc=0 直接退出（连 `process.on('exit')` 都不跑——后者系本仓
  另案不支持，干扰项）；先挂监听再 end 则全收到。
- 根因（二连）：① base 在无积压时同步调 `_final`，同步 cb 即同步派发
  finish/close，用户后挂监听全 miss（真机全异步）；② sync 底座无原生句柄，
  循环见全零即退，close 的异步尾巴永不到（§4.34 同族：IO 面的生命周期缺口必现
  为 hang/丢事件）。
- 修法：`_final` 完成 + `open` 派发改 `queueMicrotask` 递延（fd 同步建不受影响）；
  `fs_stream_open` 计数（state + 双 native + idle 门，`UNSAFE-BOUNDARY` 双标签）：
  构造持有 → close 释放，autoClose 关时 finish/end 静默释，未用流 A 段微任务自释
  （否则 patch-open 子进程形永不退出）；`autoDestroy` 随 `autoClose`（closed 语义）。
- 复现：`tests/node/fs.rs::phase10f_fs_stream_lifetime`（`w-fin/w-close/r-end` 行）。
- 推广为铁律：sync 底座的流/句柄，上线即回答"谁让循环等我"——无原生句柄即配
   计数器；"构造即完成"的同步链一律递延派发终结事件。

### 4.161 require 的 make_fn 裸值窗口 + 文件名假相关二分法（2026-09-21，dgram 轮）

- 症状：`test-dgram-async-dispose.mjs` 稳定 138（8/8），而字节相同的改名拷贝
  次次过；`rm + cp` 重建后好一次又坏；stash 旧码 3/3 干净——一度误判"文件名相关"。
- 根因：`require_cjs_file` 内 `make_fn`（`get_prop_value` 裸 JSVal）横跨
  `to_jsval` 字符串具现（可触发 GC 搬移），栈拷贝悬垂后 `call_one` 读垃圾即
  SIGBUS（§4.80 修链时漏了此窗，链下半的 rooted 全但上半没盖）。文件名/内容
  长度只改变 nursery 分配序列从而改变 GC 触发点——确定性假相关，非因果。
- 修法：入 `rooted!` 槽后再做一切分配型调用（`src/builtins/node/require.rs`，
  5 行；修后 3/3 直通）。
- 二分手法（可复用）：尺寸探针（等量死代码）→ hunk 累积二分（每次验 `Compiling`
  行，3 秒"构建"多为 no-op，行为才是真相）→ Rust/JS 分离杂交 → 最小翻转子。
  另：`from_std` 前必 `set_nonblocking(true)`（UDP 无握手安全），否则 tokio
  直接 panic（bindSync 首版现形）。
- 推广为铁律：新增 Rust→JS 调用点，函数体第一行先把全部 JS 值参数入槽
  （§4.141 的 require 版）；"改名即好"的结论默认不可信，先问分配序列。

### 4.162 fork env 丢失即指数 fork 炸弹（2026-09-21，child 轮）

- 症状：`net-reuseport` 探针打出数百个 `parent listening`（端口递增）+
  `parent closing` 交织，进程数爆炸；套件 hang。
- 根因：fork 只验 env（\0）不透传（"值忽略"记档），`new Worker(src, …)`
  未带 env——子复走父分支（`isWorker` 缺失）再 fork，指数爆炸。
  同源：net `Server.listen({reusePort})` 的 direct 路径丢选项（只
  BoundSocket-adopt 路径透传），双绑即 EADDRINUSE。
- 修法：`__normForkOpts` 存 env 拷贝 → `new Worker(src, { env: o.env })`
  （缺省 Worker 侧快照继承，等价真机缺省）；net `__doListen` 加
  reusePort 形参直通 native 第 4 参（`o.reusePort` 两分支同传）。
- 复现：reuse2.mjs（修前炸弹，修后 worker 单次 listening + exit 0）。
- 推广为铁律：凡"子复用父文件"（fork/self-spawn）的开关量（env/argv），
  丢失即自指递归——接线完备性按"子能否区分自己"逐项验。

### 4.163 Rust Stdout 块缓冲吞常驻输出（2026-09-21，child 轮）

- 症状：子进程 `write('x')` 后等 stdin，父永收不到，双边 hang；
  直接跑同代码（无 interval）却正常（"xtrue" 现形）。
- 根因：`std::io::stdout().write_all` 经块缓冲（管道无换行即滞留）；
  进程即退时 exit 刷出掩盖，常驻即永滞。console.log 带换行故无事，
  精确制导到 `process.stdout.write` 无换行 + 循环存续才现形。
- 修法：`stdout_write/stderr_write` 逐次 `flush()`（Node 写无缓冲；
  stderr 本无缓冲，对称 no-op 防后人误抄）。
- 复现：`--eval 'process.stdout.write("x"); setInterval(...)'` 管道接
  （修前零输出，修后即时见 x）。
- 推广为铁律：宿主侧一切"写 fd"面，默认逐次刷——缓冲是传输优化，
  不是语义；"退出即对、常驻即错"的分裂是块缓冲的指纹。

### 4.164 数字信号 0 无枚举值，落空即 SIGKILL（2026-09-21，child 轮）

- 症状：`kill(0)`（存在性检查）直接杀掉目标（exit null SIGKILL）。
- 根因：`Signal::try_from(0)` 无对应变体 → `unwrap_or(SIGKILL)`。
- 修法：`raw == "0"` 短路 `kill(target, None)` 纯验活（nix 口径）；
  JS 侧 `killed=true` 保留（真机同款）。
- 复现：kill0.mjs（修前误杀，修后存活到显式 kill）。
- 推广为铁律：`try_from(x).unwrap_or(默认)` 写法先问"落空值是不是合法
  输入"——0/空串类哨兵值落空即灾难，哨兵短路永远先于转换。

### 4.165 UDS listen 异步绑与同步 cp 的竞态（2026-09-21，fs 轮）

- 症状：`cp-async-socket` 报 ENOENT（应 `ERR_FS_CP_SOCKET`），自带 listen
  回调的探针却正常——同一 socket 文件时有时无。
- 根因：UDS bind 在 Rust task 内异步落定；套件 `listen(path)` 后同步
  `cp()`，文件尚未出现。真机 pipe bind 在 listen() 返回前同步完成。
- 修法：探活（connect 通即 EADDRINUSE）/清残留/bind/chmod 全改同步
  （本地 syscall，无等待），task 只接管已绑 listener 跑 accept；
  `from_std` 前照旧 nonblocking（§4.161）。
- 复现：`test-fs-cp-async-socket.mjs`（修前 ENOENT；修后 0）。
- 推广为铁律：凡"创建即同步用"（listen 后 stat/cp、bind 后 connect），
  创建语义必须是同步落定——异步落定的创建面配合同步消费必竞态；
  改时保留原错误面（探活/chmod/错误事件逐项搬，不删逻辑只换线程）。

### 4.166 宿主 mock 可见性：内部调用须经默认导出（2026-09-21，fs 轮）

- 症状：`write-stream-err` 补丁 `fs.write/fs.close` 永不触发（第二块 BAM
  丢、`close` mustCall 悬空）；`change-open` 的补丁 close 不调回即 hang。
- 根因：WriteStream 内部直调本地 `write/close` 绑定与 native，而套件
  `require('fs')` 补丁落在默认导出对象（`__api`）上——直调即绕过。
- 修法：`_write/_final` 经 `__api.write/__api.close` 调用（同一对象，
  补丁可见；无补丁即原函数，零行为差）；`close` 不等回调即走
  （补丁不调回是真机同款 fire-and-forget）；`_write` 补写 position 跟踪
  （fd 形 `start: 0` 覆盖写，autoclose-option 现形）。
- 复现：`test-fs-write-stream-err.js`（修前第二块丢；修后全绿）。
- 推广为铁律：凡"用户可 mock 的面"（fs/dns 等），内部调用一律经默认
  导出对象、不直调本地绑定——自测时顺手打一个"补丁补丁是否生效"的
  探针（直调绕过是静默的，功能全对时最难发现）。

### 4.167 fifo 双 open 死锁：数据必经已开 fd 读（2026-09-21，fs 轮）

- 症状：`test-fs-read-stream.js` 修校验后由红转 hang（TIMEOUT）；二分定位到
  fifo 子段（mkfifo + 自家 exec 写者 + `{end: 1}` reader）。
- 根因：`__doOpen` 先 `openSync` 建 fd（与写者会合成功，写者写完退出），
  再调 `read_file` 按径**二次 open**——写者已走，二次 open 等新写者永挂。
  sample 实锤：主线程 858/858 采全卡 `fs_read_file → File::open → open(2)`
  （初判"park 空转"系误读——grep 截断了下半栈，见 §4.168 手法）。
- 修法：`__doOpen` 经已开 fd 全量读（`readSync` 循环，position null 游标推进），
  不再按径重开（`src/builtins/node/fs.rs` `__doOpen`）。
- 复现：`test-fs-read-stream.js` fifo 段（修前 TIMEOUT，修后 `END "xy"`）。
- 推广为铁律：凡"创建即同步用"的会合型资源（fifo/pipe/socket），open 与读写
  必须同一句柄——按径二次打开是自杀（§4.165 的读侧版）。

### 4.168 旧 SAME1 掩盖 + 跑分两手法（2026-09-21，fs 轮·过程教训）

- 症状：本轮 5 个"新红"（read 系 ×4 + handle-read）——旧跑分全是 SAME1
  （node=1 wjs=1 双红），新跑分 node=0 wjs=1。
- 根因：旧跑分时 `test/fixtures` 尚未取全（`elipses.txt/x.txt` 缺失，node 自家
  套件也挂），双红对齐成假 SAME；fixtures 补齐（§4.158 后续）后真机转绿，
  宿主缺口现形。非本轮回归（校验/读盘路径与在修代码无交集也佐证）。
- 手法二则：① sample 读栈禁 grep 截断——`sample PID 1 | grep 关键词` 会把
  下半栈（真正的 JS→native 调用链）截掉，先看全栈再过滤，本轮因此误判
  park 一次；② `perl -e 'alarm N; exec @ARGV'` 不加 `or die` 即 §4.145 翻版——
  本轮亲手复现（相对路径二进制 exec 失败，perl 正常 exit 0，空输出当通过），
  此后跑分一律 `exec @ARGV or die` + 绝对路径。
- 推广为铁律：SAME1（双红）≠正确——fixtures/环境补齐后必重跑基线；
  "单独跑过、并行挂"查共享 tmp（§4.122），"以前双红、现在单红"查 fixtures。

### 4.169 仅 error 监听的流够不着懒 open + 补丁分支置空（2026-09-21，fs 轮）

- 症状：`createReadStream(missing)` 只挂 error 监听时无 error、无退出码异常——
  进程静默 0 退出（真机是异步 error，无监听则抛）。
- 根因：`__doOpen` 懒在 `_read`，无 data 监听即无流动、`_read` 永不跑，
  错误永不发现（node 构造后即异步 open，不依赖消费）。
- 修法：A 段微任务内先开（`__doOpen` 前置），开败即就地 `_read()` 递送
  （走既有 `__openErr` 分发；成功仍等流动消费）；补丁分支微任务内同步置空
  bytes（否则 `_read` 回落见 `__bytes===null` 误真开，patch-open 破功）
  （`src/builtins/node/fs.rs` `__holdStream`）。
- 复现：error-only 探针（修前静默退出，修后 `async-error ENOENT`；无监听形
  与真机同为 unhandled error exit=1）。
- 推广为铁律：凡"构造后即生效"的宿主语义（open/error），触发点不得绑在消费
  侧（`_read`/data）——无消费者的形状（纯 error 监听）是天然反例。

