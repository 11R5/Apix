// The pm.* API of Apix scripts, compatible with Postman's. Rust puts the
// context in `__apix_input` (JSON) before this runs and reads
// `__apix_output()` (JSON) after the scripts. Everything a script changes is
// in plain objects here, so the output is just those objects.
(function (global) {
  "use strict";

  const input = JSON.parse(global.__apix_input);
  delete global.__apix_input;

  const state = input.state;
  const scopes = input.scopes;
  const req = input.request;
  const res = input.response;
  const tests = [];
  const logs = [];
  let nextRequest;
  let skipRequest = false;

  const own = (obj, key) => Object.prototype.hasOwnProperty.call(obj, key);

  // ---------------------------------------------------------------- values

  function inspect(value, depth) {
    depth = depth || 0;
    if (value === undefined) return "undefined";
    if (value === null) return "null";
    if (typeof value === "string") return depth ? JSON.stringify(value) : value;
    if (typeof value === "function") return "[Function" + (value.name ? ": " + value.name : "") + "]";
    if (value instanceof Error) return value.name + ": " + value.message;
    if (value instanceof RegExp) return String(value);
    if (typeof value === "object") {
      try {
        return JSON.stringify(value);
      } catch (e) {
        return String(value);
      }
    }
    return String(value);
  }

  // Variables are text in Apix files, so anything set becomes text.
  function text(value) {
    if (value === undefined || value === null) return "";
    if (typeof value === "object") return JSON.stringify(value);
    return String(value);
  }

  // -------------------------------------------------------------- variables

  function scope(name, base) {
    const changes = state[name];
    function get(key) {
      key = String(key);
      if (own(changes, key)) return changes[key] === null ? undefined : changes[key];
      return own(base, key) ? base[key] : undefined;
    }
    function toObject() {
      const out = {};
      for (const key of Object.keys(base)) out[key] = base[key];
      for (const key of Object.keys(changes)) {
        if (changes[key] === null) delete out[key];
        else out[key] = changes[key];
      }
      return out;
    }
    return {
      get: get,
      has: (key) => get(key) !== undefined,
      set(key, value) {
        changes[String(key)] = text(value);
      },
      unset(key) {
        changes[String(key)] = null;
      },
      clear() {
        for (const key of Object.keys(toObject())) changes[key] = null;
      },
      toObject: toObject,
      toJSON: toObject,
      replaceIn: (value) => replaceIn(value),
    };
  }

  const globals = scope("globals", scopes.globals);
  const collectionVariables = scope("collection", scopes.collection);
  const environment = scope("environment", scopes.environment);
  environment.name = scopes.environment_name || undefined;
  const data = scopes.data || {};

  // pm.variables: the narrowest value of a name, as a request would see it.
  function anyGet(key) {
    key = String(key);
    if (own(state.local, key)) return state.local[key] === null ? undefined : state.local[key];
    if (own(data, key)) return data[key];
    for (const s of [environment, collectionVariables, globals]) {
      const value = s.get(key);
      if (value !== undefined) return value;
    }
    return undefined;
  }

  function replaceIn(value) {
    if (typeof value !== "string") return value;
    return value.replace(/\{\{([^{}]+)\}\}/g, (whole, name) => {
      const found = anyGet(name.trim());
      return found === undefined ? whole : found;
    });
  }

  const variables = {
    get: anyGet,
    has: (key) => anyGet(key) !== undefined,
    set(key, value) {
      state.local[String(key)] = text(value);
    },
    unset(key) {
      state.local[String(key)] = null;
    },
    toObject() {
      const out = Object.assign(
        {},
        globals.toObject(),
        collectionVariables.toObject(),
        environment.toObject(),
        data,
      );
      for (const key of Object.keys(state.local)) {
        if (state.local[key] === null) delete out[key];
        else out[key] = state.local[key];
      }
      return out;
    },
    replaceIn: replaceIn,
  };

  // -------------------------------------------------- headers, query, form

  // A list of { key, value, disabled } rows, like Postman's PropertyList.
  function propertyList(rows, ignoreCase) {
    const same = (a, b) => (ignoreCase ? String(a).toLowerCase() === String(b).toLowerCase() : String(a) === String(b));
    const active = () => rows.filter((row) => !row.disabled);
    const list = {
      get(key) {
        const row = active().find((r) => same(r.key, key));
        return row ? row.value : undefined;
      },
      one(key) {
        return active().find((r) => same(r.key, key));
      },
      has(key, value) {
        return active().some((r) => same(r.key, key) && (value === undefined || r.value === text(value)));
      },
      add(item, value) {
        if (typeof item === "string") {
          const colon = item.indexOf(":");
          item = value !== undefined ? { key: item, value: value } : { key: item.slice(0, colon).trim(), value: item.slice(colon + 1).trim() };
        }
        rows.push({ key: String(item.key), value: text(item.value), disabled: !!item.disabled });
      },
      upsert(item) {
        const row = rows.find((r) => same(r.key, item.key));
        if (row) {
          row.value = text(item.value);
          row.disabled = !!item.disabled;
        } else list.add(item);
      },
      remove(key) {
        const test = typeof key === "function" ? key : (r) => same(r.key, key);
        for (let i = rows.length - 1; i >= 0; i--) if (test(rows[i])) rows.splice(i, 1);
      },
      clear() {
        rows.length = 0;
      },
      count: () => active().length,
      all: () => active().map((r) => ({ key: r.key, value: r.value })),
      each(fn) {
        active().forEach(fn);
      },
      map: (fn) => active().map(fn),
      filter: (fn) => active().filter(fn),
      find: (fn) => active().find(fn),
      toObject() {
        const out = {};
        for (const row of active()) out[row.key] = row.value;
        return out;
      },
      toJSON: () => active().map((r) => ({ key: r.key, value: r.value })),
      toString: () => active().map((r) => r.key + ": " + r.value).join("\n"),
    };
    return list;
  }

  // ---------------------------------------------------------------- request

  function splitUrl(full) {
    const at = full.indexOf("?");
    if (at < 0) return { base: full, query: [] };
    const query = full
      .slice(at + 1)
      .split("&")
      .filter((p) => p.length)
      .map((pair) => {
        const eq = pair.indexOf("=");
        const decode = (s) => {
          try {
            return decodeURIComponent(s.replace(/\+/g, " "));
          } catch (e) {
            return s;
          }
        };
        return eq < 0 ? { key: decode(pair), value: "", disabled: false } : { key: decode(pair.slice(0, eq)), value: decode(pair.slice(eq + 1)), disabled: false };
      });
    return { base: full.slice(0, at), query: query };
  }

  function queryString(rows) {
    return rows
      .filter((r) => !r.disabled)
      .map((r) => encodeURIComponent(r.key) + (r.value === "" ? "" : "=" + encodeURIComponent(r.value)))
      .join("&");
  }

  function urlObject(holder) {
    const parts = () => {
      const m = /^([a-z][a-z0-9+.-]*):\/\/([^/?#]*)([^?#]*)/i.exec(holder.url) || [];
      return { protocol: m[1] || "", host: m[2] || "", path: m[3] || "" };
    };
    const query = propertyList(holder.query, false);
    return {
      query: query,
      toString() {
        const q = queryString(holder.query);
        return holder.url + (q ? (holder.url.indexOf("?") < 0 ? "?" : "&") + q : "");
      },
      getHost: () => parts().host.replace(/:\d+$/, ""),
      getPath: () => parts().path || "/",
      getQueryString: () => queryString(holder.query),
      getPathWithQuery() {
        const q = queryString(holder.query);
        return (parts().path || "/") + (q ? "?" + q : "");
      },
      get protocol() {
        return parts().protocol;
      },
      get host() {
        return parts().host.replace(/:\d+$/, "").split(".");
      },
      get port() {
        const m = /:(\d+)$/.exec(parts().host);
        return m ? m[1] : undefined;
      },
      get path() {
        return (parts().path || "/").split("/").filter((s) => s.length);
      },
      addQueryParams(params) {
        for (const p of [].concat(params)) query.add(p);
      },
      removeQueryParams(names) {
        for (const n of [].concat(names)) query.remove(typeof n === "string" ? n : n.key);
      },
      update(full) {
        const split = splitUrl(String(full));
        holder.url = split.base;
        holder.query.length = 0;
        split.query.forEach((r) => holder.query.push(r));
      },
    };
  }

  function requestObject(holder) {
    const headers = propertyList(holder.headers, true);
    let url = urlObject(holder);
    const body = holder.body;
    const bodyObject = {
      get mode() {
        return body.mode;
      },
      get raw() {
        return body.raw;
      },
      set raw(value) {
        body.raw = text(value);
        if (body.mode === "none") body.mode = "raw";
      },
      urlencoded: propertyList(body.urlencoded, false),
      update(value) {
        if (typeof value === "string") bodyObject.raw = value;
        else if (value && value.mode === "raw") bodyObject.raw = value.raw;
      },
      toString: () => (body.mode === "urlencoded" ? queryString(body.urlencoded) : body.raw || ""),
    };
    return {
      get method() {
        return holder.method;
      },
      set method(value) {
        holder.method = String(value).toUpperCase();
      },
      get url() {
        return url;
      },
      set url(value) {
        url.update(typeof value === "string" ? value : String(value));
      },
      headers: headers,
      body: bodyObject,
      addHeader: (item) => headers.add(item),
      removeHeader: (name) => headers.remove(name),
      upsertHeader: (item) => headers.upsert(item),
      addQueryParams: (params) => url.addQueryParams(params),
      removeQueryParams: (names) => url.removeQueryParams(names),
      toJSON: () => ({ method: holder.method, url: url.toString(), headers: headers.toObject(), body: bodyObject.toString() }),
    };
  }

  // -------------------------------------------------------------- assertion

  function AssertionError(message) {
    const error = new Error(message);
    error.name = "AssertionError";
    return error;
  }

  function deepEqual(a, b) {
    if (a === b) return true;
    if (typeof a === "number" && typeof b === "number") return a !== a && b !== b;
    if (a === null || b === null || typeof a !== "object" || typeof b !== "object") return false;
    if (Array.isArray(a) !== Array.isArray(b)) return false;
    const ka = Object.keys(a);
    const kb = Object.keys(b);
    if (ka.length !== kb.length) return false;
    return ka.every((k) => own(b, k) && deepEqual(a[k], b[k]));
  }

  function typeName(value) {
    if (value === null) return "null";
    if (Array.isArray(value)) return "array";
    if (value instanceof RegExp) return "regexp";
    if (value instanceof Date) return "date";
    return typeof value;
  }

  function sizeOf(value) {
    if (value === null || value === undefined) return undefined;
    if (typeof value === "string" || Array.isArray(value)) return value.length;
    if (value instanceof Map || value instanceof Set) return value.size;
    if (typeof value === "object") return Object.keys(value).length;
    return value.length;
  }

  function includes(haystack, needle, deep) {
    if (typeof haystack === "string") return haystack.indexOf(needle) >= 0;
    if (Array.isArray(haystack)) return haystack.some((x) => (deep ? deepEqual(x, needle) : x === needle));
    if (haystack && typeof haystack === "object") {
      if (needle && typeof needle === "object") {
        return Object.keys(needle).every((k) => own(haystack, k) && (deep ? deepEqual(haystack[k], needle[k]) : haystack[k] === needle[k]));
      }
      return own(haystack, needle);
    }
    return false;
  }

  function Assertion(subject, message) {
    this._subject = subject;
    this._message = message;
    this._negate = false;
    this._deep = false;
    this._any = false;
    this._own = false;
  }

  function check(a, pass, wanted, unwanted) {
    if (pass === a._negate) {
      const text = a._negate ? unwanted : wanted;
      throw AssertionError((a._message ? a._message + ": " : "") + text);
    }
    return a;
  }

  const show = (v) => inspect(v, 1);

  // Words that only read well: expect(x).to.be.an("array").
  for (const word of ["to", "be", "been", "is", "that", "which", "and", "has", "have", "with", "at", "of", "same", "does", "but", "also", "still"]) {
    Object.defineProperty(Assertion.prototype, word, {
      get() {
        return this;
      },
    });
  }

  const flags = {
    not() {
      this._negate = !this._negate;
    },
    deep() {
      this._deep = true;
    },
    any() {
      this._any = true;
    },
    all() {
      this._any = false;
    },
    own() {
      this._own = true;
    },
    nested() {},
  };
  for (const name of Object.keys(flags)) {
    Object.defineProperty(Assertion.prototype, name, {
      get() {
        flags[name].call(this);
        return this;
      },
    });
  }

  // Properties that assert when read: expect(x).to.be.true.
  const checks = {
    ok() {
      return check(this, !!this._subject, "expected " + show(this._subject) + " to be truthy", "expected " + show(this._subject) + " to be falsy");
    },
    true() {
      return check(this, this._subject === true, "expected " + show(this._subject) + " to be true", "expected " + show(this._subject) + " to not be true");
    },
    false() {
      return check(this, this._subject === false, "expected " + show(this._subject) + " to be false", "expected " + show(this._subject) + " to not be false");
    },
    null() {
      return check(this, this._subject === null, "expected " + show(this._subject) + " to be null", "expected " + show(this._subject) + " to not be null");
    },
    undefined() {
      return check(this, this._subject === undefined, "expected " + show(this._subject) + " to be undefined", "expected " + show(this._subject) + " to not be undefined");
    },
    NaN() {
      return check(this, this._subject !== this._subject, "expected " + show(this._subject) + " to be NaN", "expected " + show(this._subject) + " to not be NaN");
    },
    exist() {
      return check(this, this._subject !== null && this._subject !== undefined, "expected " + show(this._subject) + " to exist", "expected " + show(this._subject) + " to not exist");
    },
    empty() {
      return check(this, sizeOf(this._subject) === 0, "expected " + show(this._subject) + " to be empty", "expected " + show(this._subject) + " not to be empty");
    },
  };
  for (const name of Object.keys(checks)) {
    Object.defineProperty(Assertion.prototype, name, {
      get() {
        return checks[name].call(this);
      },
    });
  }

  const methods = {
    equal(value) {
      const pass = this._deep ? deepEqual(this._subject, value) : this._subject === value;
      return check(this, pass, "expected " + show(this._subject) + " to " + (this._deep ? "deeply " : "") + "equal " + show(value), "expected " + show(this._subject) + " to not " + (this._deep ? "deeply " : "") + "equal " + show(value));
    },
    eql(value) {
      return check(this, deepEqual(this._subject, value), "expected " + show(this._subject) + " to deeply equal " + show(value), "expected " + show(this._subject) + " to not deeply equal " + show(value));
    },
    above(n) {
      return check(this, this._subject > n, "expected " + show(this._subject) + " to be above " + show(n), "expected " + show(this._subject) + " to be at most " + show(n));
    },
    least(n) {
      return check(this, this._subject >= n, "expected " + show(this._subject) + " to be at least " + show(n), "expected " + show(this._subject) + " to be below " + show(n));
    },
    below(n) {
      return check(this, this._subject < n, "expected " + show(this._subject) + " to be below " + show(n), "expected " + show(this._subject) + " to be at least " + show(n));
    },
    most(n) {
      return check(this, this._subject <= n, "expected " + show(this._subject) + " to be at most " + show(n), "expected " + show(this._subject) + " to be above " + show(n));
    },
    within(low, high) {
      return check(this, this._subject >= low && this._subject <= high, "expected " + show(this._subject) + " to be within " + low + ".." + high, "expected " + show(this._subject) + " to not be within " + low + ".." + high);
    },
    closeTo(value, delta) {
      return check(this, Math.abs(this._subject - value) <= delta, "expected " + show(this._subject) + " to be close to " + value + " +/- " + delta, "expected " + show(this._subject) + " not to be close to " + value + " +/- " + delta);
    },
    a(type) {
      const actual = typeName(this._subject);
      const article = /^[aeiou]/.test(type) ? "an " : "a ";
      return check(this, actual === String(type).toLowerCase(), "expected " + show(this._subject) + " to be " + article + type, "expected " + show(this._subject) + " not to be " + article + type);
    },
    instanceOf(ctor) {
      return check(this, this._subject instanceof ctor, "expected " + show(this._subject) + " to be an instance of " + (ctor && ctor.name), "expected " + show(this._subject) + " to not be an instance of " + (ctor && ctor.name));
    },
    include(value) {
      return check(this, includes(this._subject, value, this._deep), "expected " + show(this._subject) + " to include " + show(value), "expected " + show(this._subject) + " to not include " + show(value));
    },
    property(name, value) {
      const subject = this._subject;
      const has = subject !== null && subject !== undefined && (this._own ? own(subject, name) : name in Object(subject));
      if (arguments.length < 2) {
        check(this, has, "expected " + show(subject) + " to have property " + show(name), "expected " + show(subject) + " to not have property " + show(name));
        if (!this._negate) this._subject = subject[name];
        return this;
      }
      const actual = has ? subject[name] : undefined;
      const same = this._deep ? deepEqual(actual, value) : actual === value;
      return check(this, has && same, "expected " + show(subject) + " to have property " + show(name) + " of " + show(value) + ", but got " + show(actual), "expected " + show(subject) + " to not have property " + show(name) + " of " + show(value));
    },
    ownProperty(name, value) {
      this._own = true;
      return arguments.length < 2 ? this.property(name) : this.property(name, value);
    },
    lengthOf(n) {
      const size = sizeOf(this._subject);
      return check(this, size === n, "expected " + show(this._subject) + " to have a length of " + n + " but got " + size, "expected " + show(this._subject) + " to not have a length of " + n);
    },
    keys() {
      const wanted = arguments.length === 1 && typeof arguments[0] === "object" ? [].concat(Array.isArray(arguments[0]) ? arguments[0] : Object.keys(arguments[0])) : Array.prototype.slice.call(arguments);
      const actual = Object.keys(Object(this._subject));
      const pass = this._any ? wanted.some((k) => actual.indexOf(k) >= 0) : wanted.every((k) => actual.indexOf(k) >= 0) && (this._negate || wanted.length === actual.length || this._own);
      return check(this, pass, "expected " + show(this._subject) + " to have " + (this._any ? "any of " : "") + "keys " + show(wanted), "expected " + show(this._subject) + " to not have keys " + show(wanted));
    },
    match(re) {
      return check(this, re.test(String(this._subject)), "expected " + show(this._subject) + " to match " + re, "expected " + show(this._subject) + " not to match " + re);
    },
    string(part) {
      return check(this, String(this._subject).indexOf(part) >= 0, "expected " + show(this._subject) + " to contain " + show(part), "expected " + show(this._subject) + " to not contain " + show(part));
    },
    oneOf(list) {
      return check(this, list.some((x) => (this._deep ? deepEqual(x, this._subject) : x === this._subject)), "expected " + show(this._subject) + " to be one of " + show(list), "expected " + show(this._subject) + " to not be one of " + show(list));
    },
    members(list) {
      const subject = this._subject || [];
      const has = (x) => subject.some((y) => (this._deep ? deepEqual(x, y) : x === y));
      const pass = list.every(has) && list.length === subject.length;
      return check(this, pass, "expected " + show(subject) + " to have the same members as " + show(list), "expected " + show(subject) + " to not have the same members as " + show(list));
    },
    satisfy(fn) {
      return check(this, !!fn(this._subject), "expected " + show(this._subject) + " to satisfy " + (fn.name || "the function"), "expected " + show(this._subject) + " to not satisfy " + (fn.name || "the function"));
    },
    throw(expected) {
      let thrown;
      try {
        this._subject();
      } catch (e) {
        thrown = e;
      }
      let pass = thrown !== undefined;
      if (pass && expected instanceof RegExp) pass = expected.test(thrown && thrown.message);
      else if (pass && typeof expected === "string") pass = String(thrown && thrown.message).indexOf(expected) >= 0;
      return check(this, pass, "expected the function to throw" + (expected ? " " + show(expected) : ""), "expected the function not to throw, but it threw " + show(thrown));
    },
    jsonSchema(schema) {
      const problems = [];
      validate(this._subject, schema, "", problems);
      return check(this, problems.length === 0, "expected data to match the schema: " + problems.join("; "), "expected data not to match the schema");
    },
  };
  const aliases = {
    equals: "equal",
    eq: "equal",
    eqls: "eql",
    gt: "above",
    greaterThan: "above",
    gte: "least",
    greaterThanOrEqual: "least",
    lt: "below",
    lessThan: "below",
    lte: "most",
    lessThanOrEqual: "most",
    an: "a",
    instanceof: "instanceOf",
    includes: "include",
    contain: "include",
    contains: "include",
    matches: "match",
    key: "keys",
    throws: "throw",
    Throw: "throw",
    haveOwnProperty: "ownProperty",
  };
  for (const name of Object.keys(methods)) Assertion.prototype[name] = methods[name];
  for (const name of Object.keys(aliases)) Assertion.prototype[name] = methods[aliases[name]];
  // `.to.have.length(3)` works as a call; `.lengthOf(3)` is the same.
  Object.defineProperty(Assertion.prototype, "length", {
    get() {
      const self = this;
      return function (n) {
        return self.lengthOf(n);
      };
    },
  });

  // A small JSON Schema check (type, required, properties, items, enum,
  // minimum/maximum, minLength/maxLength, pattern) for `jsonSchema`.
  function validate(value, schema, at, problems) {
    if (!schema || typeof schema !== "object") return;
    const where = at || "the value";
    if (schema.type) {
      const types = [].concat(schema.type);
      const actual = typeName(value) === "number" && Number.isInteger(value) ? ["number", "integer"] : [typeName(value)];
      if (!types.some((t) => actual.indexOf(t) >= 0)) {
        problems.push(where + " should be " + types.join(" or "));
        return;
      }
    }
    if (schema.enum && !schema.enum.some((x) => deepEqual(x, value))) problems.push(where + " should be one of " + JSON.stringify(schema.enum));
    if (typeof value === "number") {
      if (schema.minimum !== undefined && value < schema.minimum) problems.push(where + " should be at least " + schema.minimum);
      if (schema.maximum !== undefined && value > schema.maximum) problems.push(where + " should be at most " + schema.maximum);
    }
    if (typeof value === "string") {
      if (schema.minLength !== undefined && value.length < schema.minLength) problems.push(where + " should have at least " + schema.minLength + " characters");
      if (schema.maxLength !== undefined && value.length > schema.maxLength) problems.push(where + " should have at most " + schema.maxLength + " characters");
      if (schema.pattern && !new RegExp(schema.pattern).test(value)) problems.push(where + " should match " + schema.pattern);
    }
    if (value && typeof value === "object" && !Array.isArray(value)) {
      for (const name of schema.required || []) if (!own(value, name)) problems.push((at ? at + "." : "") + name + " is required");
      for (const name of Object.keys(schema.properties || {})) {
        if (own(value, name)) validate(value[name], schema.properties[name], (at ? at + "." : "") + name, problems);
      }
    }
    if (Array.isArray(value) && schema.items) value.forEach((item, i) => validate(item, schema.items, (at || "") + "[" + i + "]", problems));
  }

  function expect(subject, message) {
    return new Assertion(subject, message);
  }
  expect.fail = function (message) {
    throw AssertionError(message || "expect.fail()");
  };

  // ---------------------------------------------------------------- tests

  function runTest(name, fn) {
    if (typeof fn !== "function") {
      tests.push({ name: String(name), passed: false, skipped: false, error: "pm.test needs a function" });
      return;
    }
    try {
      const result = fn();
      if (result && typeof result.then === "function") {
        const entry = { name: String(name), passed: true, skipped: false, error: null };
        tests.push(entry);
        result.then(null, (e) => {
          entry.passed = false;
          entry.error = e && e.message ? e.message : String(e);
        });
        return;
      }
      tests.push({ name: String(name), passed: true, skipped: false, error: null });
    } catch (e) {
      tests.push({ name: String(name), passed: false, skipped: false, error: e && e.message ? e.message : String(e) });
    }
  }
  runTest.skip = function (name) {
    tests.push({ name: String(name), passed: true, skipped: true, error: null });
  };

  // --------------------------------------------------------------- response

  // pm.response, and what pm.sendRequest hands back.
  function responseObject(res) {
    if (!res) return undefined;
    const headers = propertyList(res.headers, true);
    let parsed;
    const json = () => {
      if (parsed === undefined) parsed = JSON.parse(res.body);
      return parsed;
    };
    const statusClass = (low, high) => res.code >= low && res.code <= high;
    function to(negate) {
      const assert = (pass, wanted, unwanted) => {
        if (pass === negate) throw AssertionError(negate ? unwanted : wanted);
      };
      const have = {
        status(expected) {
          if (typeof expected === "number") assert(res.code === expected, "expected response to have status code " + expected + " but got " + res.code, "expected response to not have status code " + expected);
          else assert(res.status === String(expected), "expected response to have status reason " + show(expected) + " but got " + show(res.status), "expected response to not have status reason " + show(expected));
        },
        header(name, value) {
          const actual = headers.get(name);
          if (value === undefined) assert(actual !== undefined, "expected response to have header " + show(name), "expected response to not have header " + show(name));
          else assert(actual === String(value), "expected response header " + show(name) + " to be " + show(value) + " but got " + show(actual), "expected response header " + show(name) + " to not be " + show(value));
        },
        body(value) {
          if (value === undefined) assert(res.body.length > 0, "expected response to have a body", "expected response to not have a body");
          else if (value instanceof RegExp) assert(value.test(res.body), "expected response body to match " + value, "expected response body not to match " + value);
          else assert(res.body === String(value), "expected response body to equal " + show(value), "expected response body to not equal " + show(value));
        },
        jsonBody(path, value) {
          let body;
          try {
            body = json();
          } catch (e) {
            assert(false, "expected response body to be JSON", "");
            return;
          }
          if (path === undefined) {
            assert(true, "", "expected response body not to be JSON");
            return;
          }
          if (typeof path === "object") {
            assert(deepEqual(body, path), "expected response body to equal " + show(path), "expected response body to not equal " + show(path));
            return;
          }
          const actual = String(path)
            .split(".")
            .reduce((o, k) => (o === undefined || o === null ? undefined : o[k]), body);
          if (arguments.length < 2) assert(actual !== undefined, "expected response body to have " + show(path), "expected response body to not have " + show(path));
          else assert(deepEqual(actual, value), "expected " + show(path) + " to be " + show(value) + " but got " + show(actual), "expected " + show(path) + " to not be " + show(value));
        },
        jsonSchema(schema) {
          const problems = [];
          validate(json(), schema, "", problems);
          assert(problems.length === 0, "expected response body to match the schema: " + problems.join("; "), "expected response body not to match the schema");
        },
        responseTime: {
          below: (ms) => assert(res.responseTime < ms, "expected response time to be below " + ms + " ms but got " + res.responseTime, "expected response time to not be below " + ms + " ms"),
          above: (ms) => assert(res.responseTime > ms, "expected response time to be above " + ms + " ms but got " + res.responseTime, "expected response time to not be above " + ms + " ms"),
        },
      };
      const be = {};
      const states = {
        ok: [200, 299],
        success: [200, 299],
        accepted: [202, 202],
        badRequest: [400, 400],
        unauthorized: [401, 401],
        unauthorised: [401, 401],
        forbidden: [403, 403],
        notFound: [404, 404],
        rateLimited: [429, 429],
        error: [400, 599],
        clientError: [400, 499],
        serverError: [500, 599],
        redirection: [300, 399],
        info: [100, 199],
      };
      for (const name of Object.keys(states)) {
        Object.defineProperty(be, name, {
          get() {
            const range = states[name];
            assert(statusClass(range[0], range[1]), "expected response to be " + name + " but got status " + res.code, "expected response to not be " + name + " but got status " + res.code);
            return true;
          },
        });
      }
      Object.defineProperty(be, "json", {
        get() {
          let ok = true;
          try {
            json();
          } catch (e) {
            ok = false;
          }
          assert(ok, "expected response body to be JSON", "expected response body to not be JSON");
          return true;
        },
      });
      const out = { have: have, be: be };
      if (!negate) out.not = to(true);
      return out;
    }
    const response = {
      code: res.code,
      status: res.status,
      responseTime: res.responseTime,
      responseSize: res.responseSize,
      headers: headers,
      text: () => res.body,
      json: json,
      reason: () => res.status,
      to: to(false),
    };
    Object.defineProperty(response, "body", {
      get: () => res.body,
    });
    return response;
  }

  // ------------------------------------------------------------ sendRequest

  // A Postman request description (a URL, or { url, method, header, body })
  // as the request rows Rust reads.
  function requestSpec(spec) {
    if (typeof spec === "string" || (spec && typeof spec.toString === "function" && spec.url === undefined)) {
      spec = { url: String(spec) };
    }
    const headers = [];
    const given = spec.header !== undefined ? spec.header : spec.headers;
    if (Array.isArray(given)) {
      for (const h of given) {
        if (typeof h === "string") {
          const colon = h.indexOf(":");
          headers.push({ key: h.slice(0, colon).trim(), value: h.slice(colon + 1).trim(), disabled: false });
        } else headers.push({ key: String(h.key), value: text(h.value), disabled: !!h.disabled });
      }
    } else if (given && typeof given === "object") {
      for (const key of Object.keys(given)) headers.push({ key: key, value: text(given[key]), disabled: false });
    }
    const body = spec.body || {};
    let mode = body.mode || "none";
    let raw = "";
    let urlencoded = [];
    const rows = (list) => (list || []).map((r) => ({ key: String(r.key), value: text(r.value), disabled: !!r.disabled }));
    if (mode === "raw") raw = text(body.raw);
    else if (mode === "urlencoded") urlencoded = rows(body.urlencoded);
    else if (mode === "formdata") {
      mode = "urlencoded";
      urlencoded = rows(body.formdata);
    } else if (mode === "graphql") {
      mode = "raw";
      const gql = body.graphql || {};
      raw = JSON.stringify({ query: gql.query || "", variables: typeof gql.variables === "string" ? JSON.parse(gql.variables || "null") : gql.variables || null });
      if (!headers.some((h) => h.key.toLowerCase() === "content-type")) headers.push({ key: "Content-Type", value: "application/json", disabled: false });
    } else mode = "none";
    const split = splitUrl(String(spec.url));
    return { method: String(spec.method || "GET").toUpperCase(), url: split.base, query: split.query, headers: headers, body: { mode: mode, raw: raw, urlencoded: urlencoded } };
  }

  // Waits for the response (scripts run one at a time anyway), then calls
  // `callback(error, response)` and also resolves the returned promise, so
  // both the callback style and `await pm.sendRequest(…)` work.
  function sendRequest(spec, callback) {
    let reply;
    try {
      reply = JSON.parse(global.__apix_send(JSON.stringify(requestSpec(spec))));
    } catch (e) {
      reply = { error: e && e.message ? e.message : String(e) };
    }
    const error = reply.error ? new Error(reply.error) : null;
    const response = error ? null : responseObject(reply.response);
    if (typeof callback === "function") {
      callback(error, response);
      return Promise.resolve(response);
    }
    return error ? Promise.reject(error) : Promise.resolve(response);
  }

  // ------------------------------------------------------------ script runs

  let scriptError = null;
  function describeError(e) {
    if (e && e.message !== undefined) {
      const frame = String(e.stack || "")
        .split("\n")
        .map((l) => l.trim())
        .find((l) => l.indexOf(".js:") >= 0);
      return frame ? e.message + " (" + frame + ")" : String(e.message);
    }
    return String(e);
  }
  // Each script is an async function; a throw (also after an `await`)
  // becomes the script's error.
  global.__apix_run = function (fn) {
    fn().then(null, (e) => {
      if (scriptError === null) scriptError = describeError(e);
    });
  };
  global.__apix_error = function () {
    const e = scriptError;
    scriptError = null;
    return e;
  };

  // ---------------------------------------------------------------- console

  function log(level) {
    return function () {
      const parts = [];
      for (let i = 0; i < arguments.length; i++) parts.push(inspect(arguments[i]));
      logs.push({ level: level, text: parts.join(" ") });
    };
  }
  global.console = {
    log: log("log"),
    info: log("info"),
    warn: log("warn"),
    error: log("error"),
    debug: log("debug"),
    trace: log("debug"),
  };

  // ------------------------------------------------------------ base64

  const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  global.btoa = function (data) {
    data = String(data);
    let out = "";
    for (let i = 0; i < data.length; i += 3) {
      const a = data.charCodeAt(i);
      const b = data.charCodeAt(i + 1);
      const c = data.charCodeAt(i + 2);
      if (a > 255 || b > 255 || c > 255) throw new Error("btoa: the string has characters outside Latin-1");
      const n = (a << 16) | ((b || 0) << 8) | (c || 0);
      out += B64[(n >> 18) & 63] + B64[(n >> 12) & 63] + (i + 1 < data.length ? B64[(n >> 6) & 63] : "=") + (i + 2 < data.length ? B64[n & 63] : "=");
    }
    return out;
  };
  global.atob = function (data) {
    data = String(data).replace(/[\s=]/g, "");
    let out = "";
    let bits = 0;
    let buffer = 0;
    for (const ch of data) {
      const v = B64.indexOf(ch);
      if (v < 0) throw new Error("atob: not base64");
      buffer = (buffer << 6) | v;
      bits += 6;
      if (bits >= 8) {
        bits -= 8;
        out += String.fromCharCode((buffer >> bits) & 255);
      }
    }
    return out;
  };

  // ------------------------------------------------------------------- pm

  const pm = {
    info: {
      eventName: input.event,
      iteration: input.info.iteration,
      iterationCount: input.info.iteration_count,
      requestName: input.info.request_name,
      requestId: input.info.request_id,
    },
    globals: globals,
    collectionVariables: collectionVariables,
    environment: environment,
    variables: variables,
    iterationData: {
      get: (key) => (own(data, key) ? data[key] : undefined),
      has: (key) => own(data, key),
      toObject: () => Object.assign({}, data),
    },
    request: requestObject(req),
    response: responseObject(res),
    test: runTest,
    expect: expect,
    execution: {
      setNextRequest(name) {
        nextRequest = name === null || name === undefined ? null : String(name);
      },
      skipRequest() {
        skipRequest = true;
      },
      location: [input.info.request_name],
    },
    sendRequest: sendRequest,
    visualizer: {
      set() {
        logs.push({ level: "info", text: "pm.visualizer is not supported; the template was ignored" });
      },
    },
    require(name) {
      return global.require(name);
    },
  };
  pm.environment.name = environment.name;
  global.pm = pm;
  global.apix = pm;

  global.require = function (name) {
    throw new Error("require('" + name + "') is not available yet");
  };

  // ------------------------------------------- the old Postman script API

  const legacyTests = {};
  global.tests = legacyTests;
  global.postman = {
    setEnvironmentVariable: (k, v) => environment.set(k, v),
    getEnvironmentVariable: (k) => environment.get(k),
    clearEnvironmentVariable: (k) => environment.unset(k),
    clearEnvironmentVariables: () => environment.clear(),
    setGlobalVariable: (k, v) => globals.set(k, v),
    getGlobalVariable: (k) => globals.get(k),
    clearGlobalVariable: (k) => globals.unset(k),
    clearGlobalVariables: () => globals.clear(),
    getVariable: (k) => anyGet(k),
    setNextRequest: (name) => pm.execution.setNextRequest(name),
    getResponseHeader: (name) => (res ? propertyList(res.headers, true).get(name) : undefined),
  };
  global.environment = environment.toObject();
  global.globals = globals.toObject();
  global.data = Object.assign({}, data);
  global.iteration = input.info.iteration;
  global.request = { url: pm.request.url.toString(), method: req.method, headers: pm.request.headers.toObject(), data: pm.request.body.toString() };
  if (res) {
    global.responseBody = res.body;
    global.responseCode = { code: res.code, name: res.status, detail: res.status };
    global.responseTime = res.responseTime;
    global.responseHeaders = propertyList(res.headers, true).toObject();
  }

  // ----------------------------------------------------------------- output

  global.__apix_output = function () {
    for (const name of Object.keys(legacyTests)) {
      const passed = !!legacyTests[name];
      tests.push({ name: name, passed: passed, skipped: false, error: passed ? null : "the test was false" });
    }
    return JSON.stringify({
      request: req,
      state: state,
      tests: tests,
      logs: logs,
      next_request: nextRequest === undefined ? { set: false, name: null } : { set: true, name: nextRequest },
      skip_request: skipRequest,
    });
  };
})(globalThis);
