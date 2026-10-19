import sqlite3, statistics, json, os

db = r"D:\Codigo\Dixi\.tmp-audit\db\flow.sqlite"
con = sqlite3.connect(db)
cur = con.cursor()

def stats(label, col, where=""):
    q = f'SELECT {col} FROM History WHERE {col} IS NOT NULL {where}'
    vals = [r[0] for r in cur.execute(q).fetchall() if r[0] is not None]
    if not vals:
        print(f"  {label:<26} (no data)")
        return None
    vals_s = sorted(vals)
    def pct(p):
        i = int(round((len(vals_s) - 1) * p))
        return vals_s[i]
    print(f"  {label:<26} n={len(vals):<6} min={min(vals):<12.4g} p50={pct(.5):<12.4g} "
          f"p90={pct(.9):<12.4g} p99={pct(.99):<12.4g} max={max(vals):<12.4g} mean={statistics.mean(vals):.4g}")
    return vals

print("=== HISTORY PERFORMANCE AGGREGATES (numeric only, no transcript content) ===")
print(f"  total rows: {cur.execute('SELECT COUNT(*) FROM History').fetchone()[0]}")
print()
print("-- timing (seconds unless noted) --")
stats("e2eLatency", "e2eLatency")
stats("clientNetworkLatency", "clientNetworkLatency")
stats("duration (audio)", "duration")
stats("speechDuration", "speechDuration")
stats("fallbackAsrDivergence", "fallbackAsrDivergenceScore")
stats("formattingDivergence", "formattingDivergenceScore")
print()
print("-- counts --")
stats("numWords", "numWords")
stats("numWordsCorrected", "numWordsCorrected")
stats("numDictionaryReplacements", "numDictionaryReplacements")
stats("editDistanceToDictated", "editDistanceToDictated")
print()
print("-- derived: e2e latency minus speech duration (post-speech overhead) --")
rows = cur.execute("""SELECT e2eLatency, speechDuration, duration FROM History
                      WHERE e2eLatency IS NOT NULL AND speechDuration IS NOT NULL AND speechDuration > 0""").fetchall()
ov = [r[0] - r[1] for r in rows if r[0] is not None and r[1] is not None]
if ov:
    ov_s = sorted(ov)
    p = lambda q: ov_s[int(round((len(ov_s)-1)*q))]
    print(f"  overhead n={len(ov)} min={min(ov):.3f} p50={p(.5):.3f} p90={p(.9):.3f} p99={p(.99):.3f} max={max(ov):.3f}")
else:
    print("  (no data)")
print()
print("-- words per minute actually dictated --")
wpm = []
for r in cur.execute("SELECT numWords, speechDuration FROM History WHERE numWords IS NOT NULL AND speechDuration > 0").fetchall():
    wpm.append(r[0] / (r[1] / 60.0))
if wpm:
    wpm_s = sorted(wpm)
    p = lambda q: wpm_s[int(round((len(wpm_s)-1)*q))]
    print(f"  WPM n={len(wpm)} min={min(wpm):.1f} p50={p(.5):.1f} p90={p(.9):.1f} p99={p(.99):.1f} max={max(wpm):.1f} mean={statistics.mean(wpm):.1f}")
print()
print("-- language distribution (codes only) --")
for row in cur.execute("SELECT language, COUNT(*) FROM History WHERE language IS NOT NULL GROUP BY language ORDER BY 2 DESC"):
    print(f"  {row[0]:<10} {row[1]}")
print()
print("-- detected language distribution --")
for row in cur.execute("SELECT detectedLanguage, COUNT(*) FROM History WHERE detectedLanguage IS NOT NULL GROUP BY detectedLanguage ORDER BY 2 DESC LIMIT 10"):
    print(f"  {row[0]:<10} {row[1]}")
print()
print("-- target application distribution (top 15) --")
for row in cur.execute("SELECT app, COUNT(*) FROM History WHERE app IS NOT NULL GROUP BY app ORDER BY 2 DESC LIMIT 15"):
    print(f"  {str(row[0])[:40]:<40} {row[1]}")
print()
print("-- platform / status / shareType --")
for col in ("platform", "status", "shareType", "transcriptOrigin", "appVersion"):
    print(f"  [{col}]")
    for row in cur.execute(f"SELECT {col}, COUNT(*) FROM History WHERE {col} IS NOT NULL GROUP BY {col} ORDER BY 2 DESC LIMIT 6"):
        print(f"      {str(row[0])[:36]:<36} {row[1]}")
print()
print("-- fallback usage --")
for col in ("usedFallbackAsr", "usedFallbackFormatting", "hasRevertedAI", "calledExternalAsr", "isArchived"):
    r = cur.execute(f"SELECT SUM(COALESCE({col},0)), COUNT(*) FROM History").fetchone()
    print(f"  {col:<26} true_count={r[0]} / {r[1]}")
print()
print("-- editedTextStatus --")
for row in cur.execute("SELECT editedTextStatus, COUNT(*) FROM History GROUP BY editedTextStatus ORDER BY 2 DESC"):
    print(f"  {str(row[0])[:36]:<36} {row[1]}")
print()
print("=== DICTIONARY aggregates (structure only, no phrases) ===")
print(f"  total rows: {cur.execute('SELECT COUNT(*) FROM Dictionary').fetchone()[0]}")
for row in cur.execute("SELECT source, isSnippet, manualEntry, COUNT(*) FROM Dictionary GROUP BY source, isSnippet, manualEntry ORDER BY 4 DESC"):
    print(f"  source={str(row[0])[:24]:<24} isSnippet={row[1]} manual={row[2]}  n={row[3]}")
print(f"  starred: {cur.execute('SELECT COUNT(*) FROM Dictionary WHERE isStarred=1').fetchone()[0]}")
print(f"  with replacement: {cur.execute('SELECT COUNT(*) FROM Dictionary WHERE replacement IS NOT NULL').fetchone()[0]}")
print(f"  with replacementHtml: {cur.execute('SELECT COUNT(*) FROM Dictionary WHERE replacementHtml IS NOT NULL').fetchone()[0]}")
print(f"  frequencyUsed>0: {cur.execute('SELECT COUNT(*) FROM Dictionary WHERE frequencyUsed>0').fetchone()[0]}")
print()
print("=== MEETINGS aggregates (structure only) ===")
print(f"  total: {cur.execute('SELECT COUNT(*) FROM Meetings').fetchone()[0]}")
for row in cur.execute("SELECT finalized, importSource, refineStatus, summaryReady, transcriptReady, recordedMs FROM Meetings"):
    print(f"  finalized={row[0]} importSource={row[1]} refineStatus={row[2]} summaryReady={row[3]} transcriptReady={row[4]} recordedMs={row[5]}")
print()
print("=== REMOTE NOTIFICATIONS types (catalogue, not content) ===")
for row in cur.execute("SELECT type, COUNT(*) FROM RemoteNotifications GROUP BY type ORDER BY 2 DESC"):
    print(f"  {row[0]:<34} {row[1]}")
print()
print("=== USAGE OUTBOX / TODOS / SHARED NOTES counts ===")
for t in ("UsageOutbox", "Todos", "SharedNotes", "Folders", "InstructHistory", "Polish", "Notes", "NoteImages", "UserContext", "UserVoicePreferences", "Automations", "Links", "CalendarEvents", "GranolaImportRun", "GranolaTranscriptQueue", "NotetakerChats", "FlowLensHistory", "TranscriptCorrections"):
    print(f"  {t:<24} {cur.execute(f'SELECT COUNT(*) FROM {t}').fetchone()[0]}")

con.close()
