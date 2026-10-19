import json, os, re, sys

base = r"C:\Users\DanielSan\.minimax\v2\sessions\2026\10\19\16-08-58-305-session_bXZzXzI5ZmQ0NTY3M2MyMDRkN2ZhNWY1MDFkNGEwOTI1NDU5\reports\tool-outputs"

def load(fn):
    p = os.path.join(base, fn)
    if not os.path.exists(p):
        return None
    parts = []
    with open(p, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except Exception:
                continue
            if row.get("type") == "chunk":
                parts.append(row.get("text", ""))
    return "".join(parts)

targets = {
    "status": "bbd9f9e27c9007735d6e1b5c-3a141b9305b7abe97bcfcd75a73bce4de5403deb7215b09a2882e23d337be457.readable.v1.jsonl",
    "hub":    "78de08f0e2e69074f0740a56-dd04f467c321bcdd052e7a649707ece22f224164608f436b9db4bf55e48f7c1b.readable.v1.jsonl",
}

out = []
for label, fn in targets.items():
    txt = load(fn)
    if txt is None:
        out.append(f"### {label}: artifact missing")
        continue
    out.append(f"\n{'='*70}\n### RENDERER: {label.upper()}\n{'='*70}")
    out.append(f"joined_chars={len(txt)}")
    # try to parse the normalized_result JSON blob
    obj = None
    for marker in ('{"evidence_id"',):
        i = txt.find(marker)
        if i >= 0:
            # find matching end by scanning braces
            depth = 0
            end = -1
            in_str = False
            esc = False
            for j in range(i, len(txt)):
                ch = txt[j]
                if in_str:
                    if esc:
                        esc = False
                    elif ch == "\\":
                        esc = True
                    elif ch == '"':
                        in_str = False
                    continue
                if ch == '"':
                    in_str = True
                elif ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth == 0:
                        end = j + 1
                        break
            if end > 0:
                try:
                    obj = json.loads(txt[i:end])
                except Exception as e:
                    obj = None
            break
    if obj is None:
        out.append("  (could not parse normalized_result; dumping key regex hits)")
        for pat in [r'"total_nodes":\d+', r'"node_name":"[^"]*"', r'"url":"[^"]*"']:
            hits = re.findall(pat, txt)
            out.append(f"  {pat} -> {len(hits)} hits; sample: {hits[:15]}")
        continue

    nr = obj.get("normalized_result", {})
    out.append(f"target.title      = {nr.get('target', {}).get('title')}")
    out.append(f"target.file_path  = {nr.get('target', {}).get('file_path')}")
    out.append(f"capture_window    = {nr.get('capture_window')}")
    comp = nr.get("completeness", {})
    out.append(f"completeness      = {comp.get('status')} {comp.get('conditions')}")
    dom = nr.get("dom", {})
    out.append(f"dom.total_nodes   = {dom.get('total_nodes')}")
    nodes = dom.get("nodes", [])
    # element histogram
    from collections import Counter
    cnt = Counter(n.get("node_name") for n in nodes)
    out.append(f"dom.node_name histogram: {dict(cnt.most_common(25))}")
    # interesting data-* / aria attributes
    attrs = Counter()
    for n in nodes:
        for a in n.get("attribute_names", []):
            attrs[a] += 1
    interesting = {k: v for k, v in attrs.items() if k.startswith("data-") or k.startswith("aria-") or k in ("role", "id", "class", "title", "placeholder", "type", "href", "value")}
    out.append(f"dom.attributes (interesting): {dict(sorted(interesting.items(), key=lambda kv: -kv[1])[:60])}")
    # accessibility
    acc = nr.get("accessibility", {})
    out.append(f"accessibility.total_nodes = {acc.get('total_nodes')}  text_capture={acc.get('text_capture', {}).get('status')}")
    for n in acc.get("nodes", [])[:80]:
        nm = n.get("name")
        rl = n.get("role")
        if nm:
            out.append(f"   AX [{rl}] name={nm!r} desc={(n.get('description') or '')[:60]!r}")
    # scripts metadata
    sc = nr.get("scripts", {})
    out.append(f"scripts.total = {sc.get('total')}")
    for s in sc.get("items", [])[:30]:
        out.append(f"   SCRIPT {s.get('url','')[-90:]} len={s.get('length')} module={s.get('is_module')} src_included={(s.get('source') or {}).get('included')}")
    # resources
    res = nr.get("resources", [])
    out.append(f"resources = {len(res)}")
    for r in res[:40]:
        out.append(f"   RES {r.get('type')} {r.get('url','')[-80:]} size={r.get('content_size')} mime={r.get('mime_type')}")
    # network
    net = nr.get("network", {})
    reqs = net.get("requests", [])
    out.append(f"network.requests = {len(reqs)}")
    for q in reqs[:30]:
        out.append(f"   REQ {q.get('method')} {q.get('status')} {q.get('url','')[:110]}")
    # storage keys only (never values)
    st = nr.get("storage", {})
    out.append(f"storage.local_storage_keys  = {st.get('local_storage_keys')}")
    out.append(f"storage.session_storage_keys= {st.get('session_storage_keys')}")
    out.append(f"storage.indexed_db_names    = {st.get('indexed_db_names')}")
    out.append(f"storage.cache_names         = {st.get('cache_names')}")
    # console
    con = nr.get("console", {})
    out.append(f"console.events = {len(con.get('events', []))}")
    for e in con.get("events", [])[:20]:
        out.append(f"   CONSOLE {e.get('type')}: {(e.get('text_capture') or {}).get('status')}")
    out.append(f"limitations = {nr.get('limitations')}")

with open(r"D:\Codigo\Dixi\.tmp-audit\rea-renderers.txt", "w", encoding="utf-8", newline="\n") as f:
    f.write("\n".join(out))
print("WROTE rea-renderers.txt lines=", len(out))
