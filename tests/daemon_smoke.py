#!/usr/bin/env python3
"""End-to-end daemon test against a fake compositor; never connects to real niri."""
import copy
import json
import pathlib
import socket
import subprocess
import tempfile
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
output = dict(name='eDP-1', make='Test', model='Panel', serial=None,
              physical_size=None, modes=[], current_mode=0, is_custom_mode=False,
              vrr_supported=False, vrr_enabled=False,
              logical=dict(x=0, y=0, width=1920, height=1200, scale=1, transform='Normal'))
workspace = dict(id=10, idx=1, name='notes', output='eDP-1', is_urgent=False,
                 is_active=True, is_focused=True, active_window_id=1)
window = dict(id=1, title=None, app_id='org.example.Notes', pid=None, workspace_id=10,
              is_focused=True, is_floating=False, is_urgent=False, focus_timestamp=None,
              layout=dict(pos_in_scrolling_layout=[1, 1], tile_size=[1916, 1160],
                          window_size=[1914, 1158], tile_pos_in_workspace_view=None,
                          window_offset_in_tile=[1, 1]))
state = dict(Outputs={'eDP-1': output}, Workspaces=[workspace], Windows=[window])
actions = []
streams = []
mutex = threading.RLock()


def event(value):
    with mutex:
        for stream in streams:
            stream.write((json.dumps(value) + '\n').encode())
            stream.flush()


def connection(conn):
    with conn:
        f = conn.makefile('rwb')
        for line in f:
            req = json.loads(line)
            with mutex:
                if req == 'EventStream':
                    f.write(b'{"Ok":"Handled"}\n')
                    f.flush()
                    streams.append(f)
                    while True:
                        mutex.release()
                        time.sleep(0.1)
                        mutex.acquire()
                if isinstance(req, str):
                    reply = {'Ok': {req: copy.deepcopy(state[req])}}
                else:
                    action = req['Action']
                    actions.append(action)
                    name, args = next(iter(action.items()))
                    if name == 'MoveWindowToWorkspace':
                        w = next(w for w in state['Windows'] if w['id'] == args['window_id'])
                        w['workspace_id'] = args['reference']['Id']
                        w['layout']['pos_in_scrolling_layout'] = [2, 1]
                    elif name == 'FocusWindow':
                        for w in state['Windows']:
                            w['is_focused'] = w['id'] == args['id']
                    elif name == 'MoveColumnToIndex':
                        next(w for w in state['Windows'] if w['is_focused'])['layout']['pos_in_scrolling_layout'] = [args['index'], 1]
                    else:
                        raise AssertionError(f'unexpected action: {action}')
                    reply = {'Ok': 'Handled'}
                f.write((json.dumps(reply) + '\n').encode())
                f.flush()


with tempfile.TemporaryDirectory(prefix='niri-layout-smoke-') as temp:
    tmp = pathlib.Path(temp)
    sock = tmp / 'niri.sock'
    config = tmp / 'config.kdl'
    config.write_text('app "notes" app-id="^org[.]example[.]Notes$"\nprofile "laptop" { workspace "notes" output="laptop" order=1 { columns "notes" width="100%"; }; }\n')
    server = socket.socket(socket.AF_UNIX)
    server.bind(str(sock))
    server.listen()

    def accept():
        while True:
            conn, _ = server.accept()
            threading.Thread(target=connection, args=(conn,), daemon=True).start()

    threading.Thread(target=accept, daemon=True).start()
    base = [str(ROOT / 'target/release/niri-layout'), '--socket', str(sock), '--config', str(config)]
    daemon = subprocess.Popen(base + ['daemon'], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        deadline = time.monotonic() + 5
        while not pathlib.Path(str(sock) + '.niri-layout.sock').exists() or not streams:
            assert time.monotonic() < deadline, 'daemon startup timed out'
            time.sleep(0.01)
        with mutex:
            window['is_focused'] = False
        pending = subprocess.run(base + ['apply', 'laptop', '--output', 'laptop=eDP-1'], capture_output=True, text=True, check=True)
        assert 'waiting for window focus' in pending.stdout, pending.stdout
        with mutex:
            window['is_focused'] = True
        event({'WindowFocusChanged': {'id': 1}})
        time.sleep(0.1)
        result = subprocess.run(base + ['apply', 'laptop', '--output', 'laptop=eDP-1'], capture_output=True, text=True, check=True)
        assert '0 changes' in result.stdout, result
        assert not actions
        with mutex:
            new = copy.deepcopy(window)
            new.update(id=2, workspace_id=99, is_focused=False)
            state['Windows'].append(new)
        event({'WindowOpenedOrChanged': {'window': new}})
        deadline = time.monotonic() + 5
        while not actions:
            assert time.monotonic() < deadline, 'new window not arranged'
            time.sleep(0.01)
        # Wait for the synchronous status request to confirm the daemon completed the event.
        subprocess.run(base + ['status'], capture_output=True, check=True)
        with mutex:
            assert new['workspace_id'] == 10
            assert len(actions) == 1, actions
            window['workspace_id'] = 99  # simulate a manual move of an already managed window
        event({'WindowOpenedOrChanged': {'window': window}})
        time.sleep(0.1)
        result = subprocess.run(base + ['status'], capture_output=True, text=True, check=True)
        assert json.loads(result.stdout)['selection']['profile'] == 'laptop'
        assert len(actions) == 1, 'manual move was undone'
        print('PASS: daemon selection, zero-change apply, new-window placement, manual-move preservation')
    finally:
        daemon.terminate()
        daemon.communicate(timeout=5)
        server.close()
