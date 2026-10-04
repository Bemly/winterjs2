# 踩坑分卷2（4.68–4.124）

> 本卷为 `docs/pitfalls.md`（主索引）分卷之一，只收正文；查阅先看主索引，编号 `§4.N` 全仓唯一。

### 4.68 TSFN 的 JS 回调漏标 GC 根：fsevents 真变更 139 根因（2026-09-14，M5）

- 症状：§4.67 的 139——真文件 append 后 `EXIT:139`
 （`EXC_BAD_ACCESS 0x4b4b4b4bXXXXXXXX`，高位恒定、低位浮动；调用栈
  `asyncwork::dispatch(TsfnDrain)` → `fse_dispatch_event` →
  `napi_call_function` → `call_impl` → `JS_CallFunctionValue` → JS 内崩）。
  崩点在回调**内**（`tryAttachTypedArrayElement`/`GetProperty` 都见过——
  崩点是受害现场，不是根因）；napi 实参形态正确、无 pending 遗留。
- 根因：`NapiEnv::trace` 只标 `slots/escape_slots/deferreds/refs/wrap_sym/
  modules`，漏了 `tsfns` 记录里的 `js_cb`。TSFN 的 JS 回调**只被该记录持有**
  （dispatch 截断 `slots` 后无其他根），一次 GC 后回调即悬垂，下次事件
  dispatch 取悬垂槽值进 `fn.apply` 就是 UAF。fsevents 是长驻回调 +
  transform/fetch 制造 GC 压力 → 稳定复现；m3 旧 fixture 全程无压力 → 全绿
  掩盖（§4.40 同源：旧测试从未在回调内存活期外造 nursery 压力）。
- 修法（`src/napi/env.rs`）：trace 加 `for (_, t) in &self.tsfns {
  t.js_cb.trace(trc) }`。回归：`tests/fixtures/napi/m3_tsfngc.c`
  （线程延迟 600ms 投递 3 条，JS 侧 8 轮 × 20 万小对象 ≈ 80MB nursery 压力；
  大对象直进 tenured 触发不了 minor GC，故不用大数组）+
  `tests/napi.rs::phase_napi_m3_tsfngc_roots_callback`。
  Revert-check：注释掉该行即挂（修前 139/修后过；`/tmp/wjs-tsfngc` 手动复现同，
  崩溃报告 `0x4b4b4b4b00000000` 与线上同特征）。
- 复现：`hmr-min9.mjs`（transform fetch + WS 握手 + 真 append）修前 139、
  修后 `full-reload` + `CLOSED` + `EXIT:0`；mimic 探针的 BigInt 是红鲱鱼
  （`Number(id & 0xffffn)` 混用抛 TypeError 触发 fsevents.c 的 assert，
  改纯 Number 探针即过，与 139 无关）。
- 推广为铁律：§4.40 的完整形态——**`Box<Heap>` 定址只保地址稳，trace 才保
  可达，两者缺一不可**；凡新增跨 GC 存活的 JS 值存储（含 HashMap value、
  新 napi 记录），trace 必须同步加，上线前 grep trace 覆盖。FinalizationRegistry
  在本引擎不可靠（真机同为 false），回归探针用"确定性压力 + 投递存活"断言，
  不用 canary（试过，不稳定）。

### 4.69 目录首条目 tarball 炸解包：暂存预建（2026-09-14，M5 vitest 牵引）

- 症状：`winterjs -a vitest` 在 `@types/chai@5.2.3` 必败：
  `cannot unpack @types/chai@5.2.3: unpack failed: failed to create
  node_modules/.staging-<pid>-<rand>`；其前 100+ 包全过。
- 根因（二连）：① 该包 tarball 非 `package/` 布局——条目以 `chai/` **目录条目**
  打头（`tar -tzf` 首行 `chai/`，常规 npm 包首条目即文件、无根目录条目）；
  ② tar 0.4.46 的 `unpack_in` 在父链校验里 `canonicalize(dst)`，首条目为目录
  时本仓又跳过预建父链（只对非目录条目建），暂存尚不存在即
  `failed to create <staging>`。常规包因首个文件条目的预建顺带建了暂存，
  故从未暴露。另：`chai/` 根的回落（暂存内唯一顶层目录即包根）本就写好，
  只是一直没活到那一步。
- 修法（`src/pm/install.rs::unpack_tgz`）：循环前 `create_dir_all(staging)`
  预建暂存，一行。回归：模块单测 `unpack_dir_first_non_package_root`
  （现场打目录首条目 gz，目标为尚不存在的暂存；revert-check 注释预建即挂）+
  黑盒 `phase5_install_dir_first_tarball_stub`（stub 下发→真装→require 出 42）。
  实证：修后 `-a vitest` 一遍过（25 包，`vitest@5.0.0`）。
- 过程教训：中途一次"旧二进制却装成功"系误判——`cargo test --test pm`
  会先编出**带修复的主二进制**（集成测试要 `CARGO_BIN_EXE`），其后对
  `install.rs` 的 revert/恢复编辑只改了 mtime 没改净内容，二进制实际已含修复；
  `ls -la` 的新旧比较证明不了二进制由哪版源码编出。结论打架时先查构建指纹链，
  不只看 mtime（§4.62 姊妹篇）。
- 复现：`curl registry @types/chai/-/chai-5.2.3.tgz | tar -tzf -` 首行即目录；
  修前模块单测必挂。
- 推广为铁律：凡"前 N 个全过、特定包必败"的安装失败，先 `tar -tzf` 看该包
  条目布局（根目录条目/非 package 根/符号链接三件），再怀疑网络与版本。

### 4.70 入口失败 + 开着的句柄 = 事件循环永不收割（2026-09-14，M5 fork 牵引）

- 症状：`fork("/no/such/xxx.mjs")` 父端无 error 无 exit，进程 hang（`bad-arg`
  同步校验正常；裸 eval worker 同样缺失 import 却正常 error+exit 1）。
- 根因：`run_module` 的入口 rejection 收割在 `event_loop` **之后**
  （`src/runtime.rs`）——子会话开着 parentPort（message/close 双监听）时循环
  永不 idle，收割点永不到。裸 worker 无句柄，首轮即 idle，收割正常。
  二分探针：端口在场 + 未捕获顶层拒绝即挂（`p6.mjs`），catch 住即回 END 后
  空转（`p5.mjs`，端口续命本身是正确语义）。
- 修法：`PumpStats` 加 `entry_failed` 旗——`pump_once` 内 RunJobs **之后**
  查 `entry_rejection.is_some()`（与 `process_exited`/`worker_terminated`
  同族检查点、同顺序），`event_loop` 见旗即 `break` 走既有收割上报
  （入口带专用捕获，不进 `unhandled` 表，收尾 `report_unhandled_rejections`
  不受影响；`process.exit` 优先顺序不动）。
- 复现：`tests/node/child.rs::phase9m_child_fork_errors`（修前 hang；
  另带出父断子不断 control 分支漏 `parentPort.close()`，同批修，
  `phase9m_child_fork_ipc` 的 `EXIT-EV 0 0` 覆盖）。
- 推广为铁律：凡"失败只在循环尾收割"的设计，必须回答"循环不退时失败去哪"——
  fatal 类决议（入口错/uncaught）一律检查点提前跳出，不等自然排空。

### 4.71 `Object.assign` 不自建自引用 + 真机 assert 形态先行（2026-09-14，M5）

- 症状：`assert.ok is not a function`（`phase4_node_assert_subset`），连带
  `net_echo_loopback`（服务端 handler 首行即 `assert.ok`，抛错后响应永不结束→
  链式 stall→server 不关→全进程 hang；http 回环 hang 同源，修 assert 即好，
  无 http 侧改动）。
- 根因：M5 把默认导出改可调用时写成 `Object.assign(ok, {...})`——assign 不建
  自引用键，`default.ok` 为 undefined。想当然补 `__default.ok = __default`
  又错第二遍：真机（node 26.8.2 实测）`assert.ok !== assert`，
  `assert.ok === assert.strict.ok`，名分别为 `assert`/`ok`。
- 修法（`src/builtins/node/assert.rs`）：具名 `function assert(value, message)`
  转调 `ok`，再 assign 全方法（`ok/strict/AssertionError` 等）后默认导出——
  五项逐项对真机：`typeof function`×2、自反 false、`ok===strict.ok`、
  双名、`assert(true)` 直调。
- 教训：§4.65 姊妹篇——"真机口径"必须**逐项实测**，不能只验规划的那两项
  （本次若只验 `typeof` + 直调，自引用错就漏网；`console` 黑盒的
  `assert-callable true false` 即真机逐项，修完直接绿）。
- 推广为铁律："与真机同款"断言必须列出全部可观察项并逐项对拍，缺一项即欠账。

### 4.72 fork 单槽监听 + CJS 具名上线后的黑盒同步（2026-09-14，M5）

- 症状一：`phase9m_child_fork_ipc` 修 hang 后报
  `NotSupportedError: ChildProcess event 'w9m-never'`——测试用凭空事件名验
  once/off，而 `on()` 对未知名按设计抛错。
- 修法（测试侧）：占位改 fork 路径永不触发的 `spawn` 事件——不用 error/exit
  是因单槽位 `once` 会顶掉同名常驻监听（`off` 对 exit/close 双重 wrap 也摘不净），
  不用 message/disconnect 是因流程内真会触发。注释写明三选理由。
- 症状二：`phase9j_cjs_interop_default` 的"命名导入必须失败"边界在具名导出
  上线后反绿为红（`import { v }` 成功）。
