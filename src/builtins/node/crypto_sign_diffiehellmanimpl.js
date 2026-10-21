class DiffieHellmanImpl {
  constructor(prime, generator, generatorEncoding) {
    // 10f crypto首轮：数值位长形同步生成安全素数（真机口径；`generatePrimeSync`
    // safe 复用——size 轮 verifyError 恒 0，dh 套件钉住；大位长搜索慢，记档）。
    if (typeof prime === "number") {
      // node 口径（diffiehellman.js + crypto_dh.cc New）：size 先 validateInt32
      //（非整数即 OUT_OF_RANGE-integer）；<2 即 OSSL MODULUS_TOO_SMALL；
      // <512 即 ARG_VALUE 'Invalid DH parameters'（paramgen 下限，256/511 行钉住）。
      if (!Number.isInteger(prime)) {
        const err = new RangeError(`The value of "sizeOrKey" is out of range. It must be an integer. Received ${prime}`);
        err.code = "ERR_OUT_OF_RANGE";
        throw err;
      }
      if (prime > 2147483647 || prime < -2147483648) {
        const err = new RangeError(`The value of "sizeOrKey" is out of range. It must be >= -2147483648 && <= 2147483647. Received ${prime}`);
        err.code = "ERR_OUT_OF_RANGE";
        throw err;
      }
      if (prime < 2) {
        throw __dhOsslErr("ERR_OSSL_DH_MODULUS_TOO_SMALL",
          "error:0280007E:Diffie-Hellman routines::modulus too small");
      }
      if (prime < 512) {
        const err = new TypeError("Invalid DH parameters");
        err.code = "ERR_INVALID_ARG_VALUE";
        throw err;
      }
      prime = generatePrimeSync(prime, { safe: true });
      // 数值位长形的生成元：falsy 即 2；数字须 int32（C++ 侧 Buffer 即
      // ARG_TYPE 'Second argument must be an int32'）；余者走统一步骤。
      if (!generator) generator = 2;
      else if (typeof generator === "number") __dhCheckInt32Generator(generator);
      else if (typeof generator === "string" || generator instanceof ArrayBuffer || ArrayBuffer.isView(generator)) {
        const err = new TypeError("Second argument must be an int32");
        err.code = "ERR_INVALID_ARG_TYPE";
        throw err;
      }
    }
    const primeB = (typeof prime === "string") ? __cryptBytes(prime, "prime", generatorEncoding ?? "hex") : __cryptBytes(prime, "prime");
    this.__prime = primeB;
    // 10f crypto二轮：generator 字节语义——数字即值；字符串按 generatorEncoding
    // （缺省 latin1）解码；Buffer/视图即裸字节；falsy 缺省 2（dh 套件 g=0 行钉住）。
    this.__genBytes = __dhGenBytes(generator, generatorEncoding);
    this.__gen = __dhGenNum(this.__genBytes);
    // C++ New 口径（crypto_dh.cc 实测钉死）：显式生成元 ≤1 即 ERR_OSSL_DH_BAD_GENERATOR
    //（与位长无关；缺省 2 不检）；OSSL3 叠加：位长 ≥512 且 g≥p 即抛（小素数仅
    // verifyError 置 8；2048 位 g==p 抛，dh-curves 76 行钉住）。
    const pVal = __bytesToBigint(primeB);
    const gVal = __bytesToBigint(Buffer.from(this.__genBytes));
    if (gVal < 2n) {
      throw __dhOsslErr("ERR_OSSL_DH_BAD_GENERATOR",
        "error:02800065:Diffie-Hellman routines::bad generator");
    }
    if (pVal.toString(2).length >= 512 && gVal >= pVal) {
      throw __dhOsslErr("ERR_OSSL_DH_BAD_GENERATOR",
        "error:02800065:Diffie-Hellman routines::bad generator");
    }
    this.__priv = null;
    this.__pub = null;
  }
  static group(name) {
    const hex = __DH_GROUPS[String(name).toLowerCase()];
    if (hex === undefined) {
      // 真机口径（keygen/Group 共用）：码 ERR_CRYPTO_UNKNOWN_DH_GROUP，
      // 消息无名后缀（keygen 558 行钉住）。
      const err = new Error("Unknown DH group");
      err.code = "ERR_CRYPTO_UNKNOWN_DH_GROUP";
      throw err;
    }
    return new DiffieHellmanImpl(__cryptBytes(hex, "prime", "hex"), 2);
  }
  generateKeys(encoding) {
    // node 口径（dh-generate-keys 套件钉死）：按需增量——缺私钥即新生成一对；
    // 仅缺公钥/公钥过期（与私钥失配）即由私钥重算公钥；成对即无操作。
    if (this.__priv === null) {
      const r = JSON.parse(__cryptCall(() => __wjs2_dh_genkey(this.__prime, this.__gen, this.__prime.length)));
      this.__priv = __b64dec(r.priv);
      this.__pub = __b64dec(r.pub);
    } else {
      const expectPub = __dhPubFromPriv(this.__prime, this.__genBytes, this.__priv);
      if (this.__pub === null || !Buffer.from(this.__pub).equals(expectPub)) {
        this.__pub = new Uint8Array(expectPub);
      }
    }
    // node 口径（dhGenerateKeys）：encoding 透传 getPublicKey（hex 行钉住）。
    return this.getPublicKey(encoding);
  }
  getPublicKey(encoding) {
    if (this.__pub === null) {
      const err = new Error("DH keys not generated");
      err.code = "ERR_CRYPTO_INVALID_STATE";
      throw err;
    }
    // node 口径（encode）：缺省/falsy/'buffer'（严格大小写）即回 Buffer；
    // 余下走 toString（非法编码即抛，dh 套件 BUFFER 行钉住）。
    if (!encoding || encoding === "buffer") return Buffer.from(this.__pub);
    return Buffer.from(this.__pub).toString(encoding);
  }
  getPrivateKey(encoding) {
    if (this.__priv === null) {
      const err = new Error("DH keys not generated");
      err.code = "ERR_CRYPTO_INVALID_STATE";
      throw err;
    }
    if (!encoding || encoding === "buffer") return Buffer.from(this.__priv);
    return Buffer.from(this.__priv).toString(encoding);
  }
  getPrime(encoding) {
    if (!encoding || encoding === "buffer") return Buffer.from(this.__prime);
    return Buffer.from(this.__prime).toString(encoding);
  }
  getGenerator(encoding) {
    // 10f crypto二轮：回存的 generator 字节（非单字节截断）。
    const g = this.__genBytes;
    if (!encoding || encoding === "buffer") return Buffer.from(g);
    return Buffer.from(g).toString(encoding);
  }
  setPublicKey(pub, encoding) {
    // node 口径：DH.setPublicKey 无废弃（DEP0031 仅属 ECDH，见 crypto_sign.js）。
    this.__pub = __cryptBytes(pub, "public key", encoding);
    return this;
  }
  setPrivateKey(priv, encoding) { this.__priv = __cryptBytes(priv, "private key", encoding); return this; }
  computeSecret(peer, inEnc, outEnc) {
    if (this.__priv === null) {
      const err = new Error("DH keys not generated");
      err.code = "ERR_CRYPTO_INVALID_STATE";
      throw err;
    }
    const peerB = (typeof peer === "string") ? __cryptBytes(peer, "peer", inEnc) : __cryptBytes(peer, "peer");
    // node 口径（ncrypto checkPublicKey）：y≤1 即 TOO_SMALL，y≥p-1 即 TOO_LARGE
    //（dh-curves 45/51/58 行钉住；空串 y=0 落 small）。
    const pVal = __bytesToBigint(this.__prime);
    const yVal = __bytesToBigint(peerB);
    if (yVal <= 1n) {
      const err = new Error("Supplied key is too small");
      err.code = "ERR_CRYPTO_INVALID_KEYLEN";
      throw err;
    }
    if (yVal >= pVal - 1n) {
      const err = new Error("Supplied key is too large");
      err.code = "ERR_CRYPTO_INVALID_KEYLEN";
      throw err;
    }
    const secret = __cryptCall(() => __wjs2_dh_secret(this.__prime, this.__priv, peerB));
    // node 口径（encode）：缺省/falsy/'buffer'（严格大小写）即回 Buffer。
    if (!outEnc || outEnc === "buffer") return Buffer.from(secret);
    return Buffer.from(secret).toString(outEnc);
  }
  // node 口径：verifyError 为属性（DH_check 位掩码，非方法）。
  get verifyError() { return __dhCheckFlags(this.__prime, this.__genBytes); }
}
// node 口径（DH_check 位掩码，crypto_dh.cc Check 实测 + 真机逐项钉死）：
// 偶数/奇合数即 1（P_NOT_PRIME）；奇素数（2 视素数）且 q=(p-1)/2<2 或合数即 2
//（P_NOT_SAFE_PRIME，合数 p 不查 q）；位长 <512 即 128（MODULUS_TOO_SMALL，
// 512 整界干净）；生成元 g≥p-1 即 8（NOT_SUITABLE，g≤1 构造期已抛）；按位或组合
//（17→130、23/g23→136、27/g26→137、notSafePrime→2，dh-curves 全行钉住）。
function __dhCheckFlags(primeB, genB) {
  try {
    const pBytes = Buffer.from(primeB);
    const p = __bytesToBigint(pBytes);
    let pPrime = false;
    try { pPrime = !!__cryptCall(() => __wjs2_prime_check(pBytes, 64)); } catch { pPrime = false; }
    let flags = 0;
    if ((p & 1n) === 0n || !pPrime) {
      flags |= 1;
    } else {
      const q = (p - 1n) / 2n;
      if (q < 2n) {
        flags |= 2;
      } else {
        let qHex = q.toString(16);
        if (qHex.length % 2) qHex = "0" + qHex;
        let qPrime = false;
        try { qPrime = !!__cryptCall(() => __wjs2_prime_check(Buffer.from(qHex, "hex"), 64)); } catch { qPrime = false; }
        if (!qPrime) flags |= 2;
      }
    }
    if (p.toString(2).length < 512) flags |= 128;
    const g = __bytesToBigint(genB);
    if (g >= p - 1n) flags |= 8;
    return flags;
  } catch {
    return 8;
  }
}
// OSSL 形 JS 错误（library/reason 经 `=` 赋值，原型链 setter 透传；sign
// 两件 'bye, bye' 套件钉住；opensslErrorStack 永不 `=` 触碰，见 crypto_sign.js）。
function __dhOsslErr(code, message) {
  const err = new Error(message);
  err.code = code;
  const m = message.match(/^error:[0-9A-Fa-f]+:([^:]*):(.*)$/);
  if (m) {
    err.library = m[1];
    err.reason = m[2].replace(/^:/, "");
  }
  return err;
}
// 生成元 int32 校验（createDiffieHellman 数字生成元 / 数值位长形共用；
// 非整数/越界即 OUT_OF_RANGE，真机逐项）。
function __dhCheckInt32Generator(gen) {
  if (!Number.isInteger(gen)) {
    const err = new RangeError(`The value of "generator" is out of range. It must be an integer. Received ${gen}`);
    err.code = "ERR_OUT_OF_RANGE";
    throw err;
  }
  if (gen < -2147483648 || gen > 2147483647) {
    const err = new RangeError(`The value of "generator" is out of range. It must be >= -2147483648 && <= 2147483647. Received ${gen}`);
    err.code = "ERR_OUT_OF_RANGE";
    throw err;
  }
}
export function createDiffieHellman(prime, generator, generatorEncoding) {
  if (typeof prime === "string" && __DH_GROUPS[prime.toLowerCase()] !== undefined && generator === undefined) {
    return DiffieHellmanImpl.group(prime);
  }
  // node 口径（diffiehellman.js 构造器）：第 2 参为合法编码/'buffer' 即
  // keyEncoding（素数字符串解码用），非生成元；非法编码串才顺延为生成元
  //（dh 套件 `createDiffieHellman(p1, 'buffer')` 行钉住）。
  if (typeof prime !== "number" && typeof generator === "string" &&
      (generator === "buffer" || Buffer.isEncoding(generator))) {
    generatorEncoding = generator;
    generator = undefined;
  }
  return new DiffieHellmanImpl(prime, generator, generatorEncoding);
}
// 10f crypto二轮：generator 字节语义 helpers（见构造注释）。
function __dhGenBytes(gen, genEnc) {
  // falsy（undefined/null/0/''/NaN）即缺省 2；余下见构造注释。
  if (!gen) return new Uint8Array([2]);
  if (typeof gen === "number") {
    __dhCheckInt32Generator(gen);
    const out = [];
    let v = gen;
    while (v > 0) { out.unshift(v & 255); v = Math.floor(v / 256); }
    return new Uint8Array(out);
  }
  if (typeof gen === "string") return __cryptBytes(gen, "generator", genEnc ?? "latin1");
  if (gen instanceof ArrayBuffer || ArrayBuffer.isView(gen)) return __cryptBytes(gen, "generator");
  const err = new TypeError(`The "generator" argument must be one of type number or string or an instance of ArrayBuffer, Buffer, TypedArray, or DataView.${__argTypeHelper(gen)}`);
  err.code = "ERR_INVALID_ARG_TYPE";
  throw err;
}
function __dhGenNum(bytes) {
  let v = 0n;
  for (const b of bytes) v = (v << 8n) | BigInt(b);
  return v <= 0xFFFFFFFFFFFFFFFFn ? Number(v) : NaN;
}
export function createDiffieHellmanGroup(name) { return __dhGroup(DiffieHellmanImpl.group(name)); }
export function getDiffieHellman(name) { return __dhGroup(DiffieHellmanImpl.group(name)); }
// 10f crypto首轮：真机 `crypto.DiffieHellman/DiffieHellmanGroup/ECDH` 均可无 new 调用。
function DiffieHellman(...args) { return new DiffieHellmanImpl(...args); }
Object.setPrototypeOf(DiffieHellman, DiffieHellmanImpl);
DiffieHellman.prototype = DiffieHellmanImpl.prototype;
DiffieHellman.prototype.constructor = DiffieHellman;
function DiffieHellmanGroup(name) { return __dhGroup(DiffieHellmanImpl.group(name)); }
Object.setPrototypeOf(DiffieHellmanGroup, DiffieHellmanImpl);
// 10f crypto二轮：Group 自立原型（链向 Impl 原型）：constructor 归 Group、
// setters 置 undefined（真机口径：Group 无 setters，其余方法继承可用）。
DiffieHellmanGroup.prototype = Object.create(DiffieHellmanImpl.prototype);
DiffieHellmanGroup.prototype.constructor = DiffieHellmanGroup;
DiffieHellmanGroup.prototype.setPrivateKey = undefined;
DiffieHellmanGroup.prototype.setPublicKey = undefined;
function __dhGroup(inst) { Object.setPrototypeOf(inst, DiffieHellmanGroup.prototype); return inst; }
export function diffieHellman(options) {
  const priv = options?.privateKey;
  const pub = options?.publicKey;
  const dh = (priv instanceof DiffieHellman) ? priv : null;
  if (dh !== null && pub instanceof DiffieHellman) {
    return dh.computeSecret(pub.getPublicKey());
  }
  if (__isKeyObject(priv) && __isKeyObject(pub)) {
    if (priv.__keyType === "x25519") {
      const out = __cryptCall(() => __wjs2_x_derive(priv.__material, pub.__material));
      return Buffer.from(out);
    }
    if (priv.__keyType === "x448") {
      const out = __cryptCall(() => __wjs2_x448_derive(priv.__material, pub.__material));
      return Buffer.from(out);
    }
    if (priv.__keyType === "ec") {
      const curve = priv.__detail.namedCurve;
      const pubDer = pub.__kind === "private" ? __derivePublic(pub).__material : pub.__material;
      const out = __cryptCall(() => __wjs2_ecdh_derive(curve, priv.__material, pubDer));
      return Buffer.from(out);
    }
    const err = new Error("diffieHellman needs DH/ECDH/X25519 keys");
    err.code = "ERR_NOT_SUPPORTED";
    throw err;
  }
  const err = new TypeError("diffieHellman needs { privateKey, publicKey }");
  err.code = "ERR_INVALID_ARG_TYPE";
  throw err;
}
function __bigintToBytes(v) {
  if (typeof v === "bigint") {
    if (v < 0n) {
      const err = new RangeError(`The value of "candidate" is out of range. It must be >= 0. Received ${v}n`);
      err.code = "ERR_OUT_OF_RANGE";
      throw err;
    }
    let hex = v.toString(16);
    if (hex.length % 2) hex = "0" + hex;
    return __cryptBytes(hex.length === 0 ? "00" : hex, "candidate", "hex");
  }
  if (typeof v !== "object" || v === null) {
    // node 口径（checkPrime）：仅 BigInt/ArrayBuffer/视图；数字/串/对象皆拒。
    const err = new TypeError(`The "candidate" argument must be of type bigint or an instance of ArrayBuffer, TypedArray, Buffer, or DataView.${__argTypeHelper(v)}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  return __cryptBytes(v, "candidate");
}
function __checkPrimeChecks(options) {
  // node 口径（random.js checkPrime）：options 对象；checks 缺省 0，
  // validateInt32(checks,'options.checks',0)。
  if (options === undefined) options = {};
  // validateObject 口径：数组亦拒（random 套件 Received an instance of Array）。
  if (typeof options !== "object" || options === null || Array.isArray(options)) {
    const err = new TypeError(`The "options" argument must be of type object.${__argTypeHelper(options)}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  const checks = options.checks === undefined ? 0 : options.checks;
  if (typeof checks !== "number" || !Number.isInteger(checks)) {
    const err = new TypeError(`The "options.checks" property must be of type number.${__argTypeHelper(options.checks)}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  if (checks < 0 || checks > 2147483647) {
    const err = new RangeError(`The value of "options.checks" is out of range. It must be >= 0 && <= 2147483647. Received ${options.checks}`);
    err.code = "ERR_OUT_OF_RANGE";
    throw err;
  }
  return checks + 0;
}
function __checkPrimeBytes(bytes) {
  // node 口径（CheckPrimeJob）：候选超 64MB 即 ERR_OSSL_BN_BIGNUM_TOO_LONG
  //（prime 套件 64MB 行钉住；63.5MB 可检，边界上浮记档）。
  if (bytes.byteLength >= 67108864) {
    throw __dhOsslErr("ERR_OSSL_BN_BIGNUM_TOO_LONG",
      "error:01800072:bignum routines::bignum too long");
  }
  return bytes;
}
export function checkPrimeSync(candidate, options) {
  const bytes = __checkPrimeBytes(__bigintToBytes(candidate));
  const checks = __checkPrimeChecks(options);
  return __cryptCall(() => __wjs2_prime_check(bytes, checks));
}
export function checkPrime(candidate, options, callback) {
  // node 口径（random.js checkPrime）：候选/回调/选项全同步校验（prime 套件
  // checkPrime(-1n, mustNotCall) 同步抛行钉住），仅素性判定异步。
  if (typeof options === "function") { callback = options; options = undefined; }
  const bytes = __checkPrimeBytes(__bigintToBytes(candidate));
  if (typeof callback !== "function") {
    const err = new TypeError("checkPrime requires a callback for async form");
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  const checks = __checkPrimeChecks(options);
  queueMicrotask(() => {
    try {
      callback(null, __cryptCall(() => __wjs2_prime_check(bytes, checks)));
    } catch (e) {
      callback(e);
    }
  });
}
function __bytesToBigint(u8) {
  const buf = Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength);
  const hex = buf.toString("hex").replace(/^0+(?=[0-9a-f])/i, "");
  if (hex === "") return 0n;
  return BigInt("0x" + hex);
}
// DH 公钥重算（g^x mod p，二进制平方乘 BigInt 原生；generateKeys 增量语义用）。
// 输出定长（prime 长左补零，native pad_be 同款）。
function __dhPubFromPriv(primeB, genB, privB) {
  const p = __bytesToBigint(primeB);
  const g = __bytesToBigint(genB);
  const x = __bytesToBigint(privB);
  let r = 1n;
  let b = g % p;
  let e = x;
  while (e > 0n) {
    if (e & 1n) r = (r * b) % p;
    b = (b * b) % p;
    e >>= 1n;
  }
  let hex = r.toString(16);
  if (hex.length % 2) hex = "0" + hex;
  const raw = Buffer.from(hex, "hex");
  const out = Buffer.alloc(primeB.length);
  raw.copy(out, out.length - raw.length);
  return out;
}
function __bigintToMinimalBuffer(v) {
  let hex = v.toString(16);
  if (hex.length % 2) hex = "0" + hex;
  return Buffer.from(hex === "" ? "00" : hex, "hex");
}
function __generatePrimeArgs(size, options) {
  // node 口径（random.js validateInt32(size,'size',1) + createRandomPrimeJob）：
  // 非数即 ARG_TYPE；非整数/越界即 OUT_OF_RANGE；options 对象校验；
  // safe/bigint 布尔；add/rem Buffer/视图/BigInt（负即 OUT_OF_RANGE）；
  // add < 2^bits（'invalid options.add'），rem < add（'invalid options.rem'）；
  // 有 add 无 rem 时 rem 缺省 safe?3:1（真机实测钉住）。
  if (typeof size !== "number") {
    const err = new TypeError(`The "size" argument must be of type number.${__argTypeHelper(size)}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  if (!Number.isInteger(size)) {
    const err = new RangeError(`The value of "size" is out of range. It must be an integer. Received ${size}`);
    err.code = "ERR_OUT_OF_RANGE";
    throw err;
  }
  if (size < 1 || size > 2147483647) {
    const err = new RangeError(`The value of "size" is out of range. It must be >= 1 && <= 2147483647. Received ${size}`);
    err.code = "ERR_OUT_OF_RANGE";
    throw err;
  }
  // OpenSSL 口径（BN_generate_prime_ex2）：<2 位无素数；安全素数 <6 位仅 3 位
  // 可达（tiny 探针：1→错、2safe→错、4/5safe→错，3/6safe→通）。
  if (size < 2) {
    throw __dhOsslErr("ERR_OSSL_BN_BITS_TOO_SMALL",
      "error:01800076:bignum routines::bits too small");
  }
  if (options !== undefined && typeof options === "object" && options !== null && !Array.isArray(options)) {
    const sf = options.safe === undefined ? false : options.safe;
    if (sf === true && size < 6 && size !== 3) {
      throw __dhOsslErr("ERR_OSSL_BN_BITS_TOO_SMALL",
        "error:01800076:bignum routines::bits too small");
    }
  }
  if (options === undefined) options = {};
  if (typeof options !== "object" || options === null || Array.isArray(options)) {
    const err = new TypeError(`The "options" argument must be of type object.${__argTypeHelper(options)}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  const safe = options.safe === undefined ? false : options.safe;
  if (typeof safe !== "boolean") {
    const err = new TypeError(`The "options.safe" argument must be of type boolean.${__argTypeHelper(options.safe)}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  const useBigint = options.bigint === undefined ? false : options.bigint;
  if (typeof useBigint !== "boolean") {
    const err = new TypeError(`The "options.bigint" argument must be of type boolean.${__argTypeHelper(options.bigint)}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  let { add, rem } = options;
  for (const [nm, v] of [["add", add], ["rem", rem]]) {
    if (v === undefined) continue;
    if (typeof v === "bigint") {
      if (v < 0n) {
        const err = new RangeError(`The value of "options.${nm}" is out of range. It must be >= 0. Received ${v}n`);
        err.code = "ERR_OUT_OF_RANGE";
        throw err;
      }
      continue;
    }
    if (v instanceof ArrayBuffer || ArrayBuffer.isView(v)) continue;
    const err = new TypeError(`The "options.${nm}" property must be of type bigint or an instance of ArrayBuffer, TypedArray, Buffer, or DataView.${__argTypeHelper(v)}`);
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  const toBuf = (v) => (typeof v === "bigint" ? __bigintToMinimalBuffer(v)
    : Buffer.from(v instanceof ArrayBuffer ? v : v.buffer, v.byteOffset ?? 0, v.byteLength ?? v.byteLength));
  const addN = add === undefined ? undefined
    : (typeof add === "bigint" ? add : __bytesToBigint(new Uint8Array(add instanceof ArrayBuffer ? add : add.buffer, add.byteOffset ?? 0, add.byteLength ?? add.byteLength)));
  let remN = rem === undefined ? undefined
    : (typeof rem === "bigint" ? rem : __bytesToBigint(new Uint8Array(rem instanceof ArrayBuffer ? rem : rem.buffer, rem.byteOffset ?? 0, rem.byteLength ?? rem.byteLength)));
  if (addN !== undefined && remN !== undefined && remN >= addN) {
    const err = new RangeError("invalid options.rem");
    err.code = "ERR_OUT_OF_RANGE";
    throw err;
  }
  // add < 2^bits（位长比较，免 1n<<size 巨分配；'invalid options.add'）。
  if (addN !== undefined && addN.toString(2).length > size) {
    const err = new RangeError("invalid options.add");
    err.code = "ERR_OUT_OF_RANGE";
    throw err;
  }
  if (addN !== undefined && remN === undefined) remN = safe ? 3n : 1n;
  const addBuf = addN === undefined || addN === 0n ? undefined : toBuf(addN);
  const remBuf = remN === undefined ? undefined : toBuf(remN);
  return { size, safe, useBigint, addBuf, remBuf };
}
function __generatePrimeRun(args) {
  const out = __cryptCall(() => __wjs2_prime_gen(args.size, 64, args.safe ? 1 : 0, args.addBuf, args.remBuf));
  // 真机口径：缺省回 ArrayBuffer（非 Buffer，prime 套件 Buffer.from 兼容行钉住）；
  // bigint 经 16 进制桥（`BigInt("0x…")`，零 native 改动；§4.44 同类绕行）。
  const buf = Buffer.from(out);
  if (args.useBigint === true) return BigInt("0x" + buf.toString("hex"));
  return buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
}
export function generatePrimeSync(size, options) {
  return __generatePrimeRun(__generatePrimeArgs(size, options));
}
export function generatePrime(size, options, callback) {
  // node 口径（random.js generatePrime）：size/回调/选项全同步校验（prime 套件
  // mustNotCall 行钉住），仅素数生成异步。
  if (typeof options === "function") { callback = options; options = undefined; }
  const args = __generatePrimeArgs(size, options);
  if (typeof callback !== "function") {
    const err = new TypeError("generatePrime requires a callback for async form");
    err.code = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  queueMicrotask(() => {
    try {
      callback(null, __generatePrimeRun(args));
    } catch (e) {
      callback(e);
    }
  });
}
