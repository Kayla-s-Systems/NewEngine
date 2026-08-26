#!/usr/bin/env python3
from __future__ import annotations
import argparse, hashlib, json, os, shutil, subprocess, sys, tempfile, time
from pathlib import Path

SCHEMA="northstar.migration_registry.v1"; VERSION=1

def run_json(cmd:list[str],cwd:Path)->object:
    r=subprocess.run(cmd,cwd=cwd,text=True,capture_output=True)
    if r.stderr: print(r.stderr,end="" if r.stderr.endswith("\n") else "\n",file=sys.stderr)
    if r.returncode:
        if r.stdout: print(r.stdout)
        raise RuntimeError(f"command failed rc={r.returncode}: {' '.join(cmd)}")
    return json.loads(r.stdout)

def registry(neocore:Path)->list[dict]:
    data=run_json(["cargo","run","-q","-p","newengine-migration-registry","--example","migration_registry_json"],neocore)
    if data.get("schema")!=SCHEMA or data.get("version")!=VERSION: raise RuntimeError(f"migration registry identity mismatch: {data.get('schema')} v{data.get('version')}")
    return data["migrations"]

def scan(neocore:Path,repo:Path)->list[dict]:
    return run_json(["cargo","run","-q","-p","newengine-migration-registry","--example","scan_migration_corpus","--",str(repo)],neocore)

def sha256(path:Path)->str:
    h=hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''): h.update(chunk)
    return h.hexdigest()

def check(reports:list[dict])->None:
    errors=[]
    for row in reports:
        if row["source_count"]: errors.append(f"{row['migration_id']}: legacy={row['source_count']}")
        if row.get("other_versions"): errors.append(f"{row['migration_id']}: other_versions={row['other_versions']}")
        if row.get("other_representations"): errors.append(f"{row['migration_id']}: other_representations={row['other_representations']}")
    if errors: raise RuntimeError("migration corpus is not canonical: "+"; ".join(errors))

def apply(neocore:Path,repo:Path,specs:list[dict],reports:list[dict])->Path:
    by_id={s["id"]:s for s in specs}; candidates=[]
    for report in reports:
        spec=by_id[report["migration_id"]]
        for raw in report["source_files"]:
            candidates.append((spec,Path(raw)))
    if not candidates:
        print('[P5] no migration candidates')
        return neocore/'Intermediate/AssetMigration'
    stamp=time.strftime('%Y%m%d-%H%M%S')
    backup=neocore/'Intermediate/AssetMigration'/f'p5_registry_{stamp}'
    backup.mkdir(parents=True,exist_ok=False)
    entries=[]; replaced=[]
    try:
        # Freeze a complete before-image before mutating anything.
        for spec,src in candidates:
            rel=src.resolve().relative_to(repo.resolve())
            dst=backup/'files'/rel; dst.parent.mkdir(parents=True,exist_ok=True); shutil.copy2(src,dst)
            entries.append({"migration_id":spec["id"],"relative_path":rel.as_posix(),"before_sha256":sha256(src),"before_size":src.stat().st_size,"backup_path":str(dst)})
        (backup/'manifest.before.json').write_text(json.dumps({"schema":"northstar.migration_backup.v1","registry_schema":SCHEMA,"registry_version":VERSION,"entries":entries},indent=2),encoding='utf-8')
        index={(e['migration_id'],e['relative_path']):e for e in entries}
        for spec,src in candidates:
            rel=src.resolve().relative_to(repo.resolve()); item=index[(spec['id'],rel.as_posix())]
            if sha256(src)!=item['before_sha256']: raise RuntimeError(f"source changed after backup: {rel}")
            stage=src.with_name(src.name+'.p5-new')
            if stage.exists(): stage.unlink()
            cmd=["cargo","run","-q","-p",spec["tool"]["package"],"--example",spec["tool"]["example"],"--",spec["id"],str(src),str(stage),rel.as_posix()]
            r=subprocess.run(cmd,cwd=neocore,text=True,capture_output=True)
            if r.stdout: print(r.stdout,end="" if r.stdout.endswith("\n") else "\n")
            if r.stderr: print(r.stderr,end="" if r.stderr.endswith("\n") else "\n",file=sys.stderr)
            if r.returncode: raise RuntimeError(f"migration failed rc={r.returncode} path={rel}")
            if not stage.is_file(): raise RuntimeError(f"migration did not produce stage: {rel}")
            item['after_sha256']=sha256(stage); item['after_size']=stage.stat().st_size
            os.replace(stage,src); replaced.append((src,Path(item['backup_path'])))
        final=scan(neocore,repo); check(final)
        manifest={"schema":"northstar.migration_backup.v1","registry_schema":SCHEMA,"registry_version":VERSION,"status":"committed","entries":entries,"post_corpus":final}
        (backup/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
        print(f"[P5] migration transaction COMMITTED files={len(entries)} backup={backup}")
        return backup
    except Exception:
        print(f"[P5] migration transaction FAILED; rolling back {len(replaced)} file(s)",file=sys.stderr)
        for src,bak in reversed(replaced):
            tmp=src.with_name(src.name+'.p5-rollback'); shutil.copy2(bak,tmp); os.replace(tmp,src)
        for _,src in candidates:
            stage=src.with_name(src.name+'.p5-new')
            if stage.exists(): stage.unlink()
        raise

def main()->int:
    ap=argparse.ArgumentParser(); ap.add_argument('--apply',action='store_true'); ns=ap.parse_args()
    neocore=Path(__file__).resolve().parents[1]; repo=neocore.parent.parent
    specs=registry(neocore); reports=scan(neocore,repo)
    print(json.dumps(reports,indent=2))
    if ns.apply:
        apply(neocore,repo,specs,reports)
        reports=scan(neocore,repo); check(reports)
    else:
        check(reports)
    print('[P5] MIGRATION REGISTRY CORPUS GATE PASS')
    return 0
if __name__=='__main__': raise SystemExit(main())