- 修法（测试侧）：边界翻转为成功断言（`cjs-named 41`），与
  `phase9j_cjs_interop_named` 同口径；旧"缺导出"注释标退役。
  另：`cjs_named` 的 `defaults … function …` 系 fixture/plain-object 与期望
  打架的测试 bug（`typeof` 应为 object），同批改。
- 推广为铁律：功能上线即全 grep 旧边界断言（"必须失败/必须抛"类），上线不改
  旧断言等于埋红；单槽事件设计下，测试占位事件必须选"永不触发 + 无常驻监听"
  的那一个。

### 4.73 线信封循环引用：先序 id + ref marker，两端同序才成立（2026-09-14，M5）

- 症状：vitest/tinypool 跨池消息含循环/共享引用对象——9i-2 信封语义下
  DataCloneError（循环抛错）或共享引用被拷成多份（身份丢失），池消息失真。
- 根因：9i-2 信封走"可克隆性探路 + 拒绝"，不保对象同一性。
- 修法（`worker.rs` `__packValue/__unpackValue`）：容器先序 id——打包侧首访
  `path.set(v, nextId++)`、重访出 `ref` marker；解码侧**同先序、先占位再填子项**
  （`refs[nextId++] = out` 先注册占位，子项回填），祖先后向引用恒可解。
  共享引用保留同一性（真机 structuredClone 口径）。9i-2 黑盒同步翻转
  （`w9i-circular true true true true`：自引用/自同/数组含自身/跨消息不串）。
- 复现：`tests/node/worker.rs` 循环保留用例（M5 升级前必抛 DataCloneError）。
- 推广为铁律：跨端序列化保循环 = "同序编号 + 先占位后回填"两条同时成立，
  漏任一即环解 undefined；编码器语义升级时全 grep 旧"必须抛"断言。

### 4.74 `process.stdout.write("", cb)` 无回调实现 = 等落定的协议永挂（2026-09-14，M5）

- 症状：vitest threads 池 worker 挂起——worker 线程 `flushStdio` 以
  `process.stdout.write("", cb)` 等前序块落定，本仓 write 只写不回调，cb 永不触发。
- 根因：`process_.rs` stdout/stderr 的 write 未实现 Node 的 `write(str[, cb])`
  回调面（Node：flush 后异步触发，即便直写成功也异步回）。
- 修法：write 识别函数实参，直写恒成功，cb 经 `queueMicrotask` 异步回
  （stdout/stderr 双侧）；黑盒 `write("",cb) → fired`（`tests/node/process_.rs`）。
- 推广为铁律：凡实现"带回调的写出面"，回调即使同步完成也必须异步触发
  （microtask），否则调用方"等 flush"的挂起式协议永不解锁。

### 4.75 napi 建面 AB 数据指针必须终身稳定（2026-09-15，M5 终线）

- 症状：vue-project rolldown 载荷下偶发堆腐坏（138/139 随 GC 时序漂移）。
- 根因：`napi_create_arraybuffer/buffer` 走 `JS::NewArrayBuffer`——SM 小 AB 用
  inline 存储、GC 可搬移；Node/V8 契约是 data 指针**终身稳定**，napi-rs 按
  Node 语义持指针跨 GC 读写即腐坏引擎堆。M3"偏差记档：指针须重取"实际不可
  执行——addon 无从得知何时 GC。
- 修法（`buffer.rs` `new_owned_ab`）：napi 建面一律走自有稳定存储（Rust 分配
  对齐 16 + 零填 + `NewExternalArrayBuffer` 桥 + free 回收）；JS 侧建 AB 传
  addon 的指针仍只保回调存活期（偏差记档）。
- 复现：`tests/napi.rs` 全量黑盒 + rolldown 真包载荷。
- 推广为铁律：宿主 AB 与"Node AB 指针稳定"的引擎差异，必须在 napi 建面
  一次性抹平，不得要求 addon 配合（addon 按 Node 契约书写）。

### 4.76 napi_wrap 的 ref 出参两路同发：拒发即 addon 对象无根（2026-09-15，M5 终线）

- 症状：任意对象路 napi_wrap 对非空 `result`（napi_ref 出参）报
  INVALID_ARG——napi-rs 对缓存跨回调复用的函数（PromiseRaw.then/catch 产物）
  恒带此参；拒发后对象无 ref 保活。
- 修法（`class.rs`）：任意对象路同样发初始计数 0 的 ref（类实例路同款）。
- 推广为铁律：Node 语义里"跨 scope 存活的 napi_value"只有 ref 一条正道；
  宿主对 ref 出参拒发 = 逼 addon 裸持 = 悬垂。fail-fast 前先想 addon 有无
  正当用法（§4.66 的 fail-fast 被 M4/M5 实战两次证伪为缺口）。

### 4.77 pin-all（槽位不截断）反例：finalize 链整体死亡（2026-09-15，M5 终线）

- 症状：为根治"悬垂槽位读"试 pin-all（arena 只增不截断）——external/wrap
  对象被槽位永久钉住，永不可达死态，**finalizer 从此永不跑**
  （m2 drain 探针 `free` 恒 0），external 内存无底洞泄漏（rolldown 每
  transform 产 buffer 即中招）。
- 根因：截断不只是"Node 值域契约"，更是 finalize 链的前提——槽位是 GC 根，
  不截断则回调产物永不可达死态（M2 头注早有预言，本轮实证）。
- 修法：回退截断（trampoline/dispatch/scope close 三处恢复），addon 跨窗持有
  走 §4.76 ref；测试侧 `m2_finalize` 改 drain 形（async_work 安全点回吐计数）。
- 推广为铁律：改内存管理语义前先问"finalizer 何时跑"——凡 GC 根面（槽位/表），
  收回 = 析构通道，两头（泄漏 vs 悬垂）都通向事故，正解只有 Node 口径
  （值随 scope、跨 scope 走 ref）。

### 4.78 GC sweep 内禁 addon finalizer：入队 + 安全点排空（2026-09-15，M5 终线）

- 症状（疑点，未单独复现钉死）：`napi_class_finalize` 在 GC sweep 内直调
  addon finalizer——napi-rs 的 finalizer 会经 `napi_delete_reference` 等改
  env 表 + Heap clearing barrier，即 GC 期间写堆，GC 元数据腐坏风险。
- 修法（`class.rs`/`buffer.rs`/`asyncwork.rs`/`lifecycle.rs`）：sweep 内只
  **入队** `pending_finalizers`（纯 Rust push，不碰 JSAPI/不写堆）；
  `asyncwork::dispatch` 入口（`class::drain_pending_finalizers`）/
  end_session 安全点（`lifecycle::run_wrap_finalizers`，JS 线程、无 GC 活动期）排空。
  external AB 的 contents 释放随 cb 一并延迟（external 语义：data 归 addon）。
- 复现：m2_finalize drain 形（GC 期入队 → 安全点全量 free）。
- 推广为铁律：GC 回调（finalize op）内只许纯 Rust 簿记；凡会触 JSAPI/写堆/
  改 GC 根面的 addon 回调，一律队列化到安全点。Node 的 finalizer 上下文限制
  （只许 napi 簿记面）在宿主侧必须由"延迟排空"落实，不能指望 addon 自律。

### 4.79 截断语义下跨回调裸持 napi_value = 悬垂，call_impl 先验 func 形态（2026-09-15，M5 终线）

- 症状：fixture 裸 static 存 `napi_value` 跨回调（无 ref），下一次
  `napi_call_function` 的 func 读出非 object（tag 0xfff9 系）——直接进
  JSAPI 即崩（bad func → JIT/解释器读垃圾对象）。
- 修法：`call_impl` 入口先验 `fn_v.is_object()`（tag 检查不 deref），非 object
  返 INVALID_ARG + last_error（"stale napi_value?"）——Node 同款是 UB，本仓
  给可读错当现形点；fixture 侧改 `napi_create_reference`（Node 正道）。
- 复现：m2_finalize drain 探针（ref 前必现 BAD-FUNC，ref 后干净）。
- 推广为铁律：§4.31 教训的 napi 版——跨 JS/Rust 边界的句柄生命周期，契约
  （scope 存活期/ref）违者宿主要能"可读地死"而非 UB；新增宿主入口先想
  "输入是垃圾时怎么死"。

### 4.80 CJS 包装五连链的裸 JSVal 栈拷贝：GC 搬移即悬垂（2026-09-15，M5 终线②）

- 症状：vitest worker 线程（fork 底座）加载 css-tree 深图必现 138/139，
  exit 1 静默（scripts 父进程把信号死亡映射成 1，`echo $?` 全程骗人）。
  lldb 实锤：`require_cjs_file` 闭包 → `call_one(cur, arg)`，cur/arg 位型
  0xFFF8/0x5800 垃圾，`js::Interpret` 写穿 KERN_PROTECTION_FAILURE。
- 根因：五连柯里化调用链里 `cur`（wrapper 函数）与 exports/require/module/
  filename/dirname 实参全是裸 JSVal 栈拷贝——`call_one` 入口 rooting 只保
  **调用中**，不保**调用间**；链内每次 call_one 都分配 curried 闭包可触发
  GC，nursery 搬移后栈拷贝即悬垂。§4.40/§4.68 完整形态第 N 例。
- 修法：cur 与各实参全程 `rooted!` 槽位、调用点现读现传（`.get()` 后无
  JSAPI 直入 call_one 入口 rooting，窗口为零）。
- 复现：修前 `--run node_modules/.bin/vitest` 必 138；修后全链通。
- 推广为铁律：凡"多次 JS 调用组成的链"（CJS 包装/遍历/reduce 式），链上
  中间值一律 rooted——call_one/call_two 的入口 rooting 不是链的保活凭证。
  另：父进程把子进程信号死亡映射 exit 1 会吞掉整个崩溃类，结论打架先看
  真实退出码（`--run bin` 直跑拿原始 rc）。

