/**
 * @license
 * Copyright 2019 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const rt = globalThis, gt = rt.ShadowRoot && (rt.ShadyCSS === void 0 || rt.ShadyCSS.nativeShadow) && "adoptedStyleSheets" in Document.prototype && "replace" in CSSStyleSheet.prototype, _t = Symbol(), Ut = /* @__PURE__ */ new WeakMap();
let jt = class {
  constructor(t, e, s) {
    if (this._$cssResult$ = !0, s !== _t) throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");
    this.cssText = t, this.t = e;
  }
  get styleSheet() {
    let t = this.o;
    const e = this.t;
    if (gt && t === void 0) {
      const s = e !== void 0 && e.length === 1;
      s && (t = Ut.get(e)), t === void 0 && ((this.o = t = new CSSStyleSheet()).replaceSync(this.cssText), s && Ut.set(e, t));
    }
    return t;
  }
  toString() {
    return this.cssText;
  }
};
const Q = (r) => new jt(typeof r == "string" ? r : r + "", void 0, _t), _ = (r, ...t) => {
  const e = r.length === 1 ? r[0] : t.reduce((s, i, o) => s + ((a) => {
    if (a._$cssResult$ === !0) return a.cssText;
    if (typeof a == "number") return a;
    throw Error("Value passed to 'css' function must be a 'css' function result: " + a + ". Use 'unsafeCSS' to pass non-literal values, but take care to ensure page security.");
  })(i) + r[o + 1], r[0]);
  return new jt(e, r, _t);
}, te = (r, t) => {
  if (gt) r.adoptedStyleSheets = t.map((e) => e instanceof CSSStyleSheet ? e : e.styleSheet);
  else for (const e of t) {
    const s = document.createElement("style"), i = rt.litNonce;
    i !== void 0 && s.setAttribute("nonce", i), s.textContent = e.cssText, r.appendChild(s);
  }
}, Rt = gt ? (r) => r : (r) => r instanceof CSSStyleSheet ? ((t) => {
  let e = "";
  for (const s of t.cssRules) e += s.cssText;
  return Q(e);
})(r) : r;
/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const { is: ee, defineProperty: se, getOwnPropertyDescriptor: ie, getOwnPropertyNames: re, getOwnPropertySymbols: oe, getPrototypeOf: ae } = Object, O = globalThis, Mt = O.trustedTypes, ne = Mt ? Mt.emptyScript : "", ut = O.reactiveElementPolyfillSupport, J = (r, t) => r, ot = { toAttribute(r, t) {
  switch (t) {
    case Boolean:
      r = r ? ne : null;
      break;
    case Object:
    case Array:
      r = r == null ? r : JSON.stringify(r);
  }
  return r;
}, fromAttribute(r, t) {
  let e = r;
  switch (t) {
    case Boolean:
      e = r !== null;
      break;
    case Number:
      e = r === null ? null : Number(r);
      break;
    case Object:
    case Array:
      try {
        e = JSON.parse(r);
      } catch {
        e = null;
      }
  }
  return e;
} }, mt = (r, t) => !ee(r, t), zt = { attribute: !0, type: String, converter: ot, reflect: !1, useDefault: !1, hasChanged: mt };
Symbol.metadata ?? (Symbol.metadata = Symbol("metadata")), O.litPropertyMetadata ?? (O.litPropertyMetadata = /* @__PURE__ */ new WeakMap());
let N = class extends HTMLElement {
  static addInitializer(t) {
    this._$Ei(), (this.l ?? (this.l = [])).push(t);
  }
  static get observedAttributes() {
    return this.finalize(), this._$Eh && [...this._$Eh.keys()];
  }
  static createProperty(t, e = zt) {
    if (e.state && (e.attribute = !1), this._$Ei(), this.prototype.hasOwnProperty(t) && ((e = Object.create(e)).wrapped = !0), this.elementProperties.set(t, e), !e.noAccessor) {
      const s = Symbol(), i = this.getPropertyDescriptor(t, s, e);
      i !== void 0 && se(this.prototype, t, i);
    }
  }
  static getPropertyDescriptor(t, e, s) {
    const { get: i, set: o } = ie(this.prototype, t) ?? { get() {
      return this[e];
    }, set(a) {
      this[e] = a;
    } };
    return { get: i, set(a) {
      const n = i == null ? void 0 : i.call(this);
      o == null || o.call(this, a), this.requestUpdate(t, n, s);
    }, configurable: !0, enumerable: !0 };
  }
  static getPropertyOptions(t) {
    return this.elementProperties.get(t) ?? zt;
  }
  static _$Ei() {
    if (this.hasOwnProperty(J("elementProperties"))) return;
    const t = ae(this);
    t.finalize(), t.l !== void 0 && (this.l = [...t.l]), this.elementProperties = new Map(t.elementProperties);
  }
  static finalize() {
    if (this.hasOwnProperty(J("finalized"))) return;
    if (this.finalized = !0, this._$Ei(), this.hasOwnProperty(J("properties"))) {
      const e = this.properties, s = [...re(e), ...oe(e)];
      for (const i of s) this.createProperty(i, e[i]);
    }
    const t = this[Symbol.metadata];
    if (t !== null) {
      const e = litPropertyMetadata.get(t);
      if (e !== void 0) for (const [s, i] of e) this.elementProperties.set(s, i);
    }
    this._$Eh = /* @__PURE__ */ new Map();
    for (const [e, s] of this.elementProperties) {
      const i = this._$Eu(e, s);
      i !== void 0 && this._$Eh.set(i, e);
    }
    this.elementStyles = this.finalizeStyles(this.styles);
  }
  static finalizeStyles(t) {
    const e = [];
    if (Array.isArray(t)) {
      const s = new Set(t.flat(1 / 0).reverse());
      for (const i of s) e.unshift(Rt(i));
    } else t !== void 0 && e.push(Rt(t));
    return e;
  }
  static _$Eu(t, e) {
    const s = e.attribute;
    return s === !1 ? void 0 : typeof s == "string" ? s : typeof t == "string" ? t.toLowerCase() : void 0;
  }
  constructor() {
    super(), this._$Ep = void 0, this.isUpdatePending = !1, this.hasUpdated = !1, this._$Em = null, this._$Ev();
  }
  _$Ev() {
    var t;
    this._$ES = new Promise((e) => this.enableUpdating = e), this._$AL = /* @__PURE__ */ new Map(), this._$E_(), this.requestUpdate(), (t = this.constructor.l) == null || t.forEach((e) => e(this));
  }
  addController(t) {
    var e;
    (this._$EO ?? (this._$EO = /* @__PURE__ */ new Set())).add(t), this.renderRoot !== void 0 && this.isConnected && ((e = t.hostConnected) == null || e.call(t));
  }
  removeController(t) {
    var e;
    (e = this._$EO) == null || e.delete(t);
  }
  _$E_() {
    const t = /* @__PURE__ */ new Map(), e = this.constructor.elementProperties;
    for (const s of e.keys()) this.hasOwnProperty(s) && (t.set(s, this[s]), delete this[s]);
    t.size > 0 && (this._$Ep = t);
  }
  createRenderRoot() {
    const t = this.shadowRoot ?? this.attachShadow(this.constructor.shadowRootOptions);
    return te(t, this.constructor.elementStyles), t;
  }
  connectedCallback() {
    var t;
    this.renderRoot ?? (this.renderRoot = this.createRenderRoot()), this.enableUpdating(!0), (t = this._$EO) == null || t.forEach((e) => {
      var s;
      return (s = e.hostConnected) == null ? void 0 : s.call(e);
    });
  }
  enableUpdating(t) {
  }
  disconnectedCallback() {
    var t;
    (t = this._$EO) == null || t.forEach((e) => {
      var s;
      return (s = e.hostDisconnected) == null ? void 0 : s.call(e);
    });
  }
  attributeChangedCallback(t, e, s) {
    this._$AK(t, s);
  }
  _$ET(t, e) {
    var o;
    const s = this.constructor.elementProperties.get(t), i = this.constructor._$Eu(t, s);
    if (i !== void 0 && s.reflect === !0) {
      const a = (((o = s.converter) == null ? void 0 : o.toAttribute) !== void 0 ? s.converter : ot).toAttribute(e, s.type);
      this._$Em = t, a == null ? this.removeAttribute(i) : this.setAttribute(i, a), this._$Em = null;
    }
  }
  _$AK(t, e) {
    var o, a;
    const s = this.constructor, i = s._$Eh.get(t);
    if (i !== void 0 && this._$Em !== i) {
      const n = s.getPropertyOptions(i), l = typeof n.converter == "function" ? { fromAttribute: n.converter } : ((o = n.converter) == null ? void 0 : o.fromAttribute) !== void 0 ? n.converter : ot;
      this._$Em = i;
      const c = l.fromAttribute(e, n.type);
      this[i] = c ?? ((a = this._$Ej) == null ? void 0 : a.get(i)) ?? c, this._$Em = null;
    }
  }
  requestUpdate(t, e, s, i = !1, o) {
    var a;
    if (t !== void 0) {
      const n = this.constructor;
      if (i === !1 && (o = this[t]), s ?? (s = n.getPropertyOptions(t)), !((s.hasChanged ?? mt)(o, e) || s.useDefault && s.reflect && o === ((a = this._$Ej) == null ? void 0 : a.get(t)) && !this.hasAttribute(n._$Eu(t, s)))) return;
      this.C(t, e, s);
    }
    this.isUpdatePending === !1 && (this._$ES = this._$EP());
  }
  C(t, e, { useDefault: s, reflect: i, wrapped: o }, a) {
    s && !(this._$Ej ?? (this._$Ej = /* @__PURE__ */ new Map())).has(t) && (this._$Ej.set(t, a ?? e ?? this[t]), o !== !0 || a !== void 0) || (this._$AL.has(t) || (this.hasUpdated || s || (e = void 0), this._$AL.set(t, e)), i === !0 && this._$Em !== t && (this._$Eq ?? (this._$Eq = /* @__PURE__ */ new Set())).add(t));
  }
  async _$EP() {
    this.isUpdatePending = !0;
    try {
      await this._$ES;
    } catch (e) {
      Promise.reject(e);
    }
    const t = this.scheduleUpdate();
    return t != null && await t, !this.isUpdatePending;
  }
  scheduleUpdate() {
    return this.performUpdate();
  }
  performUpdate() {
    var s;
    if (!this.isUpdatePending) return;
    if (!this.hasUpdated) {
      if (this.renderRoot ?? (this.renderRoot = this.createRenderRoot()), this._$Ep) {
        for (const [o, a] of this._$Ep) this[o] = a;
        this._$Ep = void 0;
      }
      const i = this.constructor.elementProperties;
      if (i.size > 0) for (const [o, a] of i) {
        const { wrapped: n } = a, l = this[o];
        n !== !0 || this._$AL.has(o) || l === void 0 || this.C(o, void 0, a, l);
      }
    }
    let t = !1;
    const e = this._$AL;
    try {
      t = this.shouldUpdate(e), t ? (this.willUpdate(e), (s = this._$EO) == null || s.forEach((i) => {
        var o;
        return (o = i.hostUpdate) == null ? void 0 : o.call(i);
      }), this.update(e)) : this._$EM();
    } catch (i) {
      throw t = !1, this._$EM(), i;
    }
    t && this._$AE(e);
  }
  willUpdate(t) {
  }
  _$AE(t) {
    var e;
    (e = this._$EO) == null || e.forEach((s) => {
      var i;
      return (i = s.hostUpdated) == null ? void 0 : i.call(s);
    }), this.hasUpdated || (this.hasUpdated = !0, this.firstUpdated(t)), this.updated(t);
  }
  _$EM() {
    this._$AL = /* @__PURE__ */ new Map(), this.isUpdatePending = !1;
  }
  get updateComplete() {
    return this.getUpdateComplete();
  }
  getUpdateComplete() {
    return this._$ES;
  }
  shouldUpdate(t) {
    return !0;
  }
  update(t) {
    this._$Eq && (this._$Eq = this._$Eq.forEach((e) => this._$ET(e, this[e]))), this._$EM();
  }
  updated(t) {
  }
  firstUpdated(t) {
  }
};
N.elementStyles = [], N.shadowRootOptions = { mode: "open" }, N[J("elementProperties")] = /* @__PURE__ */ new Map(), N[J("finalized")] = /* @__PURE__ */ new Map(), ut == null || ut({ ReactiveElement: N }), (O.reactiveElementVersions ?? (O.reactiveElementVersions = [])).push("2.1.2");
/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const G = globalThis, Bt = (r) => r, at = G.trustedTypes, Ht = at ? at.createPolicy("lit-html", { createHTML: (r) => r }) : void 0, Kt = "$lit$", P = `lit$${Math.random().toFixed(9).slice(2)}$`, Wt = "?" + P, le = `<${Wt}>`, L = document, Z = () => L.createComment(""), Y = (r) => r === null || typeof r != "object" && typeof r != "function", yt = Array.isArray, he = (r) => yt(r) || typeof (r == null ? void 0 : r[Symbol.iterator]) == "function", pt = `[ 	
\f\r]`, K = /<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g, Lt = /-->/g, qt = />/g, z = RegExp(`>|${pt}(?:([^\\s"'>=/]+)(${pt}*=${pt}*(?:[^ 	
\f\r"'\`<>=]|("|')|))|$)`, "g"), Nt = /'/g, Ft = /"/g, Jt = /^(?:script|style|textarea|title)$/i, ce = (r) => (t, ...e) => ({ _$litType$: r, strings: t, values: e }), f = ce(1), m = Symbol.for("lit-noChange"), u = Symbol.for("lit-nothing"), Dt = /* @__PURE__ */ new WeakMap(), B = L.createTreeWalker(L, 129);
function Gt(r, t) {
  if (!yt(r) || !r.hasOwnProperty("raw")) throw Error("invalid template strings array");
  return Ht !== void 0 ? Ht.createHTML(t) : t;
}
const de = (r, t) => {
  const e = r.length - 1, s = [];
  let i, o = t === 2 ? "<svg>" : t === 3 ? "<math>" : "", a = K;
  for (let n = 0; n < e; n++) {
    const l = r[n];
    let c, b, d = -1, x = 0;
    for (; x < l.length && (a.lastIndex = x, b = a.exec(l), b !== null); ) x = a.lastIndex, a === K ? b[1] === "!--" ? a = Lt : b[1] !== void 0 ? a = qt : b[2] !== void 0 ? (Jt.test(b[2]) && (i = RegExp("</" + b[2], "g")), a = z) : b[3] !== void 0 && (a = z) : a === z ? b[0] === ">" ? (a = i ?? K, d = -1) : b[1] === void 0 ? d = -2 : (d = a.lastIndex - b[2].length, c = b[1], a = b[3] === void 0 ? z : b[3] === '"' ? Ft : Nt) : a === Ft || a === Nt ? a = z : a === Lt || a === qt ? a = K : (a = z, i = void 0);
    const S = a === z && r[n + 1].startsWith("/>") ? " " : "";
    o += a === K ? l + le : d >= 0 ? (s.push(c), l.slice(0, d) + Kt + l.slice(d) + P + S) : l + P + (d === -2 ? n : S);
  }
  return [Gt(r, o + (r[e] || "<?>") + (t === 2 ? "</svg>" : t === 3 ? "</math>" : "")), s];
};
class X {
  constructor({ strings: t, _$litType$: e }, s) {
    let i;
    this.parts = [];
    let o = 0, a = 0;
    const n = t.length - 1, l = this.parts, [c, b] = de(t, e);
    if (this.el = X.createElement(c, s), B.currentNode = this.el.content, e === 2 || e === 3) {
      const d = this.el.content.firstChild;
      d.replaceWith(...d.childNodes);
    }
    for (; (i = B.nextNode()) !== null && l.length < n; ) {
      if (i.nodeType === 1) {
        if (i.hasAttributes()) for (const d of i.getAttributeNames()) if (d.endsWith(Kt)) {
          const x = b[a++], S = i.getAttribute(d).split(P), it = /([.?@])?(.*)/.exec(x);
          l.push({ type: 1, index: o, name: it[2], strings: S, ctor: it[1] === "." ? pe : it[1] === "?" ? fe : it[1] === "@" ? be : dt }), i.removeAttribute(d);
        } else d.startsWith(P) && (l.push({ type: 6, index: o }), i.removeAttribute(d));
        if (Jt.test(i.tagName)) {
          const d = i.textContent.split(P), x = d.length - 1;
          if (x > 0) {
            i.textContent = at ? at.emptyScript : "";
            for (let S = 0; S < x; S++) i.append(d[S], Z()), B.nextNode(), l.push({ type: 2, index: ++o });
            i.append(d[x], Z());
          }
        }
      } else if (i.nodeType === 8) if (i.data === Wt) l.push({ type: 2, index: o });
      else {
        let d = -1;
        for (; (d = i.data.indexOf(P, d + 1)) !== -1; ) l.push({ type: 7, index: o }), d += P.length - 1;
      }
      o++;
    }
  }
  static createElement(t, e) {
    const s = L.createElement("template");
    return s.innerHTML = t, s;
  }
}
function D(r, t, e = r, s) {
  var a, n;
  if (t === m) return t;
  let i = s !== void 0 ? (a = e._$Co) == null ? void 0 : a[s] : e._$Cl;
  const o = Y(t) ? void 0 : t._$litDirective$;
  return (i == null ? void 0 : i.constructor) !== o && ((n = i == null ? void 0 : i._$AO) == null || n.call(i, !1), o === void 0 ? i = void 0 : (i = new o(r), i._$AT(r, e, s)), s !== void 0 ? (e._$Co ?? (e._$Co = []))[s] = i : e._$Cl = i), i !== void 0 && (t = D(r, i._$AS(r, t.values), i, s)), t;
}
class ue {
  constructor(t, e) {
    this._$AV = [], this._$AN = void 0, this._$AD = t, this._$AM = e;
  }
  get parentNode() {
    return this._$AM.parentNode;
  }
  get _$AU() {
    return this._$AM._$AU;
  }
  u(t) {
    const { el: { content: e }, parts: s } = this._$AD, i = ((t == null ? void 0 : t.creationScope) ?? L).importNode(e, !0);
    B.currentNode = i;
    let o = B.nextNode(), a = 0, n = 0, l = s[0];
    for (; l !== void 0; ) {
      if (a === l.index) {
        let c;
        l.type === 2 ? c = new tt(o, o.nextSibling, this, t) : l.type === 1 ? c = new l.ctor(o, l.name, l.strings, this, t) : l.type === 6 && (c = new ve(o, this, t)), this._$AV.push(c), l = s[++n];
      }
      a !== (l == null ? void 0 : l.index) && (o = B.nextNode(), a++);
    }
    return B.currentNode = L, i;
  }
  p(t) {
    let e = 0;
    for (const s of this._$AV) s !== void 0 && (s.strings !== void 0 ? (s._$AI(t, s, e), e += s.strings.length - 2) : s._$AI(t[e])), e++;
  }
}
class tt {
  get _$AU() {
    var t;
    return ((t = this._$AM) == null ? void 0 : t._$AU) ?? this._$Cv;
  }
  constructor(t, e, s, i) {
    this.type = 2, this._$AH = u, this._$AN = void 0, this._$AA = t, this._$AB = e, this._$AM = s, this.options = i, this._$Cv = (i == null ? void 0 : i.isConnected) ?? !0;
  }
  get parentNode() {
    let t = this._$AA.parentNode;
    const e = this._$AM;
    return e !== void 0 && (t == null ? void 0 : t.nodeType) === 11 && (t = e.parentNode), t;
  }
  get startNode() {
    return this._$AA;
  }
  get endNode() {
    return this._$AB;
  }
  _$AI(t, e = this) {
    t = D(this, t, e), Y(t) ? t === u || t == null || t === "" ? (this._$AH !== u && this._$AR(), this._$AH = u) : t !== this._$AH && t !== m && this._(t) : t._$litType$ !== void 0 ? this.$(t) : t.nodeType !== void 0 ? this.T(t) : he(t) ? this.k(t) : this._(t);
  }
  O(t) {
    return this._$AA.parentNode.insertBefore(t, this._$AB);
  }
  T(t) {
    this._$AH !== t && (this._$AR(), this._$AH = this.O(t));
  }
  _(t) {
    this._$AH !== u && Y(this._$AH) ? this._$AA.nextSibling.data = t : this.T(L.createTextNode(t)), this._$AH = t;
  }
  $(t) {
    var o;
    const { values: e, _$litType$: s } = t, i = typeof s == "number" ? this._$AC(t) : (s.el === void 0 && (s.el = X.createElement(Gt(s.h, s.h[0]), this.options)), s);
    if (((o = this._$AH) == null ? void 0 : o._$AD) === i) this._$AH.p(e);
    else {
      const a = new ue(i, this), n = a.u(this.options);
      a.p(e), this.T(n), this._$AH = a;
    }
  }
  _$AC(t) {
    let e = Dt.get(t.strings);
    return e === void 0 && Dt.set(t.strings, e = new X(t)), e;
  }
  k(t) {
    yt(this._$AH) || (this._$AH = [], this._$AR());
    const e = this._$AH;
    let s, i = 0;
    for (const o of t) i === e.length ? e.push(s = new tt(this.O(Z()), this.O(Z()), this, this.options)) : s = e[i], s._$AI(o), i++;
    i < e.length && (this._$AR(s && s._$AB.nextSibling, i), e.length = i);
  }
  _$AR(t = this._$AA.nextSibling, e) {
    var s;
    for ((s = this._$AP) == null ? void 0 : s.call(this, !1, !0, e); t !== this._$AB; ) {
      const i = Bt(t).nextSibling;
      Bt(t).remove(), t = i;
    }
  }
  setConnected(t) {
    var e;
    this._$AM === void 0 && (this._$Cv = t, (e = this._$AP) == null || e.call(this, t));
  }
}
class dt {
  get tagName() {
    return this.element.tagName;
  }
  get _$AU() {
    return this._$AM._$AU;
  }
  constructor(t, e, s, i, o) {
    this.type = 1, this._$AH = u, this._$AN = void 0, this.element = t, this.name = e, this._$AM = i, this.options = o, s.length > 2 || s[0] !== "" || s[1] !== "" ? (this._$AH = Array(s.length - 1).fill(new String()), this.strings = s) : this._$AH = u;
  }
  _$AI(t, e = this, s, i) {
    const o = this.strings;
    let a = !1;
    if (o === void 0) t = D(this, t, e, 0), a = !Y(t) || t !== this._$AH && t !== m, a && (this._$AH = t);
    else {
      const n = t;
      let l, c;
      for (t = o[0], l = 0; l < o.length - 1; l++) c = D(this, n[s + l], e, l), c === m && (c = this._$AH[l]), a || (a = !Y(c) || c !== this._$AH[l]), c === u ? t = u : t !== u && (t += (c ?? "") + o[l + 1]), this._$AH[l] = c;
    }
    a && !i && this.j(t);
  }
  j(t) {
    t === u ? this.element.removeAttribute(this.name) : this.element.setAttribute(this.name, t ?? "");
  }
}
class pe extends dt {
  constructor() {
    super(...arguments), this.type = 3;
  }
  j(t) {
    this.element[this.name] = t === u ? void 0 : t;
  }
}
class fe extends dt {
  constructor() {
    super(...arguments), this.type = 4;
  }
  j(t) {
    this.element.toggleAttribute(this.name, !!t && t !== u);
  }
}
class be extends dt {
  constructor(t, e, s, i, o) {
    super(t, e, s, i, o), this.type = 5;
  }
  _$AI(t, e = this) {
    if ((t = D(this, t, e, 0) ?? u) === m) return;
    const s = this._$AH, i = t === u && s !== u || t.capture !== s.capture || t.once !== s.once || t.passive !== s.passive, o = t !== u && (s === u || i);
    i && this.element.removeEventListener(this.name, this, s), o && this.element.addEventListener(this.name, this, t), this._$AH = t;
  }
  handleEvent(t) {
    var e;
    typeof this._$AH == "function" ? this._$AH.call(((e = this.options) == null ? void 0 : e.host) ?? this.element, t) : this._$AH.handleEvent(t);
  }
}
class ve {
  constructor(t, e, s) {
    this.element = t, this.type = 6, this._$AN = void 0, this._$AM = e, this.options = s;
  }
  get _$AU() {
    return this._$AM._$AU;
  }
  _$AI(t) {
    D(this, t);
  }
}
const ft = G.litHtmlPolyfillSupport;
ft == null || ft(X, tt), (G.litHtmlVersions ?? (G.litHtmlVersions = [])).push("3.3.3");
const ge = (r, t, e) => {
  const s = (e == null ? void 0 : e.renderBefore) ?? t;
  let i = s._$litPart$;
  if (i === void 0) {
    const o = (e == null ? void 0 : e.renderBefore) ?? null;
    s._$litPart$ = i = new tt(t.insertBefore(Z(), o), o, void 0, e ?? {});
  }
  return i._$AI(r), i;
};
/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const H = globalThis;
let F = class extends N {
  constructor() {
    super(...arguments), this.renderOptions = { host: this }, this._$Do = void 0;
  }
  createRenderRoot() {
    var e;
    const t = super.createRenderRoot();
    return (e = this.renderOptions).renderBefore ?? (e.renderBefore = t.firstChild), t;
  }
  update(t) {
    const e = this.render();
    this.hasUpdated || (this.renderOptions.isConnected = this.isConnected), super.update(t), this._$Do = ge(e, this.renderRoot, this.renderOptions);
  }
  connectedCallback() {
    var t;
    super.connectedCallback(), (t = this._$Do) == null || t.setConnected(!0);
  }
  disconnectedCallback() {
    var t;
    super.disconnectedCallback(), (t = this._$Do) == null || t.setConnected(!1);
  }
  render() {
    return m;
  }
};
var Vt;
F._$litElement$ = !0, F.finalized = !0, (Vt = H.litElementHydrateSupport) == null || Vt.call(H, { LitElement: F });
const bt = H.litElementPolyfillSupport;
bt == null || bt({ LitElement: F });
(H.litElementVersions ?? (H.litElementVersions = [])).push("4.2.2");
/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const _e = { attribute: !0, type: String, converter: ot, reflect: !1, hasChanged: mt }, me = (r = _e, t, e) => {
  const { kind: s, metadata: i } = e;
  let o = globalThis.litPropertyMetadata.get(i);
  if (o === void 0 && globalThis.litPropertyMetadata.set(i, o = /* @__PURE__ */ new Map()), s === "setter" && ((r = Object.create(r)).wrapped = !0), o.set(e.name, r), s === "accessor") {
    const { name: a } = e;
    return { set(n) {
      const l = t.get.call(this);
      t.set.call(this, n), this.requestUpdate(a, l, r, !0, n);
    }, init(n) {
      return n !== void 0 && this.C(a, void 0, r, n), n;
    } };
  }
  if (s === "setter") {
    const { name: a } = e;
    return function(n) {
      const l = this[a];
      t.call(this, n), this.requestUpdate(a, l, r, !0, n);
    };
  }
  throw Error("Unsupported decorator location: " + s);
};
function h(r) {
  return (t, e) => typeof e == "object" ? me(r, t, e) : ((s, i, o) => {
    const a = i.hasOwnProperty(o);
    return i.constructor.createProperty(o, s), a ? Object.getOwnPropertyDescriptor(i, o) : void 0;
  })(r, t, e);
}
/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
function Qt(r) {
  return h({ ...r, state: !0, attribute: !1 });
}
/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const Zt = (r, t, e) => (e.configurable = !0, e.enumerable = !0, Reflect.decorate && typeof t != "object" && Object.defineProperty(r, t, e), e);
/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
function Yt(r, t) {
  return (e, s, i) => {
    const o = (a) => {
      var n;
      return ((n = a.renderRoot) == null ? void 0 : n.querySelector(r)) ?? null;
    };
    return Zt(e, s, { get() {
      return o(this);
    } });
  };
}
/**
 * @license
 * Copyright 2021 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
function Xt(r) {
  return (t, e) => {
    const { slot: s, selector: i } = r ?? {}, o = "slot" + (s ? `[name=${s}]` : ":not([name])");
    return Zt(t, e, { get() {
      var l;
      const a = (l = this.renderRoot) == null ? void 0 : l.querySelector(o), n = (a == null ? void 0 : a.assignedElements(r)) ?? [];
      return i === void 0 ? n : n.filter((c) => c.matches(i));
    } });
  };
}
const Et = class Et extends F {
};
Et.base = _`
    :host {
      box-sizing: border-box;
    }
    :host *,
    :host *::before,
    :host *::after {
      box-sizing: border-box;
    }
    @media (prefers-reduced-motion: reduce) {
      /* Near-zero (not zero) so animationend/transitionend still fire. */
      :host,
      :host *,
      :host *::before,
      :host *::after {
        animation-duration: 0.001ms !important;
        transition-duration: 0.001ms !important;
        animation-iteration-count: 1 !important;
        scroll-behavior: auto;
      }
    }
  `;
let p = Et;
var ye = Object.defineProperty, E = (r, t, e, s) => {
  for (var i = void 0, o = r.length - 1, a; o >= 0; o--)
    (a = r[o]) && (i = a(t, e, i) || i);
  return i && ye(t, e, i), i;
};
let $e = 0, xe = 0;
const Ct = class Ct extends p {
  constructor() {
    super(...arguments), this.value = "", this.activation = "auto", this.hashSync = !1, this._wired = !1, this._syncQueued = !1, this._warnedValue = null, this._sync = () => {
      var o, a;
      const t = this._tabs, e = this._panels;
      for (const n of t)
        n.id || (n.id = `sema-tab-${++$e}`), n.setAttribute("role", "tab");
      for (const n of e)
        n.id || (n.id = `sema-tab-panel-${++xe}`), n.setAttribute("role", "tabpanel");
      for (const n of t) {
        const l = this._panelFor(n.value);
        l ? (n.setAttribute("aria-controls", l.id), l.setAttribute("aria-labelledby", n.id)) : n.removeAttribute("aria-controls");
      }
      const s = this._enabledTabs(), i = (n) => n !== "" && s.some((l) => l.value === n);
      if (this._wired)
        i(this.value) || (this.value = ((a = s[0]) == null ? void 0 : a.value) ?? "");
      else {
        const n = this.hashSync ? window.location.hash.slice(1) : "";
        if (i(n))
          this.value = n;
        else if (!i(this.value)) {
          this.value && this._warnedValue !== this.value && (this._warnedValue = this.value, console.warn(`<sema-tabs> value="${this.value}" matches no enabled tab`));
          const l = s.find((c) => c.selected);
          this.value = ((o = l ?? s[0]) == null ? void 0 : o.value) ?? "";
        }
        this._wired = t.length > 0;
      }
      this._applySelection();
    }, this._onClick = (t) => {
      const e = this._tabFromEvent(t);
      e && this._activate(e);
    }, this._onKeydown = (t) => {
      const e = this._tabFromEvent(t);
      if (!e) return;
      const s = this._enabledTabs(), i = s.indexOf(e);
      let o;
      if (t.key === "ArrowRight") o = s[(i + 1) % s.length];
      else if (t.key === "ArrowLeft") o = s[(i - 1 + s.length) % s.length];
      else if (t.key === "Home") o = s[0];
      else if (t.key === "End") o = s[s.length - 1];
      else if (t.key === "Enter" || t.key === " ") {
        t.preventDefault(), this._activate(e);
        return;
      } else
        return;
      t.preventDefault(), !(!o || i < 0) && (this._roveTo(o), this.activation === "auto" && this._activate(o));
    }, this._onBeforeMatch = (t) => {
      const e = t.target;
      if (!(e instanceof HTMLElement) || !e.matches("sema-tab-panel")) return;
      const s = this._enabledTabs().find((i) => i.value === e.value);
      s && this._activate(s);
    }, this._onHashChange = () => {
      if (!this.hashSync) return;
      const t = this._enabledTabs().find((e) => e.value === window.location.hash.slice(1));
      t && this._activate(t);
    };
  }
  connectedCallback() {
    super.connectedCallback(), this.addEventListener("beforematch", this._onBeforeMatch), window.addEventListener("hashchange", this._onHashChange), this.hasUpdated && this.updateComplete.then(() => {
      this.isConnected && this._sync();
    });
  }
  disconnectedCallback() {
    super.disconnectedCallback(), this.removeEventListener("beforematch", this._onBeforeMatch), window.removeEventListener("hashchange", this._onHashChange);
  }
  updated(t) {
    t.has("value") && this._wired && this._applySelection();
  }
  render() {
    return f`
      <div
        class="tablist"
        role="tablist"
        part="tablist"
        aria-label=${this.getAttribute("aria-label") || "Tabs"}
        @keydown=${this._onKeydown}
        @click=${this._onClick}
      >
        <slot name="nav" @slotchange=${this._sync}></slot>
      </div>
      <slot @slotchange=${this._sync}></slot>
    `;
  }
  /** Coalesced re-wire, used by child tabs/panels when their props flip. */
  _requestSync() {
    this._syncQueued || (this._syncQueued = !0, queueMicrotask(() => {
      this._syncQueued = !1, this.isConnected && this._sync();
    }));
  }
  _enabledTabs() {
    return this._tabs.filter((t) => !t.disabled);
  }
  _panelFor(t) {
    return this._panels.find((e) => e.value === t);
  }
  _applySelection() {
    const t = this._tabs, e = t.find((i) => !i.disabled && i.value === this.value && this.value !== ""), s = e ?? this._enabledTabs()[0] ?? t[0];
    for (const i of t)
      i.selected = i === e, i.setAttribute("aria-selected", String(i === e)), i.setAttribute("tabindex", i === s ? "0" : "-1"), i.disabled ? i.setAttribute("aria-disabled", "true") : i.removeAttribute("aria-disabled");
    for (const i of this._panels)
      e && i.value === this.value ? (i.removeAttribute("hidden"), this._setPanelFocusability(i)) : (i.setAttribute("hidden", "until-found"), i.removeAttribute("tabindex"));
  }
  // APG: a tabpanel with no focusable content is itself a tab stop.
  _setPanelFocusability(t) {
    t.querySelector(
      "a[href], button:not([disabled]), input:not([disabled]), select, textarea, [tabindex], audio[controls], video[controls], sema-button, sema-input, sema-textarea, sema-select"
    ) ? t.removeAttribute("tabindex") : t.setAttribute("tabindex", "0");
  }
  _tabFromEvent(t) {
    for (const e of t.composedPath())
      if (e instanceof HTMLElement && e.matches("sema-tab")) return e;
    return null;
  }
  _activate(t) {
    t.disabled || t.value === this.value || (this.value = t.value, this._applySelection(), this.hashSync && history.replaceState(null, "", "#" + this.value), t.scrollIntoView({ block: "nearest", inline: "nearest" }), this.dispatchEvent(new CustomEvent("sema-change", {
      detail: { value: this.value },
      bubbles: !0,
      composed: !0
    })));
  }
  /** Move the roving tab stop (and focus) without selecting. */
  _roveTo(t) {
    for (const e of this._tabs) e.setAttribute("tabindex", e === t ? "0" : "-1");
    t.focus();
  }
};
Ct.styles = [
  p.base,
  _`
      :host {
        display: block;
      }
      .tablist {
        display: flex;
        gap: var(--space-lg, 24px);
        overflow-x: auto;
        border-block-end: 1px solid var(--border, #1e1e1e);
        scrollbar-width: thin;
        scrollbar-color: var(--border, #1e1e1e) transparent;
      }
    `
];
let T = Ct;
E([
  h({ reflect: !0 })
], T.prototype, "value");
E([
  h({ reflect: !0 })
], T.prototype, "activation");
E([
  h({ type: Boolean, reflect: !0, attribute: "hash-sync" })
], T.prototype, "hashSync");
E([
  Xt({ slot: "nav", selector: "sema-tab" })
], T.prototype, "_tabs");
E([
  Xt({ selector: "sema-tab-panel" })
], T.prototype, "_panels");
const kt = class kt extends p {
  constructor() {
    super(...arguments), this.value = "", this.selected = !1, this.disabled = !1;
  }
  connectedCallback() {
    super.connectedCallback(), this.slot || (this.slot = "nav");
  }
  updated(t) {
    var e;
    (t.has("disabled") || t.has("value")) && ((e = this.closest("sema-tabs")) == null || e._requestSync());
  }
  render() {
    return f`<slot></slot>`;
  }
};
kt.styles = [
  p.base,
  _`
      :host {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        font-family: var(--mono, 'JetBrains Mono', monospace);
        font-size: var(--text-xs, 11px);
        letter-spacing: 0.02em;
        padding: var(--space-sm, 8px) var(--space-xs, 4px);
        cursor: pointer;
        white-space: nowrap;
        user-select: none;
        color: var(--text-tertiary, #5a5448);
        /* Indicator overlaps the tablist's 1px bottom border. */
        border-block-end: 2px solid transparent;
        margin-block-end: -1px;
        transition: color 0.15s, border-color 0.15s;
      }
      :host(:hover) {
        color: var(--text-secondary, #a09888);
      }
      :host([selected]) {
        color: var(--gold, #c8a855);
        border-block-end-color: var(--gold, #c8a855);
      }
      :host([disabled]) {
        color: var(--text-tertiary, #5a5448);
        opacity: 0.5;
        cursor: not-allowed;
      }
      :host(:focus) {
        outline: none;
      }
      :host(:focus-visible) {
        outline: var(--focus-ring-width, 1px) solid var(--focus-ring-color-subtle, rgba(200, 168, 85, 0.5));
        outline-offset: var(--focus-ring-offset, 1px);
      }
    `
];
let I = kt;
E([
  h({ reflect: !0 })
], I.prototype, "value");
E([
  h({ type: Boolean, reflect: !0 })
], I.prototype, "selected");
E([
  h({ type: Boolean, reflect: !0 })
], I.prototype, "disabled");
const St = class St extends p {
  constructor() {
    super(...arguments), this.value = "";
  }
  updated(t) {
    var e;
    t.has("value") && ((e = this.closest("sema-tabs")) == null || e._requestSync());
  }
  render() {
    return f`<div class="panel" part="base"><slot></slot></div>`;
  }
};
St.styles = [
  p.base,
  _`
      :host {
        display: block;
      }
      /* :host { display } defeats the UA [hidden] rule, so restore it — but not
         for until-found, which must stay laid out (content-visibility: hidden)
         or find-in-page can never match it. */
      :host([hidden]:not([hidden='until-found'])) {
        display: none !important;
      }
      .panel {
        padding-block-start: var(--space-md, 16px);
      }
    `
];
let nt = St;
E([
  h({ reflect: !0 })
], nt.prototype, "value");
customElements.define("sema-tabs", T);
customElements.define("sema-tab", I);
customElements.define("sema-tab-panel", nt);
/**
 * @license
 * Copyright 2018 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const y = (r) => r ?? u;
/**
 * @license
 * Copyright 2017 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const q = { ATTRIBUTE: 1, PROPERTY: 3, BOOLEAN_ATTRIBUTE: 4 }, Ae = (r) => (...t) => ({ _$litDirective$: r, values: t });
class we {
  constructor(t) {
  }
  get _$AU() {
    return this._$AM._$AU;
  }
  _$AT(t, e, s) {
    this._$Ct = t, this._$AM = e, this._$Ci = s;
  }
  _$AS(t, e) {
    return this.update(t, e);
  }
  update(t, e) {
    return this.render(...e);
  }
}
/**
 * @license
 * Copyright 2020 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const Ee = (r) => r.strings === void 0, Ce = {}, ke = (r, t = Ce) => r._$AH = t;
/**
 * @license
 * Copyright 2020 Google LLC
 * SPDX-License-Identifier: BSD-3-Clause
 */
const $t = Ae(class extends we {
  constructor(r) {
    if (super(r), r.type !== q.PROPERTY && r.type !== q.ATTRIBUTE && r.type !== q.BOOLEAN_ATTRIBUTE) throw Error("The `live` directive is not allowed on child or event bindings");
    if (!Ee(r)) throw Error("`live` bindings can only contain a single expression");
  }
  render(r) {
    return r;
  }
  update(r, [t]) {
    if (t === m || t === u) return t;
    const e = r.element, s = r.name;
    if (r.type === q.PROPERTY) {
      if (t === e[s]) return m;
    } else if (r.type === q.BOOLEAN_ATTRIBUTE) {
      if (!!t === e.hasAttribute(s)) return m;
    } else if (r.type === q.ATTRIBUTE && e.getAttribute(s) === t + "") return m;
    return ke(r), t;
  }
}), xt = '.control{width:100%;box-sizing:border-box;font-family:var(--mono, "JetBrains Mono", monospace);font-size:var(--text-md, 13px);line-height:1.5;padding:8px 11px;background:var(--bg-editor, #0a0a0a);color:var(--text-primary, #d8d0c0);border:1px solid var(--border, #1e1e1e);border-radius:var(--radius-sm, 3px);outline:none;caret-color:var(--gold, #c8a855);transition:border-color .15s,box-shadow .15s}.control::placeholder{color:var(--text-tertiary, #5a5448)}.control:focus-visible{border-color:var(--gold-dim, rgba(200, 168, 85, .5));box-shadow:0 0 0 1px var(--gold-dim, rgba(200, 168, 85, .5))}.control:disabled{opacity:.5;cursor:not-allowed}';
var Se = Object.defineProperty, M = (r, t, e, s) => {
  for (var i = void 0, o = r.length - 1, a; o >= 0; o--)
    (a = r[o]) && (i = a(t, e, i) || i);
  return i && Se(t, e, i), i;
};
const lt = class lt extends p {
  constructor() {
    super(...arguments), this.value = "", this.type = "text", this.placeholder = "", this.name = "", this.disabled = !1, this.required = !1, this.readonly = !1, this._internals = this.attachInternals(), this._onInput = (t) => {
      this.value = t.target.value, this._internals.setFormValue(this.value);
    }, this._onChange = () => {
      this.dispatchEvent(new Event("change", { bubbles: !0, composed: !0 }));
    }, this._onKeydown = (t) => {
      var e;
      t.key === "Enter" && !t.isComposing && ((e = this._internals.form) == null || e.requestSubmit());
    };
  }
  // Host aria-* attributes (set e.g. by <sema-field>) must be mirrored onto the
  // inner control, where AT computes name/description — re-render when they change.
  static get observedAttributes() {
    return [...super.observedAttributes, "aria-label", "aria-description", "aria-invalid"];
  }
  attributeChangedCallback(t, e, s) {
    super.attributeChangedCallback(t, e, s), t.startsWith("aria-") && this.requestUpdate();
  }
  updated(t) {
    t.has("value") && this._internals.setFormValue(this.value);
  }
  formResetCallback() {
    this.value = "", this._internals.setFormValue("");
  }
  render() {
    return f`<input
      class="control"
      part="control"
      .value=${$t(this.value)}
      type=${this.type}
      placeholder=${this.placeholder}
      ?disabled=${this.disabled}
      ?required=${this.required}
      ?readonly=${this.readonly}
      maxlength=${y(this.maxlength)}
      aria-label=${this.getAttribute("aria-label") || this.name || "input"}
      aria-description=${y(this.getAttribute("aria-description") ?? void 0)}
      aria-invalid=${y(this.getAttribute("aria-invalid") ?? void 0)}
      @input=${this._onInput}
      @change=${this._onChange}
      @keydown=${this._onKeydown}
    />`;
  }
};
lt.formAssociated = !0, lt.styles = [
  p.base,
  Q(xt),
  _`
      :host {
        display: block;
      }
      :host([readonly]) .control {
        opacity: 0.6;
        cursor: default;
      }
    `
];
let g = lt;
M([
  h()
], g.prototype, "value");
M([
  h()
], g.prototype, "type");
M([
  h()
], g.prototype, "placeholder");
M([
  h()
], g.prototype, "name");
M([
  h({ type: Boolean, reflect: !0 })
], g.prototype, "disabled");
M([
  h({ type: Boolean, reflect: !0 })
], g.prototype, "required");
M([
  h({ type: Boolean, reflect: !0 })
], g.prototype, "readonly");
M([
  h({ type: Number })
], g.prototype, "maxlength");
customElements.define("sema-input", g);
const Pe = ".sema-scroll{scrollbar-width:thin;scrollbar-color:var(--border, #1e1e1e) transparent}";
var Oe = Object.defineProperty, C = (r, t, e, s) => {
  for (var i = void 0, o = r.length - 1, a; o >= 0; o--)
    (a = r[o]) && (i = a(t, e, i) || i);
  return i && Oe(t, e, i), i;
};
const It = typeof CSS < "u" && typeof CSS.supports == "function" && CSS.supports("field-sizing", "content"), ht = class ht extends p {
  constructor() {
    super(...arguments), this.value = "", this.placeholder = "", this.name = "", this.rows = 4, this.disabled = !1, this.required = !1, this.readonly = !1, this.autosize = !1, this._internals = this.attachInternals(), this._onInput = (t) => {
      this.value = t.target.value, this._internals.setFormValue(this.value), this.autosize && !It && this._autoGrow();
    }, this._onChange = () => {
      this.dispatchEvent(new Event("change", { bubbles: !0, composed: !0 }));
    };
  }
  get _ta() {
    var t;
    return ((t = this.shadowRoot) == null ? void 0 : t.querySelector("textarea")) ?? null;
  }
  // Host aria-* attributes (set e.g. by <sema-field>) must be mirrored onto the
  // inner control, where AT computes name/description — re-render when they change.
  static get observedAttributes() {
    return [...super.observedAttributes, "aria-label", "aria-description", "aria-invalid"];
  }
  attributeChangedCallback(t, e, s) {
    super.attributeChangedCallback(t, e, s), t.startsWith("aria-") && this.requestUpdate();
  }
  updated(t) {
    t.has("value") && this._internals.setFormValue(this.value), this.autosize && !It && this._autoGrow();
  }
  formResetCallback() {
    this.value = "", this._internals.setFormValue("");
  }
  /** scrollHeight fallback for browsers without CSS `field-sizing`. */
  _autoGrow() {
    const t = this._ta;
    t && (t.style.height = "auto", t.style.height = `${t.scrollHeight + 2}px`);
  }
  render() {
    return f`<textarea
      class="control sema-scroll"
      part="control"
      .value=${$t(this.value)}
      rows=${this.rows}
      placeholder=${this.placeholder}
      ?disabled=${this.disabled}
      ?required=${this.required}
      ?readonly=${this.readonly}
      maxlength=${y(this.maxlength)}
      aria-label=${this.getAttribute("aria-label") || this.name || "textarea"}
      aria-description=${y(this.getAttribute("aria-description") ?? void 0)}
      aria-invalid=${y(this.getAttribute("aria-invalid") ?? void 0)}
      @input=${this._onInput}
      @change=${this._onChange}
    ></textarea>`;
  }
};
ht.formAssociated = !0, ht.styles = [
  p.base,
  Q(xt),
  Q(Pe),
  _`
      :host {
        display: block;
      }
      .control {
        resize: vertical;
        min-height: 4em;
      }
      :host([autosize]) .control {
        field-sizing: content;
        resize: none;
        overflow: hidden;
        min-height: 3lh;
        max-height: 16lh;
      }
      :host([readonly]) .control {
        opacity: 0.6;
        cursor: default;
        resize: none;
      }
    `
];
let v = ht;
C([
  h()
], v.prototype, "value");
C([
  h()
], v.prototype, "placeholder");
C([
  h()
], v.prototype, "name");
C([
  h({ type: Number })
], v.prototype, "rows");
C([
  h({ type: Boolean, reflect: !0 })
], v.prototype, "disabled");
C([
  h({ type: Boolean, reflect: !0 })
], v.prototype, "required");
C([
  h({ type: Boolean, reflect: !0 })
], v.prototype, "readonly");
C([
  h({ type: Number })
], v.prototype, "maxlength");
C([
  h({ type: Boolean, reflect: !0 })
], v.prototype, "autosize");
customElements.define("sema-textarea", v);
const Te = 'a[href],button:not([disabled]),textarea:not([disabled]),input:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])';
function vt(r) {
  const t = [];
  function e(s) {
    if (s instanceof HTMLElement && s.matches(Te) && !t.includes(s) && t.push(s), s.shadowRoot && s.shadowRoot.mode === "open")
      for (const i of s.shadowRoot.children)
        e(i);
    if (s instanceof HTMLSlotElement)
      for (const i of s.assignedElements({ flatten: !0 }))
        e(i);
    for (const i of s.children)
      e(i);
  }
  return e(r), t;
}
const A = [];
let W = 0;
class Ue {
  constructor(t, e) {
    this._previouslyFocused = null, this._activated = !1, this._attached = !1, this._didLockScroll = !1, this._rafId = null, this._host = t, this._getContainer = e.getContainer, this._isActive = e.isActive, this._lockScroll = e.lockScroll ?? !1, this._initialFocus = e.initialFocus ?? "first-focusable", this._boundKeydown = this._onKeydown.bind(this), t.addController(this);
  }
  hostConnected() {
    this._isActive(this._host) && (this._activate(), this._rafId = requestAnimationFrame(() => {
      this._rafId = null, this._activated && this._host.isConnected && this._focusFirstTabbable();
    }));
  }
  hostUpdated() {
    const t = this._isActive(this._host);
    t && !this._activated && (this._activate(), this._focusFirstTabbable()), !t && this._activated && this._deactivate();
  }
  hostDisconnected() {
    this._cancelPendingFocus(), this._activated && this._deactivate();
  }
  _activate() {
    this._activated || (this._activated = !0, this._previouslyFocused = document.activeElement, A.length > 0 && A[A.length - 1]._detach(), A.push(this), this._getContainer(this._host) && this._attach(), this._lockScroll && this._lockBodyScroll());
  }
  _deactivate() {
    if (!this._activated) return;
    this._activated = !1, this._cancelPendingFocus(), this._detach();
    const t = A.indexOf(this);
    t !== -1 && A.splice(t, 1), A.length > 0 && A[A.length - 1]._attach(), this._didLockScroll && this._unlockBodyScroll();
    const e = this._previouslyFocused;
    e instanceof HTMLElement && document.activeElement !== e && e.focus({ preventScroll: !0 });
  }
  _attach() {
    if (this._attached) return;
    const t = this._getContainer(this._host);
    t && (t.addEventListener("keydown", this._boundKeydown), this._attached = !0);
  }
  _detach() {
    if (!this._attached) return;
    const t = this._getContainer(this._host);
    t == null || t.removeEventListener("keydown", this._boundKeydown), this._attached = !1;
  }
  _cancelPendingFocus() {
    this._rafId !== null && (cancelAnimationFrame(this._rafId), this._rafId = null);
  }
  _focusFirstTabbable() {
    const t = this._getContainer(this._host);
    if (!t) return;
    if (this._initialFocus === "autofocus") {
      const s = t.querySelector("[autofocus]");
      if (s) {
        s.focus();
        return;
      }
    }
    const e = vt(t);
    e.length > 0 ? e[0].focus() : t.focus();
  }
  _onKeydown(t) {
    if (t.key !== "Tab") return;
    const e = this._getContainer(this._host);
    if (!e) return;
    const s = vt(e);
    if (s.length === 0) {
      t.preventDefault();
      return;
    }
    const i = t.composedPath(), o = s.findIndex((l) => i.includes(l));
    if (o === -1) return;
    const a = s[0], n = s[s.length - 1];
    t.shiftKey ? s[o] === a && (t.preventDefault(), n.focus()) : s[o] === n && (t.preventDefault(), a.focus());
  }
  _lockBodyScroll() {
    W === 0 && (document.body.style.overflow = "hidden"), W++, this._didLockScroll = !0;
  }
  _unlockBodyScroll() {
    this._didLockScroll && (W = Math.max(0, W - 1), W === 0 && (document.body.style.overflow = ""), this._didLockScroll = !1);
  }
}
var Re = Object.defineProperty, et = (r, t, e, s) => {
  for (var i = void 0, o = r.length - 1, a; o >= 0; o--)
    (a = r[o]) && (i = a(t, e, i) || i);
  return i && Re(t, e, i), i;
};
const Pt = class Pt extends p {
  constructor() {
    super(...arguments), this.open = !1, this.placement = "bottom-start", this.openOn = "click", this.modal = !1, this._triggerEl = null, this._focusTrap = new Ue(this, {
      getContainer: () => this._panel,
      isActive: () => this.open && this.modal
    }), this._onDocPointer = (t) => {
      t.composedPath().includes(this) || this.hide(!1);
    }, this._onTriggerClick = () => {
      this.openOn === "click" && this.toggle();
    }, this._onPointerEnter = () => {
      this.openOn === "hover" && this.show();
    }, this._onPointerLeave = () => {
      this.openOn === "hover" && this.hide(!1);
    }, this._onKeydown = (t) => {
      this.open && (t.key === "Escape" ? (t.stopPropagation(), t.preventDefault(), this.hide(!0)) : t.key === "Tab" && !this.modal && this.hide(!0));
    }, this._onFocusOut = (t) => {
      if (this.modal || !this.open) return;
      const e = t.relatedTarget;
      e && this.contains(e) || this.hide(!1);
    }, this._onSelect = () => this.hide(!0);
  }
  disconnectedCallback() {
    var t;
    super.disconnectedCallback(), document.removeEventListener("pointerdown", this._onDocPointer, !0), this.open && ((t = this._triggerEl) == null || t.setAttribute("aria-expanded", "false"), this.open = !1), this._triggerEl = null;
  }
  get _trigger() {
    return this.querySelector('[slot="trigger"]');
  }
  show() {
    if (this.open) return;
    this._triggerEl = this._trigger, this.open = !0;
    const t = this._triggerEl;
    t && (t.setAttribute("aria-expanded", "true"), t.setAttribute(
      "aria-haspopup",
      this.querySelector("sema-menu") ? "menu" : this.modal ? "dialog" : "true"
    )), document.addEventListener("pointerdown", this._onDocPointer, !0), this.dispatchEvent(new CustomEvent("sema-open", { bubbles: !0, composed: !0 })), this.updateComplete.then(() => {
      var s;
      if (!this.open) return;
      const e = this.querySelector("sema-menu");
      e != null && e.focusFirst ? e.focusFirst() : (s = vt(this._panel)[0]) == null || s.focus();
    });
  }
  /** Close the popover. By default returns focus to the trigger (Esc/Tab/select). */
  hide(t = !0) {
    if (!this.open) return;
    this.open = !1, document.removeEventListener("pointerdown", this._onDocPointer, !0);
    const e = this._trigger ?? this._triggerEl;
    e == null || e.setAttribute("aria-expanded", "false"), t && (e == null || e.focus({ preventScroll: !0 })), this.dispatchEvent(new CustomEvent("sema-close", { bubbles: !0, composed: !0 }));
  }
  toggle() {
    this.open ? this.hide() : this.show();
  }
  // child menu chose an item
  render() {
    return f`
      <span
        class="trigger"
        part="trigger"
        @click=${this._onTriggerClick}
        @pointerenter=${this._onPointerEnter}
        @pointerleave=${this._onPointerLeave}
        @keydown=${this._onKeydown}
      >
        <slot name="trigger"></slot>
      </span>
      <div
        class="panel"
        part="panel"
        role="presentation"
        ?hidden=${!this.open}
        @keydown=${this._onKeydown}
        @focusout=${this._onFocusOut}
        @sema-select=${this._onSelect}
        @pointerenter=${this._onPointerEnter}
        @pointerleave=${this._onPointerLeave}
      >
        <slot></slot>
      </div>
    `;
  }
};
Pt.styles = [
  p.base,
  _`
      :host {
        display: inline-block;
        position: relative;
      }
      .panel {
        position: absolute;
        z-index: 300;
        min-width: max-content;
        background: var(--bg-elevated, #141414);
        border: 1px solid var(--border, #1e1e1e);
        border-radius: var(--radius-md, 4px);
        padding: var(--space-xs, 4px);
        box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
      }
      .panel[hidden] {
        display: none;
      }
      :host([placement='bottom-start']) .panel {
        top: calc(100% + 4px);
        left: 0;
      }
      :host([placement='bottom-end']) .panel {
        top: calc(100% + 4px);
        right: 0;
      }
      :host([placement='top-start']) .panel {
        bottom: calc(100% + 4px);
        left: 0;
      }
      :host([placement='top-end']) .panel {
        bottom: calc(100% + 4px);
        right: 0;
      }
      :host([placement='left']) .panel {
        right: calc(100% + 4px);
        top: 0;
      }
      :host([placement='right']) .panel {
        left: calc(100% + 4px);
        top: 0;
      }
    `
];
let U = Pt;
et([
  h({ type: Boolean, reflect: !0 })
], U.prototype, "open");
et([
  h({ reflect: !0 })
], U.prototype, "placement");
et([
  h({ attribute: "open-on" })
], U.prototype, "openOn");
et([
  h({ type: Boolean, reflect: !0 })
], U.prototype, "modal");
et([
  Yt(".panel")
], U.prototype, "_panel");
customElements.define("sema-popover", U);
var Me = Object.defineProperty, k = (r, t, e, s) => {
  for (var i = void 0, o = r.length - 1, a; o >= 0; o--)
    (a = r[o]) && (i = a(t, e, i) || i);
  return i && Me(t, e, i), i;
}, w;
const $ = (w = class extends p {
  constructor() {
    super(...arguments), this.value = "", this.name = "", this.placeholder = "Select…", this.disabled = !1, this.required = !1, this.native = !1, this._entries = [], this._open = !1, this._listboxId = `sema-listbox-${++w._uid}`, this._internals = this.attachInternals(), this._onOpen = () => {
      this._open = !0, requestAnimationFrame(() => {
        var e, s;
        if (!this._open) return;
        (s = ((e = this.shadowRoot) == null ? void 0 : e.querySelector(
          `.option[data-value="${CSS.escape(this.value)}"]:not([disabled])`
        )) ?? this._enabledOptions()[0]) == null || s.focus();
      });
    }, this._sync = () => {
      const t = [];
      for (const e of Array.from(this.children))
        e instanceof HTMLOptGroupElement ? t.push({
          label: e.label,
          options: Array.from(e.querySelectorAll("option")).map((s) => this._readOption(s))
        }) : e instanceof HTMLOptionElement && t.push(this._readOption(e));
      this._entries = t, this.value || (this.value = this._firstValue()), this._internals.setFormValue(this.value), this._syncValidity();
    }, this._onTriggerKeydown = (t) => {
      var e;
      (t.key === "ArrowDown" || t.key === "ArrowUp") && (t.preventDefault(), (e = this._pop) == null || e.show());
    }, this._onListKeydown = (t) => {
      var o;
      const e = this._enabledOptions();
      if (e.length === 0) return;
      const s = (o = this.shadowRoot) == null ? void 0 : o.activeElement, i = s ? e.indexOf(s) : -1;
      switch (t.key) {
        case "ArrowDown":
          t.preventDefault(), e[(i + 1 + e.length) % e.length].focus();
          break;
        case "ArrowUp":
          t.preventDefault(), e[(i - 1 + e.length) % e.length].focus();
          break;
        case "Home":
          t.preventDefault(), e[0].focus();
          break;
        case "End":
          t.preventDefault(), e[e.length - 1].focus();
          break;
        case "Enter":
        case " ":
          t.preventDefault(), (s ?? e[0]).click();
          break;
      }
    }, this._onNativeChange = (t) => {
      this.value = t.target.value, this._internals.setFormValue(this.value), this.dispatchEvent(new Event("change", { bubbles: !0, composed: !0 }));
    };
  }
  firstUpdated() {
    this._sync();
  }
  // Host aria-* attributes (set e.g. by <sema-field>) must be mirrored onto the
  // inner control, where AT computes name/description — re-render when they change.
  static get observedAttributes() {
    return [...super.observedAttributes, "aria-label", "aria-description", "aria-invalid"];
  }
  attributeChangedCallback(t, e, s) {
    super.attributeChangedCallback(t, e, s), t.startsWith("aria-") && this.requestUpdate();
  }
  updated(t) {
    var e;
    if (t.has("value") && this._internals.setFormValue(this.value), (t.has("value") || t.has("required")) && this._syncValidity(), this.native) {
      const s = (e = this.shadowRoot) == null ? void 0 : e.querySelector("select");
      s && s.value !== this.value && (s.value = this.value);
    }
  }
  formResetCallback() {
    this.value = this._firstValue(), this._internals.setFormValue(this.value), this._syncValidity();
  }
  _syncValidity() {
    var t;
    if (this.required && !this.value) {
      const e = ((t = this.shadowRoot) == null ? void 0 : t.querySelector(this.native ? "select" : ".trigger")) ?? void 0;
      this._internals.setValidity({ valueMissing: !0 }, "Please select an option", e);
    } else
      this._internals.setValidity({});
  }
  _flat() {
    return this._entries.flatMap((t) => "options" in t ? t.options : [t]);
  }
  _firstValue() {
    var t;
    return ((t = this._flat()[0]) == null ? void 0 : t.value) ?? "";
  }
  _labelFor(t) {
    var e;
    return ((e = this._flat().find((s) => s.value === t)) == null ? void 0 : e.label) ?? null;
  }
  _readOption(t) {
    return { value: t.value, label: t.textContent ?? "", disabled: t.disabled };
  }
  _select(t) {
    var e;
    this.value = t, this._internals.setFormValue(t), this._syncValidity(), (e = this._pop) == null || e.hide(), this.dispatchEvent(new Event("change", { bubbles: !0, composed: !0 }));
  }
  _enabledOptions() {
    var t;
    return Array.from(((t = this.shadowRoot) == null ? void 0 : t.querySelectorAll(".option:not([disabled])")) ?? []);
  }
  _optionTpl(t) {
    const e = t.value === this.value;
    return f`<button
      class="option"
      role="option"
      type="button"
      tabindex="-1"
      data-value=${t.value}
      aria-selected=${String(e)}
      ?disabled=${t.disabled}
      @click=${() => this._select(t.value)}
    >
      <span class="check">${e ? "✓" : ""}</span><span>${t.label}</span>
    </button>`;
  }
  _renderCustom() {
    const t = this._labelFor(this.value);
    return f`
      <sema-popover
        placement="bottom-start"
        @sema-open=${this._onOpen}
        @sema-close=${() => this._open = !1}
      >
        <button
          slot="trigger"
          class="control trigger"
          part="control"
          type="button"
          ?disabled=${this.disabled}
          aria-haspopup="listbox"
          aria-expanded=${String(this._open)}
          aria-controls=${this._listboxId}
          aria-label=${this.getAttribute("aria-label") || this.name || "select"}
          aria-description=${y(this.getAttribute("aria-description") ?? void 0)}
          aria-invalid=${y(this.getAttribute("aria-invalid") ?? void 0)}
          @keydown=${this._onTriggerKeydown}
        >
          <span class="label ${t === null ? "placeholder" : ""}">${t ?? this.placeholder}</span>
          <span class="chevron" aria-hidden="true">▾</span>
        </button>
        <div
          class="listbox"
          id=${this._listboxId}
          role="listbox"
          aria-label=${this.getAttribute("aria-label") || this.name || "options"}
          @keydown=${this._onListKeydown}
        >
          ${this._entries.map(
      (e) => "options" in e ? f`<div class="group-label" role="presentation">${e.label}</div>
                  ${e.options.map((s) => this._optionTpl(s))}` : this._optionTpl(e)
    )}
        </div>
      </sema-popover>
      <slot @slotchange=${this._sync}></slot>
    `;
  }
  _renderNative() {
    return f`
      <select
        class="control"
        part="control"
        .value=${$t(this.value)}
        ?disabled=${this.disabled}
        ?required=${this.required}
        aria-label=${this.getAttribute("aria-label") || this.name || "select"}
        aria-description=${y(this.getAttribute("aria-description") ?? void 0)}
        aria-invalid=${y(this.getAttribute("aria-invalid") ?? void 0)}
        @change=${this._onNativeChange}
      >
        ${this._entries.map(
      (t) => "options" in t ? f`<optgroup label=${t.label}>
                ${t.options.map((e) => f`<option value=${e.value} ?disabled=${e.disabled}>${e.label}</option>`)}
              </optgroup>` : f`<option value=${t.value} ?disabled=${t.disabled}>${t.label}</option>`
    )}
      </select>
      <slot @slotchange=${this._sync}></slot>
    `;
  }
  render() {
    return this.native ? this._renderNative() : this._renderCustom();
  }
}, w.formAssociated = !0, w.styles = [
  p.base,
  Q(xt),
  _`
      :host {
        display: block;
      }
      /* custom trigger */
      .trigger {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 8px;
        cursor: pointer;
        text-align: left;
      }
      .trigger .label {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }
      .placeholder {
        color: var(--text-tertiary, #5a5448);
      }
      .chevron {
        flex-shrink: 0;
        font-size: 0.7em;
        color: var(--text-tertiary, #5a5448);
        transition: transform 0.15s;
      }
      .trigger[aria-expanded='true'] .chevron {
        transform: rotate(180deg);
      }
      /* custom listbox */
      .listbox {
        display: flex;
        flex-direction: column;
        min-width: 160px;
        max-height: 256px;
        overflow-y: auto;
        scrollbar-width: thin;
        scrollbar-color: var(--border, #1e1e1e) transparent;
      }
      .group-label {
        font-family: var(--mono, 'JetBrains Mono', monospace);
        font-size: var(--text-xxs, 10px);
        text-transform: uppercase;
        letter-spacing: 0.06em;
        color: var(--text-tertiary, #5a5448);
        padding: 6px 11px 3px;
      }
      .option {
        display: flex;
        align-items: center;
        gap: 8px;
        width: 100%;
        font-family: var(--mono, 'JetBrains Mono', monospace);
        font-size: var(--text-sm, 12px);
        text-align: left;
        padding: 6px 11px;
        border: none;
        border-radius: var(--radius-sm, 3px);
        background: transparent;
        color: var(--text-primary, #d8d0c0);
        cursor: pointer;
        white-space: nowrap;
      }
      .option:hover:not([disabled]),
      .option:focus-visible {
        background: var(--gold-glow, rgba(200, 168, 85, 0.08));
        color: var(--gold, #c8a855);
        outline: none;
      }
      .option:focus-visible {
        box-shadow: inset 0 0 0 1px var(--gold-dim, rgba(200, 168, 85, 0.5));
      }
      .option[aria-selected='true'] {
        color: var(--gold, #c8a855);
      }
      .option[disabled] {
        color: var(--text-tertiary, #5a5448);
        cursor: not-allowed;
      }
      .check {
        width: 1em;
        text-align: center;
      }
      select.control {
        cursor: pointer;
      }
      slot {
        display: none;
      }
    `
], w._uid = 0, w);
k([
  h()
], $.prototype, "value");
k([
  h()
], $.prototype, "name");
k([
  h()
], $.prototype, "placeholder");
k([
  h({ type: Boolean, reflect: !0 })
], $.prototype, "disabled");
k([
  h({ type: Boolean, reflect: !0 })
], $.prototype, "required");
k([
  h({ type: Boolean, reflect: !0 })
], $.prototype, "native");
k([
  Qt()
], $.prototype, "_entries");
k([
  Qt()
], $.prototype, "_open");
k([
  Yt("sema-popover")
], $.prototype, "_pop");
let ze = $;
customElements.define("sema-select", ze);
var Be = Object.defineProperty, At = (r, t, e, s) => {
  for (var i = void 0, o = r.length - 1, a; o >= 0; o--)
    (a = r[o]) && (i = a(t, e, i) || i);
  return i && Be(t, e, i), i;
};
const Ot = class Ot extends p {
  constructor() {
    super(...arguments), this.label = "", this.hint = "", this.error = "", this._control = null, this._onSlotChange = (t) => {
      var i;
      const e = t.target.assignedElements({ flatten: !0 }), s = e.find((o) => o.matches("input, textarea, select, sema-input, sema-textarea, sema-select")) ?? e[0] ?? null;
      if (s !== this._control) {
        for (const o of ["aria-label", "aria-description", "aria-invalid"]) (i = this._control) == null || i.removeAttribute(o);
        this._control = s;
      }
      this._applyA11y();
    };
  }
  updated(t) {
    (t.has("label") || t.has("hint") || t.has("error")) && this._applyA11y();
  }
  // Shadow boundaries rule out IDREF associations (aria-labelledby/-describedby),
  // so mirror label/hint/error onto the control as plain string aria attributes.
  _applyA11y() {
    const t = this._control;
    if (!t) return;
    this.label ? t.setAttribute("aria-label", this.label) : t.removeAttribute("aria-label");
    const e = this.error || this.hint;
    e ? t.setAttribute("aria-description", e) : t.removeAttribute("aria-description"), this.error ? t.setAttribute("aria-invalid", "true") : t.removeAttribute("aria-invalid");
  }
  render() {
    const t = this.error || this.hint;
    return f`
      <label class="field" part="field">
        ${this.label ? f`<span class="label" part="label">${this.label}</span>` : u}
        <slot @slotchange=${this._onSlotChange}></slot>
        ${t ? f`<span class="msg ${this.error ? "error" : ""}" part="message">${t}</span>` : u}
      </label>
    `;
  }
};
Ot.styles = [
  p.base,
  _`
      :host {
        display: block;
      }
      .field {
        display: flex;
        flex-direction: column;
        gap: var(--space-xs, 4px);
      }
      .label {
        font-family: var(--mono, 'JetBrains Mono', monospace);
        font-size: var(--text-xs, 11px);
        letter-spacing: 0.04em;
        color: var(--text-secondary, #a09888);
      }
      .msg {
        font-family: var(--mono, 'JetBrains Mono', monospace);
        font-size: var(--text-xxs, 10px);
        color: var(--text-tertiary, #5a5448);
      }
      .msg.error {
        color: var(--error, #c85555);
      }
    `
];
let V = Ot;
At([
  h()
], V.prototype, "label");
At([
  h()
], V.prototype, "hint");
At([
  h()
], V.prototype, "error");
customElements.define("sema-field", V);
var He = Object.defineProperty, st = (r, t, e, s) => {
  for (var i = void 0, o = r.length - 1, a; o >= 0; o--)
    (a = r[o]) && (i = a(t, e, i) || i);
  return i && He(t, e, i), i;
};
const ct = class ct extends p {
  constructor() {
    super(...arguments), this.variant = "primary", this.size = "md", this.disabled = !1, this.danger = !1;
  }
  render() {
    const t = this.getAttribute("aria-label");
    return f`
      <button class="button" type="button" ?disabled=${this.disabled} part="button"
              aria-label=${t || u}>
        <slot></slot>
        ${this.shortcut ? f`<span class="shortcut">${this.shortcut}</span>` : ""}
      </button>
    `;
  }
};
ct.shadowRootOptions = {
  ...F.shadowRootOptions,
  delegatesFocus: !0
}, ct.styles = [
  p.base,
  _`
      :host {
        display: inline-block;
        vertical-align: middle;
      }
      :host([variant="icon"]) {
        display: inline-flex;
      }

      .button {
        font-family: var(--mono, 'JetBrains Mono', monospace);
        cursor: pointer;
        transition: color 0.15s, background 0.15s, border-color 0.15s, opacity 0.15s;
        line-height: 1;
        white-space: nowrap;
        text-decoration: none;
        border: none;
        background: transparent;
        color: inherit;
        -webkit-font-smoothing: antialiased;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 0.4em;
      }
      .button::-moz-focus-inner { border: 0; }
      .button:focus { outline: none; }
      .button:focus-visible {
        outline: var(--focus-ring-width, 1px) solid var(--focus-ring-color-subtle, rgba(200, 168, 85, 0.5));
        outline-offset: var(--focus-ring-offset, 1px);
        border-radius: 3px;
      }
      .button:disabled {
        opacity: 0.4;
        cursor: not-allowed;
        pointer-events: none;
      }

      /* ── primary ── */
      :host([variant="primary"]) .button {
        background: var(--gold, #c8a855);
        color: var(--bg, #0c0c0c);
        padding: 14px 35px;
        border-radius: 6px;
        font-size: var(--text-lg, 14px);
        font-weight: 500;
        letter-spacing: 0.04em;
      }
      :host([variant="primary"]) .button:hover:not(:disabled) { background: var(--gold-bright, #e3c878); opacity: 1; }
      :host([variant="primary"]) .button:active:not(:disabled) { opacity: 0.7; }
      :host([variant="primary"]) .button:focus-visible {
        outline: 2px solid var(--text-primary, #d8d0c0);
        outline-offset: 3px;
        border-radius: 6px;
      }

      /* ── secondary ── */
      :host([variant="secondary"]) .button {
        background: transparent;
        color: var(--text-primary, #d8d0c0);
        padding: 14px 35px;
        border-radius: 6px;
        font-size: var(--text-lg, 14px);
        letter-spacing: 0.04em;
        border: 1px solid var(--border, #1e1e1e);
      }
      :host([variant="secondary"]) .button:hover:not(:disabled) {
        border-color: var(--text-tertiary, #5a5448);
        color: var(--gold, #c8a855);
      }

      /* ── ghost ── */
      :host([variant="ghost"]) .button {
        background: transparent;
        color: var(--text-tertiary, #5a5448);
        padding: 14px 35px;
        border-radius: 6px;
        font-size: var(--text-lg, 14px);
        letter-spacing: 0.04em;
      }
      :host([variant="ghost"]) .button:hover:not(:disabled) { color: var(--text-primary, #d8d0c0); }

      /* ── icon ── */
      :host([variant="icon"]) {
        width: 32px;
        height: 32px;
      }
      :host([variant="icon"]) .button {
        width: 32px;
        height: 32px;
        border-radius: 4px;
        color: var(--text-tertiary, #5a5448);
        font-size: var(--text-md, 13px);
        padding: 0;
      }
      :host([variant="icon"]) .button:hover:not(:disabled) {
        color: var(--gold, #c8a855);
        background: var(--gold-glow, rgba(200, 168, 85, 0.08));
      }

      /* ── pill ── */
      :host([variant="pill"]) .button {
        background: transparent;
        color: var(--gold, #c8a855);
        padding: 6px 16px;
        border: 1px solid var(--gold-dim, rgba(200, 168, 85, 0.5));
        border-radius: 20px;
        font-size: var(--text-sm, 12px);
        letter-spacing: 0.03em;
      }
      :host([variant="pill"]) .button:hover:not(:disabled) {
        background: var(--gold-glow, rgba(200, 168, 85, 0.08));
        border-color: var(--gold, #c8a855);
      }

      /* ── run ── */
      :host([variant="run"]) .button {
        background: var(--gold, #c8a855);
        color: var(--bg, #0c0c0c);
        padding: 5px 14px;
        border-radius: 3px;
        font-size: var(--text-xs, 11px);
        letter-spacing: 0.05em;
      }
      :host([variant="run"]) .button:hover:not(:disabled) { opacity: 0.85; }
      :host([variant="run"]) .button:active:not(:disabled) { opacity: 0.7; }
      :host([variant="run"]) .button:focus-visible {
        outline: 2px solid var(--text-primary, #d8d0c0);
        outline-offset: 3px;
        border-radius: 3px;
      }

      /* shortcut badge inside run */
      .shortcut {
        font-family: system-ui, -apple-system, sans-serif;
        font-size: var(--text-xxs, 10px);
        opacity: 0.7;
        margin-left: 8px;
        background: rgba(0, 0, 0, 0.2);
        font-weight: bold;
        line-height: 1;
        padding: 2px 6px;
        border-radius: 4px;
        pointer-events: none;
        white-space: nowrap;
      }

      /* ── debug ── */
      :host([variant="debug"]) .button {
        width: 28px;
        height: 24px;
        border-radius: 3px;
        border: 1px solid var(--border, #1e1e1e);
        color: var(--text-secondary, #a09888);
        font-family: system-ui, -apple-system, sans-serif;
        font-size: var(--text-md, 13px);
        background: transparent;
      }
      :host([variant="debug"]) .button:hover:not(:disabled) {
        background: var(--gold-glow, rgba(200, 168, 85, 0.08));
        color: var(--gold, #c8a855);
        border-color: var(--gold-dim, rgba(200, 168, 85, 0.5));
      }
      :host([variant="debug"]) .button:focus-visible {
        outline-offset: 0;
        border-radius: 3px;
      }
      :host([variant="debug"][danger]) .button:hover:not(:disabled) {
        color: var(--error, #c85555);
        border-color: var(--error, #c85555);
      }

      /* ── action ── */
      :host([variant="action"]) .button {
        width: 24px;
        height: 24px;
        border-radius: 3px;
        background: var(--bg-elevated, #141414);
        color: var(--text-tertiary, #5a5448);
        font-size: var(--text-xxs, 10px);
      }
      :host([variant="action"]) .button:hover:not(:disabled) {
        color: var(--gold, #c8a855);
        background: var(--gold-glow, rgba(200, 168, 85, 0.08));
      }
      :host([variant="action"][danger]) .button:hover:not(:disabled) {
        color: var(--error, #c85555);
      }

      /* ── slot content layout ── */
      .button ::slotted(svg) {
        width: 16px;
        height: 16px;
        flex-shrink: 0;
      }
      :host([variant="action"]) .button ::slotted(svg) {
        width: 13px;
        height: 13px;
      }

      /* size=sm — compact toolbar metrics; placed last so it overrides the
         form-scale text variants (secondary/ghost/primary) on equal specificity. */
      :host([size="sm"]) .button {
        height: var(--control-height-sm, 22px);
        box-sizing: border-box;
        padding: 0 14px;
        font-size: var(--text-xs, 11px);
        border-radius: var(--radius-sm, 3px);
      }
      /* icon is a fixed square — sm shrinks the box to the shared control height. */
      :host([size="sm"][variant="icon"]) {
        width: var(--control-height-sm, 22px);
        height: var(--control-height-sm, 22px);
      }
      :host([size="sm"][variant="icon"]) .button {
        width: var(--control-height-sm, 22px);
        height: var(--control-height-sm, 22px);
        padding: 0;
      }
    `
];
let R = ct;
st([
  h({ reflect: !0 })
], R.prototype, "variant");
st([
  h({ reflect: !0 })
], R.prototype, "size");
st([
  h({ type: Boolean, reflect: !0 })
], R.prototype, "disabled");
st([
  h({ type: Boolean, reflect: !0 })
], R.prototype, "danger");
st([
  h({ attribute: "shortcut" })
], R.prototype, "shortcut");
customElements.define("sema-button", R);
var Le = Object.defineProperty, wt = (r, t, e, s) => {
  for (var i = void 0, o = r.length - 1, a; o >= 0; o--)
    (a = r[o]) && (i = a(t, e, i) || i);
  return i && Le(t, e, i), i;
};
const Tt = class Tt extends p {
  constructor() {
    super(...arguments), this.variant = "neutral", this.pill = !1, this.dot = !1;
  }
  render() {
    return f`
      <span class="badge" part="badge">
        ${this.dot ? f`<span class="dot" aria-hidden="true"></span>` : ""}
        <slot></slot>
      </span>
    `;
  }
};
Tt.styles = [
  p.base,
  _`
      :host {
        display: inline-flex;
        vertical-align: middle;

        /* Per-variant palette, overridden by :host([variant=…]) below. */
        --_badge-bg: transparent;
        --_badge-border: var(--border, #1e1e1e);
        --_badge-fg: var(--text-secondary, #a09888);
      }

      :host([variant='gold']) {
        --_badge-bg: var(--gold-glow, rgba(200, 168, 85, 0.08));
        --_badge-border: var(--gold-dim, rgba(200, 168, 85, 0.5));
        --_badge-fg: var(--gold, #c8a855);
      }
      :host([variant='success']) {
        --_badge-bg: color-mix(in srgb, var(--success, #6a9955) 12%, transparent);
        --_badge-border: color-mix(in srgb, var(--success, #6a9955) 40%, transparent);
        --_badge-fg: var(--success, #6a9955);
      }
      :host([variant='error']) {
        --_badge-bg: var(--error-bg, rgba(200, 85, 85, 0.06));
        --_badge-border: color-mix(in srgb, var(--error, #c85555) 40%, transparent);
        --_badge-fg: var(--error, #c85555);
      }

      .badge {
        display: inline-flex;
        align-items: center;
        gap: 0.35em;
        font-family: var(--mono, 'JetBrains Mono', monospace);
        font-size: var(--text-xxs, 10px);
        line-height: 1;
        letter-spacing: 0.04em;
        white-space: nowrap;
        padding: 4px 7px;
        border: 1px solid var(--_badge-border);
        border-radius: var(--radius-sm, 3px);
        background: var(--_badge-bg);
        color: var(--_badge-fg);
      }

      :host([pill]) .badge {
        padding: 4px 11px;
        border-radius: var(--radius-pill, 20px);
      }

      .dot {
        width: 0.4em;
        height: 0.4em;
        border-radius: var(--radius-full, 50%);
        background: currentColor;
        flex-shrink: 0;
      }
    `
];
let j = Tt;
wt([
  h({ reflect: !0 })
], j.prototype, "variant");
wt([
  h({ type: Boolean, reflect: !0 })
], j.prototype, "pill");
wt([
  h({ type: Boolean, reflect: !0 })
], j.prototype, "dot");
customElements.define("sema-badge", j);
