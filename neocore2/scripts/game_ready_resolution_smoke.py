#!/usr/bin/env python3
"""Windows smoke for authored UI scaling, resize propagation and hit-test parity."""
from __future__ import annotations
import ctypes, json, os, subprocess, sys, time
from collections import Counter
from ctypes import wintypes
from pathlib import Path

WM_CLOSE=0x10; GWL_STYLE=-16; GWL_EXSTYLE=-20; SWP_NOZORDER=0x4; SW_RESTORE=9
MOUSEEVENTF_LEFTDOWN=2; MOUSEEVENTF_LEFTUP=4; KEYEVENTF_KEYUP=2; VK_ESCAPE=0x1B
class Rect(ctypes.Structure): _fields_=[('left',ctypes.c_long),('top',ctypes.c_long),('right',ctypes.c_long),('bottom',ctypes.c_long)]
class Point(ctypes.Structure): _fields_=[('x',ctypes.c_long),('y',ctypes.c_long)]

def main()->int:
 if os.name!='nt': print('Windows required',file=sys.stderr); return 2
 root=Path(__file__).resolve().parents[1]; exe=root/'target/release/game-ready-fps.exe'
 logs=root/'cache/logs'; smoke=root/'target/smoke'; smoke.mkdir(parents=True,exist_ok=True)
 out_path=smoke/'game-ready-resolution.stdout.log'; err_path=smoke/'game-ready-resolution.stderr.log'
 before_boot={p.resolve() for p in logs.glob('current.ulog.*.bootstrap.ndjson')}
 before_std={p.resolve() for p in logs.glob('current.ulog.*.ndjson') if not p.name.endswith('.bootstrap.ndjson') and '.orphan.' not in p.name}
 user32=ctypes.windll.user32; proc=None; result=0
 def stderr_text():
  try:return err_path.read_text(encoding='utf-8',errors='replace')
  except FileNotFoundError:return ''
 def wait_for(needle,timeout,label):
  deadline=time.monotonic()+timeout
  while time.monotonic()<deadline:
   if proc.poll() is not None: raise RuntimeError(f'exit {proc.returncode} waiting for {label}')
   if needle in stderr_text(): print(f'PASS {label}',flush=True); return
   time.sleep(.2)
  raise RuntimeError(f'timeout waiting for {label}: {needle}')
 def windows(pid):
  found=[]; cb_type=ctypes.WINFUNCTYPE(ctypes.c_bool,wintypes.HWND,wintypes.LPARAM)
  @cb_type
  def cb(hwnd,_):
   owner=ctypes.c_ulong(); user32.GetWindowThreadProcessId(hwnd,ctypes.byref(owner))
   if owner.value==pid and user32.IsWindow(hwnd):
    rect=Rect(); user32.GetClientRect(hwnd,ctypes.byref(rect)); area=(rect.right-rect.left)*(rect.bottom-rect.top)
    found.append((area,bool(user32.IsWindowVisible(hwnd)),int(hwnd),rect))
   return True
  user32.EnumWindows(cb,0); return sorted(found,key=lambda item:(item[1],item[0]),reverse=True)
 def game_window():
  candidates=[w for w in windows(proc.pid) if w[1] and w[0]>100000]
  if not candidates: raise RuntimeError('game window not found')
  return candidates[0][2]
 def client_size(hwnd):
  rect=Rect(); user32.GetClientRect(hwnd,ctypes.byref(rect)); return rect.right-rect.left,rect.bottom-rect.top
 def resize_client(hwnd,width,height):
  style=user32.GetWindowLongW(hwnd,GWL_STYLE); exstyle=user32.GetWindowLongW(hwnd,GWL_EXSTYLE); rect=Rect(0,0,width,height)
  dpi=user32.GetDpiForWindow(hwnd) if hasattr(user32,'GetDpiForWindow') else 96
  if hasattr(user32,'AdjustWindowRectExForDpi'): user32.AdjustWindowRectExForDpi(ctypes.byref(rect),style,False,exstyle,dpi)
  else:user32.AdjustWindowRectEx(ctypes.byref(rect),style,False,exstyle)
  user32.ShowWindow(hwnd,SW_RESTORE); user32.SetWindowPos(hwnd,0,80,80,rect.right-rect.left,rect.bottom-rect.top,SWP_NOZORDER)
  deadline=time.monotonic()+8
  while time.monotonic()<deadline:
   if client_size(hwnd)==(width,height): print(f'PASS client resize {width}x{height}',flush=True); return
   time.sleep(.1)
  raise RuntimeError(f'resize mismatch target={width}x{height} actual={client_size(hwnd)}')
 def latest_boot():
  values=[p for p in logs.glob('current.ulog.*.bootstrap.ndjson') if p.resolve() not in before_boot]
  return max(values,key=lambda p:p.stat().st_mtime_ns) if values else None
 def wait_boot(fragment,timeout=10):
  deadline=time.monotonic()+timeout
  while time.monotonic()<deadline:
   path=latest_boot()
   if path and fragment in path.read_text(encoding='utf-8',errors='replace'): print(f'PASS resize metric {fragment}',flush=True); return
   time.sleep(.2)
  raise RuntimeError(f'missing bootstrap metric {fragment}')
 def activate(hwnd): user32.ShowWindow(hwnd,SW_RESTORE); user32.SetForegroundWindow(hwnd); user32.SetFocus(hwnd); time.sleep(.2)
 def click_design(hwnd,x,y):
  width,height=client_size(hwnd); point=Point(round(x*width/1600),round(y*height/900)); user32.ClientToScreen(hwnd,ctypes.byref(point)); activate(hwnd); user32.SetCursorPos(point.x,point.y); time.sleep(.15); user32.mouse_event(MOUSEEVENTF_LEFTDOWN,0,0,0,0); time.sleep(.07); user32.mouse_event(MOUSEEVENTF_LEFTUP,0,0,0,0); print(f'INPUT scaled click client={width}x{height}',flush=True)
 def escape(hwnd): activate(hwnd); user32.keybd_event(VK_ESCAPE,0,0,0); time.sleep(.07); user32.keybd_event(VK_ESCAPE,0,KEYEVENTF_KEYUP,0)
 def transition(a,b,t):return f"screen profile: presentation flow transition flow='game.frontend' from='{a}' to='{b}' trigger='{t}'"
 env=os.environ.copy()
 for key in ('NEWENGINE_PLUGIN_DIR','NEWENGINE_PLUGINS_DIR','NEWENGINE_PLATFORM_RUNTIME_DIR','NEWENGINE_PLATFORM_EARLY_LOG','NEWENGINE_WINIT_EARLY_LOG'):env.pop(key,None)
 try:
  with out_path.open('w',encoding='utf-8') as out,err_path.open('w',encoding='utf-8') as err:
   proc=subprocess.Popen([str(exe),'--no-startup-window'],cwd=root,env=env,stdout=out,stderr=err)
   wait_for("authored game .neui mounted ref='ui/frontend/main_menu.neui@surface'",45,'main menu mounted')
   hwnd=game_window(); resize_client(hwnd,1280,720); wait_boot('size=1280x720')
   click_design(hwnd,240,336); wait_for(transition('main_menu','loading','game.start'),10,'scaled main-menu hit test')
   wait_for(transition('loading','gameplay','runtime_ready'),120,'loading -> gameplay')
   resize_client(hwnd,1024,576); wait_boot('size=1024x576')
   escape(hwnd); wait_for(transition('gameplay','pause','engine.ui.primary.toggle'),10,'pause at 1024x576')
   wait_for("authored game .neui mounted ref='ui/engine/pause_menu.neui@surface'",20,'scaled pause menu mounted')
   click_design(hwnd,260,419); wait_for(transition('pause','gameplay','game.resume'),10,'scaled Resume hit test')
 except Exception as error:
  print(f'FAIL {error}',file=sys.stderr); result=1
 finally:
  if proc is not None and proc.poll() is None:
   for window in windows(proc.pid): user32.PostMessageW(window[2],WM_CLOSE,0,0)
   try:proc.wait(timeout=12)
   except subprocess.TimeoutExpired:proc.kill();proc.wait()
  if proc is not None:
   print(f'PROCESS exit_code={proc.returncode}',flush=True)
   if proc.returncode!=0:result=1
 new_std=[p for p in logs.glob('current.ulog.*.ndjson') if p.resolve() not in before_std and not p.name.endswith('.bootstrap.ndjson') and '.orphan.' not in p.name]
 if not new_std: print('FAIL no run shard',file=sys.stderr); result=1
 else:
  path=max(new_std,key=lambda p:p.stat().st_mtime_ns); records=[];bad=0
  for line in path.read_text(encoding='utf-8',errors='replace').splitlines():
   if not line.strip():continue
   try:records.append(json.loads(line))
   except json.JSONDecodeError:bad+=1
  levels=Counter(str(record.get('level','')).upper() for record in records)
  print(f'ULOG rows={len(records)} bad_json={bad} levels={dict(levels)}')
  if bad or levels.get('ERROR',0) or levels.get('FATAL',0):result=1
 if result==0:print('RESOLUTION_SMOKE_OK')
 else:
  print(f'STDOUT {out_path}');print(f'STDERR {err_path}')
 return result
if __name__=='__main__':raise SystemExit(main())