### 4.81 require 条件族：resolver 必须按调用方分流（2026-09-15，M5 终线②）

- 症状：`require('magic-string')` 拿到 ESM namespace，`new MagicString()`
  报 is not a constructor（真 node 正常）。
- 根因：resolver 单例写死 `["node","import"]` 条件——require 走 import 条件
  挑了 ESM 入口。真机口径（26.8.2 对拍）：require 严格走 `["node","require"]`，
  imports-only 包直接 `ERR_PACKAGE_PATH_NOT_EXPORTED`（无 import 回落）；
  require(esm) 只作用于"已解析文件是 ESM"的情形。
- 修法：`resolve.rs` 条件族双单例（`Cond::Import/Require`）+ `resolve_require`
  入口，require.rs 四调用点（require_value/resolve/resolve_from/静态名跟随）
  全切。
- 推广为铁律：module resolution 的条件族是调用方属性（import vs require），
  共享 resolver 必须参数化；"require 拿到 ESM 就垫 default"类的互操作补丁
  在双条件包面前全是错药。

### 4.82 语义升级（require(esm)+detect-module）后旧边界断言全翻转（2026-09-15，M5 终线②）

- 症状：require(esm) 上线后 `phase4_require_errors`/`phase9k` 静默挂
  （"ESM 拒绝"断言反绿为红）；`phase9j` 具名发现 fixture（自定义 `__es(...)`
  转出）link 期报缺导出。
- 根因：① 4dbb202 上线 require(esm)/detect-module 时未 grep 旧"必须抛"
  断言（§4.72 二进宫）；② 自定义转出包装真机 cjs-module-lexer 同样不识别
  （实测：静态 import `__es` 形 link 期同败），我们的静态 lexer 行为已对等。
- 修法：断言翻转对真机逐项实测（typeless ESM .js require 成功、__exportStar
  形真机识别——fixture 改该形 + 本地补 helper 定义）。
- 推广为铁律：语义升级的收尾动作 = grep 全部旧断言逐个对真机；fixture 的
  "聪明写法"若真机不认，宁可改 fixture 也不给宿主加超集。
  附：`-r test:unit` 里 hello.test.js（node:test）在 vitest 4 下真机同败
  （"No test suite found"，exit 1）——真机对等即验收，别替 fixture 修世界。

### 4.83 无编码 fs 读必须返回 Buffer：String(buf) = utf8 内容（2026-09-15，M5 终线②）

- 症状：vite PostCSS 配置加载报 `JSON.parse ... column 4` 假错。
- 根因：`readFileSync`（无编码）返回裸 Uint8Array——`String(buf)` 走
  TypedArray join 成 "byte,byte,…"，JSON.parse 隐式 ToString 后把字节列表
  当 JSON（`{` 的字节 123 → 解析出 "123" 后 column 4 报错）。
- 修法：`__fsDecode(encoding=null)` 统一 `Buffer.from(bytes)`（Node 语义：
  isBuffer/toString()/String(buf) 全 utf8 口径）；两调用点（sync + fh 读）
  一处收口。
- 推广为铁律：Node API 返回"Buffer 的地方"必须是真 Buffer（Uint8Array 子类
  不够——toString/String/isBuffer 三面都要对）；裸 TypedArray 与 Buffer 的
  差异隐在隐式转换里，JSON.parse(buf) 是现成探针。

### 4.84 napi_get_cb_info 余槽必须填 undefined（node Args() 契约）（2026-09-15，M6）

- 症状：官方 js-native-api 3_callbacks 在本仓 139——addon 在 `argc==1` 断言
  后照读 `args[1]`（请求 2 槽、实际 1 参）。
- 根因：node 的 `FunctionCallbackWrapper::Args()`（js_native_api_v8.cc）
  把请求槽位余下部分**全填同一 undefined**——这是书面契约外的硬契约，
  官方套件直接依赖；我们的实现只填实际个数，余槽留 addon 栈垃圾 = UB 读。
- 修法：照抄 node——`put(UndefinedValue())` 单槽共享填满请求槽位，`*argc`
  回写实际个数。
- 复现：`tests/napi.rs::phase_napi_m6_official_js_native_api_spot_check`
  （官方 2_function_arguments + 3_callbacks 原文 verbatim，vendored 头编译）。
- 推广为铁律：宿主实现 napi 面时，"官方套件怎么写"本身就是契约的一部分——
  选点回归（M6）不是仪式，是抓这类暗契约的唯一网；遇 addon 读"没给出的
  参数"先查宿主填充语义再骂 addon。

### 4.85 定时器实参从未展开：`fire_due` 直调 + 缓存的 helper 从未被读（2026-09-15，10a）

- 症状：`setTimeout((a, b) => ..., 0, 'x', 7)` 的回调收到 `(cb自身, args数组)`，
  而非 `('x', 7)`——定时器实参透传全灭。
- 根因：`fire_due` 用 `JS_CallFunctionValue(fun, [cb, args])` 直调回调
  （§4.9 禁 `Rooted<ValueArray>` 的语境下写错了调用形状）；而 prelude 的
  `__wjs_call(cb, args)` 展开器虽有缓存位（`RootedState::call_fn`，runtime 起
  即 set + trace），却没有任何读者——set 时没人接线，读侧永远直调。
  旧用例全用闭包传参（`setTimeout(() => resolve(v))`），躲过全部回归。
- 修法：`fire_due` 经 `call_two(cx, global, call_fn, cb, args)` 走 `__wjs_call`
  展开（`src/builtins/timers.rs`；`call_two` 的 UNSAFE-BOUNDARY 覆盖 + 本条）。
- 复现：`tests/builtins.rs::phase10a_immediate_and_timeout_class`
  （`args x 7` 行；修前为 `args <fn> x,7`）。
- 推广为铁律："缓存了" ≠ "用上了"——新增缓存位必须同时提交第一个读者，
  reviewer 按"set 有无 get"逐项对；回调传参形态（闭包 vs 实参）是两种覆盖，
  只测一种等于没测。
- 附带（同批 10a-3）：全局 `setImmediate` 缺失即补（setTimeout(0) 近似，
  check 语义记档）；Timeout/Immediate 改真类（constructor.name 真机口径，
  类本体不出 prelude 块作用域——真机 `globalThis.Timeout === undefined`，
  污染全局即错）；plan.md 9b"裸 number"记档同步勘误（M5 起已返回对象）。

### 4.86 Writable 默认 `autoDestroy:true` 会杀保活连接（2026-09-15，10b）

- 症状：客户端请求发出去服务端永远收不到（`ref-got` 缺席），`_final` 后跟一条
  无来由的 `_destroy`（堆栈：`finish@writable → destroy@http_framing`）。
- 根因：`autoDestroy` 默认 true——upload 一 finish 就自动 destroy，而本仓
  `_destroy` 负责杀 socket（连接未建好即杀，请求死在半路）。
  连带：`emitClose` 默认 true 会在 upload finish 后自动发 req 'close'，
  与 9d 口径（close 在响应收齐后）冲突，造成双 close。
- 修法：`ServerResponse`/`OutgoingMessage` 一律
  `super({ autoDestroy: false })`（销毁只显式：server.close/agent.destroy/
  用户 destroy/对端死亡）；`OutgoingMessage` 另加 `emitClose: false`，
  req 'close' 由 `__finishResponse`/`__onSockCloseEv` 手动在 res 'end' 后发。
- 复现：任意 `http.get`（修前服务端零收到；`tests/node/http.rs` 全挂）。
- 推广为铁律：凡把"连接"装进 stream 壳，`autoDestroy/emitClose` 默认值先按
  连接语义重审一遍——流的生死 ≠ 连接的生死。

### 4.87 增量泵的"等更多数据"必须带空 rest（2026-09-15，10b）

- 症状：GET→POST 交界 flaky hang（约 1/3；单 POST 全过，加日志全过）。
- 根因：`__pumpChunked` 等数据的 `return { done: false }` 不带 `rest`，
  调用方 `st.buf = r.rest` 把缓冲置 `undefined`——下个包一到
  `__concat(undefined, …)` 抛错，外层 catch 直接 `sock.destroy()`。
  包不拆就全到（`done:true` 带 rest），一拆就炸，故 flaky。
- 修法：6 处 `return { done: false, rest: new Uint8Array(0) }`（余字节已进
  `fr.buf` 内部态，调用方覆盖为空即对）；CL 泵本就带 rest，无事。
- 定位手法：可复现的 ping-pong 循环（20 轮，成功打点、卡住即停）比反复跑
  原测试更快——`pp.mjs` 式"窄探针 + 全事件追踪"三轮即抓到（本次还抓到
  `cli-close` 早于服务端完工，顺藤摸到 destroy 链）。
- 推广为铁律：增量解析器的返回形状必须全字段齐备（含"无进展"分支）；
  flaky hang 先怀疑包边界，再怀疑逻辑（`pp.mjs` 留档思路，不入库）。

### 4.88 TLS 写半部 drop 不发 close_notify，双边读端永 block（2026-09-15，10b）

- 症状：https keep-alive 功能全对（复用计数/`reusedSocket` 全绿），但进程
  永不退出（lsof 双边 ESTABLISHED；destroy 全送达、purge 全缺席）。
- 根因：tokio TCP 写半部 drop 自带 FIN，tokio-rustls 写半部 drop 不发
  close_notify——destroy 后双边读端各 block 各的，Close 永不到（9d 纯 TCP
  从未暴露）。
- 修法（`src/builtins/node/net.rs` writer task）：`NetCmd::Close` 路径先
  `w.shutdown().await` 再 break（TLS 发 close_notify + 关 TCP 写；
  TCP 侧与 drop 语义重复，无害）。
