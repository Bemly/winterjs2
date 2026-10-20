// ── MIMEType/MIMEParams（WHATWG MIME 解析子集；真机 26.8.2 逐项对过）──────
// essence 错：`for a type/subtype in "<essence>" is invalid[ at N]`（空串无 at）；
// setter 同口径（shown = 所赋值）；type/subtype 小写化；essence setter 是 no-op；
// 无 `=` 的参数段跳过；名小写化、值大小写保留；空值/特殊字符序列化加引号；
// MIMEParams 无 size/sort/forEach，delete 回 undefined。
function mimeBadCharIndex(s) {
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    const ok = (c >= 48 && c <= 57) || (c >= 65 && c <= 90) || (c >= 97 && c <= 122) ||
      c === 33 || c === 35 || c === 36 || c === 37 || c === 38 || c === 39 || c === 42 ||
      c === 43 || c === 45 || c === 46 || c === 94 || c === 95 || c === 96 || c === 124 || c === 126;
    if (!ok) return i;
  }
  return -1;
}
function mimeCheckToken(kind, shown, value) {
  if (value === '') throw new ERR_INVALID_MIME_SYNTAX(kind, shown);
  const bad = mimeBadCharIndex(value);
  if (bad !== -1) throw new ERR_INVALID_MIME_SYNTAX(kind, shown, bad);
}
function mimeQuoteValue(value) {
  if (value === '' || /[^!#$%&'*+\-.^_`|~0-9A-Za-z]/.test(value)) {
    return '"' + value.replace(/(["\\])/g, '\\$1') + '"';
  }
  return value;
}
class MIMEParams {
  #pairs;
  constructor(pairs = []) { this.#pairs = pairs; }
  get(name) {
    name = String(name).toLowerCase();
    for (const [k, v] of this.#pairs) {
      if (k === name) return v;
    }
    return null;
  }
  set(name, value) {
    name = String(name).toLowerCase();
    value = String(value);
    mimeCheckToken('parameter name', name, name);
    this.#pairs = this.#pairs.filter(([k]) => k !== name);
    this.#pairs.push([name, value]);
  }
  has(name) {
    name = String(name).toLowerCase();
    return this.#pairs.some(([k]) => k === name);
  }
  delete(name) {
    name = String(name).toLowerCase();
    this.#pairs = this.#pairs.filter(([k]) => k !== name);
  }
  keys() { return this.#pairs.map(([k]) => k).values(); }
  values() { return this.#pairs.map(([, v]) => v).values(); }
  entries() { return this.#pairs.map(([k, v]) => [k, v]).values(); }
  toString() {
    return this.#pairs.map(([k, v]) => `${k}=${mimeQuoteValue(v)}`).join(';');
  }
}
class MIMEType {
  #type;
  #subtype;
  #params;
  constructor(input) {
    if (typeof input !== 'string') throw new ERR_INVALID_ARG_TYPE('input', 'string', input);
    // 本体逐字 node internal/mime parseTypeAndSubtype 口径：前导 HTTP 空白
    // 跳过（仅 space/\t/\n/\r，\v\f 不算）；type 收到 '/' 为止须非空纯 token；
    // subtype 收到 ';' 为止，去尾 HTTP 空白后须非空，尾空白区前有非 token
    // （含前导空格、\v\f）即非法。
    const isHttpWs = (c) => c === ' ' || c === '\t' || c === '\n' || c === '\r';
    const isTokenChar = (code) => code <= 0xFF && mimeBadCharIndex(String.fromCharCode(code)) === -1;
    let p = 0;
    while (p < input.length && isHttpWs(input[p])) p++;
    const typeStart = p;
    while (p < input.length && input[p] !== '/') p++;
    if (p === typeStart || p >= input.length) {
      throw new ERR_INVALID_MIME_SYNTAX('type', input);
    }
    const rawType = input.slice(typeStart, p);
    mimeCheckToken('type', input, rawType);
    p++;
    const subStart = p;
    let subEnd = p, firstBad = -1;
    while (p < input.length && input[p] !== ';') {
      const code = input.charCodeAt(p);
      if (!isTokenChar(code)) {
        if (firstBad === -1) firstBad = p;
      }
      if (!isHttpWs(input[p])) subEnd = p + 1;
      p++;
    }
    if (subEnd === subStart || (firstBad !== -1 && firstBad < subEnd)) {
      // shown 挂全串（真机 `in "t/x y/z;q=1"` 口径；旧实现误挂 essence 前段）。
      // 空 subtype 无下标（旧口径），非法下标相对 subtype 起始。
      if (subEnd === subStart) throw new ERR_INVALID_MIME_SYNTAX('subtype', input);
      throw new ERR_INVALID_MIME_SYNTAX('subtype', input, firstBad - subStart);
    }
    const type = rawType.toLowerCase(), subtype = input.slice(subStart, subEnd).toLowerCase();
    this.#type = type;
    this.#subtype = subtype;
    const semi = p < input.length ? p : -1;
    // 参数表逐字 node internal/mime parseParameters 口径（位置游标，无预切分）：
    // 名收至 ';'/ '='（前导 HTTP 空白已在轮首跳过），须非空纯 token；
    // 无值参数（撞 ';'）忽略；值首字符为 '"' 进引号分支（转义解，
    // 尾部杂物收到 ';' 为止），否则裸收至 ';'（尾空白剥离）；
    // 值非法（quoted-token 集外且位于尾空白区前）即整项丢弃；
    // 重名首个胜出。
    const isQuotedToken = (code) => code === 0x09 || (code >= 0x20 && code <= 0x7E) || (code >= 0x80 && code <= 0xFF);
    const pairs = [];
    if (semi !== -1) {
      const body = input.slice(semi + 1), end = body.length;
      let q = 0;
      while (q < end) {
        while (q < end && isHttpWs(body[q])) q++;
        const nameStart = q;
        while (q < end && body[q] !== ';' && body[q] !== '=') q++;
        const rawName = body.slice(nameStart, q);
        if (q < end) {
          if (body[q] === ';') { q++; continue; }
          q++;
        }
        if (q >= end) break;
        let value;
        if (body[q] === '"') {
          q++;
          let v = '';
          for (;;) {
            let chunk = q;
            while (q < end && body[q] !== '"' && body[q] !== '\\') q++;
            v += body.slice(chunk, q);
            if (q >= end) break;
            if (body[q] === '\\') {
              q++;
              if (q >= end) { v += '\\'; break; }
              v += body[q]; q++;
              continue;
            }
            break;
          }
          while (q < end && body[q] !== ';') q++;
          let bad = false;
          for (let j = 0; j < v.length; j++) {
            if (!isQuotedToken(v.charCodeAt(j))) { bad = true; break; }
          }
          if (bad) { if (q < end) q++; continue; }
          value = v;
        } else {
          const vs = q;
          let ve = q, badAt = -1;
          while (q < end && body[q] !== ';') {
            const code = body.charCodeAt(q);
            if (!isQuotedToken(code) && badAt === -1) badAt = q;
            if (!isHttpWs(body[q])) ve = q + 1;
            q++;
          }
          if (ve === vs || (badAt !== -1 && badAt < ve)) { if (q < end) q++; continue; }
          value = body.slice(vs, ve);
        }
        if (rawName !== '' && mimeBadCharIndex(rawName) === -1) {
          const name = rawName.toLowerCase();
          if (!pairs.some(([k]) => k === name)) pairs.push([name, value]);
        }
        if (q < end) q++;
      }
    }
    this.#params = new MIMEParams(pairs);
  }
  get type() { return this.#type; }
  set type(v) {
    v = String(v);
    mimeCheckToken('type', v, v);
    this.#type = v.toLowerCase();
  }
  get subtype() { return this.#subtype; }
  set subtype(v) {
    v = String(v);
    mimeCheckToken('subtype', v, v);
    this.#subtype = v.toLowerCase();
  }
  get essence() { return `${this.#type}/${this.#subtype}`; }
  set essence(_) {}
  get params() { return this.#params; }
  // node 口径（mime-whatwg 套件）：parse 先 String 化再解析，
  // 失败回 null（构造器抛错，parse 吞错；String 化在 try 外，
  // Symbol 输入的 TypeError 照常透出，真机同款）。
  static parse(input) {
    input = `${input}`;
    try {
      return new MIMEType(input);
    } catch {
      return null;
    }
  }
  toString() {
    const ps = this.#params.toString();
    return ps ? `${this.essence};${ps}` : this.essence;
  }
}
