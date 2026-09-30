import "@testing-library/jest-dom/vitest"
import { cleanup } from "@testing-library/react"
import { afterEach } from "vitest"

class TestResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

Object.defineProperty(globalThis, "ResizeObserver", { value: TestResizeObserver, writable: true })
Object.defineProperty(Element.prototype, "scrollIntoView", { value() {}, writable: true })

// jsdom 不实现 `matchMedia`，而 ThemeProvider 会在挂载时读取系统主题。
if (!window.matchMedia) {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener() {},
      removeEventListener() {},
      addListener() {},
      removeListener() {},
      dispatchEvent: () => false,
    }),
  })
}

// Node 22.4+ 暴露了实验性的全局 localStorage：未加 --localstorage-file 时它的 getter
// 返回 undefined，会遮蔽 jsdom 自带的 Storage 实现（`localStorage.clear()` 报
// "Cannot read properties of undefined"）。这里补一个内存实现，行为与浏览器一致，
// 避免测试结果依赖 Node 版本。
function memoryStorage(): Storage {
  const entries = new Map<string, string>()
  return {
    get length() { return entries.size },
    key: (index) => Array.from(entries.keys())[index] ?? null,
    getItem: (key) => entries.get(String(key)) ?? null,
    setItem: (key, value) => { entries.set(String(key), String(value)) },
    removeItem: (key) => { entries.delete(String(key)) },
    clear: () => { entries.clear() },
  } as Storage
}

for (const key of ["localStorage", "sessionStorage"] as const) {
  if (!globalThis[key]) {
    Object.defineProperty(globalThis, key, { value: memoryStorage(), writable: true, configurable: true })
  }
}

afterEach(cleanup)