- 连带（同案第二漏）：对端 FIN 到达**池化空闲** socket 时只发 End（写端活着
  不收尾），而池 socket 永不会再有人 end——`net.js`/`tls.js` 的 end 处理器
  首行加池检查：`__inPool` 即 destroy（半关不可复用，Node 同样踢出池）；
  Agent 侧 `__release/__acquire` 置 `__inPool` 位；泵层加 `reader_done` 旗，
  writer 退出时读端已走则补 Close（`close_once` 防双发，与既有 EOF 路径收敛）。
- 复现：`tests/node/https.rs::phase10b_https_keepalive_reuse`（修前永 hang；
  http 同形不 hang 是 TCP FIN 掩盖，不是语义对）。
- 推广为铁律：传输换底座（TCP→TLS）后，"关闭收敛"必须重走一遍——drop/
  shutdown/EOF 三者的底座语义各不相同，9d 的 TCP 经验不自动继承。

### 4.89 `end()` 与 connect 竞速：`_final` 时 socket 未就绪则请求永不发出（2026-09-15，10b）

- 症状：`http.get` 后服务端零收到、无报错、无 error，进程空转到 watchdog。
- 根因：`Writable.end()` 同步走完 `_final` 时 socket 尚未连通（连通要过事件
  循环），旧代码只管"连通时发出"，未连通分支置了 `pendingFinal` 旗却无人消费。
- 修法：connect/secureConnect/复用 attach 三处统一收敛——先 `__tryFlush()`
  （holdback 转 chunked），再消费 `pendingFinal`（`__flushFinal`：头未发走 CL
  快捷，已发补 0-chunk；漏后者即 POST 交界 hang，见 §4.87 同源）。
- 推广为铁律：凡"构造即发"（请求/连接）遇"异步就绪"（connect/握手），就绪
  回调必须同时服务"已就绪数据"与"已结束标记"两件，缺一件即半吊子挂起。
  定位时先分清"没发出去"（服务端零收到，curl 对照）还是"没解析出来"。

### 4.90 vm 重跑即炸：sync-in 的重定义必须回落赋值（2026-09-15，10c-3）

- 症状：同 context 第二次 `runInContext` 即 `vm could not define sandbox property`
  ——当前轮新建了全局（function/var 声明）时必发；纯求值重跑无事。
- 根因：每次 run 前 `__syncIn` 把沙箱属性逐个 `JS_DefineProperty` 重打一遍；
  上轮 sync-out 又同步回来的新全局（值为跨 compartment CCW）在重定义时失败。
  纯数据种子（`{x:1}`）重打无事，故 9i 旧测试全绿掩盖（REPL 是首个高频复用者）。
- 修法：`vm_set` 里 define 失败即回落 `JS_SetProperty` 赋值语义（更新值、
  保留既有描述符；双失败才抛）；收敛函数 `set_prop_value` 进 `jsapi_glue`
  （`UNSAFE-BOUNDARY` + 覆盖测试名）。
  附带同案：DONT_CONTEXTIFY 全局跑 `runInContext` 首轮即炸（sync-in 逐个重打
  标准内建）——同修法一并治愈。
- 复现：`tests/node/vm.rs::phase10c_vm_rerun_with_new_globals`（修前第二跑必炸）。
- 推广为铁律：凡"每次调用前全量同步"的设计，必须回答"已存在项怎么办"——
  define-if-absent + set-if-present 两条路，缺回落即二次调用必炸；
  旧测试只跑一次的面，新功能复用即现形（§4.24 多 run 教训的 vm 版）。

### 4.91 turso 懒执行 vs Node prepare eager：校验走只备不步进的 cols（2026-09-15，10d）

- 症状：`db.prepare("NOPE SYNTAX @@")` 不抛（真机 `ERR_SQLITE_ERROR` 在 prepare 期）。
- 根因：turso `conn.query` 只备语句不步进——坏 SQL 要到首次 `next()` 才暴露；
  本仓 prepare 纯 JS 构造，从不碰 Rust，错误自然延迟到 run/get/all。
- 修法：Rust 加 `Cols` op（只调 `query()` 取 `columns()` 元数据，不 `next()`，
  无副作用、INSERT 亦不执行）；`DatabaseSync.prepare()` 内先调一次校验，
  坏 SQL 即抛；`columns()` 复用同一 op。
- 附带同案：hickory `Name.to_string()` 带 FQDN 尾点（`localhost.`）——Node 口径
  无尾点，Rust 侧统一 `trim_dot`（`src/builtins/node/dns.rs`）。
- 复现：`tests/node/sqlite.rs::phase10d_sqlite_errors_boundary`（`prep-err` 行修前缺席）。
- 推广为铁律：宿主底座"懒"的面（prepare/query 构造），对齐 Node eager 语义时
  必须显式加一次无副作用的校验调用；校验调用本身不得有副作用（不步进/不执行）。

### 4.92 RustCrypto 泛型顺序与 AEAD 两侧：ccm 的 M/N 反直觉 + 解密 GHASH over 密文（2026-09-15，10e）

- 症状一：`ccm::Ccm<Aes128, U12, U8>` 编不过（`SealedTag for U12` 不满足）。
- 根因：`Ccm<C, M, N>` 的 M=tag 长、N=nonce 长（文档注记，非直觉顺序；
  例 `Ccm<Aes256, U10, U13>` 是 tag 10 + nonce 13）。
- 修法：分发宏按 `(key → tag → nonce)` 逐级 match（`src/builtins/node/crypto.rs`
  `ccm_crypt`，注释写明顺序）。
- 症状二：GCM-J0 手工路径加密与真机逐字节一致，解密自家密文即拒收。
- 根因：GHASH 的输入写成了解密输出（明文）——tag 是 over **密文**算的，
  加解密两分支必须各取各的输入（`gcm_manual` 的 `(gct, glen)` 分流）。
- 复现：`crypto_gcm_j0_known_answer`（修前解密断言挂；加密向量先绿极具误导性——
  加密绿 ≠ 路径对，AEAD 必须加解密双向断言）。
- 推广为铁律：AEAD 手工路径的测试必须含"自加密自解密 + 真机交叉"双向；
  泛型顺序以库文档注记为准，不按直觉（Ccm/Ctr32BE 这类双长度泛型先查）。

### 4.93 后台 sleep/kill 判 hang 必看退出码（2026-09-15，10e，§4.62/§4.67 姊妹篇）

- 症状：cluster 探针 `sleep 10; kill` 后输出停在中间，误判事件循环 hang。
- 根因：进程早已正常退出（EXIT=0），sleep 到点 kill 扑空——输出截断是杀时
  机问题，不是 hang；直接跑取 `$?` 即见 0。
- 修法：凡判 hang，先直接跑取退出码（+ 超时门）；后台法只用于必现 hang 的
  堆栈/输出采集，且一律落盘读。
- 推广为铁律：无退出码的 hang 结论不可信；自然退出（idle 收敛正确）与 hang
  在输出上看起来一样，区分只认退出码。

### 4.94 park 唤醒集与存活判定集必须分家：unrefed 到点要醒、不续命、不算 progress（2026-09-15，10f timers）

- 症状三连：① unrefd-interval-still-fires 的 1ms unrefed interval 迟到 1s 才触发（睡到 refed 看门 1s 到点）；② unref.js 的 1ms unrefed interval 空转整整 10 秒（单轮 pump ≈1.1ms > interval 1ms，每轮都到期 → `progressed` 永真 → 永不 idle，LONG_TIME mustNotCall 必炸）；③ unrefed-in-callback 永 hang（callback 内 `unref()` 时 entry 已摘表，native noop，重排丢旗 → interval 恒 refed）。
- 根因：park 目标、存活判定、progressed 三个概念混用同一个 `next_deadline`/timers 计数。node 口径：uv poll 超时含 unrefed timer（要醒）、`uv_loop_alive` 只看 refed（要退）、unrefed 触发不算续命（防转），但其回调的 microtask 仍要一轮 RunJobs（§4.18）。
- 修法：`next_wake()`（含 unrefed，只给 park）与 `next_deadline()`（refed-only，给 idle）分家（`src/builtins/timers.rs` + `runtime.rs` park 点）；`PumpStats` 拆 `timers`/`timers_unrefed`，progressed 只数 refed，`idle && unrefed-only` 给**一轮** grace continue 后即 break；unref 旗加 plain 侧账 `unrefed_ids`（触发期 unref 落账，重排时并读）。
- 复现：`tests/builtins.rs::phase10f_timer_face_unref_uncaught`（修前分别迟到 1s/空转 10s/永 hang）。
- 推广为铁律：凡事件源带 ref 语义，"到点唤醒"与"存活判定"必须显式分家；progressed 只统计能续命的源，任何触发的 microtask 都要一轮 RunJobs 兜底——§4.18 完整形态的 ref 版。

### 4.95 宿主调用户回调一律 Reflect.apply，禁走实例属性查找（2026-09-15，10f timers）

- 症状：user-call 套件挂——回调 `fn.call/.apply` 被猴子补丁成字符串后，`self._onTimeout.apply(...)` 报 not a function（node 同套件全过）。
- 根因：实例上的方法查找先命中自有补丁；node 用内部 ReflectApply（内建，无视补丁）。
- 修法：prelude timers 触发改 `Reflect.apply(self._onTimeout, self, self._timerArgs)`（实参也 live 读，套件 unenroll 直改字段同源）。
- 推广为铁律：宿主侧凡"以方法形式"调用用户提供的函数，一律走 Reflect/内建引用；用户对象上的同名属性是它的数据，不是我们的调用机制。

### 4.96 CJS→ESM 移植的 primordials 残留：未定义符号在错误路径换错误类型（2026-09-15，10f timers）

