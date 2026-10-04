#!/usr/bin/env python3
"""标识符阶段命名门禁（AGENTS 4.271-4.274）：计划/轮次/批次号禁进标识符。

Scope: src/ tests/ benches/ 的标识符声明位 + fixture/通道名 + console 断言标签
  + 环境变量键 + 非历史文档的测试指针对。注释与 plan3/journal/parity 历史豁免。
原则（4.274）：只禁与历史计划 token 碰撞的——phaseN/pN_/mN_/轮次 rN/round/
  批次 bN/batch/组 gN/分层 tN/T10E/切片 c4x/10x/m9i。纯局部序号（rq2/s2/cli2/
  res2/e1）与域语义（算法名/协议版本/类型名/方向缩写）不在此列，下方白名单备查。
用法: python3 scripts/check-naming.py；命中即列出并 exit 1。
"""
import re
import subprocess
import sys

ROOT = subprocess.run(
    ["git", "rev-parse", "--show-toplevel"],
    capture_output=True, text=True,
).stdout.strip()

# 标识符声明位：fn/mod/struct/enum/trait/type/const/static（+JS function/class）
DECL = re.compile(
    r"^\s*(?:pub(?:\([^)]*\))?\s*)?"
    r"(?:unsafe\s+)?(?:extern\s+\"C\"\s+)?"
    r"(?:fn|mod|struct|enum|trait|type|const|static|function|class)\s+"
    r"([A-Za-z0-9_]+)"
)
# b64/base64、http2/h2_ 属域语义，显式豁免
ID_BAD = re.compile(
    r"phase[0-9]|phase_napi|phase_mapper|phase_g92"
    r"|(?<![A-Za-z])p[0-9]_|_m[0-9]_|_r[0-9][a-z]?_"
    r"|round[0-9]|batch[0-9]|_(?:g[0-9]+|b(?!64)[0-9]*)_"
    r"|[Cc]4[xX]|misc[0-9]|tier[0-9]|stage[0-9]"
    r"|t[0-9]_handler|T10E|t10e|10[a-g]\b|same[01]|base1[0-9]|m9i"
)
# fixture 文件名串：历史计划 token（http2/h2/x448/ed448/sha512/utf8 等域语义豁免）
FIX_BAD = re.compile(
    r"10[a-g]|t10e|[Cc]4[xX]|batch[0-9]|round[0-9]|misc[0-9]"
    r"|g5b2|p10f|ch-10f|tn-10f|r[0-9]\.mjs|m0-|(?<![A-Za-z])t2-"
)
FIX_OK = re.compile(
    r"http2|h2_|x448|ed448|x25519|sha512|utf-?8|u8|win32|aes192|"
    r"chacha20|argv0|uuid7|h3-|v1-|latin1|sha3-|winterjs2-|ws99-|"
    r"secp[0-9]|modp[0-9]|pkcs|mgf1|no[0-9]|b64"
)
# console 断言标签首 token：仅历史计划族；局部序号/域语义不在此列
TAG_BAD = re.compile(
    r"^([pr][0-9]+|r[0-9]+|j[0-9]+|b[0-8]|m0|t[12]|batch[0-9]+|"
    r"tier[0-9]+|m9i|t10e|g5)[- ]"
)
ENV_BAD = re.compile(r"\b(?:WJS|WINTERJS2)_[A-Z0-9_]*[TPRMBG][0-9]\b")
DOC_BAD = re.compile(
    r"phase[0-9][a-z]?_[a-z0-9]|phase_napi|_[rgbm][0-9][a-z]?_|"
    r"round[0-9]|batch[0-9]|c4x|misc[0-9]|t[0-9]_handler|T10E"
)


def tracked(patterns):
    out = subprocess.run(
        ["git", "ls-files", *patterns],
        capture_output=True, text=True, cwd=ROOT,
    ).stdout.split()
    return [p for p in out if not p.startswith("sample/")]


def main():
    hits = []
    files = tracked(["*.rs", "src/**/*.js"])
    for f in files:
        try:
            lines = open(f"{ROOT}/{f}", encoding="utf-8").read().splitlines()
        except (UnicodeDecodeError, FileNotFoundError):
            continue
        for i, ln in enumerate(lines, 1):
            s = ln.strip()
            if s.startswith(("//", "*")):
                continue  # 注释豁免
            m = DECL.match(ln)
            if m and ID_BAD.search(m.group(1)):
                hits.append(f"{f}:{i}: 标识符计划号: {m.group(1)}")
            for lit in re.findall(r'"([^"]+)"', ln):
                if re.search(r"\.(mjs|js|json|txt|pem)(?![A-Za-z0-9])", lit):
                    if FIX_BAD.search(lit) and not FIX_OK.search(lit):
                        hits.append(f"{f}:{i}: fixture 计划号: {lit[:70]}")
                if f.startswith("tests/"):
                    head = lit.split()[0] if lit.split() else ""
                    if TAG_BAD.match(head):
                        hits.append(f"{f}:{i}: 断言标签计划号: {lit[:60]}")
            for key in re.findall(r"\b(?:WJS|WINTERJS2)_[A-Z0-9_]+\b", ln):
                if ENV_BAD.search(key):
                    hits.append(f"{f}:{i}: 环境变量键计划号: {key}")
    for doc in [
        "docs/dependencies.md",
        "docs/dependencies2.md",
        "docs/dependencies3.md",
        "docs/metrics.md",
        "README.md",
        "README.zh.md",
    ]:
        try:
            lines = open(f"{ROOT}/{doc}", encoding="utf-8").read().splitlines()
        except FileNotFoundError:
            continue
        for i, ln in enumerate(lines, 1):
            if DOC_BAD.search(ln):
                hits.append(f"{doc}:{i}: 文档陈旧测试指针")
    if hits:
        print("check-naming: 命中计划号命名（4.271-4.274）：")
        print("\n".join(hits))
        return 1
    print("check-naming: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
