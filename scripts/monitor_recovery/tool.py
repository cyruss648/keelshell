#!/usr/bin/python3
"""Only df/ps/ss symlinks in the owned fixture point at this dispatcher."""

import os
import sys

from fixture import TOOLS, tool

name = os.path.basename(sys.argv[0])
if name not in TOOLS:
    sys.exit(126)
sys.exit(tool(name, sys.argv[1:]))