- 症状：promises-scheduler 套件报 `rejects: unexpected throw`——真因是移植稿里 `return PromiseReject(...)`（node primordials 名）未随行，运行时 ReferenceError，assert.rejects 把它当 mismatch 报——两层错掩盖一层。
- 修法：`Promise.reject(...)`；移植面收尾 grep primordials 名单残留（PromiseReject/PromiseResolve/NumberIsNaN/MathMax…）。
- 教训：错误文案不可信（§4.66 Either 兜底同源）；"mismatch" 类报错先打印实际 rejection 值再动断言。

### 4.97 全局单例的方法族必须 this 基；`--run` 完成值回显随 CJS 主模块化消失（2026-09-15，10f timers）

- 症状一：process-tampering（`globalThis.process = {}` 后 setImmediate）——node common 载入期 `const process = globalThis.process` 捕获真身，我们的 on/emit 等方法体读**全局** process 绑定，tamper 后 `undefined["exit"]` 炸。
- 修法：process 监听器族方法一律 `this` 基（`__wjs_emit` 顺带回监听数，供 `__wjs_uncaught` 判"是否已处理"——探针为 0 时 native 保持 pending 原样走 fatal，错误信息不降级）。
- 症状二：`run_file_from_tempdir` 断言 `--run` 回显完成值 `42`——b701cf6 typeless .js 入口走 CJS require 主模块后无 rval，回显消失（HEAD 实测同红，非本轮回归）。真机 `node app.js` 本就无回显——静默才是对等，测试改执行效应断言（§4.82 老断言翻转）。
- 推广为铁律：全局单例的方法族，方法体禁解引用全局绑定（`this` 或载入期捕获二选一）；"回显类"断言在入口语义升级后逐个对真机。

### 4.98 自写编码器不能指望 `encodeURIComponent` 兜底：unreserved 恒放行（2026-09-16，10f url）

- 症状：`pathToFileURL('/foo~')` 出 `file:///foo~`，套件期望 `%7E`；条件表里明明有 `ch === '~'`。
- 根因：`encodeURIComponent` 对 unreserved（`~ ! ' ( ) * - . _` + 字母数字）**恒原样放行**——
  条件命中了，编码函数却吐回原字符。同理 `noEscapeAuth` 逐字符手工编码里 `%XX` 由
  `toString(16)` 生成是安全的，但混用 `encodeURIComponent` 的分支必须逐字符核对其放行集。
- 修法：被收字符里凡 unreserved 者手动给字面量（`out += ch === '~' ? '%7E' : encodeURIComponent(ch)`）。
- 推广为铁律：凡"自选编码集 + 借 encodeURIComponent 执行"的编码器，放行集 = 自己的表
  ∩ encodeURIComponent 的放行集，交集外字符一律手动字面量；新增编码集先对真机
  逐字符 diff（本轮 `~`/`^`/`|`/`[`/`]` 全是套件期望反推出来的）。
- 复现：`test-url-pathtofileurl.js` 尾部 `'/foo\r\n\t<>"#%{}|^[\\~]`?bar'` 用例（修前 `%5C~`）。

### 4.99 `assert.throws` 函数形期望：`instanceof` 必须以 Error 子类为门（2026-09-16，10f url）

- 症状：`assert.throws(fn, (e) => e instanceof URIError)` 报 "unexpected throw"，
  校验器**从未被调用**（打点实证）；同一校验器直接调用全过。
- 根因：旧实现先做 `e instanceof expected` 再回落校验器——箭头函数无
  `prototype`，`e instanceof arrow` 按 OrdinaryHasInstance 取 `C.prototype`
  即抛 TypeError，被外层 `catch { ok = false }` 整体吞掉，校验器永不到达。
  类（有 prototype）不受影响，故旧套件全绿掩盖。
- 修法：`expected.prototype !== undefined && expected.prototype instanceof Error`
  才走 instanceof（node 口径：Error 子类 = 构造器形，其余 = 校验器形）；
  instanceof 失败仍回落校验器调用。
- 复现：`tests/node/url.rs::phase10f_url_parity_suite`（`urierr` 行；修前 REJECTED）。
- 推广为铁律：对"函数既可能是构造器也可能是校验器"的双形态参数，形态判定
  （prototype 链）必须先于使用形态的运算符；`instanceof` 右侧无 prototype
  是抛错不是 false，凡 try/catch 包 instanceof 都要想到这一层。

### 4.100 肉眼同形异码点：探针先核对码点，NFKD/NFKC 跟 UTS46 走（2026-09-16，10f url）

- 症状一：探 `'℀'` 用 `\u2440` 白转一轮（`instanceof URIError` 校验块其实早修好了）——
  U+2100 与 U+2440 打印**一模一样**（都是 ℀），但 NFKD 分解迥异
  （U+2100→`a/c` 真斜杠、U+2440 不分解）；套件用的是 U+2100（hexdump 才实锤）。
- 症状二：IDNA 映射用 NFKD 后 punycode 出 `xn--bucher-xyd`，真机期望
  `xn--bcher-kva`——分解态没做**规范组合**，`u+◌̈` 没回到预组合 `ü`。
- 修法：IDNA 标签走 `normalize('NFKC')`（NFKD 分解 + 组合，两套件关注点都覆盖：
  badIDNA 靠分解段、punycode 形靠组合段）；ignored 码点（软连字符 U+00AD）删除
  后空标签即抛。
- 推广为铁律：凡"同形字符"对比实验（对拍/探针/fixture），先 `hexdump`/码点核对
  再下结论；Unicode 归一化选 NFD/NFKD/NFC/NFKC 不是口味——语义对齐哪个标准
  （UTS46=分解+组合）就用哪个，分解态直接喂下游（punycode/hex 表）必错形。

### 4.101 `exit`/`close` 事件 node 是双参 `(code, signal)`；spawn 默认 stdio 是 pipe（2026-09-16，10f url）

- 症状：`spawnPromisified` 解构 `close(code, signal)` 全收 undefined（`{status:1}`
  透进断言）；`child.stderr.setEncoding` 报 not a function / is null。
- 根因（三连）：① 派发把 `{status, signal}` 单对象当唯一实参（fork 文档曾把它
  合理化为"与 spawn 同形"——两处错互相印证≠对）；② spawn 默认 stdio 写成
  `inherit×3`（node 缺省 `pipe×3`，数组缺项也补 pipe）；③ stdout/stderr 给的是
  Web ReadableStream（无 `on('data')/setEncoding`）——自家黑盒用 `getReader()`
  编码了实现偏差（§4.65 同源）。
- 修法：派发走 `call_two(handler, status, signal)`；fork 路径同翻；访问器 wrap
  改双参落定 exitCode/signalCode；默认 stdio pipe；流面改 legacy Readable
  （`on/once/off/setEncoding/pause/resume/destroy`，data 缺省 Buffer
  （§4.83）、setEncoding 后为串），自家黑盒 2 处 `getReader()` 同步翻转。
- 推广为铁律：事件回调的**实参形状**是跨边界契约（用户代码逐名解构），移植时
  以真机签名逐字对拍，不做"对象打包"的自作主张；旧文档的"同形"引用链要溯源
  到真机，不能拿自家另一处偏差当依据。

### 4.102 `process.emitWarning` 是 nextTick 异步派发（2026-09-16，10f url）

- 症状：`test-url-parse-deprecation` 的"先 `url.parse('foo')` 后
  `expectWarning`（挂监听）"序列在同步派发下警告丢失（监听挂上前已走 stderr）。
- 根因：node 口径 warning 经 nextTick 异步派发（真机实证：emitWarning 后同步
  读收集器为空、setImmediate 后才见）——同步派发是实现偏差；10f timers 的
  `phase10f_timer_face` 黑盒同步收集 `warn []` 也在全量回归现形。
- 修法：`process_.rs` emitWarning 改 `queueMicrotask` 派发（监听列表**现读**，
  microtask 前挂的监听有效）；timers 黑盒收集点同步移到 await 之后（真机
  口径翻转）。
- 复现：`tests/node/url.rs::phase10f_url_parity_suite`（`dep0169` 行）+
  `tests/builtins.rs::phase10f_timer_face_unref_uncaught`（`warn` 行）。
- 推广为铁律：凡 node 文档写明"异步派发/异步回调"的面（warning/写入回调等
  §4.74 同族），即便宿主能同步完成也必须异步触发；同步完成的便捷性不是契约，
  套件时序（先触发后挂监听）就是按异步写的。

### 4.103 Proxy 包 Uint8Array 必须透传 newTarget，否则子类化全灭（2026-09-16，10f buffer）

- 症状：`Readable.fromWeb` 回的 chunk `constructor.name` 是 `Uint8Array` 非
  `Buffer`（`phase9b_stream_web_and_consumers` 的 `f2w 1 Uint8Array`；真机 `Buffer`）。
- 根因：`Uint8Array` 文案桥 Proxy 的 `construct(target, args)` 未收第三参
  `newTarget`，`Reflect.construct(target, args)` 即按基类构造——`class X extends
  Uint8Array`（含内部 `FastBuffer` 的显式 `super(...args)` 与空构造器两形）
  全灭为基类原型。注释还写了"extends 透传不受影响"，想当然，未实测。
- 修法：`construct(target, args, newTarget)` + `Reflect.construct(target, args,
  newTarget)`（`src/builtins/mod.rs`）。
- 复现：`new (class E extends Uint8Array {})(4)` 的 `proto===E.prototype`
  （修前 false；`tests/node/buffer.rs::phase10f_buffer_parity_fixes` 的 `subclass` 行）。
