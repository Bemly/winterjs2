//! `node:internal/event_target`（Node `lib/internal/event_target.js` 的 events 所需面，MIT）。
//!
//! 本仓无全局 EventTarget 基类（AbortSignal 为极简自有实现），`isEventTarget`
//! 以鸭子类型近似（有 addEventListener 且无 `_events` → EventTarget，如
//! AbortSignal）；`kEvents`/`kResistStopPropagation` 仅出符号；
//! `getEventListeners`/`listenerCount` 经 `__wjs2_etListeners` 侧读桥见直接注册
//! （prelude 全局 EventTarget 存储）；Event/CustomEvent/EventTarget 透传全局
//! 同一类（global-customevent 套件）。品牌检查/NodeEventTarget/弱监听另案。

/// 内嵌 ESM 源。
pub const SOURCE: &str = r#"
// Copyright Joyent, Inc. and other Node contributors. MIT.
// Subset of node:internal/event_target needed by node:events (see module docs).
const kEvents = Symbol('kEvents');
const kResistStopPropagation = Symbol.for('resistStopPropagation');
const kWatermarkData = Symbol.for('nodejs.watermarkData');

function isEventTarget(obj) {
  if (obj === null || (typeof obj !== 'object' && typeof obj !== 'function')) return false;
  return typeof obj.addEventListener === 'function' &&
    typeof obj.removeEventListener === 'function' &&
    obj._events === undefined;
}

function isNodeEventTarget(obj) {
  return false; // 本仓无 NodeEventTarget 子类（net/stream 未接）
}

// onfoo 属性处理器（eventtarget 套件；node defineEventHandler 口径：
// 同事件单包装器，set 替换 handler（null 即停用），get 取当前函数或 null；
// 与 addEventListener 并存同发）。
const __onfooWrappers = new WeakMap();
function makeEventHandler(handler) {
  function eventHandler(...args) {
    if (typeof eventHandler.handler !== "function") return;
    return eventHandler.handler.apply(this, args);
  }
  eventHandler.handler = handler;
  return eventHandler;
}
function defineEventHandler(emitter, name, event = name) {
  const prop = `on${name}`;
  function get() {
    const m = __onfooWrappers.get(this);
    const w = m?.get(event);
    return (w && typeof w.handler === "function") ? w.handler : null;
  }
  function set(value) {
    if (typeof value !== "function" && (typeof value !== "object" || value === null)) value = null;
    let m = __onfooWrappers.get(this);
    if (!m) { m = new Map(); __onfooWrappers.set(this, m); }
    let w = m.get(event);
    if (w) {
      w.handler = value;
    } else {
      w = makeEventHandler(value);
      m.set(event, w);
      this.addEventListener(event, w);
    }
  }
  Object.defineProperty(emitter, prop, { get, set, configurable: true, enumerable: true });
}

// 全局同一性（global-customevent 套件：global === internal；类定义见 prelude
// 全局，本模块只做透传——品牌检查等深面另案）。
const Event = globalThis.Event;
const CustomEvent = globalThis.CustomEvent;
const EventTarget = globalThis.EventTarget;

export { kEvents, kResistStopPropagation, kWatermarkData, isEventTarget, isNodeEventTarget, Event, CustomEvent, EventTarget, makeEventHandler, defineEventHandler };
export default { kEvents, kResistStopPropagation, kWatermarkData, isEventTarget, isNodeEventTarget, Event, CustomEvent, EventTarget, makeEventHandler, defineEventHandler };
"#;
