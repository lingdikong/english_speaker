// 前端与宿主之间的小适配层：同一份前端既能跑在 CEF 里，也能跑在开发宿主（HTTP + SSE）里。
//
//   invoke(cmd, args) -> Promise<data>   请求/响应，对应 es-desktop `Bridge::handle_json`
//   onEvent(cb)                          核心事件推送（CoreEvent JSON）
//
// CEF：页面里有 `window.cefQuery`（CEF message router），事件由宿主执行
//      `window.__es_emit(json)` 推回。
// 开发宿主：`POST /__es/invoke`，事件走 `GET /__es/events`（Server-Sent Events）。

import type { CoreEvent } from './types';

type Listener = (e: CoreEvent) => void;

interface CefQueryArgs {
  request: string;
  persistent?: boolean;
  onSuccess: (response: string) => void;
  onFailure: (code: number, message: string) => void;
}

declare global {
  interface Window {
    cefQuery?: (args: CefQueryArgs) => number;
    __es_emit?: (event: CoreEvent | string) => void;
  }
}

const listeners = new Set<Listener>();

function dispatch(raw: CoreEvent | string) {
  const e: CoreEvent = typeof raw === 'string' ? JSON.parse(raw) : raw;
  for (const l of listeners) {
    try {
      l(e);
    } catch (err) {
      console.error(err);
    }
  }
}

// CEF 宿主通过它把事件推进来；开发宿主也复用这一入口
window.__es_emit = dispatch;

export const hostKind: 'cef' | 'http' = window.cefQuery ? 'cef' : 'http';

function send(request: string): Promise<string> {
  if (window.cefQuery) {
    return new Promise((resolve, reject) => {
      window.cefQuery!({
        request,
        onSuccess: resolve,
        onFailure: (code, msg) => reject(new Error(`cefQuery ${code}: ${msg}`)),
      });
    });
  }
  return fetch('/__es/invoke', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: request,
  }).then((r) => r.text());
}

export async function invoke<T = unknown>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const text = await send(JSON.stringify({ cmd, ...args }));
  const resp = JSON.parse(text) as { ok: boolean; data?: T; error?: string };
  if (!resp.ok) throw new Error(resp.error ?? 'unknown error');
  return resp.data as T;
}

let sse: EventSource | null = null;

export function onEvent(cb: Listener): () => void {
  listeners.add(cb);
  if (hostKind === 'http' && !sse) {
    sse = new EventSource('/__es/events');
    sse.onmessage = (m) => dispatch(m.data);
  }
  return () => listeners.delete(cb);
}