- 推广为铁律：凡包装全局构造器的 Proxy，`construct`/`get` 的后位参
  （`newTarget`/`receiver`）默认透传，不确定即全透；"不受影响"类断言必须配
  子类化探针（`extends` + `new` + `proto===` 三件），不能只测直接构造。

### 4.104 伪 ArrayBuffer 须品牌拒收，不能信 tag（2026-09-16，10f buffer）

- 症状：`Buffer.from(fakeAB)`（`Object.setPrototypeOf(AB, ArrayBuffer)` 伪造）
  报 `incompatible Object` 引擎文案，与套件 `ERR_INVALID_ARG_TYPE / an instance
  of AB` 对不上。
- 根因：`__wjs_bufIsAnyAB` 只看 `instanceof` + `toString` tag——伪造链两者全过；
  真机走 V8 `IsArrayBuffer` 内部槽检查，伪造即拒。直接进 `FromArrayBuffer` 即在
  引擎内抛 incompatible，断言对不上。
- 修法：`Buffer.from` 的 AB 分支先 `void value.byteLength` 试探（真槽可读，
  伪造抛），失败落空到尾部统一 invalid-arg（`__wjs_bufSpecificType` 的
  `an instance of AB` 口径正好对上）（`src/builtins/mod.rs`）。
- 复现：`tests/node/buffer.rs::phase10f_buffer_parity_fixes` 的 `brand` 行；
  套件 `test-buffer-arraybuffer.js`（修前 `incompatible`）。
- 推广为铁律：凡"is-X"判定走 `instanceof`/tag 的，伪造原型链即视为已撞——
  关键入口（`from`/`isUtf8` 等）必须加一次内部槽试探（读 `byteLength`/`slice`
  等）；§4.54"看 OID 不看坐标"的 JS 品牌版。

### 4.105 全局 structuredClone 的 transfer 须 detach，否则 isAscii 视残留为真（2026-09-16，10f buffer）

- 症状：`isAscii`/`isUtf8` 的 detach 段修前 `after false false false`
  （应全 true）——`structuredClone(ab, {transfer:[ab]})` 后 `ab.byteLength` 仍 1。
- 根因：`src/builtins/clone.rs` 的 JSON 中转实现完全忽略 `options.transfer`，
  只克隆不 detach；`__wjs_bufAsU8` 的 detached 容错（视空）永无触发机会。
- 修法：`structured_clone` 入口先走 transfer 列表（`transfer` 数组 + 
  `IsArrayBufferObject` 品牌 + `DetachArrayBuffer`，失败跳过不致命），再走既有
  JSON 中转（返回值仍中转语义，detach 系副作用为准）。
- 复现：`tests/node/buffer.rs::phase10f_buffer_parity_fixes` 的 `detached` 行；
  套件 `test-buffer-isascii.js`/`test-buffer-isutf8.js`（修前 `Expected false
  strictEqual true`，行号恒 `254:53` 系 common 断言包装）。
- 推广为铁律：凡"带选项的克隆/投递"（transfer/neutering），副作用（detach）
  与返回值同等重要；JSON 中转实现上线新选项必须先问"源端状态变了吗"。

### 4.106 池 AB 不可转移：共享要池化、转移要拒收，两件缺一即挂池套件（2026-09-16，10f buffer）

- 症状：`test-buffer-pool-untransferable.js` 修前 `a.buffer !== b.buffer`
  首断言即挂（本仓直接分配，无池）。
- 根因：Node 小串（`< poolSize/2`）走 64KB 池（`fromStringFast` 口径，8 字节对齐，
  满即新池），池 AB 经 V8 `markAsUntransferable` 标记——`postMessage(..., [pool])`
  抛 `DataCloneError(25)`、`pool.transfer()` 抛 `TypeError`，且事后不 detach。
  本仓三件全缺。
- 修法（纯 JS + worker 拒收，零新依赖）：prelude 建池（`__wjs_bufPoolAB`/
  `__wjs_bufPooled: WeakSet` 全局暴露/`__wjs_bufPoolOffset` + 对齐/满转）+
  `fromStringFast` 小串走池（`scratch` 视图写入 + 实长推进）+
  `ArrayBuffer.prototype.transfer` 对池内抛 TypeError +
  worker `__normTransfer` 对池内抛 `DataCloneError(25)`（`__dataCloneErr` 补
  `code=25`；视图取 `buffer` 同判）。
- 复现：`tests/node/buffer.rs::phase10f_buffer_parity_fixes` 的
  `pool-share/post/still/xfer/still2` 五行；套件修前首断言挂、修后 `0/0 ✅`。
- 推广为铁律：凡"性能优化有可观察共享"（池化/缓存/复用），对拍前先查"共享 +
  不可转移/不可变"二元组——只做共享不做拒收，转移套件必挂；`code`（25）与
  `name` 同为契约，补一漏一即红。

### 4.107 `util.inspect` depth -1 空容器显体 + 函数不走 primitives（2026-09-16，10f buffer）

- 症状：`test-buffer-from.js` 的 `{__proto__:null}` 期望
  `[Object: null prototype] {}`，本仓 common  helper 算出 `[Object]`——
  `Buffer.from` 的实际报错是对的（`__wjs_bufSpecificType` 口径），期望串错了，
  两边对不上。
- 根因（二连）：① `inspect2` 的 `depth<0` 一刀切回 `[Array]`/`[Object]`——
  真机空容器仍显体（`[]`/`{}`/`Foo {}`/`[Object: null prototype] {}`），非空才
  显 `[Prefix]`（`[Object]`/`[Foo]`/`[Object: null prototype]`/`[Array]`/
  `[Map]`/`[Set]`/`[Uint8Array]`）；Date/Error/RegExp/Promise/函数照常展开。
  ② `typeof value !== 'object'` 把函数送进 `formatPrimitive` 回 `unknown`——
  Node `formatValue` 明确排除函数（`!== 'object' && !== 'function'`）。
- 修法（`src/builtins/node/internal/inspect.rs`）：`depth<0` 按空/非空分流
  （空走 `prefix+{}`/`[]`/`Map(0) {}`，非空走 `[Prefix]`；`constructorName null`
  即 null-proto）；primitives 门加 `&& !== 'function'`。
- 复现：`tests/node/buffer.rs::phase10f_buffer_parity_fixes` 间接（`from` 门）；
  探针 `inspect({__proto__:null},{depth:-1})` 修前 `[Object]`、修后与真机同串。
- 推广为铁律：`depth` 是"剩余层数"不是"开关"——空与非空在截断点语义不同；
  `typeof` 三态（object/function/primitive）写早退条件必须三态全列，漏 function
  即 `unknown`。

### 4.108 `process.nextTick` 裸 throw 变 unhandled rejection：uncaught 路由须先探监听（2026-09-16，10f diagnostics_channel）

- 症状：`bind-store`/`safe-subscriber-errors` 修前报 `unhandled rejection: Error:
  fail/nope` exit=1，而套件等的是 `uncaughtException` 监听触发后 exit=0。
- 根因：移植稿把 Node 的 `triggerUncaughtException(err)` 转译成
  `process.nextTick(() => { throw err; })`——本仓 nextTick 系 microtask，
  回调内 throw 落进 rejection 表，循环尾按 fatal 收割；真机 nextTick 回调抛错
  走 uncaughtException（有监听即交付）。
- 修法：`__dcUncaught(err)`——nextTick 内先探
  `process.listenerCount('uncaughtException')`，有则 `emit`，无则 throw
  走既有 fatal（`src/builtins/node/diagnostics_channel.rs`，两处 throw 点同改）。
- 复现：`tests/node/diagnostics_channel.rs::phase10f_diagnostics_channel_parity_fixes`
  的 `uncaught` 行（修前 `["sub-boom"]` 缺席 + exit=1）。
- 推广为铁律：凡转译 `triggerUncaughtException`/`_fatalException`，一律经
  "探监听 → emit/throw" 两件套，不许裸 throw 赌运行时语义（timers 的 Rust 侧
  `uncaught_fn` 同族，见 `src/builtins/timers.rs`）。

### 4.109 `AsyncLocalStorage.enterWith` 是文档化主入口，`enter` 只是别名（2026-09-16，10f diagnostics_channel）

- 症状：`run-stores-scope` 首错即 `store.enterWith is not a function`——移植稿只给了
  遗留 `enter`。
- 根因：Node 文档化的是 `enterWith`（`enter` 系遗留），套件只调前者；两者语义同
  （进入 store 直至被 run/exit 切换）。
- 修法：`enterWith` 与 `enter` 同体并列（`src/builtins/node/async_hooks.rs`，
  头注同步）。
- 复现：同上 `phase10f_…_fixes` 的 `enter/restored` 行。
- 推广为铁律：移植"公开面"以真机文档方法名为准，不以内部实现名（`enter`）为准；
  遗留别名与文档主入口并存时两个都要给。

### 4.110 `instanceof` 右侧自定义 `hasInstance` 内禁裸调 `getPrototypeOf`（2026-09-16，10f diagnostics_channel）

- 症状：`tracing-channel-args-types` 的 `tracingChannel({})` 期望
  `Cannot convert undefined or null to object`，本仓出小写 `can't convert…`。
- 根因：`Channel[Symbol.hasInstance]` 内 `Object.getPrototypeOf(undefined)`——
  同一操作两引擎文案不同（SM 小写/V8 大写），而 Node 恰把该引擎错原样抛给用户，
  套件用正则钉住（§4.99 的 `instanceof` 右侧形态门是另一面：那是右值为函数时的
  抛错，本条是 hasInstance 内部操作的文案）。
- 修法：空值（`undefined`/`null`）先行直抛 V8 文案
  `Cannot convert undefined or null to object`（`diagnostics_channel.rs`；
  非空值沿旧路，`getPrototypeOf('')` 等装箱语义两边一致，无需桥）。
