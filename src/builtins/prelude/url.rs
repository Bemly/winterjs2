//! WHATWG URL（URL/URLSearchParams/TextEncoder-Decoder/btoa-atob）（prelude 分域；拼接顺序见 mod.rs）。
pub const URL_JS: &str = r#"
// ---- Phase 3a: URL / URLSearchParams / TextEncoder/Decoder / base64 / crypto ----
globalThis.btoa = (s) => __wjs2_btoa(String(s));
globalThis.atob = (s) => __wjs2_atob(String(s));
const __wjs2_urlState = new WeakMap();
const __wjs2_uspState = new WeakMap();
// node 口径（whatwg 套件 11 件点名）：USP 方法的非法接收者一律
// TypeError ERR_INVALID_THIS（原 WeakMap miss 的裸 TypeError 无码）。
function __wjs2_uspBrand(usp) {
  const s = __wjs2_uspState.get(usp);
  if (s === undefined) {
    const e = new TypeError('Value of "this" must be of type URLSearchParams');
    e.code = "ERR_INVALID_THIS";
    throw e;
  }
  return s;
}
// WHATWG urlencoded 序列化：lone surrogate 按 UTF-8 编码规则落 FFFD
//（Rust 侧 serde_json 拒收 lone surrogate，直接送入即 "bad query pairs"
// 抛错；custom-searchparams 套件点名 %EF%BF%BD）。
// WebIDL USVString 转换：Symbol 即抛（显式 String() 不抛，真机 TypeError）。
function __wjs2_toUSVString(s) {
  if (typeof s === "symbol") throw new TypeError("Cannot convert a Symbol value to a string");
  return String(s);
}
function __wjs2_uspClean(s) {
  if (typeof s === "symbol") throw new TypeError("Cannot convert a Symbol value to a string");
  let out = "";
  for (const ch of String(s)) {
    const cp = ch.codePointAt(0);
    out += (cp >= 0xD800 && cp <= 0xDFFF) ? "�" : ch;
  }
  return out;
}
function __wjs2_setHref(urlObj, newHref) {
  const st = __wjs2_urlState.get(urlObj);
  st.href = newHref;
  if (st.usp) {
    const search = __wjs2_url_get(newHref, "search");
    const q = search.startsWith("?") ? search.slice(1) : search;
    __wjs2_uspState.get(st.usp).pairs = q === "" ? [] : JSON.parse(__wjs2_usp_parse(__wjs2_uspClean(q)));
  }
}
function __wjs2_pushSearch(urlObj) {
  const st = __wjs2_urlState.get(urlObj);
  if (!st.usp) return;
  const q = __wjs2_usp_serialize(JSON.stringify(__wjs2_uspState.get(st.usp).pairs));
  st.href = __wjs2_url_set(st.href, "search", q === "" ? "" : "?" + q);
}
function __wjs2_uspFromUrl(urlObj) {
  const usp = new URLSearchParams("");
  __wjs2_uspState.get(usp).parent = urlObj;
  const st = __wjs2_urlState.get(urlObj);
  const search = __wjs2_url_get(st.href, "search");
  const q = search.startsWith("?") ? search.slice(1) : search;
  __wjs2_uspState.get(usp).pairs = q === "" ? [] : JSON.parse(__wjs2_usp_parse(__wjs2_uspClean(q)));
  st.usp = usp;
  return usp;
}
function __wjs2_uspTouch(usp) {
  const s = __wjs2_uspState.get(usp);
  if (s.parent) __wjs2_pushSearch(s.parent);
}
// node 口径：解析失败抛 TypeError "Invalid URL"，带 code=ERR_INVALID_URL、input（有 base 时另带 base）
// ——修前是原生层文案的普通 Error（"TypeError: Invalid URL: …"），按 code 断言的套件全挂。
function __wjs2_urlParseOrThrow(url, base) {
  try {
    return base === undefined ? __wjs2_url_parse(url) : __wjs2_url_parse(url, base);
  } catch {
    const e = new TypeError("Invalid URL");
    e.code = "ERR_INVALID_URL";
    e.input = url;
    if (base !== undefined) e.base = base;
    throw e;
  }
}
// node C++ 品牌文案（invalidthis 套件逐字正则）：href/search 取值 +
// toString/toJSON（经 href 取值透出）走 "Receiver must be an instance of
// class URL"；其余取值/全部设值走 "Cannot read private member #context …"。
function __wjs2_urlBrandRecv(self) {
  if (__wjs2_urlState.get(self) === undefined) {
    throw new TypeError("Receiver must be an instance of class URL");
  }
}
function __wjs2_urlBrand(self) {
  const st = __wjs2_urlState.get(self);
  if (st === undefined) {
    throw new TypeError("Cannot read private member #context from an object whose class did not declare it");
  }
  return st;
}
globalThis.URL = class URL {
  constructor(url, base) {
    // WebIDL 必填首参：零实参即 ERR_MISSING_ARGS（custom-parsing 套件点名；
    // 显式 undefined 仍走 "undefined" 解析 → ERR_INVALID_URL，真机同款）。
    if (arguments.length === 0) {
      const e = new TypeError('The "url" argument must be specified');
      e.code = "ERR_MISSING_ARGS";
      throw e;
    }
    const href = __wjs2_urlParseOrThrow(__wjs2_toUSVString(url), base === undefined ? undefined : __wjs2_toUSVString(base));
    __wjs2_urlState.set(this, { href, usp: null });
  }
  static canParse(url, base) {
    // 零实参即 MISSING_ARGS（真机口径；try 外抛，余下 internal/natives 依赖另案）。
    if (arguments.length === 0) {
      const e = new TypeError('The "url" argument must be specified');
      e.code = "ERR_MISSING_ARGS";
      throw e;
    }
    try {
      if (base === undefined) __wjs2_url_parse(__wjs2_toUSVString(url));
      else __wjs2_url_parse(__wjs2_toUSVString(url), __wjs2_toUSVString(base));
      return true;
    } catch { return false; }
  }
  get href() { __wjs2_urlBrandRecv(this); return __wjs2_urlState.get(this).href; }
  set href(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_urlParseOrThrow(__wjs2_toUSVString(v))); }
  get protocol() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "protocol"); }
  set protocol(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "protocol", __wjs2_toUSVString(v))); }
  get username() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "username"); }
  set username(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "username", __wjs2_toUSVString(v))); }
  get password() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "password"); }
  set password(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "password", __wjs2_toUSVString(v))); }
  get host() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "host"); }
  set host(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "host", __wjs2_toUSVString(v))); }
  get hostname() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "hostname"); }
  set hostname(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "hostname", __wjs2_toUSVString(v))); }
  get port() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "port"); }
  set port(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "port", __wjs2_toUSVString(v))); }
  get pathname() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "pathname"); }
  set pathname(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "pathname", __wjs2_toUSVString(v))); }
  get search() { __wjs2_urlBrandRecv(this); return __wjs2_url_get(this.href, "search"); }
  set search(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "search", __wjs2_toUSVString(v))); }
  get hash() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "hash"); }
  set hash(v) { __wjs2_urlBrand(this); __wjs2_setHref(this, __wjs2_url_set(this.href, "hash", __wjs2_toUSVString(v))); }
  get origin() { __wjs2_urlBrand(this); return __wjs2_url_get(this.href, "origin"); }
  // origin 只读：显式 setter 先走 USVString 转换（setters 套件点名 toString/
  // Symbol 透出），再抛 V8 形只读错（properties 套件逐字正则）。
  set origin(v) {
    __wjs2_urlBrand(this);
    __wjs2_toUSVString(v);
    throw new TypeError("Cannot set property origin of [object URL] which has only a getter");
  }
  get searchParams() {
    const st = __wjs2_urlBrand(this);
    if (!st.usp) return __wjs2_uspFromUrl(this);
    return st.usp;
  }
  // searchParams 只读（properties 套件点名 V8 形只读错；setters 套件同 origin 口径先转换）。
  set searchParams(v) {
    __wjs2_urlBrand(this);
    __wjs2_toUSVString(v);
    throw new TypeError("Cannot set property searchParams of [object URL] which has only a getter");
  }
  toString() { __wjs2_urlBrandRecv(this); return this.href; }
  toJSON() { __wjs2_urlBrandRecv(this); return this.href; }
};
// for-in 可见性（custom-properties 套件点名顺序：toString 先行，其次各
// 访问器，toJSON 收尾；类语法默认不可枚举，此处逐项翻为可枚举——先删后建，
// 重建顺序即枚举顺序，取值器原样保留）。
for (const __wjs2_k of ["toString", "href", "origin", "protocol", "username", "password", "host", "hostname", "port", "pathname", "search", "searchParams", "hash", "toJSON"]) {
  const __wjs2_d = Object.getOwnPropertyDescriptor(URL.prototype, __wjs2_k);
  delete URL.prototype[__wjs2_k];
  Object.defineProperty(URL.prototype, __wjs2_k, { ...__wjs2_d, enumerable: true });
}
Object.defineProperty(URL.prototype, Symbol.toStringTag, { value: "URL", configurable: true });
// URLSearchParams 迭代器：自有对象（非 Generator）——原型挂
// "URLSearchParams Iterator" toStringTag（custom-tostringtag 套件点名原型与实例双侧）。
const __wjs2_uspIterProto = { [Symbol.toStringTag]: "URLSearchParams Iterator" };
// 迭代器 next 的品牌门（keys/values/entries 套件点名 ERR_INVALID_THIS）。
function __wjs2_uspIterBrand(self) {
  if ((self === null || (typeof self !== "object" && typeof self !== "function")) ||
      Object.getPrototypeOf(self) !== __wjs2_uspIterProto) {
    const e = new TypeError('Value of "this" must be of type URLSearchParamsIterator');
    e.code = "ERR_INVALID_THIS";
    throw e;
  }
}
function __wjs2_uspQ(s) {
  return `'${String(s).replace(/\\/g, "\\\\").replace(/'/g, "\\'")}'`;
}
function __wjs2_uspWrap(title, items, options, emptyForm) {
  if (items.length === 0) return emptyForm;
  const single = `${title} { ${items.join(", ")} }`;
  const __bl = options !== null && typeof options === "object" ? options.breakLength : undefined;
  if (typeof __bl === "number" && single.length > __bl) {
    return `${title} {\n  ${items.join(",\n  ")} }`;
  }
  return single;
}
function __wjs2_uspIter(pairs, kind) {
  let i = 0;
  const it = {
    next() {
      __wjs2_uspIterBrand(this);
      if (i >= pairs.length) return { value: undefined, done: true };
      const [k, v] = pairs[i++];
      return { value: kind === 0 ? k : kind === 1 ? v : [k, v], done: false };
    },
    [Symbol.iterator]() { return this; },
    // 10f：迭代器 inspect 显示剩余项（custom-searchparams-inspect 套件逐字）。
    [Symbol.for("nodejs.util.inspect.custom")](depth, options) {
      __wjs2_uspIterBrand(this);
      // 10f：depth<0 本体不再下钻（inspect 核心未拦时此处兜底，真机 '[Object]'）。
      if (typeof depth === "number" && depth < 0) return "[Object]";
      const rest = pairs.slice(i);
      const items = rest.map(([k, v]) => kind === 0 ? __wjs2_uspQ(k) : kind === 1 ? __wjs2_uspQ(v) : `[ ${__wjs2_uspQ(k)}, ${__wjs2_uspQ(v)} ]`);
      return __wjs2_uspWrap("URLSearchParams Iterator", items, options, "URLSearchParams Iterator {  }");
    },
  };
  Object.setPrototypeOf(it, __wjs2_uspIterProto);
  return it;
}
// WebIDL 必填参（各 searchparams-* 套件点名）：缺参即 ERR_MISSING_ARGS。
function __wjs2_missingArgs(names, plural) {
  const e = new TypeError(`The ${names} argument${plural ? "s" : ""} must be specified`);
  e.code = "ERR_MISSING_ARGS";
  throw e;
}
globalThis.URLSearchParams = class URLSearchParams {
  constructor(init) {
    let pairs;
    // Symbol 直投 USVString 转换错（union 末分支，真机 TypeError）。
    if (typeof init === "symbol") throw new TypeError("Cannot convert a Symbol value to a string");
    if (init === undefined) pairs = [];
    else if (typeof init === "string") {
      const q = init.startsWith("?") ? init.slice(1) : init;
      // 入参串先落 FFFD（lone surrogate 的 UTF-8 编码替换；串内原生 lone
      // 进 Rust 侧 serde_json 即 "bad query pairs" 抛错，真机落 %EF%BF%BD）。
      pairs = q === "" ? [] : JSON.parse(__wjs2_usp_parse(__wjs2_uspClean(q)));
    } else if (init !== null && (typeof init === "object" || typeof init === "function") && Symbol.iterator in Object(init)) {
      // 10f：序列分支按 key-presence 判定（`{[Symbol.iterator]: 42}` 走
      // 'Query pairs must be iterable'，非可调用才抛；真机逐字）。
      // 每项须恰为二元组（多/少一律 tuple 文案；custom-searchparams 构造器套件）。
      // （本文件惯例：手工 TypeError + code，不走 codes 注册表。）
      if (typeof init[Symbol.iterator] !== "function") {
        const __e = new TypeError("Query pairs must be iterable");
        __e.code = "ERR_ARG_NOT_ITERABLE";
        throw __e;
      }
      const __tupleErr = () => {
        const __e = new TypeError("Each query pair must be an iterable [name, value] tuple");
        __e.code = "ERR_INVALID_TUPLE";
        return __e;
      };
      pairs = [];
      for (const p of init) {
        if (p === null || p === undefined || typeof p[Symbol.iterator] !== "function") {
          throw __tupleErr();
        }
        const __pi = p[Symbol.iterator]();
        const __k = __pi.next();
        if (__k.done) throw __tupleErr();
        const __v = __pi.next();
        if (__v.done) throw __tupleErr();
        if (!__pi.next().done) throw __tupleErr();
        pairs.push([__wjs2_uspClean(__k.value), __wjs2_uspClean(__v.value)]);
      }
    } else if (typeof init === "object" && init !== null) {
      // 10f：record 分支按 Map 收集（含 symbol 键，转换即抛；清洗后撞键
      // 后者赢；custom-searchparams 构造器畸形键套件）。
      const __m = new Map();
      for (const k of Reflect.ownKeys(init)) __m.set(__wjs2_uspClean(k), __wjs2_uspClean(init[k]));
      pairs = [...__m];
    } else if (init !== undefined) {
      // 10f：标量经 USVString 兜底（null→"null"/false→"false"/0→"0"，真机逐字）。
      pairs = [[__wjs2_uspClean(init), ""]];
    } else throw new TypeError("URLSearchParams: unsupported init");
    __wjs2_uspState.set(this, { pairs, parent: null });
  }
  get size() { return __wjs2_uspBrand(this).pairs.length; }
  append(n, v) {
    // 品牌门先行（call(undefined) 无参形点名 INVALID_THIS 而非 MISSING_ARGS）。
    const s = __wjs2_uspBrand(this);
    if (arguments.length < 2) __wjs2_missingArgs('"name" and "value"', true);
    s.pairs.push([__wjs2_uspClean(n), __wjs2_uspClean(v)]); __wjs2_uspTouch(this);
  }
  delete(n, v) {
    const s = __wjs2_uspBrand(this);
    if (arguments.length === 0) __wjs2_missingArgs('"name"', false);
    n = __wjs2_uspClean(n);
    // 值参同样先行转换（空表 filter 不回调，真机仍抛转换错）。
    if (v !== undefined) v = __wjs2_uspClean(v);
    s.pairs = (v === undefined)
      ? s.pairs.filter((p) => p[0] !== n)
      : s.pairs.filter((p) => !(p[0] === n && p[1] === v));
    __wjs2_uspTouch(this);
  }
  get(n) {
    const brand = __wjs2_uspBrand(this);
    if (arguments.length === 0) __wjs2_missingArgs('"name"', false);
    n = __wjs2_uspClean(n);
    const p = brand.pairs.find((p) => p[0] === n);
    return p ? p[1] : null;
  }
  getAll(n) {
    const brand = __wjs2_uspBrand(this);
    if (arguments.length === 0) __wjs2_missingArgs('"name"', false);
    n = __wjs2_uspClean(n);
    return brand.pairs.filter((p) => p[0] === n).map((p) => p[1]);
  }
  has(n, v) {
    const brand = __wjs2_uspBrand(this);
    if (arguments.length === 0) __wjs2_missingArgs('"name"', false);
    n = __wjs2_uspClean(n);
    // 值参同样先行转换（空表 some 不回调，真机仍抛转换错）。
    if (v !== undefined) v = __wjs2_uspClean(v);
    const ps = brand.pairs;
    return (v === undefined)
      ? ps.some((p) => p[0] === n)
      : ps.some((p) => p[0] === n && p[1] === v);
  }
  set(n, v) {
    const s = __wjs2_uspBrand(this);
    if (arguments.length < 2) __wjs2_missingArgs('"name" and "value"', true);
    n = __wjs2_uspClean(n); v = __wjs2_uspClean(v);
    let found = false;
    s.pairs = s.pairs.filter((p) => {
      if (p[0] !== n) return true;
      if (!found) { p[1] = v; found = true; return true; }
      return false;
    });
    if (!found) s.pairs.push([n, v]);
    __wjs2_uspTouch(this);
  }
  sort() {
    __wjs2_uspBrand(this).pairs.sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
    __wjs2_uspTouch(this);
  }
  toString() { return __wjs2_usp_serialize(JSON.stringify(__wjs2_uspBrand(this).pairs)); }
  // 10f：inspect 面（custom-searchparams-inspect 套件逐字；空形 `{}`、
  // 迭代器另见工厂；无 receiver 即 ERR_INVALID_THIS）。
  [Symbol.for("nodejs.util.inspect.custom")](depth, options) {
    if (typeof depth === "number" && depth < 0) return "[Object]";
    const pairs = __wjs2_uspBrand(this).pairs;
    const items = pairs.map(([k, v]) => `${__wjs2_uspQ(k)} => ${__wjs2_uspQ(v)}`);
    return __wjs2_uspWrap("URLSearchParams", items, options, "URLSearchParams {}");
  }
  keys() { return __wjs2_uspIter(__wjs2_uspBrand(this).pairs, 0); }
  values() { return __wjs2_uspIter(__wjs2_uspBrand(this).pairs, 1); }
  entries() { return __wjs2_uspIter(__wjs2_uspBrand(this).pairs, 2); }
  forEach(cb, thisArg) {
    // 品牌门先行（call(undefined) 无回调形点名 INVALID_THIS）。
    const brand = __wjs2_uspBrand(this);
    // 回调校验（custom-searchparams 套件点名码；文案逐字对真机）。
    if (typeof cb !== "function") {
      let recv;
      if (cb === undefined) recv = "undefined";
      else if (cb === null) recv = "null";
      else if (typeof cb === "object") recv = `an instance of ${cb.constructor?.name ?? "Object"}`;
      else recv = `type ${typeof cb} (${typeof cb === "string" ? `'${cb}'` : String(cb)})`;
      const e = new TypeError(`The "callback" argument must be of type function. Received ${recv}`);
      e.code = "ERR_INVALID_ARG_TYPE";
      throw e;
    }
    for (const [k, v] of brand.pairs) cb.call(thisArg, v, k, this);
  }
};
// Symbol.iterator 与 entries 同一函数对象（custom-searchparams 套件点名同一性）。
Object.defineProperty(URLSearchParams.prototype, Symbol.iterator, { value: URLSearchParams.prototype.entries, writable: true, configurable: true });
Object.defineProperty(URLSearchParams.prototype, Symbol.toStringTag, { value: "URLSearchParams", configurable: true });
globalThis.URLPattern = (function () {
  // URLPattern（plan3 §5 专项）：WHATWG 匹配语义由 `urlpattern` 轮子承载，
  // 此处只做 WebIDL 重载分流 + 取值 + 结果组装。错误自含（prelude 禁 import
  // errors 面，文案逐字对真机 node 26.8.2）。
  const state = new WeakMap();
  function brand(self) {
    const st = state.get(self);
    if (st === undefined) throw new TypeError("Illegal invocation");
    return st;
  }
  function coded(code, message) {
    const err = new TypeError(message);
    err.code = code;
    return err;
  }
  const COMPONENTS = ["protocol", "username", "password", "hostname", "port", "pathname", "search", "hash"];
  // init 字典读取：仅收字符串（数字/null 等一律视为缺席，真机口径）；
  // 属性读取走正常取值（用户 getter 抛错天然透传）。
  function readInit(obj) {
    const out = {};
    for (const k of COMPONENTS) {
      if (k in obj) {
        const v = obj[k];
        if (typeof v === "string") out[k] = v;
      }
    }
    return out;
  }
  // 构造输入归一：undefined/null → {}；string/对象直通；其余 ARG_TYPE。
  function normInput(input) {
    if (input === undefined || input === null) return {};
    if (typeof input === "string" || typeof input === "object") return input;
    throw coded("ERR_INVALID_ARG_TYPE", "Input must be an object or a string");
  }
  // exec/test 输入归一（文案不同，见 test-urlpattern-types）。
  function normMatchInput(input) {
    if (input === undefined || input === null) return {};
    if (typeof input === "string" || typeof input === "object") return input;
    throw coded("ERR_INVALID_ARG_TYPE", "URLPattern input needs to be a string or an object");
  }
  // base 归一：undefined → 缺席（null）；null → "null"（WebIDL 字符串化）；
  // string 直通；其余 ARG_TYPE。
  function normBase(base) {
    if (base === undefined) return null;
    if (base === null) return "null";
    if (typeof base === "string") return base;
    throw coded("ERR_INVALID_ARG_TYPE", "baseURL must be a string");
  }
  function matchPayload(input) {
    return (typeof input === "string") ? JSON.stringify(input) : JSON.stringify(readInit(input));
  }
  function buildResult(comps, inputEcho) {
    function comp(c) {
      const groups = {};
      for (const k of Object.keys(c.groups)) {
        const v = c.groups[k];
        groups[k] = (v === null) ? undefined : v;
      }
      return { groups, input: c.input };
    }
    // 键序钉死（test-urlpattern.js deepStrictEqual）：hash,hostname,inputs,
    // password,pathname,port,protocol,search,username。
    return {
      hash: comp(comps.hash),
      hostname: comp(comps.hostname),
      inputs: [inputEcho],
      password: comp(comps.password),
      pathname: comp(comps.pathname),
      port: comp(comps.port),
      protocol: comp(comps.protocol),
      search: comp(comps.search),
      username: comp(comps.username),
    };
  }
  function URLPattern(input, baseOrOptions, maybeOptions) {
    if (!(this instanceof URLPattern)) {
      throw coded("ERR_CONSTRUCT_CALL_REQUIRED", "Cannot call constructor without `new`");
    }
    let pattern, base = null, options;
    if (arguments.length >= 3 || typeof baseOrOptions === "string") {
      // 三参形或 (input, baseURL) 形（`undefined` base 亦 present，字符串化后解析）。
      pattern = normInput(input);
      base = (typeof baseOrOptions === "string") ? baseOrOptions : String(baseOrOptions);
      options = maybeOptions;
      if (options !== undefined && options !== null
        && (typeof options !== "object" && typeof options !== "function")) {
        throw coded("ERR_INVALID_ARG_TYPE", "options must be an object");
      }
      if (typeof pattern !== "string") {
        // 字典 + base → 构造失败（真机口径，非 OPERATION_FAILED）。
        throw coded("ERR_INVALID_URL_PATTERN", "Failed to construct URLPattern");
      }
    } else if (baseOrOptions !== undefined && baseOrOptions !== null
      && typeof baseOrOptions !== "object" && typeof baseOrOptions !== "function") {
      throw coded("ERR_INVALID_ARG_TYPE", "second argument must be a string or object");
    } else {
      pattern = normInput(input);
      options = baseOrOptions;
      if (options !== undefined && options !== null
        && (typeof options !== "object" && typeof options !== "function")) {
        throw coded("ERR_INVALID_ARG_TYPE", "options must be an object");
      }
    }
    const ignoreCase = !!(options && options.ignoreCase);
    const payload = (typeof pattern === "string") ? JSON.stringify(pattern) : JSON.stringify(readInit(pattern));
    let parsed;
    try {
      parsed = JSON.parse(__wjs2_urlpattern_parse(payload, base, ignoreCase));
    } catch {
      throw coded("ERR_INVALID_URL_PATTERN", "Failed to construct URLPattern");
    }
    state.set(this, { id: parsed.id, comps: parsed });
  }
  for (const k of COMPONENTS) {
    Object.defineProperty(URLPattern.prototype, k, {
      get() { return brand(this).comps[k]; },
      enumerable: true, configurable: true,
    });
  }
  Object.defineProperty(URLPattern.prototype, "hasRegExpGroups", {
    get() { return brand(this).comps.hasRegExpGroups; },
    enumerable: true, configurable: true,
  });
  URLPattern.prototype.test = function (input, base) {
    const st = brand(this);
    const norm = normMatchInput(input);
    const b = normBase(base);
    if (b !== null && typeof norm !== "string") {
      throw coded("ERR_OPERATION_FAILED", "Failed to test URLPattern");
    }
    return __wjs2_urlpattern_test(st.id, matchPayload(norm), b);
  };
  URLPattern.prototype.exec = function (input, base) {
    const st = brand(this);
    const norm = normMatchInput(input);
    const b = normBase(base);
    if (b !== null && typeof norm !== "string") {
      throw coded("ERR_OPERATION_FAILED", "Failed to exec URLPattern");
    }
    const raw = __wjs2_urlpattern_exec(st.id, matchPayload(norm), b);
    if (raw === null || raw === "null") return null;
    return buildResult(JSON.parse(raw), norm);
  };
  return URLPattern;
})();
globalThis.TextEncoder = class TextEncoder {
  get encoding() { return "utf-8"; }
  encode(s) { return __wjs2_te_encode(String(s === undefined ? "" : s)); }
  encodeInto(s, dest) { return JSON.parse(__wjs2_te_encode_into(String(s), dest)); }
};
// native 经 report_error 抛的是裸 Error（message 带 "TypeError: …" 前缀）：
// 转真类型 + 挂码（fatal 系列挂 INVALID_ENCODED_DATA，未知编码挂
// NOT_SUPPORTED；其余 native 错只转类型不挂码——存量无码行为不变）。
function __wjs2_tdTyped(e, code) {
  const m = String((e && e.message) || e);
  const mm = m.match(/^(TypeError|RangeError): ([\s\S]*)$/);
  let t = e, body = m;
  if (e instanceof TypeError || e instanceof RangeError) {
    t = e;
  } else if (mm) {
    body = mm[2];
    t = mm[1] === 'RangeError' ? new RangeError(body) : new TypeError(body);
  }
  if (t && typeof t === 'object' && t.code === undefined &&
      (/^The encoded data was not valid for encoding /.test(body) || /^unknown encoding /.test(body))) {
    try { t.code = code; } catch {}
  }
  throw t;
}
globalThis.TextDecoder = class TextDecoder {
  #label; #fatal; #ignoreBOM; #streamId;
  constructor(label = "utf-8", options) {
    // 未知编码：RangeError + ERR_ENCODING_NOT_SUPPORTED（真机码，invalid-label 套件点名）；
    // 另见 __wjs2_td_canonical 的 ASCII-空白口径（encoding.rs）。
    try {
      this.#label = __wjs2_td_canonical(String(label));
    } catch (e) {
      __wjs2_tdTyped(e, "ERR_ENCODING_NOT_SUPPORTED");
    }
    this.#fatal = !!(options && options.fatal);
    this.#ignoreBOM = !!(options && options.ignoreBOM);
    this.#streamId = undefined;
  }
  get encoding() { return this.#label; }
  get fatal() { return this.#fatal; }
  get ignoreBOM() { return this.#ignoreBOM; }
  // fatal 解码失败：TypeError + ERR_ENCODING_INVALID_ENCODED_DATA
  //（fatal/fatal-streaming/utf16-surrogates 套件点名；native 只抛裸 TypeError）。
  #finish(id, view, last) {
    try {
      return __wjs2_td_stream_feed(id, view, last);
    } catch (e) {
      __wjs2_tdTyped(e, "ERR_ENCODING_INVALID_ENCODED_DATA");
    }
  }
  decode(input, options) {
    let view = input;
    if (view === undefined) view = undefined;
    else if (view instanceof ArrayBuffer) view = new Uint8Array(view);
    else if (typeof SharedArrayBuffer !== "undefined" && view instanceof SharedArrayBuffer) {
      // 10f：SAB 快照拷贝后解码（真机读当前字节；跨会话真共享另案）。
      view = new Uint8Array(view);
    } else if (ArrayBuffer.isView(view)) {
      const __buf = view.buffer;
      if (typeof SharedArrayBuffer !== "undefined" && __buf instanceof SharedArrayBuffer) {
        // SAB 视图快照拷贝（真机同款；底座 `view_bytes` 仍拒 shared，拷贝后恒过）。
        view = new Uint8Array(view);
      } else if (!(view instanceof Uint8Array)) {
        view = new Uint8Array(__buf, view.byteOffset, view.byteLength);
      }
    } else if (view !== undefined && view !== null && !(view instanceof Uint8Array)) {
      // R3b：非 BufferSource 即 ERR_INVALID_ARG_TYPE（真机文案逐字；consumers 套件点名码）。
      const e = new TypeError(`The "list" argument must be an instance of SharedArrayBuffer, ArrayBuffer or ArrayBufferView. Received ${view === null ? "null" : typeof view}`);
      e.code = "ERR_INVALID_ARG_TYPE";
      throw e;
    }
    if (options && options.stream) {
      // 流式：有状态解码器攒截断序列（跨片多字节/stateful 编码正确）；
      // 中途 view 缺省视为空片（只推进状态，不收尾）。
      if (this.#streamId === undefined) {
        this.#streamId = __wjs2_td_stream_open(this.#label, this.#fatal ? 1 : 0, this.#ignoreBOM ? 1 : 0);
      }
      return this.#finish(this.#streamId, view, 0);
    }
    if (this.#streamId !== undefined) {
      // 非流式调用即收尾（含攒下的截断序列），id 自动回收
      //（fatal 抛错 native 侧同样回收，此处先摘除防复用）。
      const id = this.#streamId;
      this.#streamId = undefined;
      return this.#finish(id, view, 1);
    }
    if (view === undefined) return __wjs2_td_decode(this.#label, 0, 0, undefined);
    try {
      return __wjs2_td_decode(this.#label, this.#fatal ? 1 : 0, this.#ignoreBOM ? 1 : 0, view);
    } catch (e) {
      __wjs2_tdTyped(e, "ERR_ENCODING_INVALID_ENCODED_DATA");
    }
  }
};
// WebIDL 全局面不可枚举（custom-global 套件点名 URL/URLSearchParams 描述符）。
// 注意：已存在属性的部分描述符只改指定字段，enumerable 须显式写 false。
for (const __wjs2_g of ["URL", "URLSearchParams", "URLPattern", "TextEncoder", "TextDecoder"]) {
  Object.defineProperty(globalThis, __wjs2_g, { value: globalThis[__wjs2_g], writable: true, enumerable: false, configurable: true });
}
"#;
