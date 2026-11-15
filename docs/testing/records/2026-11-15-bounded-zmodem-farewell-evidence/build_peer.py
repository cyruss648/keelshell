import hashlib, json, os, shutil, subprocess
from pathlib import Path
batch=Path(__file__).resolve().parent
source=batch/"external-source/lrzsz-0.13.1"
build=batch/"build-original"
build.mkdir()
commands=([str(source/"configure"),"--disable-nls","--prefix="+str(batch/"peer")],["make","-j4"])
for index,command in enumerate(commands):
    with (batch/("peer-build-"+str(index)+".log")).open("xb") as stream:
        subprocess.run(command,cwd=build,stdout=stream,stderr=subprocess.STDOUT,check=True,timeout=100)
peer=batch/"peer"
peer.mkdir()
for name in ("lrz","lsz"):
    shutil.copy2(build/"src"/name,peer/name)
    print(json.dumps({"binary":str(peer/name),"sha256":hashlib.sha256((peer/name).read_bytes()).hexdigest()}))