- 复现：同上 `t1` 行；探针 `dc.tracingChannel({})` 修前小写、修后与真机同串。
- 推广为铁律：文案桥只桥"套件正则钉住且真机可观测"的位点；桥的触发条件能窄则窄
  （本条仅空值），不全局 patch 引擎内建（`Object.getPrototypeOf` 本体不动）。

### 4.111 运行时从不 emit `unhandledRejection`：tracePromise 无 catch 即 fatal（2026-09-16，10f diagnostics_channel，偏离另案）

- 症状：`tracing-channel-promise-unhandled`（注册 `process.on('unhandledRejection')`
  后故意不接 tracePromise 的拒绝）修前 `unhandled rejection: Error: test` exit=1，
  监听永不触发。
- 根因：本仓 rejection 只在循环尾收割报 fatal（`runtime.rs`
  `report_unhandled_rejections`），从无 `emit('unhandledRejection')` 面；
  `src/` 全 grep 无该事件名。补齐需运行时收割点加"探监听 → emit，无则 fatal"
  （`__wjs_uncaught` 同族的新 `__wjs_unhandled`），属跨切片改动。
- 修法（本切片）：不修，记 🟡偏离（`docs/bun-parity.md` diagnostics_channel 节）；
  不在 dc 侧用"急于 emit"伪造语义（调用方后接 `.catch` 时真机还会有
  `rejectionHandled`，急 emit 即错上加错）。
- 推广为铁律：事件面缺口（emit 点在运行时）不在功能切片内用功能侧补丁绕过；
  绕过能过当前用例、必坏相邻语义（handled/unhandled 双生面）。

### 4.112 阻塞 native 停转事件循环：定制查询改投递+轮询（2026-09-16，10f dns）

- 症状：stub-DNS 套件（multi-channel/resolveany 系）全 hang——查询包明明已发出
  （对端 python 可收），stub 回包/自发包却永不到 JS。
- 根因：定制查询是 microtask 内**阻塞** native（`rx.recv()` 等 helper 线程）——
  JS 线程停转期间 dgram 到包无人分发；待 native 超时返回，测试早已错过收包窗口
  （server 永不 close 即 hang）。§4.46/§4.18 的 DNS 版：10d"同步阻塞与 lookup
  同哲学"在 stub 并发下不成立。
- 修法：投递即返 + 5ms refed 轮询收割——`__wjs_dns_job_start`（spawn 线程跑查询，
  结果进 `dns_jobs` 表）/`__wjs_dns_job_poll`（取走即摘）/`__wjs_dns_job_forget`
  （cancel 摘除）；cancel 纯 JS 即刻合成 ECANCELLED（线程迟归自然沉底，不多等
  一轮超时）；运行时零改动（未动 pump/idle：轮询 interval 自带保活）。
- 复现：`wjs-dns-stub` 探针（修前 `stub got` 缺席 + ETIMEOUT；修后即达）。
- 推广为铁律：凡"查询等回包"与"回包靠本循环分发"同现，查询路径永不阻塞——
  投递/轮询/遗忘三件是最小闭环；"线程迟归自然沉底 + JS 侧即刻合成"是 cancel
  的标准形（强杀线程不可取）。

### 4.113 `assert.throws(fn, Error)` 裸类须先 `instanceof`，门错即全灭（2026-09-16，10f dns）

- 症状：`setlocaladdress` 全线 `throws: unexpected throw`——抛的明明是 Error。
- 根因：§4.99 的门写成 `expected.prototype instanceof Error`（仅 Error 子类为真）——
  裸 `Error` 本体（`Error.prototype` 的原型是 `Object.prototype`）被踢进校验函数分支，
  `Error(e)` 回对象 `!== true` 永假。真机（lib/assert.js）门是
  `expected.prototype !== undefined && actual instanceof expected`，再以
  "是否 Error 构造器族"分流（是则直接不过，不当校验器调）。
- 修法：照抄真机三段（`src/builtins/node/assert.rs` `__checkThrow`；Error 族判定走
  `getPrototypeOf` 链找 `Error`，箭头函数无 prototype 照旧落校验器，§4.99 不动）。
- 复现：`assert.throws(() => { throw new TypeError("x"); }, Error)`（修前不过）。
- 推广为铁律：§4.99 的延续——转译断言库的形态判定必须与 lib 原文逐行对拍，
  "看起来等价"的门（`X.prototype instanceof Error` vs `actual instanceof X`）
  在本体/子类边界上必然分叉；改断言库必跑全量（本轮全绿才敢合）。

### 4.114 双 `Received` 口径：ARG_TYPE 用 helper 形，ARG_VALUE 用 inspect 形（2026-09-16，10f dns）

- 症状：`setservers-type-check`（`Received type string ('x')`）与
  `lookupService`（`Received 'fasdfdsaf'`）对字符串期望互斥——统一即顾此失彼。
- 根因：Node 两处文案源不同：`ERR_INVALID_ARG_TYPE` 用 invalidArgTypeHelper
  （字符串 `type string ('x')`），`ERR_INVALID_ARG_VALUE` 用 kInspect
  （字符串裸 `'x'`）。移植时合写成一个 `__dnsReceived` 即撞墙。
- 修法：`__dnsReceived`（helper 形）与 `__dnsInspectValue`（inspect 形）分家，
  前者供 ARG_TYPE，后者供 ARG_VALUE（`src/builtins/node/dns.rs`）。
- 复现：上两个套件行（修前各对一半）。
- 推广为铁律：同词（`Received`）不同源（helper vs inspect）的文案，初见即分家；
  报错文案的"源头函数"与"调用点"同等重要，抄文案先问出自哪个函数。

### 4.115 移植先读全文件：文件头校验段漏读=返工三轮（2026-09-16，10f 过程教训·非代码坑）

- 症状：`channel-timeout` 修完"主体"仍红——漏读文件头 20 行构造器校验段
  （`{timeout: null/true/...}` 与 `-2/4.2/2**31` 两批）；`resolveany.js` 漏读
  `validateResults` 的 SOA-`type` 键（误作裸对象）；`setlocaladdress` 漏读
  `ERR_INVALID_ARG_VALUE` 全文（自编 `invalid IPv4` 错两遍）。
- 根因：`sed -n '25,60p` 式抽查只看了"主体"，校验段恰在文件头注释之后。
- 修法：以后套件先 `cat` 全文（本轮三个文件皆 <120 行有效段），再列需求表；
  本条即 hunger：需求表（test-dns.js 花了整轮）之后零返工。
- 推广为铁律：点名前通读套件全文（含头 30 行的校验表）；"先跑再读"只适用于
  找崩点，不适用于定需求。

### 4.116 跨域微任务先 AutoRealm 进执行 global 再 RunJSMicroTask（2026-09-17，10f vm）

- 症状：vm 沙箱内 `queueMicrotask`/promise 反应在排空点 SEGV（exit 139）——
  test-vm-script-after-evaluate 探针必现；纯主域微任务从不出事。
- 根因：`RunJSMicroTask` 带 DEBUG assert：任务的执行 global 必须等于
  `cx->global()`。本仓 RustJobQueue glue（§4.7）在**主域**排空整个队列，
  vm 的 cross-compartment 微任务未进任务 realm 即 assert abort（SM 内部队的
  runJobs 从不犯此错——它逐任务进对应 realm）。
- 修法（`src/jobqueue.rs` `run_jobs`）：逐任务先
  `GetExecutionGlobalFromJSMicroTask`（null 则同 SM 内部队 `continue` 跳过），
  `AutoRealm::new` 进执行 global，再 `RunJSMicroTask(realm.raw_cx(), …)`；
  realm 对象随迭代丢弃自动还原。
- 复现：vm 沙箱内 `queueMicrotask(() => …)` 后排空（修前 139，修后 exit=1
  可读错；余下 mustNotCall 文案面属微任务模式偏离，bun-parity vm 节记档）；
  `tests/node/vm.rs::phase10f_vm_parity_sync_and_errors` 同路回归。
- 推广为铁律：§4.1 的微任务版——"之后要调 JSAPI 先回 realm"不只适用于
  evaluate 返回后，**逐任务**的引擎回调同样适用；凡引擎 API 的 DEBUG assert
  写明上下文前提，宿主 glue 照 SM 内部队同款实现即免踩（读引擎源码找 assert
  前提比猜崩点快）。

### 4.117 cjs goal 探测禁包络形：包装会吞语句语义（2026-09-17，10f vm，§4.59 姊妹）

- 症状：CJS 分类的 cjs goal 复核若用 `(function(){ … })` 包络（想顺便容忍
  顶层 return），phase9k require(esm) 的 `export default 2` fixture 被误判
  CJS——export 藏进函数体后 oxc 无错也无模块信号，探测"通过"。
- 根因：包装改变语句的顶层性：包络形里 `export` 不再是顶层语句，oxc 既不报
  错也不置 `has_module_syntax`；顶层 `return` 本就无需包装容忍——
  `SourceType::with_commonjs(true)` 即 node 函数包装语义。§4.59 已钉
  `with_module(false)` ≠ 无模块信号，真相在 `module_record`。
- 修法（`src/modules.rs` `cjs_goal_probe`）：**裸文本** + `with_commonjs(true)`
  解析，`无错 && !has_module_syntax` 才判 CJS；歧义集分类改双值
  `script_goal_probe`（可解析/模块信号分读）——有模块信号判 ESM、可解析判
  CJS、解析失败（顶层 return 等 CJS 体专属语法）才落 cjs goal 复核；require
  侧经 `load_cjs_js`（`src/loader/transpile.rs`，`with_commonjs` 同 goal）转译。
