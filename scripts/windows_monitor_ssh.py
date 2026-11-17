#!/usr/bin/env python3
"""Run the isolated Windows SSH acceptance lab; never install or modify a service."""

import argparse
from pathlib import Path

from windows_monitor.ssh import run_live


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seconds", type=float, default=240)
    parser.add_argument("--source", required=True, help="Exact clean Git commit tested by this runner")
    args = parser.parse_args()
    return run_live(args.test_binary, args.output, args.seconds, args.source)


if __name__ == "__main__":
    raise SystemExit(main())
