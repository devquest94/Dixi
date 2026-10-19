import sqlite3, sys, json, os

db = r"D:\Codigo\Dixi\.tmp-audit\db\flow.sqlite"
out = r"D:\Codigo\Dixi\.tmp-audit\schema.txt"

con = sqlite3.connect(db)
con.row_factory = sqlite3.Row
cur = con.cursor()

lines = []
def w(s=""):
    lines.append(s)

rows = cur.execute(
    "SELECT type, name, tbl_name, sql FROM sqlite_master "
    "WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name"
).fetchall()

tables = [r for r in rows if r["type"] == "table"]
indexes = [r for r in rows if r["type"] == "index"]
views = [r for r in rows if r["type"] == "view"]
triggers = [r for r in rows if r["type"] == "trigger"]

w("=== FLOW.SQLITE SCHEMA (structure only, no row data) ===")
w(f"tables={len(tables)} indexes={len(indexes)} views={len(views)} triggers={len(triggers)}")
w("")

# column-level detail with row counts (counts only, never content)
w("=== TABLES + COLUMNS + ROW COUNTS ===")
for t in tables:
    name = t["name"]
    try:
        cnt = cur.execute(f'SELECT COUNT(*) FROM "{name}"').fetchone()[0]
    except Exception as e:
        cnt = f"ERR:{e}"
    w("")
    w(f"-- TABLE {name}  (rows={cnt})")
    try:
        cols = cur.execute(f'PRAGMA table_info("{name}")').fetchall()
        for c in cols:
            cid, cname, ctype, notnull, dflt, pk = c
            w(f"   {cname:<34} {ctype:<16} notnull={notnull} pk={pk} default={dflt}")
    except Exception as e:
        w(f"   [col error] {e}")

w("")
w("=== VIEWS ===")
for v in views:
    w(f"-- VIEW {v['name']}")
    w(f"   {v['sql']}")

w("")
w("=== TRIGGERS ===")
for t in triggers:
    w(f"-- TRIGGER {t['name']} on {t['tbl_name']}")
    w(f"   {(t['sql'] or '')[:400]}")

w("")
w("=== INDEXES (index -> table, columns, unique) ===")
for i in indexes:
    try:
        info = cur.execute(f'PRAGMA index_info("{i["name"]}")').fetchall()
        cols = ", ".join(f"{r['name']}" for r in info)
    except Exception:
        cols = "?"
    uniq = ""
    try:
        for r in cur.execute(f'PRAGMA index_list("{i["tbl_name"]}")').fetchall():
            if r["name"] == i["name"]:
                uniq = " UNIQUE" if r["unique"] else ""
    except Exception:
        pass
    w(f"-- INDEX {i['name']} on {i['tbl_name']}{uniq}: ({cols})")

# FTS tables are virtual; note them
w("")
w("=== VIRTUAL / FTS TABLES ===")
for t in tables:
    try:
        sql = (t["sql"] or "").upper()
        if "VIRTUAL TABLE" in sql:
            w(f"-- {t['name']}")
            w(f"   {t['sql'][:600]}")
    except Exception:
        pass

con.close()

with open(out, "w", encoding="utf-8", newline="\n") as f:
    f.write("\n".join(lines))

print(f"WROTE {out} lines={len(lines)}")