- 复现：`tests/node/require.rs::phase10f_cjs_top_level_return_entry_and_dep`
  （入口/依赖顶层 return 双形 + `export default` 不误判；修前 ESM 进 CJS 垫片）。
- 推广为铁律：解析探测永不包装——包络形会吞语句的顶层性（export 信号消失），
  也会反向吸收体（compileFunction 的 `});` 提前闭合，同轮实证）；"想容忍什么"
  就用对应 goal（`with_commonjs`）表达，不手写包装。oxc 的**信号位**
  （`module_record.has_module_syntax`）才是分类真相，"无错"从来不是。

### 4.118 nextTick 双层调度：原生队列收割，microtask 合并队列语义必反（2026-09-17，10f stream）

- 症状：stream 对拍 10 件 DIFF 全族——pipe-error-unhandled 等 5 件把
  `process.on('uncaughtException')` 期望的同步 throw 落成 unhandledRejection；
  compose post-loop throw 1 件管线提前 clean 收工拆 error 监听（composed 流
  error/close 双丢、toArray 永悬）；修 nextTick 后另现 139（SIGSEGV）。
- 根因（两层）：① node 的 tick/微任务是**两层队列**——同步期入队的 tick 先于
  微任务、微任务期入队的 tick 等**整轮微任务排空**后才跑（V8 checkpoint 原子性，
  RunMicrotasks 不可被 nextTick 插队）；旧实现 `queueMicrotask(() => cb())` 把
  两层并成 FIFO——destroy 的 `nextTick(emitErrorNT)` 抛错变 rejection、
  duplexify 的 finish 信号抢在 generator 续体前（② 的直接受害者）。
  ② 修复时的 batch-drain 把多条目裸 JSVal 攥在 Rust Vec 里横跨回调——回调触发
  GC 即悬垂（§4.80 完整形态 N 连击：fire_due 是逐条摘取立即 rooting，没照抄）。
- 修法：① `process.nextTick(cb, args)` 入**原生队列**
  （`RootedState.next_ticks: Vec<NextTickEntry>`，`Box<Heap>` 定址）；
  `pump_once` 内 `drain → RunJobs → drain` 循环（微任务期入队的 tick 由下一轮
  drain 收割）——node 双层语义自然成立；回调抛错照 fire_due 口径路由
  （探 `__wjs_uncaught_count` → 分发/fatal）。② drain 改**逐条摘取立即
  rooting**（`remove(0)` + rooted! 后再调，§4.80 纪律）。
- 复现：`tests/node/stream.rs::phase10f_stream_parity_tick_scheduler_and_fs_readstream`
  （tick-order 行：`start,micro,tick` 序——microtask 期入队的 tick 恒后于既有
  微任务；修前为 `start,tick,micro` 反转序）；batch 形即 139（pe2 探针 3/3）。
- 推广为铁律：移植 node 异步 API 先问"它排在 tick 队列还是微任务队列"——
  `queueMicrotask` 一把梭对 nextTick/微任务**互相入队**的场景必然语义反转；
  原生队列 + pump 前后收割是最小正解。**§4.80 三进宫**：从队列批量取出的
  JSVal 一律当场逐条 rooting，"先攒一批再逐个处理"在 JS 值上永远不成立。

### 4.119 `__fsCall` 闭包内抛的校验错误被 `__fsErr` 重包成 UNKNOWN（2026-09-17，10f fs）

- 症状：`fs.fchmod(1, '123x')` 期望 `ERR_INVALID_ARG_VALUE`，实测 `UNKNOWN`。
- 根因：`__fsModeNum` 在 `__fsCall("fchmod", ..., () => __wjs_fs_fchmod(fd, __fsModeNum(mode)))`
  闭包内求值，抛出的 JS 错误（已带 code）被 `__fsErr` 按"native 消息无 code"路径
  再包一层 → code 落 UNKNOWN。
- 修法：`__fsErr` 先查 `e.code`——JS 侧已带 code（≠UNKNOWN）的错误直通
  （补 path 后原样重抛）；仅 native report_error 产的无 code 消息走 node 形状重包。
- 复现：`test-fs-fchmod.js`（修前 `throws: unexpected throw`）。
- 推广为铁律：错误包装函数必须区分"JS 侧语义错误（已有 code）"与
  "native io 消息（无 code 需整形）"，后者才重包（§4.37 症状一同源第三例）。

### 4.120 `__cb1` 末参即 cb 的校验次序与 node 相反：f 系 fd/mode 先于 cb（2026-09-17，10f fs）

- 症状：`fs.fchmod(1, '123x')` 期望 `ERR_INVALID_ARG_VALUE`（mode 校验先抛），
  实测 `ERR_INVALID_ARG_TYPE`（callback）——`__cb1` 先验末参为函数，把 `'123x'`
  当 cb 报了错。
- 根因：node 回调 API 的校验次序分两族——readFile 族 cb 先验，f 系（fchmod/
  fchown/fstat/ftruncate/fsync/fdatasync/futimes）fd/mode 实参先验、cb 最后。
- 修法：f 系手写 `__fdCb` 包装（syncFn 先跑、cb 缺省再补 callback 类型错）；
  `__cb1` 保持 cb 先验（readFile 族语义）。
- 复现：`test-fs-fchmod.js` M5；模块探针 `fs.fchmod(1,'123x')` 单独跑必现。
- 推广为铁律：移植 node 校验次序必须逐 API 对 `lib/fs.js` 原文——"统一先验 cb"
  在 f 系全错；黑盒断言 message/code 时先真机实测错误种类再写。

### 4.121 `fs_err` 包装吞 raw errno：io_code 落 UNKNOWN（2026-09-17，10f fs）

- 症状：`fs.mkdirSync(file/sub, {recursive:true})` 期望 `ENOTDIR`，实测 `UNKNOWN`
  （消息里还是 fs_err 的双层 Display）。
- 根因：native 用 `fs_err::create_dir_all`——其 `Error` 的 Display 带自身上下文且
  raw errno 不在 `io_code` 期待的位置。
- 修法：native 改 `std::fs::create_dir_all/create_dir` 直用（report_io 拿到真
  io::Error → raw_os_error → ENOTDIR）；配套 `uv_msg` errno→node 消息表 +
  report_io 产 node 形状（`CODE: <uv msg>, <syscall> '<path>'`）+ `__fsErr` 直通。
- 复现：`test-fs-mkdir.js` 父为文件/父链含文件两件。
- 推广为铁律：新 syscall native 禁引 fs_err 系包装（io_code/uv_msg 依赖
  raw_os_error）；错误消息形状一次性对 node（`CODE: msg, syscall 'path'`），
  JS 层只补属性不重排。

### 4.122 对拍并行跑分未设 TEST_THREAD_ID：全进程共享 `.tmp.0` 互踩（2026-09-17，10f fs）

- 症状：mkdtempDisposable 套件单独跑全过，与其它 fs 件并行跑恒挂——
  teardown 报 `EACCES: rm '.tmp.N'`，残留 0444/0644 目录连锁污染后续轮次。
- 根因：node `common/tmpdir.js` 的目录名 = `.tmp.` + `TEST_SERIAL_ID ||
  TEST_THREAD_ID || '0'`——缺省全部进程共用 `.tmp.0`；某件 chmod 共享目录
  0444 后，其它件的 `tmpdir.refresh()` 全炸，错误又被归因到当前件。
- 修法：跑分脚本 `run1` 按线程注入唯一 `TEST_THREAD_ID`；手工并行探针同样
  显式设；清理残留目录 `chmod -R u+rwx`（u+w 不含遍历位，rm 不掉）。
- 复现：`wjs-10f-par.py` 未注入版并行跑 test-fs 全域（互踩随机现形）。
- 推广为铁律：并行跑 node 套件必须逐进程设 TEST_THREAD_ID/TEST_SERIAL_ID；
  "单独跑过、并行挂"先查共享临时目录，再怀疑代码（§4.41 读全局态姊妹篇）。

### 4.123 命名函数表达式遮蔽外层绑定 → custom promisify 无限递归（2026-09-17，10f fs）

- 症状：`promisify(fs.exists)` 拒绝原因就是 `true`（回调首参当 err）+ 
  `InternalError: too much recursion`。
- 根因：`exists[Symbol.for(...)] = function exists(path) { ... exists(path, resolve) }`
  ——内层命名遮蔽外层导出，自递归；拒绝值恰是 truthy 的 `true`。
- 修法：内层匿名（`function (path) { ... 外层 exists ... }`）。
- 复现：`test-fs-promisified.js`。
- 推广为铁律：给既有函数挂 `promisify.custom` 时内层**禁用同名命名函数表达式**
  ——命名 FE 的名字在其作用域内遮蔽外层绑定， lexically 就地自指。

### 4.124 Buffer/TypedArray 自带 `Symbol.iterator`（吐数字）：视图必须排除在"可迭代 data"分支外（2026-09-17，10f fs）

- 症状：`fh.writeFile(Buffer)` 报 `The "chunk" argument ... Received number`。
- 根因：`typeof data[Symbol.iterator] === "function"` 对 Buffer/TypedArray 也成立
  （迭代出 number）——Buffer 被当逐块可迭代收集，块校验当场拒绝。
- 修法：可迭代分支加 `!ArrayBuffer.isView(data)` 前置门；块校验
  （string/Buffer/TypedArray/DataView）照 node 逐字（`ERR_INVALID_ARG_TYPE`，
  write 系参数名 "buffer"、writeFile/appendFile 系 "data"）。
- 复现：`test-fs-promises-file-handle-writeFile.js` doWriteBuffer。
- 推广为铁律：`Symbol.iterator` 存在性 ≠ "集合类型"判据——TypedArray 全家都是
  iterator；分支判据按"视图先收、迭代器次之"排序，视图门永远在前。

