"""Drives an engine through a real ibus-daemon: one input context, key events from a JSON script,
and every commit / preedit / lookup table the engine sends printed as one JSON line each.

Script: {"keys": [[keyval, keycode, state], ...]} — each key is sent as a press and then a release
(state | IBUS_RELEASE_MASK) unless the entry has a fourth element "press" or "release";
["focus_out"] / ["focus_in"] / ["reset"] call those, and
["surrounding", text, cursor] sets the surrounding text."""
import json, os, subprocess, sys, time
import gi
gi.require_version("IBus", "1.0")
from gi.repository import GLib, IBus

RELEASE = 1 << 30
engine_bin, script_path = sys.argv[1], sys.argv[2]
address = os.environ["IBUS_ADDRESS"]
daemon = subprocess.Popen(["ibus-daemon", "--single", "--panel=disable", "--emoji-extension=disable",
                           "--config=disable", "--cache=none", "--address", address] + (["--verbose"] if os.environ.get("SHANJIE_DAEMON_LOG") else []),
                          stdout=subprocess.DEVNULL, stderr=None if os.environ.get("SHANJIE_DAEMON_LOG") else subprocess.DEVNULL)
engine = None
log = []

def pump(seconds=0.05):
    ctx = GLib.MainContext.default()
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        while ctx.iteration(False):
            pass
        time.sleep(0.005)

def wait_for(predicate, seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if predicate():
            return True
        pump(0.05)
    return False

def text(t):
    return t.get_text() if t is not None else None

try:
    bus = None
    def connected():
        global bus
        bus = bus or IBus.Bus.new()
        return bus.is_connected()
    if not wait_for(connected, 10):
        raise SystemExit("private ibus-daemon did not start")
    engine = subprocess.Popen([engine_bin], env=dict(os.environ))
    if not wait_for(lambda: bus.name_has_owner("org.freedesktop.IBus.Shanjie"), 30):
        raise SystemExit("engine did not register")
    ic = bus.create_input_context("shanjie-test")
    ic.set_capabilities(IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.LOOKUP_TABLE | IBus.Capabilite.FOCUS | IBus.Capabilite.SURROUNDING_TEXT)
    ic.connect("commit-text", lambda _ic, t: log.append({"commit": text(t)}))
    ic.connect("update-preedit-text", lambda _ic, t, cursor, visible: log.append({"preedit": text(t), "cursor": cursor, "visible": visible}))
    ic.connect("hide-preedit-text", lambda _ic: log.append({"preedit_hidden": True}))
    def lookup(_ic, table, visible):
        n = table.get_number_of_candidates()
        log.append({"lookup": [text(table.get_candidate(i)) for i in range(n)], "visible": visible,
                    "cursor": table.get_cursor_pos() if table.is_cursor_visible() else None,
                    "vertical": table.get_orientation() == IBus.Orientation.VERTICAL})
    ic.connect("update-lookup-table", lookup)
    ic.connect("hide-lookup-table", lambda _ic: log.append({"lookup_hidden": True}))
    ic.connect("forward-key-event", lambda _ic, kv, kc, st: log.append({"forward": [kv, kc, st]}))
    ic.connect("delete-surrounding-text", lambda _ic, off, n: log.append({"delete_surrounding": [off, n]}))
    ic.focus_in()
    # A runtime-registered component is only found by SetGlobalEngine (bus_ibus_impl's register_engine_list),
    # not by an input context's SetEngine; this daemon is private, so the global engine is ours.
    bus.set_global_engine("shanjie")
    pump(0.5)
    desc = ic.get_engine()
    if desc is None or desc.get_name() != "shanjie":
        raise SystemExit("input context did not switch to shanjie: %r" % (desc.get_name() if desc else None))
    script = json.load(open(script_path))
    for entry in script["keys"]:
        if entry[0] in ("focus_in", "focus_out", "reset"):
            log.append({"step": entry[0]})
            getattr(ic, entry[0])()
            pump(0.2)
            continue
        if entry[0] == "surrounding":
            ic.set_surrounding_text(IBus.Text.new_from_string(entry[1]), entry[2], entry[2])
            pump(0.1)
            continue
        keyval, keycode, state = entry[:3]
        phases = [entry[3]] if len(entry) > 3 else ["press", "release"]
        for phase in phases:
            st = state | (RELEASE if phase == "release" else 0)
            handled = ic.process_key_event(keyval, keycode, st)
            log.append({"key": [keyval, keycode, st], "handled": handled})
            pump(0.05)
        pump(float(script.get("settle", 0.05)))
    pump(float(script.get("final_wait", 0.3)))
    for item in log:
        print(json.dumps(item, ensure_ascii=False))
finally:
    if engine is not None:
        engine.terminate()
    daemon.terminate()
