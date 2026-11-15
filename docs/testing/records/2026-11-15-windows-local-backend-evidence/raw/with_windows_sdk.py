"""Apply a task-local SDK environment without changing Cargo or shell settings."""

import json
import os
from pathlib import Path
import sys

specification = json.loads((Path(__file__).parent / "windows-sdk/parent-check-r2.json").read_text())
environment = dict(os.environ)
environment.update(specification["env_delta"])
command = sys.argv[1:] or specification["argv"]
os.execvpe(command[0], command, environment)
