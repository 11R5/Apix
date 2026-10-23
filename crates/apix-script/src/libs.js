// What `require(...)` offers in Apix scripts. Postman ships whole npm
// packages; Apix ships the parts API scripts use, written here, so nothing
// is downloaded and the binary stays small:
//
//   crypto-js  MD5, SHA1, SHA224/256/384/512, their HMACs, enc.Hex/Base64/Utf8/Latin1
//   uuid       v4, validate
//   lodash     the common collection, object and string helpers (also global `_`)
//   moment     parsing, format, add/subtract, diff, comparisons, utc
//   ajv, tv4   JSON Schema validation (the checker behind pm.expect().jsonSchema)
//   chai       expect and a small assert
//
// Hashing runs in Rust (`__apix_hash`, `__apix_hmac`, `__apix_random`).
(function (global) {
  "use strict";

  const factories = {};
  const loaded = {};
  const define = (name, factory) => (factories[name] = factory);

  global.require = function (name) {
    const key = String(name).replace(/^node:/, "");
    if (!own(loaded, key)) {
      if (!own(factories, key)) {
        throw new Error("require('" + name + "'): not built into Apix (available: " + Object.keys(factories).sort().join(", ") + ")");
      }
      loaded[key] = factories[key]();
    }
    return loaded[key];
  };

  function own(obj, key) {
    return Object.prototype.hasOwnProperty.call(obj, key);
  }

  // ------------------------------------------------------------- crypto-js

  define("crypto-js", function () {
    const HEX = "0123456789abcdef";
    const toHex = (bytes) => bytes.map((b) => HEX[b >> 4] + HEX[b & 15]).join("");
    const fromHex = (text) => {
      const out = [];
      for (let i = 0; i + 1 < text.length; i += 2) out.push(parseInt(text.substr(i, 2), 16));
      return out;
    };
    function utf8(text) {
      const out = [];
      for (const ch of String(text)) {
        const c = ch.codePointAt(0);
        if (c < 0x80) out.push(c);
        else if (c < 0x800) out.push(0xc0 | (c >> 6), 0x80 | (c & 63));
        else if (c < 0x10000) out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
        else out.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 63), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
      }
      return out;
    }
    function fromUtf8(bytes) {
      let out = "";
      for (let i = 0; i < bytes.length; ) {
        const b = bytes[i];
        let c;
        let n;
        if (b < 0x80) (c = b), (n = 1);
        else if (b >> 5 === 6) (c = b & 31), (n = 2);
        else if (b >> 4 === 14) (c = b & 15), (n = 3);
        else (c = b & 7), (n = 4);
        for (let k = 1; k < n; k++) c = (c << 6) | (bytes[i + k] & 63);
        out += String.fromCodePoint(c);
        i += n;
      }
      return out;
    }

    function WordArray(bytes) {
      this.bytes = bytes || [];
      this.sigBytes = this.bytes.length;
    }
    WordArray.prototype.toString = function (encoder) {
      return (encoder || Hex).stringify(this);
    };
    WordArray.prototype.concat = function (other) {
      this.bytes = this.bytes.concat(other.bytes);
      this.sigBytes = this.bytes.length;
      return this;
    };
    WordArray.prototype.clone = function () {
      return new WordArray(this.bytes.slice());
    };
    Object.defineProperty(WordArray.prototype, "words", {
      get() {
        const words = [];
        for (let i = 0; i < this.bytes.length; i++) words[i >>> 2] |= this.bytes[i] << (24 - (i % 4) * 8);
        return words;
      },
    });

    const Hex = { stringify: (wa) => toHex(wa.bytes), parse: (text) => new WordArray(fromHex(String(text))) };
    const Latin1 = {
      stringify: (wa) => wa.bytes.map((b) => String.fromCharCode(b)).join(""),
      parse: (text) => new WordArray(Array.from(String(text), (c) => c.charCodeAt(0) & 255)),
    };
    const Utf8 = { stringify: (wa) => fromUtf8(wa.bytes), parse: (text) => new WordArray(utf8(text)) };
    const Base64 = { stringify: (wa) => btoa(Latin1.stringify(wa)), parse: (text) => Latin1.parse(atob(String(text))) };
    const Base64url = {
      stringify: (wa) => Base64.stringify(wa).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, ""),
      parse: (text) => {
        let s = String(text).replace(/-/g, "+").replace(/_/g, "/");
        while (s.length % 4) s += "=";
        return Base64.parse(s);
      },
    };

    const bytesOf = (data) => (data instanceof WordArray ? data.bytes : utf8(data === undefined || data === null ? "" : data));
    const hasher = (alg) => (message) => new WordArray(fromHex(global.__apix_hash(alg, toHex(bytesOf(message)))));
    const hmac = (alg) => (message, key) => new WordArray(fromHex(global.__apix_hmac(alg, toHex(bytesOf(message)), toHex(bytesOf(key)))));
    const unsupported = (name) => () => {
      throw new Error("CryptoJS." + name + " is not built into Apix");
    };

    return {
      MD5: hasher("MD5"),
      SHA1: hasher("SHA1"),
      SHA224: hasher("SHA224"),
      SHA256: hasher("SHA256"),
      SHA384: hasher("SHA384"),
      SHA512: hasher("SHA512"),
      HmacMD5: hmac("MD5"),
      HmacSHA1: hmac("SHA1"),
      HmacSHA224: hmac("SHA224"),
      HmacSHA256: hmac("SHA256"),
      HmacSHA384: hmac("SHA384"),
      HmacSHA512: hmac("SHA512"),
      enc: { Hex: Hex, Latin1: Latin1, Utf8: Utf8, Base64: Base64, Base64url: Base64url },
      lib: {
        WordArray: {
          create(words, sigBytes) {
            if (!words) return new WordArray([]);
            const count = sigBytes === undefined ? words.length * 4 : sigBytes;
            const bytes = [];
            for (let i = 0; i < count; i++) bytes.push((words[i >>> 2] >>> (24 - (i % 4) * 8)) & 255);
            return new WordArray(bytes);
          },
          random: (count) => new WordArray(fromHex(global.__apix_random(count))),
        },
      },
      AES: { encrypt: unsupported("AES"), decrypt: unsupported("AES") },
      PBKDF2: unsupported("PBKDF2"),
    };
  });

  // ------------------------------------------------------------------- uuid

  define("uuid", function () {
    const v4 = () => {
      const hex = global.__apix_random(16).split("");
      hex[12] = "4";
      hex[16] = "89ab"[parseInt(hex[16], 16) & 3];
      const s = hex.join("");
      return s.slice(0, 8) + "-" + s.slice(8, 12) + "-" + s.slice(12, 16) + "-" + s.slice(16, 20) + "-" + s.slice(20, 32);
    };
    const validate = (text) => /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(String(text));
    return { v4: v4, validate: validate };
  });

  // ----------------------------------------------------------------- lodash

  define("lodash", function () {
    const isObject = (v) => v !== null && (typeof v === "object" || typeof v === "function");
    const toPath = (path) => (Array.isArray(path) ? path : String(path).replace(/\[(\w+)\]/g, ".$1").split(".").filter((p) => p.length));
    const iteratee = (fn) => {
      if (typeof fn === "function") return fn;
      if (typeof fn === "string") return (o) => _.get(o, fn);
      if (Array.isArray(fn)) return (o) => _.get(o, fn[0]) === fn[1];
      if (isObject(fn)) return (o) => Object.keys(fn).every((k) => global.__apix_deep_equal(o && o[k], fn[k]));
      return (o) => o;
    };
    const values = (c) => (Array.isArray(c) ? c : isObject(c) ? Object.keys(c).map((k) => c[k]) : []);
    const _ = {
      get(obj, path, fallback) {
        let cur = obj;
        for (const key of toPath(path)) {
          if (cur === null || cur === undefined) return fallback;
          cur = cur[key];
        }
        return cur === undefined ? fallback : cur;
      },
      set(obj, path, value) {
        const keys = toPath(path);
        let cur = obj;
        keys.forEach((key, i) => {
          if (i === keys.length - 1) cur[key] = value;
          else {
            if (!isObject(cur[key])) cur[key] = /^\d+$/.test(keys[i + 1]) ? [] : {};
            cur = cur[key];
          }
        });
        return obj;
      },
      has: (obj, path) => {
        let cur = obj;
        for (const key of toPath(path)) {
          if (!isObject(cur) || !own(cur, key)) return false;
          cur = cur[key];
        }
        return true;
      },
      isEqual: (a, b) => global.__apix_deep_equal(a, b),
      cloneDeep: (v) => (v === undefined ? undefined : JSON.parse(JSON.stringify(v))),
      clone: (v) => (Array.isArray(v) ? v.slice() : isObject(v) ? Object.assign({}, v) : v),
      pick: (obj, ...keys) => {
        const out = {};
        for (const k of [].concat(...keys)) if (obj && own(obj, k)) out[k] = obj[k];
        return out;
      },
      omit: (obj, ...keys) => {
        const drop = [].concat(...keys);
        const out = {};
        for (const k of Object.keys(obj || {})) if (drop.indexOf(k) < 0) out[k] = obj[k];
        return out;
      },
      merge(target, ...sources) {
        for (const source of sources) {
          for (const k of Object.keys(source || {})) {
            if (isObject(source[k]) && !Array.isArray(source[k]) && isObject(target[k])) _.merge(target[k], source[k]);
            else target[k] = source[k];
          }
        }
        return target;
      },
      keys: (o) => Object.keys(o || {}),
      values: values,
      entries: (o) => Object.keys(o || {}).map((k) => [k, o[k]]),
      map: (c, fn) => values(c).map(iteratee(fn)),
      filter: (c, fn) => values(c).filter(iteratee(fn)),
      reject: (c, fn) => values(c).filter((x) => !iteratee(fn)(x)),
      find: (c, fn) => values(c).find(iteratee(fn)),
      findIndex: (a, fn) => (a || []).findIndex(iteratee(fn)),
      some: (c, fn) => values(c).some(iteratee(fn)),
      every: (c, fn) => values(c).every(iteratee(fn)),
      each(c, fn) {
        if (Array.isArray(c)) c.forEach(fn);
        else for (const k of Object.keys(c || {})) fn(c[k], k, c);
        return c;
      },
      reduce: (c, fn, init) => values(c).reduce(fn, init),
      includes: (c, v) => (typeof c === "string" ? c.indexOf(v) >= 0 : values(c).indexOf(v) >= 0),
      uniq: (a) => (a || []).filter((x, i, all) => all.indexOf(x) === i),
      uniqBy: (a, fn) => {
        const f = iteratee(fn);
        const seen = [];
        return (a || []).filter((x) => (seen.indexOf(f(x)) >= 0 ? false : (seen.push(f(x)), true)));
      },
      flatten: (a) => [].concat(...(a || [])),
      flattenDeep: (a) => (a || []).reduce((acc, x) => acc.concat(Array.isArray(x) ? _.flattenDeep(x) : x), []),
      compact: (a) => (a || []).filter(Boolean),
      chunk: (a, size) => {
        const out = [];
        for (let i = 0; i < (a || []).length; i += size || 1) out.push(a.slice(i, i + (size || 1)));
        return out;
      },
      difference: (a, ...others) => (a || []).filter((x) => ![].concat(...others).includes(x)),
      intersection: (a, ...others) => (a || []).filter((x) => others.every((o) => o.includes(x))),
      union: (...arrays) => _.uniq([].concat(...arrays)),
      first: (a) => (a || [])[0],
      head: (a) => (a || [])[0],
      last: (a) => (a || [])[(a || []).length - 1],
      take: (a, n) => (a || []).slice(0, n === undefined ? 1 : n),
      sortBy: (c, ...fns) => {
        const keys = [].concat(...fns).map(iteratee);
        return values(c)
          .slice()
          .sort((x, y) => {
            for (const key of keys) {
              const a = key(x);
              const b = key(y);
              if (a < b) return -1;
              if (a > b) return 1;
            }
            return 0;
          });
      },
      orderBy: (c, fns, orders) => {
        const keys = [].concat(fns).map(iteratee);
        const dirs = [].concat(orders || []);
        return values(c)
          .slice()
          .sort((x, y) => {
            for (let i = 0; i < keys.length; i++) {
              const a = keys[i](x);
              const b = keys[i](y);
              const sign = dirs[i] === "desc" ? -1 : 1;
              if (a < b) return -sign;
              if (a > b) return sign;
            }
            return 0;
          });
      },
      groupBy: (c, fn) => {
        const f = iteratee(fn);
        const out = {};
        for (const x of values(c)) (out[f(x)] = out[f(x)] || []).push(x);
        return out;
      },
      keyBy: (c, fn) => {
        const f = iteratee(fn);
        const out = {};
        for (const x of values(c)) out[f(x)] = x;
        return out;
      },
      countBy: (c, fn) => {
        const f = iteratee(fn);
        const out = {};
        for (const x of values(c)) out[f(x)] = (out[f(x)] || 0) + 1;
        return out;
      },
      sum: (a) => (a || []).reduce((s, x) => s + x, 0),
      sumBy: (a, fn) => (a || []).reduce((s, x) => s + iteratee(fn)(x), 0),
      max: (a) => ((a || []).length ? Math.max(...a) : undefined),
      min: (a) => ((a || []).length ? Math.min(...a) : undefined),
      maxBy: (a, fn) => _.orderBy(a, [fn], ["desc"])[0],
      minBy: (a, fn) => _.sortBy(a, fn)[0],
      mean: (a) => _.sum(a) / (a || []).length,
      size: (c) => (typeof c === "string" || Array.isArray(c) ? c.length : Object.keys(c || {}).length),
      range(start, end, step) {
        if (end === undefined) (end = start), (start = 0);
        step = step || (end < start ? -1 : 1);
        const out = [];
        for (let i = start; step > 0 ? i < end : i > end; i += step) out.push(i);
        return out;
      },
      times: (n, fn) => Array.from({ length: n }, (x, i) => (fn ? fn(i) : i)),
      random(low, high, floating) {
        if (high === undefined) (high = low === undefined ? 1 : low), (low = 0);
        const r = low + Math.random() * (high - low);
        return floating || !Number.isInteger(low) || !Number.isInteger(high) ? r : Math.floor(low + Math.random() * (high - low + 1));
      },
      sample: (c) => {
        const all = values(c);
        return all[Math.floor(Math.random() * all.length)];
      },
      shuffle: (c) => {
        const all = values(c).slice();
        for (let i = all.length - 1; i > 0; i--) {
          const j = Math.floor(Math.random() * (i + 1));
          [all[i], all[j]] = [all[j], all[i]];
        }
        return all;
      },
      isEmpty: (v) => v === null || v === undefined || (typeof v === "object" ? Object.keys(v).length === 0 : typeof v === "string" || Array.isArray(v) ? v.length === 0 : true),
      isArray: Array.isArray,
      isString: (v) => typeof v === "string",
      isNumber: (v) => typeof v === "number",
      isInteger: Number.isInteger,
      isBoolean: (v) => typeof v === "boolean",
      isFunction: (v) => typeof v === "function",
      isObject: isObject,
      isPlainObject: (v) => isObject(v) && !Array.isArray(v) && Object.getPrototypeOf(v) === Object.prototype,
      isNil: (v) => v === null || v === undefined,
      isNull: (v) => v === null,
      isUndefined: (v) => v === undefined,
      camelCase: (s) => _.words(s).map((w, i) => (i ? w[0].toUpperCase() + w.slice(1).toLowerCase() : w.toLowerCase())).join(""),
      kebabCase: (s) => _.words(s).map((w) => w.toLowerCase()).join("-"),
      snakeCase: (s) => _.words(s).map((w) => w.toLowerCase()).join("_"),
      capitalize: (s) => (s ? String(s)[0].toUpperCase() + String(s).slice(1).toLowerCase() : ""),
      upperFirst: (s) => (s ? String(s)[0].toUpperCase() + String(s).slice(1) : ""),
      words: (s) => String(s || "").match(/[A-Z]{2,}(?=[A-Z][a-z]|\b|\d)|[A-Z]?[a-z]+|[A-Z]+|\d+/g) || [],
      trim: (s) => String(s || "").trim(),
      padStart: (s, n, c) => String(s).padStart(n, c === undefined ? " " : c),
      padEnd: (s, n, c) => String(s).padEnd(n, c === undefined ? " " : c),
      repeat: (s, n) => String(s).repeat(n),
      startsWith: (s, t) => String(s).startsWith(t),
      endsWith: (s, t) => String(s).endsWith(t),
      toUpper: (s) => String(s).toUpperCase(),
      toLower: (s) => String(s).toLowerCase(),
      identity: (v) => v,
      noop: () => undefined,
      now: () => Date.now(),
    };
    _.forEach = _.each;
    return _;
  });

  // ----------------------------------------------------------------- moment

  define("moment", function () {
    const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const DAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    const UNITS = { y: "year", year: "year", years: "year", M: "month", month: "month", months: "month", w: "week", week: "week", weeks: "week", d: "day", day: "day", days: "day", h: "hour", hour: "hour", hours: "hour", m: "minute", minute: "minute", minutes: "minute", s: "second", second: "second", seconds: "second", ms: "millisecond", millisecond: "millisecond", milliseconds: "millisecond" };
    const MS = { week: 604800000, day: 86400000, hour: 3600000, minute: 60000, second: 1000, millisecond: 1 };
    const pad = (n, w) => String(Math.abs(n)).padStart(w || 2, "0");

    function Moment(time, utc) {
      this._d = new Date(time);
      this._utc = !!utc;
    }
    const P = Moment.prototype;
    const get = (m, part) => m._d[(m._utc ? "getUTC" : "get") + part]();
    const set = (m, part, v) => m._d[(m._utc ? "setUTC" : "set") + part](v);

    P.clone = function () {
      return new Moment(this._d.getTime(), this._utc);
    };
    P.utc = function () {
      return new Moment(this._d.getTime(), true);
    };
    P.local = function () {
      return new Moment(this._d.getTime(), false);
    };
    P.isValid = function () {
      return !isNaN(this._d.getTime());
    };
    P.valueOf = function () {
      return this._d.getTime();
    };
    P.unix = function () {
      return Math.floor(this._d.getTime() / 1000);
    };
    P.toDate = function () {
      return new Date(this._d.getTime());
    };
    P.toISOString = function () {
      return this._d.toISOString();
    };
    P.toJSON = P.toISOString;
    P.toString = function () {
      return this._d.toString();
    };
    P.utcOffset = function () {
      return this._utc ? 0 : -this._d.getTimezoneOffset();
    };
    for (const [name, part] of [
      ["year", "FullYear"],
      ["month", "Month"],
      ["date", "Date"],
      ["hours", "Hours"],
      ["minutes", "Minutes"],
      ["seconds", "Seconds"],
      ["milliseconds", "Milliseconds"],
    ]) {
      P[name] = function (value) {
        if (value === undefined) return get(this, part);
        set(this, part, value);
        return this;
      };
    }
    P.day = function () {
      return get(this, "Day");
    };
    P.hour = P.hours;
    P.minute = P.minutes;
    P.second = P.seconds;

    P.add = function (amount, unit) {
      if (typeof amount === "object") {
        for (const k of Object.keys(amount)) this.add(amount[k], k);
        return this;
      }
      const u = UNITS[unit || "ms"] || "millisecond";
      if (u === "year") set(this, "FullYear", get(this, "FullYear") + amount);
      else if (u === "month") set(this, "Month", get(this, "Month") + amount);
      else this._d = new Date(this._d.getTime() + amount * MS[u]);
      return this;
    };
    P.subtract = function (amount, unit) {
      if (typeof amount === "object") {
        for (const k of Object.keys(amount)) this.add(-amount[k], k);
        return this;
      }
      return this.add(-amount, unit);
    };
    P.startOf = function (unit) {
      const u = UNITS[unit] || unit;
      const order = ["year", "month", "day", "hour", "minute", "second"];
      const at = order.indexOf(u === "week" ? "day" : u);
      if (at < 0) return this;
      if (at < 1) set(this, "Month", 0);
      if (at < 2) set(this, "Date", 1);
      if (at < 3) set(this, "Hours", 0);
      if (at < 4) set(this, "Minutes", 0);
      if (at < 5) set(this, "Seconds", 0);
      set(this, "Milliseconds", 0);
      if (u === "week") set(this, "Date", get(this, "Date") - get(this, "Day"));
      return this;
    };
    P.endOf = function (unit) {
      return this.startOf(unit)
        .add(1, unit)
        .subtract(1, "ms");
    };
    P.diff = function (other, unit, floating) {
      const that = moment(other);
      const u = UNITS[unit || "ms"] || "millisecond";
      let value;
      if (u === "year" || u === "month") {
        const months = (get(this, "FullYear") - get(that, "FullYear")) * 12 + (get(this, "Month") - get(that, "Month"));
        value = u === "year" ? months / 12 : months;
      } else value = (this.valueOf() - that.valueOf()) / MS[u];
      return floating ? value : value < 0 ? Math.ceil(value) : Math.floor(value);
    };
    P.isBefore = function (other) {
      return this.valueOf() < moment(other).valueOf();
    };
    P.isAfter = function (other) {
      return this.valueOf() > moment(other).valueOf();
    };
    P.isSame = function (other, unit) {
      if (!unit) return this.valueOf() === moment(other).valueOf();
      return this.clone().startOf(unit).valueOf() === moment(other).clone().startOf(unit).valueOf();
    };
    P.isBetween = function (a, b) {
      return this.isAfter(a) && this.isBefore(b);
    };
    P.format = function (pattern) {
      if (pattern === undefined) pattern = "YYYY-MM-DDTHH:mm:ssZ";
      const offset = this.utcOffset();
      const zone = (sep) => (offset < 0 ? "-" : "+") + pad(Math.floor(Math.abs(offset) / 60)) + sep + pad(Math.abs(offset) % 60);
      const h = get(this, "Hours");
      const tokens = {
        YYYY: () => String(get(this, "FullYear")),
        YY: () => pad(get(this, "FullYear") % 100),
        MMMM: () => MONTHS[get(this, "Month")],
        MMM: () => MONTHS[get(this, "Month")].slice(0, 3),
        MM: () => pad(get(this, "Month") + 1),
        M: () => String(get(this, "Month") + 1),
        DD: () => pad(get(this, "Date")),
        D: () => String(get(this, "Date")),
        dddd: () => DAYS[get(this, "Day")],
        ddd: () => DAYS[get(this, "Day")].slice(0, 3),
        HH: () => pad(h),
        H: () => String(h),
        hh: () => pad(h % 12 || 12),
        h: () => String(h % 12 || 12),
        mm: () => pad(get(this, "Minutes")),
        m: () => String(get(this, "Minutes")),
        ss: () => pad(get(this, "Seconds")),
        s: () => String(get(this, "Seconds")),
        SSS: () => pad(get(this, "Milliseconds"), 3),
        A: () => (h < 12 ? "AM" : "PM"),
        a: () => (h < 12 ? "am" : "pm"),
        ZZ: () => zone(""),
        Z: () => zone(":"),
        X: () => String(this.unix()),
        x: () => String(this.valueOf()),
      };
      return pattern.replace(/\[([^\]]*)\]|YYYY|YY|MMMM|MMM|MM|M|DD|D|dddd|ddd|HH|H|hh|h|mm|m|ss|s|SSS|A|a|ZZ|Z|X|x/g, (token, literal) => (literal !== undefined ? literal : tokens[token]()));
    };

    function parse(input) {
      if (input === undefined) return Date.now();
      if (input instanceof Moment) return input.valueOf();
      if (input instanceof Date || typeof input === "number") return new Date(input).getTime();
      const text = String(input);
      // A date without a time is local midnight, as in moment.
      const plain = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text);
      if (plain) return new Date(+plain[1], +plain[2] - 1, +plain[3]).getTime();
      return new Date(text).getTime();
    }
    function moment(input) {
      return new Moment(parse(input), false);
    }
    moment.utc = (input) => new Moment(parse(input), true);
    moment.unix = (seconds) => new Moment(seconds * 1000, false);
    moment.isMoment = (v) => v instanceof Moment;
    moment.duration = (amount, unit) => {
      const ms = amount * (MS[UNITS[unit || "ms"] || "millisecond"] || 1);
      return {
        asMilliseconds: () => ms,
        asSeconds: () => ms / 1000,
        asMinutes: () => ms / 60000,
        asHours: () => ms / 3600000,
        asDays: () => ms / 86400000,
      };
    };
    return moment;
  });

  // ------------------------------------------------------ ajv, tv4, chai

  define("ajv", function () {
    function Ajv() {
      this.errors = null;
    }
    const toErrors = (problems) => (problems.length ? problems.map((message) => ({ instancePath: "", message: message })) : null);
    Ajv.prototype.compile = function (schema) {
      const check = (data) => {
        check.errors = toErrors(global.__apix_validate(data, schema));
        return check.errors === null;
      };
      return check;
    };
    Ajv.prototype.validate = function (schema, data) {
      this.errors = toErrors(global.__apix_validate(data, schema));
      return this.errors === null;
    };
    Ajv.prototype.errorsText = function (errors) {
      return (errors || this.errors || []).map((e) => e.message).join(", ") || "No errors";
    };
    Ajv.prototype.addSchema = function () {
      return this;
    };
    Ajv.prototype.addFormat = function () {
      return this;
    };
    Ajv.default = Ajv;
    return Ajv;
  });

  define("tv4", function () {
    const tv4 = {
      error: null,
      validate(data, schema) {
        const problems = global.__apix_validate(data, schema);
        tv4.error = problems.length ? { message: problems[0], dataPath: "" } : null;
        return problems.length === 0;
      },
      validateResult(data, schema) {
        const valid = tv4.validate(data, schema);
        return { valid: valid, error: tv4.error, missing: [] };
      },
      validateMultiple(data, schema) {
        const problems = global.__apix_validate(data, schema);
        return { valid: problems.length === 0, errors: problems.map((m) => ({ message: m })), missing: [] };
      },
    };
    return tv4;
  });

  define("chai", function () {
    const expect = global.pm.expect;
    const fail = (message) => {
      const error = new Error(message);
      error.name = "AssertionError";
      throw error;
    };
    const assert = (value, message) => value || fail(message || "expected " + value + " to be truthy");
    assert.ok = assert.isOk = assert;
    assert.equal = (a, b, m) => a == b || fail(m || "expected " + JSON.stringify(a) + " to equal " + JSON.stringify(b));
    assert.notEqual = (a, b, m) => a != b || fail(m || "expected " + JSON.stringify(a) + " to not equal " + JSON.stringify(b));
    assert.strictEqual = (a, b, m) => a === b || fail(m || "expected " + JSON.stringify(a) + " to equal " + JSON.stringify(b));
    assert.deepEqual = (a, b, m) => global.__apix_deep_equal(a, b) || fail(m || "expected " + JSON.stringify(a) + " to deeply equal " + JSON.stringify(b));
    assert.isTrue = (v, m) => v === true || fail(m || "expected " + v + " to be true");
    assert.isFalse = (v, m) => v === false || fail(m || "expected " + v + " to be false");
    assert.exists = (v, m) => (v !== null && v !== undefined) || fail(m || "expected value to exist");
    assert.include = (h, n, m) => expect(h, m).to.include(n);
    assert.lengthOf = (v, n, m) => expect(v, m).to.have.lengthOf(n);
    assert.property = (o, p, m) => expect(o, m).to.have.property(p);
    assert.isArray = (v, m) => Array.isArray(v) || fail(m || "expected " + JSON.stringify(v) + " to be an array");
    assert.fail = (m) => fail(m || "assert.fail()");
    return { expect: expect, assert: assert };
  });

  // Postman's older sandbox had these as globals.
  for (const [name, module] of [
    ["CryptoJS", "crypto-js"],
    ["_", "lodash"],
    ["tv4", "tv4"],
  ]) {
    Object.defineProperty(global, name, { get: () => global.require(module), configurable: true });
  }
})(globalThis);
